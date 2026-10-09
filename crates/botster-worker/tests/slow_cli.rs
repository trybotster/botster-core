//! Checks of the prebuilt worker's command-line boundary.
#![cfg(feature = "slow")]

#[path = "common/candidate.rs"]
mod candidate;
#[path = "../../botster-core-sys/tests/common/process_guard.rs"]
mod process_guard;

use botster_core_contract::prelude::InstanceId;
use botster_core_link::launch::WorkerLaunch;
use std::process::{Command, Output, Stdio};

fn output(launch: Option<&WorkerLaunch>) -> Output {
    use std::os::unix::process::CommandExt;
    let root = tempfile::tempdir().unwrap();
    let guard = process_guard::GroupGuard::new(root.path());
    let mut command = Command::new("/bin/sh");
    command
        .args(["-c", &format!("{}exec \"$@\"", guard.prefix()), "worker"])
        .arg(candidate::worker_binary())
        .env_clear()
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .process_group(0);
    if let Some(launch) = launch {
        command.args(launch.args()).envs(launch.env());
    }
    let child = command.spawn().unwrap();
    let (sent, received) = std::sync::mpsc::channel();
    // This test thread owns the Child through its final reap. The guard owns only group membership.
    let thread = std::thread::spawn(move || {
        let _ = sent.send(child.wait_with_output());
    });
    let result = received
        // timer: deadline — bounds the real command-line worker's exit and output.
        .recv_timeout(std::time::Duration::from_secs(10));
    // The anchor retains group membership even when the test thread has reaped the worker.
    drop(guard);
    let output = result
        .expect("the command-line worker ended before the deadline")
        .unwrap();
    thread.join().unwrap();
    output
}

#[test]
fn invalid_arguments_return_usage_failure() {
    let output = output(None);
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.starts_with(b"botster-worker: "));
    assert!(output.stderr.len() > b"botster-worker: \n".len());
}

#[test]
fn failed_control_connection_returns_driver_failure() {
    let root = tempfile::tempdir().unwrap();
    let launch = WorkerLaunch {
        control: root.path().join("missing"),
        instance: InstanceId("1-1".into()),
        host_epoch: 7,
        token: [5; 32],
        endpoint: root.path().join("e"),
        startup_ms: WorkerLaunch::millis(
            botster_core_contract::prelude::CoreLimits::default().startup,
        ),
    };
    let output = output(Some(&launch));
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.starts_with(b"botster-worker: "));
    assert!(output.stderr.len() > b"botster-worker: \n".len());
}
