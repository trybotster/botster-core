//! Workers as processes (plan 2.3, `Process`): spawn, the exit of a child, identity by pid and start time, and the signal of a
//! process group.
//!
//! A worker is started with an exact environment and in its own process group, so that its group can be signalled without
//! touching the host (LC-5, SV-9). A process is signalled only when its identity still matches: a pid that was reused by an
//! unrelated process is never killed (AD-6).

use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, SpawnSpec,
};
use rustix::process::{kill_process_group, Pid, Signal};
use std::collections::{BTreeSet, VecDeque};
use std::io;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Command, Stdio};
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
        let mut child = command.spawn().map_err(|e| SpawnError {
            errno: e.raw_os_error().unwrap_or(0),
        })?;
        let pid = child.id();
        // The child is ours and unreaped, so its pid is not reused: the start time that we read is its own.
        let start_time = start_time(pid).unwrap_or(0);
        let identity = ProcessIdentity { pid, start_time };
        let exits = Arc::clone(&self.exits);
        let notify = self.notify.clone();
        let reaper = std::thread::Builder::new()
            .name("botster-reaper".into())
            .spawn(move || {
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
            });
        if let Err(e) = reaper {
            // No thread can reap the child: end it, so that nothing is left running that the host cannot see.
            if let Some(pid) = i32::try_from(pid).ok().and_then(Pid::from_raw) {
                let _: io::Result<()> =
                    kill_process_group(pid, Signal::KILL).map_err(io::Error::from);
            }
            return Err(SpawnError {
                errno: e.raw_os_error().unwrap_or(0),
            });
        }
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
