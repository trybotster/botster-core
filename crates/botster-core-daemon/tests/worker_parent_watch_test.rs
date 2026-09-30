//! The session worker waits for its first control connection, but not for a
//! launcher that is gone. Once it has a connection it outlives the launcher.

#![cfg(unix)]

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::process::{Child, Command, Stdio};

use botster_core::{encode_hello, PROTOCOL_VERSION};
use botster_core_test_support::bounded_wait::HANG_GUARD;
use botster_core_test_support::fixture_gate::wait_pid_exit;
use botster_core_test_support::real_worker::WorkerBinary;

/// A worker that reported readiness on its control socket, and the launcher
/// pid it was told to watch.
struct WaitingWorker {
    worker: Child,
    launcher: Child,
    socket: std::path::PathBuf,
    dir: std::path::PathBuf,
}

impl WaitingWorker {
    fn start() -> Self {
        let binary = WorkerBinary::from_env().expect("the worker binary is verified");
        let nanos = botster_core_test_support::unique::stamp();
        let dir = std::env::temp_dir().join(format!("bw-{}-{nanos}", std::process::id()));
        let socket = dir.join("w.sock");
        // The launcher stands in as a process that lives until the test ends
        // it: it blocks on its standard input.
        let launcher = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .expect("spawn the stand-in launcher");
        let mut worker = Command::new(&binary.path)
            .arg("--control-socket")
            .arg(&socket)
            .arg("--parent-pid")
            .arg(launcher.id().to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn the worker");
        // Readiness: the worker prints one line once its socket is bound.
        let mut ready = String::new();
        BufReader::new(worker.stdout.as_mut().expect("worker stdout"))
            .read_line(&mut ready)
            .expect("read the readiness line");
        if !ready.starts_with("botster-session-worker-ready") {
            // The worker did not report readiness. End it, so that reading
            // its stderr cannot wait for a live process, and report what it
            // wrote.
            let _ = worker.kill();
            let status = worker.wait().expect("reap the worker");
            let mut stderr = String::new();
            let _ = worker
                .stderr
                .take()
                .expect("worker stderr")
                .read_to_string(&mut stderr);
            panic!("unexpected first line {ready:?}; worker status after the kill {status:?}; stderr {stderr:?}");
        }
        Self {
            worker,
            launcher,
            socket,
            dir,
        }
    }

    /// End the launcher and wait for its exit event.
    fn end_launcher(&mut self) {
        drop(self.launcher.stdin.take());
        assert!(
            wait_pid_exit(self.launcher.id(), HANG_GUARD),
            "the stand-in launcher exited"
        );
        self.launcher.wait().expect("reap the launcher");
    }

    fn worker_stderr(&mut self) -> String {
        let mut text = String::new();
        self.worker
            .stderr
            .take()
            .expect("worker stderr")
            .read_to_string(&mut text)
            .expect("read the worker's stderr");
        text
    }
}

impl Drop for WaitingWorker {
    fn drop(&mut self) {
        // The test's own children, ended by their pids.
        let _ = self.worker.kill();
        let _ = self.worker.wait();
        let _ = self.launcher.kill();
        let _ = self.launcher.wait();
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// No connection ever comes when the launcher is gone: the worker exits, and
/// removes its socket.
#[test]
fn a_worker_stops_waiting_when_its_launcher_exits_before_connecting() {
    let mut waiting = WaitingWorker::start();
    waiting.end_launcher();

    assert!(
        wait_pid_exit(waiting.worker.id(), HANG_GUARD),
        "the worker did not exit after its launcher exited"
    );
    let status = waiting.worker.wait().expect("reap the worker");
    assert!(!status.success(), "the worker reports the failure");
    let stderr = waiting.worker_stderr();
    assert!(
        stderr.contains("launching process exited"),
        "the worker says why it stopped: {stderr:?}"
    );
    assert!(!waiting.socket.exists(), "the worker removed its socket");
}

/// A worker that has a connection keeps serving when the launcher exits: the
/// exit that follows comes from the handshake, not from the launcher watch.
#[test]
fn a_connected_worker_outlives_its_launcher() {
    let mut waiting = WaitingWorker::start();
    let mut link = UnixStream::connect(&waiting.socket).expect("connect to the worker");
    // A hello the worker refuses. The worker's reply to it names the
    // handshake as the reason it stopped, which shows it got past the
    // accept after the launcher was gone.
    waiting.end_launcher();
    link.write_all(&encode_hello(PROTOCOL_VERSION.wrapping_add(1)))
        .expect("send the hello");
    link.flush().expect("flush");

    assert!(
        wait_pid_exit(waiting.worker.id(), HANG_GUARD),
        "the worker did not end the refused handshake"
    );
    let _ = waiting.worker.wait().expect("reap the worker");
    let stderr = waiting.worker_stderr();
    assert!(
        stderr.contains("unsupported parent protocol version"),
        "the worker stopped for the handshake: {stderr:?}"
    );
    assert!(
        !stderr.contains("launching process exited"),
        "the launcher watch fired on a connected worker: {stderr:?}"
    );
}
