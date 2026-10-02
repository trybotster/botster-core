//! A process group that a real-process test owns (BUILD.md testing rule 10): the test starts a program in its own group, and the
//! group ends on every exit path, a panic included.
//!
//! The guard kills only the group that it started, by the pid that it holds. It never kills by name or pattern.

use rustix::process::{kill_process_group, Pid, Signal};
use std::io;
use std::os::unix::process::CommandExt;
use std::process::{Child, Command};

/// A child in its own process group. Dropping the guard kills the whole group and reaps the child.
#[derive(Debug)]
pub struct OwnedGroup {
    child: Child,
    group: Pid,
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
        Ok(OwnedGroup { child, group: pid })
    }

    /// The pid of the group leader, which is the id of the group.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// The child, for its pipes and its status.
    pub fn child(&mut self) -> &mut Child {
        &mut self.child
    }

    /// Ends the group now and reaps the leader. A group that is already gone is not an error.
    pub fn kill(&mut self) {
        // ESRCH means that the group already ended: nothing is left to kill, and that is the goal.
        let _ = kill_process_group(self.group, Signal::KILL);
        let _ = self.child.wait();
    }
}

impl Drop for OwnedGroup {
    fn drop(&mut self) {
        self.kill();
    }
}
