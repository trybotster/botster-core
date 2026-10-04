//! A worker program for the slow tests: a shell script that the real `Core` starts, and the guard that ends it on every exit
//! path, panics included (BUILD.md testing rule 10, plan R12).

use rustix::process::{kill_process_group, waitpid, Pid, Signal, WaitOptions};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Waits without using the CPU and ends when the test process, the worker's parent, is gone: a test that is killed runs no
/// guard, and nothing may outlive it. Each `sleep` is a background job that `wait` waits for, so a signal ends the wait at
/// once. The interval of 1 s bounds how long a worker outlives its parent; it is not a timeout of a test.
pub const WAIT_WHILE_THE_PARENT_LIVES: &str =
    "while kill -0 $PPID 2>/dev/null; do /bin/sleep 1 >/dev/null 2>&1 & wait $!; done";

/// Makes a FIFO. The worker scripts use FIFOs with external commands (`/bin/echo`, `/usr/bin/tee`), never with shell
/// builtins.
pub fn mkfifo(path: &Path) {
    let made = std::process::Command::new("/usr/bin/mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
}

/// A worker program: a shell script that first records its pid, then runs `body`. `Core` starts a worker as the leader of
/// its own process group, so the guard ends the whole worker, with every command of the script, by that group.
pub struct ScriptWorker {
    pub path: PathBuf,
    pid_file: PathBuf,
    armed: bool,
}

impl ScriptWorker {
    pub fn new(dir: &Path, body: &str) -> ScriptWorker {
        let path = dir.join("worker.sh");
        let pid_file = dir.join("worker.pid");
        std::fs::write(
            &path,
            format!("#!/bin/sh\necho $$ > '{}'\n{body}\n", pid_file.display()),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        ScriptWorker {
            path,
            pid_file,
            armed: true,
        }
    }

    /// The pid of the worker, once its script recorded it.
    pub fn pid(&self) -> Option<Pid> {
        std::fs::read_to_string(&self.pid_file)
            .ok()
            .and_then(|text| text.trim().parse::<i32>().ok())
            .and_then(Pid::from_raw)
    }

    /// The worker has ended and was reaped: the guard must not signal a group whose pid may be reused.
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ScriptWorker {
    /// Kills the worker's group and reaps the worker in test code, whatever `Core` did: the worker is a child of the test
    /// process, and this `waitpid` reaps it unless `Core`'s reaper thread reaped it first.
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Some(pid) = self.pid() {
            let _ = kill_process_group(pid, Signal::KILL);
            let _ = waitpid(Some(pid), WaitOptions::empty());
        }
    }
}
