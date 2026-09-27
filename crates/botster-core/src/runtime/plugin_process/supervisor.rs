//! The deadline owner and the group killer of one plugin process.
//!
//! One supervisor thread per process holds every deadline (startup,
//! shutdown, and, later, cancel grace). No other thread owns a timer.

use std::sync::{Arc, Condvar, Mutex, PoisonError};
use std::thread;
use std::time::Instant;

use super::PluginKillReason;

/// Kills the process group and serializes every kill with the reap, so a
/// kill never reaches a reused process group id.
#[derive(Debug)]
pub(super) struct ProcessKiller {
    pgid: libc::pid_t,
    state: Mutex<KillState>,
}

#[derive(Debug, Default)]
pub(super) struct KillState {
    /// Set by the exit watch once the leader is reaped. No kill after that.
    pub reaped: bool,
    /// The first kill reason whose SIGKILL the kernel accepted while the
    /// leader was not already exiting.
    pub first_delivered: Option<PluginKillReason>,
}

impl ProcessKiller {
    pub(super) fn new(pgid: libc::pid_t) -> Self {
        Self {
            pgid,
            state: Mutex::new(KillState::default()),
        }
    }

    /// Kill the group for `reason`. The reason is recorded only when the
    /// kernel accepted the signal, and only the first such reason is kept.
    ///
    /// A kill that reaches a leader already exiting cannot be the cause of
    /// its death, so it is sent (it still ends other group members) but not
    /// recorded. This matters for EOF: a leader that dies of a signal the
    /// parent did not send (for example SIGKILL at the `RLIMIT_CPU` hard
    /// limit) closes the socket as it exits, and the reader's cleanup kill
    /// must not then claim the death (plan section 7.5, rule 4).
    pub(super) fn kill(&self, reason: PluginKillReason) {
        let mut state = self.lock();
        if state.reaped {
            return;
        }
        let exiting = leader_exiting(self.pgid);
        if signal_group(self.pgid) && !exiting && state.first_delivered.is_none() {
            state.first_delivered = Some(reason);
        }
    }

    /// Run `reap` with kills excluded. The exit watch calls this while the
    /// leader is still unreaped, so its pid (and the group id) cannot be
    /// reused while a kill can still be sent.
    pub(super) fn reap_with<T>(&self, reap: impl FnOnce(&KillState) -> T) -> T {
        let mut state = self.lock();
        // Post-exit cleanup of any other group member. It is not a cause:
        // the leader already exited, so it is not recorded.
        let _ = signal_group(self.pgid);
        let result = reap(&state);
        state.reaped = true;
        result
    }

    /// The first kill reason the kernel accepted, if any.
    #[cfg(test)]
    pub(super) fn state_for_test(&self) -> Option<PluginKillReason> {
        self.lock().first_delivered.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, KillState> {
        // Only this module's bookkeeping runs under the lock.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Whether the unreaped leader `pid` has already begun to exit. The caller
/// holds the killer's lock, so the leader cannot be reaped (and its pid
/// reused) meanwhile.
///
/// On Linux a dying task sets `PF_EXITING` before it releases its files, so
/// whenever the socket reaches EOF because the leader died, the flag is
/// already visible in `/proc/<pid>/stat`. Elsewhere, only a leader that is
/// already a zombie is detected.
fn leader_exiting(pid: libc::pid_t) -> bool {
    #[cfg(target_os = "linux")]
    if proc_exiting(pid) {
        return true;
    }
    zombie(pid)
}

/// `PF_EXITING` from `include/linux/sched.h`.
#[cfg(target_os = "linux")]
const PF_EXITING: u64 = 0x0000_0004;

/// Read the leader's state and flags from `/proc/<pid>/stat` (fields 3 and
/// 9, after the parenthesized command name).
#[cfg(target_os = "linux")]
fn proc_exiting(pid: libc::pid_t) -> bool {
    let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) else {
        return false;
    };
    let Some((_, fields)) = stat.rsplit_once(')') else {
        return false;
    };
    let mut fields = fields.split_whitespace();
    let state = fields.next();
    let flags = fields.nth(5).and_then(|flags| flags.parse::<u64>().ok());
    matches!(state, Some("Z" | "X")) || flags.is_some_and(|flags| flags & PF_EXITING != 0)
}

/// Whether the leader is a zombie now. Does not reap.
fn zombie(pid: libc::pid_t) -> bool {
    // SAFETY: a zeroed siginfo_t is a valid output record.
    let mut info: libc::siginfo_t = unsafe { std::mem::zeroed() };
    loop {
        // SAFETY: info is a valid output record; WNOWAIT leaves the leader
        // for the exit watch to reap.
        let result = unsafe {
            libc::waitid(
                libc::P_PID,
                pid as libc::id_t,
                &mut info,
                libc::WEXITED | libc::WNOHANG | libc::WNOWAIT,
            )
        };
        if result == 0 {
            // SAFETY: waitid filled `info` for a child event, so the child
            // fields are the valid union member.
            #[cfg(target_os = "linux")]
            let reporter = unsafe { info.si_pid() };
            #[cfg(not(target_os = "linux"))]
            let reporter = info.si_pid;
            return reporter == pid
                && matches!(
                    info.si_code,
                    libc::CLD_EXITED | libc::CLD_KILLED | libc::CLD_DUMPED
                );
        }
        if std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return false;
        }
    }
}

fn signal_group(pgid: libc::pid_t) -> bool {
    // SAFETY: killpg only sends a signal; pgid is our unreaped child's group.
    unsafe { libc::killpg(pgid, libc::SIGKILL) == 0 }
}

/// Identity of one armed deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DeadlineId(u64);

/// What an expired deadline does.
pub(super) enum Expiry {
    /// Kill the group for this reason.
    Kill(PluginKillReason),
    /// Ask the guard, which decides under its own lock whether a kill is
    /// still needed. An expiry taken from the book but not yet acted on is
    /// therefore still arbitrated against a concurrent settlement.
    Guarded(Arc<dyn ExpiryGuard>),
    /// Test seam: report that the supervisor processed this instant.
    #[cfg(test)]
    Probe(std::sync::mpsc::Sender<()>),
}

/// A deadline whose kill depends on state that can change until the kill.
pub(super) trait ExpiryGuard: Send + Sync {
    fn expire(&self, killer: &ProcessKiller);
}

#[derive(Default)]
struct Book {
    next_id: u64,
    entries: Vec<(DeadlineId, Instant, Expiry)>,
    stopped: bool,
}

/// Handle to the supervisor thread's deadline book.
pub(super) struct Supervisor {
    book: Arc<(Mutex<Book>, Condvar)>,
}

impl Supervisor {
    pub(super) fn start(killer: Arc<ProcessKiller>, name: String) -> std::io::Result<Self> {
        let book = Arc::new((Mutex::new(Book::default()), Condvar::new()));
        let thread_book = book.clone();
        thread::Builder::new()
            .name(name)
            .spawn(move || run(&thread_book, &killer))?;
        Ok(Self { book })
    }

    /// Kill the group for `reason` at `at`, unless disarmed first.
    pub(super) fn arm(&self, at: Instant, reason: PluginKillReason) -> DeadlineId {
        self.arm_expiry(at, Expiry::Kill(reason))
    }

    /// Run `expiry` at `at`, unless disarmed first.
    pub(super) fn arm_expiry(&self, at: Instant, expiry: Expiry) -> DeadlineId {
        let (lock, cvar) = &*self.book;
        let mut book = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let id = DeadlineId(book.next_id);
        book.next_id += 1;
        book.entries.push((id, at, expiry));
        cvar.notify_one();
        id
    }

    pub(super) fn disarm(&self, id: DeadlineId) {
        let (lock, _) = &*self.book;
        let mut book = lock.lock().unwrap_or_else(PoisonError::into_inner);
        book.entries.retain(|(entry, _, _)| *entry != id);
    }

    /// End the thread. The exit watch calls this after the reap.
    pub(super) fn stop(&self) {
        let (lock, cvar) = &*self.book;
        lock.lock().unwrap_or_else(PoisonError::into_inner).stopped = true;
        cvar.notify_one();
    }
}

fn run(book: &(Mutex<Book>, Condvar), killer: &ProcessKiller) {
    let (lock, cvar) = book;
    let mut guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        if guard.stopped {
            return;
        }
        let now = Instant::now();
        let (due, pending): (Vec<_>, Vec<_>) = std::mem::take(&mut guard.entries)
            .into_iter()
            .partition(|(_, at, _)| *at <= now);
        guard.entries = pending;
        if !due.is_empty() {
            // Act outside the book lock: a guard takes its own lock, and the
            // book lock is always taken after it, never before.
            drop(guard);
            for (_, _, expiry) in due {
                match expiry {
                    Expiry::Kill(reason) => killer.kill(reason),
                    Expiry::Guarded(expiry) => expiry.expire(killer),
                    #[cfg(test)]
                    Expiry::Probe(reached) => {
                        let _ = reached.send(());
                    }
                }
            }
            guard = lock.lock().unwrap_or_else(PoisonError::into_inner);
            continue;
        }
        let next = guard.entries.iter().map(|(_, at, _)| *at).min();
        guard = match next {
            // timer: deadline — the earliest armed plugin-process deadline; expiry kills the process group
            Some(at) => {
                cvar.wait_timeout(guard, at.saturating_duration_since(now))
                    .unwrap_or_else(PoisonError::into_inner)
                    .0
            }
            None => cvar.wait(guard).unwrap_or_else(PoisonError::into_inner),
        };
    }
}
