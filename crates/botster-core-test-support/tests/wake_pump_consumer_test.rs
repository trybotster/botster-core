//! Isolated Hub-shaped consumer proof for the Core wake pump seam.

use std::fs;
use std::path::PathBuf;
use std::process::Command;

#[test]
fn isolated_hub_data_plane_consumer_uses_one_owner_thread() {
    let consumer =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/consumers/hub-data-plane-shaped");
    let source = fs::read_to_string(consumer.join("src/lib.rs")).expect("read consumer source");
    for required in [
        "std::thread::spawn",
        "CoreDaemon::new",
        "wake_pump_control",
        "wait_pump",
        "pump_woken",
        "session_registry_state",
        ".spawn(",
        ".attach(",
        "bind_waking_terminal_adapter",
        ".input(",
        ".resize(",
        ".detach(",
        ".shutdown(",
    ] {
        assert!(source.contains(required), "consumer must use {required}");
    }
    for forbidden in ["unsafe", "Arc<Mutex<CoreDaemon>>", "WakePumpHost"] {
        assert!(
            !source.contains(forbidden),
            "consumer must not contain {forbidden}"
        );
    }

    let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target");
    let output = Command::new(env!("CARGO"))
        .args(["test", "--quiet", "--offline"])
        .current_dir(&consumer)
        .env("CARGO_TARGET_DIR", workspace_target)
        .output()
        .expect("cargo test hub-data-plane-shaped consumer");
    assert!(
        output.status.success(),
        "hub-data-plane-shaped consumer failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
