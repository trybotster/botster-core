//! A child that a test starts and owns on every path (BUILD.md testing rule 10): its status is read only after its exit was
//! observed within a deadline, and its drop ends it and reaps it, a panic included. In group mode the child leads its own
//! group, and the drop ends every member of that group in the rounds of `rounds.rs`, with the unreaped leader as the reserve
//! that holds the group id.
//!
//! The owner hands out the pipes of the child, never the `Child`: nothing outside it can reap the child early, so its pid
//! (and, in group mode, the group id) stays its own until the drop. It reaps only this child, by its exact pid.

use crate::platform::{await_end, live_members, pid, Waited};
use crate::rounds::{end_members, reserved_kill};
use crate::{fail, Deadline};
use rustix::process::Pid;
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus};

/// A child of the test, owned on every path. See the module documentation.
#[derive(Debug)]
pub struct OwnedChild {
    child: Child,
    pid: Pid,
    /// Whether the child leads its own group, which the drop ends.
    group: bool,
    /// The status, once the child was reaped.
    status: Option<ExitStatus>,
}

impl OwnedChild {
    /// Starts `command` as a child of the test.
    ///
    /// # Errors
    /// The spawn failed.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Self::start(command, false)
    }

    /// Starts `command` as the leader of a new process group, which the drop ends.
    ///
    /// # Errors
    /// The spawn failed.
    pub fn spawn_group(command: &mut Command) -> io::Result<Self> {
        command.process_group(0);
        Self::start(command, true)
    }

    fn start(command: &mut Command, group: bool) -> io::Result<Self> {
        let child = command.spawn()?;
        let pid = pid(child.id())?;
        Ok(Self {
            child,
            pid,
            group,
            status: None,
        })
    }

    /// The pid of the child; in group mode, also the id of its group.
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    pub fn take_stdin(&mut self) -> Option<ChildStdin> {
        self.child.stdin.take()
    }

    pub fn take_stdout(&mut self) -> Option<ChildStdout> {
        self.child.stdout.take()
    }

    pub fn take_stderr(&mut self) -> Option<ChildStderr> {
        self.child.stderr.take()
    }

    /// Sends `SIGKILL` to the child alone (not its group). The child is unreaped until the owner reaps it, so the pid is
    /// still this child's.
    ///
    /// # Errors
    /// The signal failed, or the child was already reaped.
    pub fn kill(&mut self) -> io::Result<()> {
        if self.status.is_some() {
            return Err(io::Error::other("the child was already reaped"));
        }
        self.child.kill()
    }

    /// Whether the child has exited, observed within `deadline` without reaping it: in group mode the unreaped leader keeps
    /// the group id for the drop.
    ///
    /// # Errors
    /// The exit could not be observed.
    pub fn exited_within(&self, deadline: Deadline) -> io::Result<bool> {
        if self.status.is_some() {
            return Ok(true);
        }
        Ok(await_end(self.pid, deadline)? != Waited::Deadline)
    }

    /// The exit status of the child, once its exit was observed within the cleanup bound. In group mode the drop still ends
    /// the group's other members; the leader's reap releases the group id only after it.
    ///
    /// # Panics
    /// The child did not end within the bound (it stays owned, and the drop still ends it), or it cannot be observed.
    pub fn status(&mut self) -> ExitStatus {
        self.status_by(Deadline::cleanup())
    }

    /// The exit status of the child, once its exit was observed by `deadline`.
    ///
    /// # Panics
    /// The child did not end by the deadline (it stays owned, and the drop still ends it), or it cannot be observed.
    pub fn status_by(&mut self, deadline: Deadline) -> ExitStatus {
        if let Some(status) = self.status {
            return status;
        }
        match self.exited_within(deadline) {
            Ok(true) => {}
            Ok(false) => panic!(
                "the test child {} did not end within {:?}",
                self.id(),
                deadline.limit()
            ),
            Err(error) => panic!("the test child {} cannot be observed: {error}", self.id()),
        }
        if self.group {
            // The leader holds the group id until the group's other members are ended: the drop does both.
            return self.observed_status();
        }
        let status = self.reap().unwrap_or_else(|error| {
            panic!("the test child {} cannot be reaped: {error}", self.id())
        });
        self.status = Some(status);
        status
    }

    /// The status of the exited, unreaped leader, read without reaping it.
    fn observed_status(&self) -> ExitStatus {
        use rustix::process::{waitid, WaitId, WaitIdOptions};
        use std::os::unix::process::ExitStatusExt;
        // The exit was observed: the wait (which never reaps) returns once the exit completes.
        let options = WaitIdOptions::EXITED | WaitIdOptions::NOWAIT;
        match waitid(WaitId::Pid(self.pid), options) {
            Ok(Some(status)) => match (status.exit_status(), status.terminating_signal()) {
                (Some(code), _) => ExitStatus::from_raw((code & 0xff) << 8),
                (None, Some(signal)) => ExitStatus::from_raw(signal & 0x7f),
                (None, None) => panic!("the test child {} has no exit status", self.id()),
            },
            Ok(None) => panic!("the test child {} has not exited", self.id()),
            Err(error) => panic!("the test child {} cannot be observed: {error}", self.id()),
        }
    }

    /// Reaps the child, whose exit was observed: a wait for its exact pid, which returns once the exit completes. (macOS
    /// sends the exit event when the exit starts, so a `WNOHANG` wait can still find nothing at that moment.)
    fn reap(&mut self) -> io::Result<ExitStatus> {
        self.child.wait()
    }

    /// The end of the child on the drop path: in group mode the rounds over its group (the unreaped leader is the reserve),
    /// otherwise a kill of its pid. Then its reap.
    fn end(&mut self) -> Result<(), String> {
        if self.status.is_some() {
            return Ok(());
        }
        let deadline = Deadline::cleanup();
        let id = self.id();
        if self.group {
            end_members(
                || reserved_kill(self.pid, self.pid),
                || live_members(self.pid),
                |member| await_end(member.pid, deadline),
                || deadline.expired(),
            )
            .map_err(|failure| format!("the group {id}: {}", failure.report(deadline.limit())))?;
        } else {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.status = Some(status);
                    return Ok(());
                }
                Ok(None) => {}
                Err(error) => {
                    return Err(format!("the test child {id} cannot be checked: {error}"))
                }
            }
            self.child
                .kill()
                .map_err(|error| format!("the test child {id} cannot be killed: {error}"))?;
        }
        match await_end(self.pid, deadline) {
            Ok(Waited::Exited | Waited::Gone) => {}
            Ok(Waited::Deadline) => {
                return Err(format!(
                    "the test child {id} did not end within {:?} after SIGKILL",
                    deadline.limit()
                ))
            }
            Err(error) => return Err(format!("the test child {id} cannot be observed: {error}")),
        }
        let status = self
            .reap()
            .map_err(|error| format!("the test child {id} cannot be reaped: {error}"))?;
        self.status = Some(status);
        Ok(())
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Err(report) = self.end() {
            fail(report);
        }
    }
}
