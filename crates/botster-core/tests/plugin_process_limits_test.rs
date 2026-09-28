//! Real-process tests of the plugin process host's resource limits (plan
//! section 8): the memory cap, `RLIMIT_CPU`, and `RLIMIT_NOFILE`.
//!
//! Requires `script/prebuild-worker`.
#![cfg(all(feature = "local-runtime", unix))]

use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use botster_core::actor::{PluginHandlerKind, PluginHandlerRef, PluginInvocationContext};
use botster_core::runtime::plugin_process::{
    LoadFrame, PluginConfig, PluginExitCause, PluginLogCredits, PluginProcess, PluginProcessConfig,
    PluginProcessExited, PluginProcessRlimits, PluginReplyCredits, PluginSources, SandboxProfile,
};
use botster_core::session::RequestId;
use botster_core::{
    BoundaryJson, PluginCancellationToken, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginKey, PluginRuntime,
};
use botster_core_test_support::real_worker::WorkerBinary;
use serde_json::{json, Value};

/// Bound for every event a test waits for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(30);
/// A deadline that must not expire during a test.
const GENEROUS: Duration = Duration::from_secs(30);

fn config() -> PluginProcessConfig {
    PluginProcessConfig {
        worker_path: WorkerBinary::plugin_test_worker_from_env()
            .unwrap_or_else(|failure| panic!("{failure}"))
            .path,
        cwd: std::env::temp_dir(),
        env: Vec::new(),
        rlimits: PluginProcessRlimits::default(),
        sandbox: SandboxProfile(BoundaryJson(json!({}))),
        memory_cap_bytes: None,
        max_frame_bytes: 1024 * 1024,
        startup_deadline: GENEROUS,
        shutdown_deadline: GENEROUS,
        cancel_grace: GENEROUS,
        max_in_flight_invokes: 2,
        stderr_tail_bytes: 4096,
        ingress_bytes: 64 * 1024,
        reply_credits: PluginReplyCredits {
            count: 2,
            bytes: 64 * 1024,
        },
        log_credits: PluginLogCredits {
            count: 8,
            bytes: 16 * 1024,
        },
    }
}

fn start(config: PluginProcessConfig) -> Arc<PluginProcess> {
    let load = LoadFrame {
        sources: PluginSources(BoundaryJson(json!({}))),
        config: PluginConfig(BoundaryJson(json!({ "mode": "report" }))),
    };
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = tx.send(PluginProcess::spawn(&config, &load));
    });
    let (process, _) = rx
        // timer: deadline — the worker reaches Loaded; expiry fails the test
        .recv_timeout(EVENT_DEADLINE)
        .expect("spawn resolves")
        .expect("loaded");
    Arc::new(process)
}

fn request(handler_id: &str, payload: Value) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId(format!("limits-{handler_id}")),
        handler: PluginHandlerRef {
            plugin_key: PluginKey("limits".to_string()),
            kind: PluginHandlerKind::Command,
            handler_id: handler_id.to_string(),
        },
        timeout_ms: 60_000,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: None,
            metadata: None,
        },
        payload: BoundaryJson(payload),
    }
}

fn run(process: &Arc<PluginProcess>, handler_id: &str, payload: Value) -> PluginInvocationResult {
    let (tx, rx) = mpsc::channel();
    let process = process.clone();
    let request = request(handler_id, payload);
    thread::spawn(move || {
        let _ = tx.send(PluginRuntime::invoke(
            &*process,
            request,
            PluginCancellationToken::new(),
        ));
    });
    // timer: deadline — the invocation settles by result or exit; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .expect("the invocation settles")
}

fn await_exit(process: &PluginProcess) -> PluginProcessExited {
    let (tx, rx) = mpsc::channel();
    process.install_exit_notifier(Arc::new(move || {
        let _ = tx.send(());
    }));
    // timer: deadline — the exit watch reports the reaped exit; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .expect("the exit watch reaps the process");
    process.exit().expect("recorded before the notifier")
}

fn failure_kind(result: &PluginInvocationResult) -> PluginInvocationFailureKind {
    match result {
        PluginInvocationResult::Failed(failure) => failure.kind.clone(),
        PluginInvocationResult::Completed(_) => panic!("expected a failure, got {result:?}"),
    }
}

#[test]
fn allocation_under_the_memory_cap_completes() {
    let mut config = config();
    config.memory_cap_bytes = Some(64 * 1024 * 1024);
    let process = start(config);
    let result = run(
        &process,
        "allocate",
        json!({ "step": 1024 * 1024, "steps": 8 }),
    );
    assert!(
        matches!(&result, PluginInvocationResult::Completed(_)),
        "{result:?}"
    );
    assert!(process.exit().is_none());
}

#[test]
fn allocation_over_the_memory_cap_is_a_memory_cap_kill() {
    let mut config = config();
    config.memory_cap_bytes = Some(64 * 1024 * 1024);
    let process = start(config);
    let result = run(&process, "allocate", json!({ "step": 1024 * 1024 }));
    assert_eq!(
        failure_kind(&result),
        PluginInvocationFailureKind::WorkerKilled
    );
    assert_eq!(await_exit(&process).cause, PluginExitCause::MemoryCap);
}

/// The kernel signal at the `RLIMIT_CPU` limit. Core sets soft = hard, and
/// Linux checks the hard limit (SIGKILL) before the soft one (SIGXCPU);
/// macOS sends SIGXCPU.
#[cfg(target_os = "linux")]
const CPU_LIMIT_SIGNAL: i32 = libc::SIGKILL;
#[cfg(not(target_os = "linux"))]
const CPU_LIMIT_SIGNAL: i32 = libc::SIGXCPU;

#[test]
fn the_cpu_rlimit_ends_a_spinning_worker_as_a_crash_not_a_parent_kill() {
    let mut config = config();
    config.rlimits.cpu_seconds = Some(1);
    let process = start(config);
    let result = run(&process, "spin", json!(null));
    assert_eq!(
        failure_kind(&result),
        PluginInvocationFailureKind::WorkerCrashed
    );
    assert_eq!(
        await_exit(&process).cause,
        PluginExitCause::Crashed {
            signal: Some(CPU_LIMIT_SIGNAL),
            code: None
        }
    );
}

#[test]
fn the_open_files_rlimit_bounds_the_worker() {
    let mut config = config();
    config.rlimits.open_files = Some(16);
    let process = start(config);
    let result = run(&process, "open_files", json!(null));
    let PluginInvocationResult::Completed(success) = &result else {
        panic!("{result:?}");
    };
    let report = &success.payload.as_ref().expect("a report").0;
    assert_eq!(report["errno"], json!(libc::EMFILE));
    // Descriptors 0-4 come from the parent and 5 is the worker's parent-exit
    // watch; the rest of the limit is open.
    assert_eq!(report["opened"], json!(16 - 6));
}
