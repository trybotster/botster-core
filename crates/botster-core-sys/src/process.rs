//! Workers as processes (plan 2.3, `Process`): spawn, the exit of a child, identity by pid and start time, and the signal of a
//! process group.
//!
//! A worker is started with an exact environment and in its own process group, so that its group can be signalled without
//! touching the host (LC-5, SV-9). A process is signalled only when its identity still matches: a pid that was reused by an
//! unrelated process is never killed (AD-6).

use crate::signal::{signal_group, signal_process};
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, SpawnSpec,
};
use rustix::process::Signal;
use std::collections::{BTreeSet, VecDeque};
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Condvar, Mutex};

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

/// The exits that the reaper threads reaped, and a condition variable for the callers that wait for one.
type Exits = Arc<(Mutex<VecDeque<(ProcessIdentity, ExitStatus)>>, Condvar)>;

/// Called by a reaper thread after it queued an exit: the host wakes and polls (TM-6).
pub type Notify = Arc<dyn Fn() + Send + Sync>;

fn exit_status(status: std::process::ExitStatus) -> ExitStatus {
    match (status.code(), status.signal()) {
        (Some(code), _) => ExitStatus::Code(code),
        (None, Some(signal)) => ExitStatus::Signal(signal),
        (None, None) => ExitStatus::Code(-1),
    }
}

/// The children that the host started. A reaper thread owns each child: it waits for the exit, reaps it, queues the exit and
/// calls the notifier, so the host wakes at the moment a worker ends and never leaves a zombie.
pub struct Children {
    exits: Exits,
    /// The pids that were started and whose exit was not taken yet.
    live: BTreeSet<u32>,
    notify: Option<Notify>,
}

impl Default for Children {
    fn default() -> Children {
        Children {
            exits: Arc::new((Mutex::new(VecDeque::new()), Condvar::new())),
            live: BTreeSet::new(),
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
        let (hand_over, take) = std::sync::mpsc::sync_channel::<(Child, ProcessIdentity)>(1);
        let exits = Arc::clone(&self.exits);
        let notify = self.notify.clone();
        std::thread::Builder::new()
            .name("botster-reaper".into())
            .spawn(move || {
                let Ok((mut child, identity)) = take.recv() else {
                    return;
                };
                let status = child.wait().map_or(ExitStatus::Code(-1), exit_status);
                let (queue, ready) = &*exits;
                queue
                    .lock()
                    .unwrap_or_else(std::sync::PoisonError::into_inner)
                    .push_back((identity, status));
                ready.notify_all();
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
        hand_over
            .send((child, identity))
            .expect("the reaper thread waits for its child");
        self.live.insert(pid);
        Ok(identity)
    }

    /// The next child that has ended, reaped, or `None`. It never blocks.
    pub fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        let exit = self
            .exits
            .0
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .pop_front()?;
        self.live.remove(&exit.0.pid);
        Some(exit)
    }

    /// Waits for the exit of the child `pid`. It blocks until the child ends: a caller that must not block uses
    /// [`Children::poll_exit`]. `None` when `pid` is not a live child.
    pub fn wait_exit(&mut self, pid: u32) -> Option<(ProcessIdentity, ExitStatus)> {
        if !self.live.contains(&pid) {
            return None;
        }
        let (queue, ready) = &*self.exits;
        let mut queue = queue
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        loop {
            if let Some(at) = queue.iter().position(|(id, _)| id.pid == pid) {
                let exit = queue.remove(at)?;
                self.live.remove(&pid);
                return Some(exit);
            }
            queue = ready
                .wait(queue)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
    }

    /// Signals the process group of `identity`, only when the identity still matches (AD-6).
    pub fn signal_group(&self, identity: ProcessIdentity, signal: GroupSignal) {
        if identity_state(identity) != IdentityState::Matches {
            return;
        }
        self.kill_group(identity.pid, signal);
    }

    fn kill_group(&self, pid: u32, signal: GroupSignal) {
        // The group may already be gone: that is the goal, so a failure is not an error. A refused target (the pattern rule
        // of `signal`) is never signalled.
        let _ = match signal {
            GroupSignal::Term => signal_group(pid, Signal::TERM),
            GroupSignal::Kill => signal_group(pid, Signal::KILL),
            // The worker-control signal goes to the worker process alone: its group holds no payload, and a default
            // `SIGUSR1` would end a descendant of the worker (LC-5).
            GroupSignal::EndPayload => signal_process(pid, Signal::USR1),
        };
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

    /// The notifier is kept by `with_notify` and not by `new`.
    #[test]
    fn with_notify_keeps_its_notifier() {
        assert!(Children::new().notify.is_none());
        assert!(Children::with_notify(Arc::new(|| {})).notify.is_some());
    }

    /// An exit is a code or a signal.
    #[test]
    fn an_exit_is_a_code_or_a_signal() {
        assert_eq!(
            exit_status(std::process::ExitStatus::from_raw(3 << 8)),
            ExitStatus::Code(3)
        );
        assert_eq!(
            exit_status(std::process::ExitStatus::from_raw(9)),
            ExitStatus::Signal(9)
        );
    }
}
