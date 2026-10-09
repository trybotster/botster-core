//! A process group that a real-process test owns (BUILD.md testing rule 10): the test starts a program in its own group, and the
//! group ends on every exit path, a panic included.
//!
//! The guard kills only the group that it started, by the pid that it holds. It never kills by name or pattern.
//!
//! **No stale id.** A process group id can be reused after the group is gone. The guard keeps the leader unreaped until it has
//! signalled the group: an unreaped leader, even an exited one, keeps its pid and so its group id (POSIX). The guard hands out
//! the pipes of the child, never the child, so nothing outside it can reap the leader early. Cleanup retires the id: a second
//! cleanup, or the drop after a cleanup, signals nothing.

use rustix::process::{waitid, Pid, Signal, WaitId, WaitIdOptions};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command};

/// A child in its own process group. Dropping the guard kills the whole group and reaps the child.
#[derive(Debug)]
pub struct OwnedGroup {
    child: Child,
    /// The group id while the group may still exist. `None` once cleanup has run.
    group: Option<Pid>,
}

impl OwnedGroup {
    /// Starts `command` as the leader of a new process group.
    pub fn spawn(mut command: Command) -> io::Result<OwnedGroup> {
        command.process_group(0);
        let child = command.spawn()?;
        let pid = i32::try_from(child.id())
            .ok()
            .and_then(Pid::from_raw)
            .ok_or_else(|| io::Error::other("the child has no valid pid"))?;
        Ok(OwnedGroup {
            child,
            group: Some(pid),
        })
    }

    /// The pid of the group leader, which is the id of the group.
    pub fn pid(&self) -> u32 {
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

    /// True when the leader has exited. The leader stays unreaped (a zombie keeps its pid), so the group id stays ours until
    /// cleanup. After cleanup the answer is true as well.
    pub fn leader_exited(&self) -> io::Result<bool> {
        let Some(group) = self.group else {
            return Ok(true);
        };
        let status = waitid(
            WaitId::Pid(group),
            WaitIdOptions::EXITED | WaitIdOptions::NOHANG | WaitIdOptions::NOWAIT,
        )?;
        Ok(status.is_some())
    }

    /// Blocks until the leader has exited, without reaping it: the group id stays ours. Returns at once after cleanup.
    pub fn wait_for_leader_exit(&self) -> io::Result<()> {
        let Some(group) = self.group else {
            return Ok(());
        };
        waitid(
            WaitId::Pid(group),
            WaitIdOptions::EXITED | WaitIdOptions::NOWAIT,
        )?;
        Ok(())
    }

    /// True until cleanup has run: the group id is still held.
    pub fn is_active(&self) -> bool {
        self.group.is_some()
    }

    /// Ends the group and reaps the leader. It runs once: the id is retired first, so a repeat, or the drop after it, signals
    /// nothing.
    pub fn kill(&mut self) {
        let Some(group) = self.group.take() else {
            return;
        };
        // The leader is unreaped here, so the group id is still ours. ESRCH would mean that the group is already empty: nothing
        // is left to kill, and that is the goal.
        let _ = botster_core_sys::signal::signal_group(
            group.as_raw_nonzero().get().unsigned_abs(),
            Signal::KILL,
        );
        let _ = self.child.wait();
    }
}

impl Drop for OwnedGroup {
    fn drop(&mut self) {
        self.kill();
    }
}
