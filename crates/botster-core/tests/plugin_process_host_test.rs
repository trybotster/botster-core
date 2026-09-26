//! Real-process tests of the plugin process host: launch hygiene, startup
//! supervision through `Loaded`, the sole reaper, and exit classification.
//!
//! Requires `script/prebuild-worker`; the scripted test worker is verified
//! against the candidate manifest before any test starts it.
#![cfg(all(feature = "local-runtime", unix))]

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use botster_core::runtime::plugin_process::{
    LoadFrame, PluginConfig, PluginExitCause, PluginKillReason, PluginProcess, PluginProcessConfig,
    PluginProcessError, PluginProcessExited, PluginProcessRlimits, PluginRegistration,
    PluginSources, SandboxProfile,
};
use botster_core::BoundaryJson;
use botster_core_test_support::real_worker::WorkerBinary;
use serde_json::{json, Value};

/// Bound for every event a test waits for; expiry fails the test instead of
/// hanging the harness.
const EVENT_DEADLINE: Duration = Duration::from_secs(30);
/// Startup bound for tests that expect the worker to finish its startup.
const GENEROUS_STARTUP: Duration = Duration::from_secs(30);
const OPEN_FILES: u64 = 64;

fn worker() -> PathBuf {
    WorkerBinary::plugin_test_worker_from_env()
        .unwrap_or_else(|failure| panic!("{failure}"))
        .path
}

fn cwd() -> PathBuf {
    std::env::temp_dir()
        .canonicalize()
        .expect("canonical temp dir")
}

fn config(sandbox: Value) -> PluginProcessConfig {
    PluginProcessConfig {
        worker_path: worker(),
        cwd: cwd(),
        env: vec![(
            OsString::from("PLUGIN_ENV_PROBE"),
            OsString::from("present"),
        )],
        rlimits: PluginProcessRlimits {
            open_files: Some(OPEN_FILES),
            ..PluginProcessRlimits::default()
        },
        sandbox: SandboxProfile(BoundaryJson(sandbox)),
        memory_cap_bytes: None,
        max_frame_bytes: 1024 * 1024,
        startup_deadline: GENEROUS_STARTUP,
        shutdown_deadline: GENEROUS_STARTUP,
        stderr_tail_bytes: 4096,
    }
}

fn load(mode: &str) -> LoadFrame {
    LoadFrame {
        sources: PluginSources(BoundaryJson(json!({}))),
        config: PluginConfig(BoundaryJson(json!({ "mode": mode }))),
    }
}

type SpawnResult = Result<(PluginProcess, PluginRegistration), PluginProcessError>;

/// Spawn on a helper thread, so a startup that never resolves fails the test
/// at the event bound instead of hanging it.
fn spawn(config: PluginProcessConfig, load: LoadFrame) -> SpawnResult {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(PluginProcess::spawn(&config, &load));
    });
    // timer: deadline — spawn resolves at Loaded or after the reaped exit; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .expect("spawn must resolve through Loaded or a reaped exit")
}

fn startup_exit(result: SpawnResult) -> PluginProcessExited {
    match result {
        Err(PluginProcessError::Exited(exit)) => exit,
        Err(other) => panic!("expected an exit during startup, got {other}"),
        Ok(_) => panic!("expected an exit during startup, got Loaded"),
    }
}

/// Wait for the exit watch to reap the process, as an event.
fn await_exit(process: &PluginProcess) -> PluginProcessExited {
    let (tx, rx) = mpsc::channel();
    process.install_exit_notifier(Arc::new(move || {
        let _ = tx.send(());
    }));
    // timer: deadline — the exit watch reports the reaped exit; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .expect("the exit watch must reap the process");
    process
        .exit()
        .expect("an exit is recorded before the notifier runs")
}

#[test]
fn a_worker_starts_with_only_the_allowlisted_environment_and_descriptors() {
    // A descriptor that the parent opened without close-on-exec: the child's
    // hygiene step must close it.
    // SAFETY: F_DUPFD of stdin returns a new descriptor, without
    // close-on-exec, at or above 64, which this test owns.
    let leaked = unsafe { libc::fcntl(0, libc::F_DUPFD, 64) };
    assert!(
        leaked >= 64,
        "the leak probe must sit above the kept descriptors"
    );

    let (process, registration) = spawn(config(json!({})), load("report")).expect("loaded");
    // SAFETY: closes the descriptor that this test duplicated above.
    unsafe { libc::close(leaked) };

    let report = registration.0 .0;
    let env: BTreeMap<String, String> =
        serde_json::from_value(report["env"].clone()).expect("env map");
    assert_eq!(
        env,
        BTreeMap::from([("PLUGIN_ENV_PROBE".to_string(), "present".to_string())])
    );
    assert_eq!(report["fds"], json!([0, 1, 2, 3, 4]));
    assert_eq!(report["cwd"], json!(cwd()));
    assert_eq!(report["nofile"], json!(OPEN_FILES));
    assert_eq!(report["sandbox_applied_before_load"], json!(true));

    process.stop();
    assert_eq!(await_exit(&process).cause, PluginExitCause::Stopped);
}

#[test]
fn a_failing_sandbox_hook_fails_bootstrap_and_the_process_is_reaped() {
    match spawn(config(json!({ "fail": "sandbox denied" })), load("report")) {
        Err(PluginProcessError::BootstrapFailed { reason, exit }) => {
            assert_eq!(reason, "sandbox denied");
            assert!(exit.pid > 0);
        }
        Err(other) => panic!("expected BootstrapFailed, got {other}"),
        Ok(_) => panic!("expected BootstrapFailed, got Loaded"),
    }
}

#[test]
fn a_failing_load_reports_load_failed_and_the_process_is_reaped() {
    match spawn(config(json!({})), load("fail")) {
        Err(PluginProcessError::LoadFailed { reason, .. }) => {
            assert_eq!(reason, "scripted load failure");
        }
        Err(other) => panic!("expected LoadFailed, got {other}"),
        Ok(_) => panic!("expected LoadFailed, got Loaded"),
    }
}

#[test]
fn a_stuck_load_is_killed_at_the_startup_deadline() {
    let mut config = config(json!({}));
    config.startup_deadline = Duration::from_millis(200);
    let exit = startup_exit(spawn(config, load("spin")));
    assert_eq!(
        exit.cause,
        PluginExitCause::Killed(PluginKillReason::StartupDeadline)
    );
}

#[test]
fn a_child_that_closes_its_channel_but_stays_alive_is_killed() {
    let exit = startup_exit(spawn(config(json!({})), load("close_ipc_then_wait")));
    assert_eq!(
        exit.cause,
        PluginExitCause::Killed(PluginKillReason::TransportClosed)
    );
}

#[test]
fn a_panic_is_reported_through_the_fatal_cause_pipe() {
    let exit = startup_exit(spawn(config(json!({})), load("panic")));
    assert_eq!(exit.cause, PluginExitCause::Panic);
    let stderr = String::from_utf8_lossy(&exit.stderr_tail);
    assert!(
        stderr.contains("scripted panic during load"),
        "stderr tail: {stderr}"
    );
}

#[test]
fn an_abort_is_a_crash_by_signal_not_a_transport_close() {
    let exit = startup_exit(spawn(config(json!({})), load("abort")));
    assert_eq!(
        exit.cause,
        PluginExitCause::Crashed {
            signal: Some(libc::SIGABRT),
            code: None,
        }
    );
}

#[test]
fn a_requested_kill_is_classified_as_requested() {
    let (process, _) = spawn(config(json!({})), load("report")).expect("loaded");
    process.kill();
    assert_eq!(
        await_exit(&process).cause,
        PluginExitCause::Killed(PluginKillReason::Requested)
    );
}

#[test]
fn a_dropped_handle_still_stops_and_reaps_its_process() {
    let (process, _) = spawn(config(json!({})), load("report")).expect("loaded");
    let (tx, rx) = mpsc::channel();
    process.install_exit_notifier(Arc::new(move || {
        let _ = tx.send(());
    }));
    drop(process);
    // timer: deadline — the exit watch reaps after the handle is gone; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .expect("the exit watch reaps a stopped process after its handle is dropped");
}
