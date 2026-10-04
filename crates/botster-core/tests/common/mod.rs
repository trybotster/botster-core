//! A worker program for the slow tests: a shell script that the real `Core` starts, and the guard that ends it on every exit
//! path, panics included (BUILD.md testing rule 10).

use rustix::process::{kill_process_group, Pid, Signal};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Makes a FIFO. The worker scripts wait on FIFOs with external commands (`/bin/cat`, `/usr/bin/tee`), never with shell
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

    /// The worker ends by itself now: the guard must not signal a group whose pid may be reused.
    pub fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for ScriptWorker {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let pid = std::fs::read_to_string(&self.pid_file)
            .ok()
            .and_then(|text| text.trim().parse::<i32>().ok())
            .and_then(Pid::from_raw);
        // A test disarms the guard before it lets the worker end by itself, so an armed guard finds the worker alive: the
        // group is still the worker's.
        if let Some(pid) = pid {
            let _ = kill_process_group(pid, Signal::KILL);
        }
    }
}
