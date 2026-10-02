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
use std::collections::BTreeMap;
use std::io;
use std::os::unix::process::{CommandExt, ExitStatusExt};
use std::process::{Child, Command, Stdio};

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

/// The children that the host started. The host reaps them itself, so it never leaves a zombie.
#[derive(Debug, Default)]
pub struct Children {
    running: BTreeMap<u32, (Child, u64)>,
}

impl Children {
    pub fn new() -> Children {
        Children::default()
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
        let child = command.spawn().map_err(|e| SpawnError {
            errno: e.raw_os_error().unwrap_or(0),
        })?;
        let pid = child.id();
        // The child is ours and unreaped, so its pid is not reused: the start time that we read is its own.
        let start_time = start_time(pid).unwrap_or(0);
        self.running.insert(pid, (child, start_time));
        Ok(ProcessIdentity { pid, start_time })
    }

    /// The next child that has ended, reaped, or `None`. It never blocks.
    pub fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        let mut ended = None;
        for (pid, (child, start)) in &mut self.running {
            if let Ok(Some(status)) = child.try_wait() {
                let status = match (status.code(), status.signal()) {
                    (Some(code), _) => ExitStatus::Code(code),
                    (None, Some(signal)) => ExitStatus::Signal(signal),
                    (None, None) => ExitStatus::Code(-1),
                };
                ended = Some((*pid, *start, status));
                break;
            }
        }
        let (pid, start_time, status) = ended?;
        self.running.remove(&pid);
        // The process has ended, so its start time is gone; the identity that the host kept holds the original value.
        Some((ProcessIdentity { pid, start_time }, status))
    }

    /// Waits for the exit of the child `pid` and reaps it. It blocks until the child ends: a caller that must not block uses
    /// [`Children::poll_exit`].
    pub fn wait_exit(&mut self, pid: u32) -> Option<(ProcessIdentity, ExitStatus)> {
        let (mut child, start_time) = self.running.remove(&pid)?;
        let status = child.wait().ok()?;
        let status = match (status.code(), status.signal()) {
            (Some(code), _) => ExitStatus::Code(code),
            (None, Some(signal)) => ExitStatus::Signal(signal),
            (None, None) => ExitStatus::Code(-1),
        };
        Some((ProcessIdentity { pid, start_time }, status))
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
