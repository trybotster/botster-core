//! Checks of the prebuilt worker's command-line boundary.
#![cfg(feature = "slow")]

#[path = "common/candidate.rs"]
mod candidate;

use botster_core_contract::prelude::InstanceId;
use botster_core_link::launch::WorkerLaunch;
use std::process::Command;

#[test]
fn invalid_arguments_return_usage_failure() {
    let output = Command::new(candidate::worker_binary())
        .env_clear()
        .output()
        .unwrap();
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
    };
    let output = Command::new(candidate::worker_binary())
        .args(launch.args())
        .env_clear()
        .envs(launch.env())
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert!(output.stderr.starts_with(b"botster-worker: "));
    assert!(output.stderr.len() > b"botster-worker: \n".len());
}
