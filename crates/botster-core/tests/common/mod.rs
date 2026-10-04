//! A worker program for the slow tests: a shell script that the real `Core` starts, and the guard that ends it on every exit
//! path, panics included (BUILD.md testing rule 10, plan R12).

use rustix::process::Pid;

#[path = "../../../botster-core-sys/tests/common/process_guard.rs"]
mod process_guard;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

/// Waits without using the CPU and ends when the test process, the worker's parent, is gone: a test that is killed runs no
/// guard, and nothing may outlive it. Each `sleep` is a background job that `wait` waits for, so a signal ends the wait at
/// once. The interval of 1 s bounds how long a worker outlives its parent; it is not a timeout of a test. A `$PPID` of 1 means
/// that the parent was gone before the worker started.
pub const WAIT_WHILE_THE_PARENT_LIVES: &str =
    "while [ \"$PPID\" != 1 ] && kill -0 $PPID 2>/dev/null; do /bin/sleep 1 >/dev/null 2>&1 & wait $!; done";

/// Makes a FIFO. The worker scripts use FIFOs with external commands (`/bin/echo`, `/usr/bin/tee`), never with shell
/// builtins.
pub fn mkfifo(path: &Path) {
    let made = std::process::Command::new("/usr/bin/mkfifo")
        .arg(path)
        .status()
        .expect("mkfifo runs");
    assert!(made.success());
}

/// A worker script registers its group before it records its PID or runs its body.
/// The test owns the anchor before Core can start the worker.
pub struct ScriptWorker {
    pub path: PathBuf,
    pid_file: PathBuf,
    _guard: process_guard::GroupGuard,
}

impl ScriptWorker {
    pub fn new(dir: &Path, body: &str) -> ScriptWorker {
        let path = dir.join("worker.sh");
        let pid_file = dir.join("worker.pid");
        let guard = process_guard::GroupGuard::new(dir);
        std::fs::write(
            &path,
            format!(
                "#!/bin/sh\n{}echo $$ > '{}'\n{body}\n",
                guard.prefix(),
                pid_file.display()
            ),
        )
        .unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        ScriptWorker {
            path,
            pid_file,
            _guard: guard,
        }
    }

    /// The pid of the worker, once its script recorded it.
    pub fn pid(&self) -> Option<Pid> {
        std::fs::read_to_string(&self.pid_file)
            .ok()
            .and_then(|text| text.trim().parse::<i32>().ok())
            .and_then(Pid::from_raw)
    }
}
