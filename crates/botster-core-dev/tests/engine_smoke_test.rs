//! Smoke tests for the dev-only engine harness.

use botster_core_dev::run_engine_smoke;

#[cfg(unix)]
#[test]
fn dev_harness_exercises_non_hub_host_profile_engine_path() {
    let report = run_engine_smoke().expect("engine smoke harness should run");

    assert!(report.ran_real_embedder);
    assert_eq!(report.admitted_host_profile_id, "minimal-test-host");
    assert_eq!(
        report.admitted_required_capability,
        botster_core::Capability {
            surface: botster_core::CapabilitySurface::Mcp,
            scope: Some("minimal-test-host.run".to_string())
        }
    );
    assert!(report.admitted_capability_drove_plugin_handler);
    assert_eq!(
        report.engine_surface,
        "BotsterEngine<LocalProcessRuntime, LocalProcessWorkerRuntime>"
    );
    assert!(report.single_engine_session_and_plugin);
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
    assert_eq!(
        report.screen_text, "",
        "generic local worker path exposes ScreenReady but does not own a shadow terminal parser"
    );
    assert_eq!(report.snapshot_bytes, 0);
    assert_eq!(report.snapshot_size, Some((30, 100)));
    assert_eq!(
        report.activity_status,
        botster_core::SessionActivityStatus::Active
    );
    assert!(report.plugin_invocation_completed);
    assert_eq!(report.plugin_invocation_value, "allowed");
    assert!(report.plugin_missing_capability_rejected);
    assert_eq!(report.plugin_denial_failure_kind, "HandlerFailed");
    assert!(report.denied_plugin_runtime_not_called);
    assert_eq!(
        report.custom_host_requirements,
        vec![
            "host Botster version",
            "enablement decision",
            "source provenance",
            "bootstrap entrypoint",
            "required provider names",
            "required capabilities",
            "explicit spawn request fields",
            "client and subscription ids",
            "logical clocks",
            "plugin worker registration and runtime"
        ]
    );
    assert!(report.shutdown_observed);
}
