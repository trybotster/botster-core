//! Smoke tests for the dev-only engine harness.

use botster_core_dev::run_engine_smoke;

#[cfg(unix)]
#[test]
fn dev_harness_exercises_real_default_engine_path() {
    let report = run_engine_smoke().expect("engine smoke harness should run");

    assert!(report.ran_real_embedder);
    assert_eq!(report.spawned_session_id.0, "real-embedder-session");
    assert_eq!(report.attached_client_id.0, "real-embedder-client");
    assert_eq!(report.executable, "sh");
    assert_eq!(report.working_directory, ".");
    assert!(
        report.startup_output.contains("ready"),
        "startup output should arrive through subscribed client egress"
    );
    assert_eq!(report.terminal_input, "ping-embedder\n");
    assert!(
        report.echoed_output.contains("echo:ping-embedder"),
        "input should reach the local command and echo through client egress"
    );
    assert_eq!(report.resized_to, Some((30, 100)));
    assert!(
        report.screen_text.contains("echo:ping-embedder"),
        "read screen should return plain terminal text from the default local runtime"
    );
    assert!(
        report.snapshot_bytes > 0,
        "capture snapshot should return opaque snapshot payload bytes"
    );
    assert_eq!(report.snapshot_size, Some((30, 100)));
    assert_eq!(
        report.activity_status,
        botster_core::SessionActivityStatus::Active
    );
    assert!(report.shutdown_observed);
}
