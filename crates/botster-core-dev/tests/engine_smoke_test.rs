//! Smoke tests for the dev-only engine harness.

use botster_core_dev::run_engine_smoke;

#[test]
fn dev_harness_exercises_public_engine_path() {
    let report = run_engine_smoke().expect("engine smoke harness should run");

    assert_eq!(report.spawned_session_id.0, "engine-smoke-session");
    assert_eq!(report.attached_client_id.0, "engine-smoke-client");
    assert_eq!(report.terminal_input, "echo engine-smoke\n");
    assert_eq!(report.client_output, "engine-smoke-output\n");
    assert_eq!(report.output_activity_at, Some(10));
    assert_eq!(report.notifications, vec!["Inbox smoke notice"]);
    assert!(report.session_notification_routed);
    assert_eq!(report.plugin_result, "fake plugin handler invoked");
    assert!(report.shutdown_requested);
}
