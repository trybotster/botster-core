//! A child that a test starts and owns on every path (BUILD.md testing rule 10): its status is read only after its exit was
//! observed within a deadline, and its drop ends it and reaps it, a panic included. In group mode the child leads its own
//! group, and the drop ends every member of that group in the rounds of `rounds.rs`, with the unreaped leader as the reserve
//! that holds the group id.
//!
//! The owner hands out the pipes of the child, never the `Child`: nothing outside it can reap the child early, so its pid
//! (and, in group mode, the group id) stays its own until the drop. It reaps only this child, by its exact pid.

use crate::platform::{await_end, await_status, live_members, pid, Waited};
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

    /// The exit status of the child, once it is available within the cleanup bound. In group mode the drop still ends the
    /// group's other members; the leader's reap releases the group id only after it.
    ///
    /// # Panics
    /// The child did not end within the bound (it stays owned, and the drop still ends it), or it cannot be observed.
    pub fn status(&mut self) -> ExitStatus {
        self.status_by(Deadline::cleanup())
    }

    /// The exit status of the child, once it is available by `deadline`.
    ///
    /// # Panics
    /// The child did not end by the deadline (it stays owned, and the drop still ends it), or it cannot be observed.
    pub fn status_by(&mut self, deadline: Deadline) -> ExitStatus {
        match self.exit_by(deadline) {
            Ok(Some(status)) => status,
            Ok(None) => panic!(
                "the test child {} did not end within {:?}",
                self.id(),
                deadline.limit()
            ),
            Err(error) => panic!("the test child {} cannot be observed: {error}", self.id()),
        }
    }

    /// The exit status of the child once it is available by `deadline`, or `None` when the deadline came first (the child
    /// stays owned, and the drop still ends it). Outside group mode the child is then reaped, by its exact pid. In group
    /// mode the leader stays unreaped: it holds the group id until the drop has ended the group's other members.
    ///
    /// # Errors
    /// The child could not be observed or reaped.
    pub fn exit_by(&mut self, deadline: Deadline) -> io::Result<Option<ExitStatus>> {
        if let Some(status) = self.status {
            return Ok(Some(status));
        }
        if self.group {
            return await_status(self.pid, deadline);
        }
        let status = reap_within(&mut self.child, deadline)?;
        self.status = status;
        Ok(status)
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
        match reap_within(&mut self.child, deadline) {
            Ok(Some(status)) => {
                self.status = Some(status);
                Ok(())
            }
            Ok(None) => Err(format!(
                "the test child {id} did not end within {:?} after SIGKILL",
                deadline.limit()
            )),
            Err(error) => Err(format!("the test child {id} cannot be reaped: {error}")),
        }
    }
}

/// Reaps `child`, a child of this process, once its status is available by `deadline`: `None` when the deadline came first
/// (the child stays unreaped). The status is available before the reap, so the reap of the exact pid does not block.
///
/// # Errors
/// The child could not be observed or reaped.
pub(crate) fn reap_within(child: &mut Child, deadline: Deadline) -> io::Result<Option<ExitStatus>> {
    if await_status(pid(child.id())?, deadline)?.is_none() {
        return Ok(None);
    }
    match child.try_wait()? {
        Some(status) => Ok(Some(status)),
        None => Err(io::Error::other(format!(
            "the child {} has no status after its exit",
            child.id()
        ))),
    }
}

impl Drop for OwnedChild {
    fn drop(&mut self) {
        if let Err(report) = self.end() {
            fail(report);
        }
    }
}
