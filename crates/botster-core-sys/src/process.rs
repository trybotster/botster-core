//! Workers as processes (plan 2.3, `Process`): spawn, the exit of a child, identity by pid and start time, and the signal of a
//! process group.
//!
//! A worker is started with an exact environment and in its own process group, so that its group can be signalled without
//! touching the host (LC-5, SV-9). A process is signalled only when its identity still matches: a pid that was reused by an
//! unrelated process is never killed (AD-6).

use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, SpawnSpec,
};
use rustix::process::{kill_process_group, waitid, Pid, Signal, WaitId, WaitIdOptions};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

/// The start time of `pid` in the unit of the platform, or `None` when no such process exists. Only equality has a meaning
/// (AD-6).
pub fn start_time(pid: u32) -> Option<u64> {
    #[cfg(target_os = "macos")]
    {
        use libproc::bsd_info::BSDInfo;
        use libproc::proc_pid::pidinfo;
        let info = pidinfo::<BSDInfo>(i32::try_from(pid).ok()?, 0).ok()?;
        Some(info.pbi_start_tvsec * 1_000_000 + info.pbi_start_tvusec)
    }
    #[cfg(target_os = "linux")]
    {
        let stat = std::fs::read_to_string(format!("/proc/{pid}/stat")).ok()?;
        // The command name is in parentheses and may hold spaces: the fields start after its closing one. Field 22
        // (`starttime`) is the 20th after the state, which is the first.
        let after = stat.rsplit_once(") ")?.1;
        after.split_whitespace().nth(19)?.parse().ok()
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = pid;
        None
    }
}

/// What a pid names now, compared with an identity (AD-6).
pub fn identity_state(identity: ProcessIdentity) -> IdentityState {
    match start_time(identity.pid) {
        None => IdentityState::Absent,
        Some(t) if t == identity.start_time => IdentityState::Matches,
        Some(_) => IdentityState::Reused,
    }
}

/// The exits that the reaper threads reaped, in order.
type Exits = Arc<Mutex<VecDeque<(ProcessIdentity, ExitStatus)>>>;

/// The children that are not reaped yet, by pid. A child in this table cannot have its pid reused, so a signal to it under
/// the table's lock reaches it and nothing else (AD-6). The reaper takes the child out, and reaps it, under the same lock.
type Unreaped = Arc<Mutex<BTreeMap<u32, Child>>>;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // No panic leaves these tables half written, so a poisoned lock still holds usable state.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Blocks until `pid` can be reaped, and returns its status without reaping it (`WNOWAIT`).
fn wait_unreaped(pid: u32) -> ExitStatus {
    let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
        return ExitStatus::Code(-1);
    };
    loop {
        match waitid(
            WaitId::Pid(pid),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        ) {
            Ok(Some(status)) => {
                if let Some(signal) = status.terminating_signal() {
                    return ExitStatus::Signal(signal);
                }
                if let Some(code) = status.exit_status() {
                    return ExitStatus::Code(code);
                }
            }
            Err(rustix::io::Errno::INTR) | Ok(None) => {}
            // ECHILD: the child is no longer ours to wait for. It cannot happen while it is in `Unreaped`.
            Err(_) => return ExitStatus::Code(-1),
        }
    }
}

/// Called by a reaper thread after it queued an exit: the host wakes and polls (TM-6).
pub type Notify = Arc<dyn Fn() + Send + Sync>;

/// The children that the host started. A reaper thread watches each child: it waits until the child can be reaped, reaps it
/// under the lock of `unreaped`, queues the exit and calls the notifier, so the host wakes at the moment a worker ends and
/// never leaves a zombie.
pub struct Children {
    exits: Exits,
    unreaped: Unreaped,
    notify: Option<Notify>,
}

impl Default for Children {
    fn default() -> Children {
        Children {
            exits: Arc::default(),
            unreaped: Arc::default(),
            notify: None,
        }
    }
}

impl Children {
    pub fn new() -> Children {
        Children::default()
    }

    /// The notifier that each reaper thread calls after an exit is queued (the wake of the host).
    pub fn with_notify(notify: Notify) -> Children {
        Children {
            notify: Some(notify),
            ..Children::default()
        }
    }

    /// Starts `spec` in a new process group, with exactly `spec.env` and no standard stream (plan 2.3).
    pub fn spawn(&mut self, spec: &SpawnSpec) -> Result<ProcessIdentity, SpawnError> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .env_clear()
            .envs(spec.env.iter().map(|(k, v)| (k, v)))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .process_group(0);
        if let Some(cwd) = &spec.cwd {
            command.current_dir(cwd);
        }
        // The reaper thread exists before the child: a thread that cannot start leaves no child behind, and a child that
        // cannot start ends the thread (its sender is dropped). So every child that runs has a thread that reaps it.
        let (hand_over, take) = std::sync::mpsc::sync_channel::<ProcessIdentity>(1);
        let exits = Arc::clone(&self.exits);
        let unreaped = Arc::clone(&self.unreaped);
        let notify = self.notify.clone();
        std::thread::Builder::new()
            .name("botster-reaper".into())
            .spawn(move || {
                let Ok(identity) = take.recv() else {
                    return;
                };
                let status = wait_unreaped(identity.pid);
                // The reap happens under the lock that `signal_group` holds: the pid is never freed between its check and
                // its signal (AD-6).
                if let Some(mut child) = lock(&unreaped).remove(&identity.pid) {
                    // The child can be reaped now, so this wait returns at once.
                    let _: io::Result<std::process::ExitStatus> = child.wait();
                }
                lock(&exits).push_back((identity, status));
                if let Some(notify) = notify {
                    notify();
                }
            })
            .map_err(|e| SpawnError {
                errno: e.raw_os_error().unwrap_or(0),
            })?;
        let child = command.spawn().map_err(|e| SpawnError {
            errno: e.raw_os_error().unwrap_or(0),
        })?;
        let pid = child.id();
        // The child is ours and unreaped, so its pid is not reused: the start time that we read is its own.
        let start_time = start_time(pid).unwrap_or(0);
        let identity = ProcessIdentity { pid, start_time };
        lock(&self.unreaped).insert(pid, child);
        hand_over
            .send(identity)
            .expect("the reaper thread waits for its child");
        Ok(identity)
    }

    /// The next child that has ended, reaped, or `None`. It never blocks.
    pub fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        lock(&self.exits).pop_front()
    }

    /// Signals the process group of `identity`, only when the identity still matches (AD-6). The check and the signal
    /// happen under the lock of the unreaped children, so a child of this host cannot be reaped, and its pid reused, in
    /// between. A process that this host did not start (an adopted worker) has no such guarantee: its parent may reap it at
    /// any time, so the check is as close to the signal as the operating system allows.
    pub fn signal_group(&self, identity: ProcessIdentity, signal: GroupSignal) {
        let _unreaped = lock(&self.unreaped);
        if identity_state(identity) != IdentityState::Matches {
            return;
        }
        self.kill_group(identity.pid, signal);
    }

    fn kill_group(&self, pid: u32, signal: GroupSignal) {
        let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) else {
            return;
        };
        let signal = match signal {
            GroupSignal::Term => Signal::TERM,
            GroupSignal::Kill => Signal::KILL,
            // The worker-control signal goes to the worker process alone: its group holds no payload, and a default
            // `SIGUSR1` would end a descendant of the worker (LC-5).
            GroupSignal::EndPayload => {
                let _: io::Result<()> =
                    rustix::process::kill_process(pid, Signal::USR1).map_err(io::Error::from);
                return;
            }
        };
        // The group may already be gone: that is the goal, so a failure is not an error.
        let _: io::Result<()> = kill_process_group(pid, signal).map_err(io::Error::from);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AD-6: the start time of a live process is stable, is not a placeholder, and differs from the start time of another
    /// process; the start time of a pid that does not exist is `None`.
    #[test]
    fn the_start_time_names_a_process() {
        let me = std::process::id();
        let first = start_time(me).expect("this process exists");
        assert_eq!(start_time(me), Some(first), "stable");
        assert!(first > 1, "not a placeholder");
        // A pid above the pid range of every platform.
        assert_eq!(start_time(u32::MAX - 1), None);
        let parent = std::os::unix::process::parent_id();
        if parent != me {
            if let Some(other) = start_time(parent) {
                assert_ne!(other, first, "another process has another start time");
            }
        }
    }

    /// AD-6: an identity matches while the pid and the start time agree, and is `Reused` when the start time differs and
    /// `Absent` when no process has the pid.
    #[test]
    fn an_identity_is_matched_reused_or_absent() {
        let me = std::process::id();
        let start = start_time(me).unwrap();
        let identity = ProcessIdentity {
            pid: me,
            start_time: start,
        };
        assert_eq!(identity_state(identity), IdentityState::Matches);
        assert_eq!(
            identity_state(ProcessIdentity {
                pid: me,
                start_time: start + 1
            }),
            IdentityState::Reused
        );
        assert_eq!(
            identity_state(ProcessIdentity {
                pid: u32::MAX - 1,
                start_time: 1
            }),
            IdentityState::Absent
        );
    }

    /// AD-6: a signal goes only to the process that the identity names: a reused pid is never signalled (the test would end
    /// with `SIGUSR1` otherwise).
    #[test]
    fn a_reused_identity_is_never_signalled() {
        let me = std::process::id();
        let children = Children::new();
        children.signal_group(
            ProcessIdentity {
                pid: me,
                start_time: start_time(me).unwrap() + 1,
            },
            GroupSignal::EndPayload,
        );
        children.signal_group(
            ProcessIdentity {
                pid: u32::MAX - 1,
                start_time: 1,
            },
            GroupSignal::EndPayload,
        );
    }
}
