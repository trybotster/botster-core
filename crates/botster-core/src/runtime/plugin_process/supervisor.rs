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
    /// The first kill reason whose SIGKILL the kernel accepted.
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
    pub(super) fn kill(&self, reason: PluginKillReason) {
        let mut state = self.lock();
        if state.reaped {
            return;
        }
        if signal_group(self.pgid) && state.first_delivered.is_none() {
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

    fn lock(&self) -> std::sync::MutexGuard<'_, KillState> {
        // Only this module's bookkeeping runs under the lock.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn signal_group(pgid: libc::pid_t) -> bool {
    // SAFETY: killpg only sends a signal; pgid is our unreaped child's group.
    unsafe { libc::killpg(pgid, libc::SIGKILL) == 0 }
}

/// Identity of one armed deadline.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DeadlineId(u64);

#[derive(Default)]
struct Book {
    next_id: u64,
    entries: Vec<(DeadlineId, Instant, PluginKillReason)>,
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
        let (lock, cvar) = &*self.book;
        let mut book = lock.lock().unwrap_or_else(PoisonError::into_inner);
        let id = DeadlineId(book.next_id);
        book.next_id += 1;
        book.entries.push((id, at, reason));
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
        let mut due = Vec::new();
        guard.entries.retain(|(_, at, reason)| {
            if *at <= now {
                due.push(reason.clone());
                false
            } else {
                true
            }
        });
        if !due.is_empty() {
            drop(guard);
            for reason in due {
                killer.kill(reason);
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
