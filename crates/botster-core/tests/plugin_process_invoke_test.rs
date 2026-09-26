//! Real-process tests of the plugin process host's invoke path: forwarding,
//! cancel of queued and running invocations, the cancel-grace kill, crash
//! containment, group kill, shutdown supervision, and protocol violations.
//!
//! Requires `script/prebuild-worker`.
#![cfg(all(feature = "local-runtime", unix))]

use std::ffi::{CString, OsString};
use std::fs::OpenOptions;
use std::io::Read;
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use botster_core::actor::{PluginHandlerKind, PluginHandlerRef, PluginInvocationContext};
use botster_core::engine::{
    PluginHandlerRegistration, PluginWorkerEngine, PluginWorkerEngineConfig,
    PluginWorkerRegistration,
};
use botster_core::runtime::plugin_process::{
    LoadFrame, PluginConfig, PluginExitCause, PluginKillReason, PluginProcess, PluginProcessConfig,
    PluginProcessExited, PluginProcessRlimits, PluginSources, SandboxProfile,
};
use botster_core::session::RequestId;
use botster_core::{
    BoundaryJson, PluginCancellationToken, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginKey, PluginLoadSpec, PluginRuntime,
};
use botster_core_test_support::real_worker::WorkerBinary;
use serde_json::json;

/// Bound for every event a test waits for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(30);
/// A deadline that must not expire during a test that expects progress.
const GENEROUS: Duration = Duration::from_secs(30);
/// A deadline that a test expects to expire.
const SHORT: Duration = Duration::from_millis(200);

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
    // timer: deadline — the worker reaches Loaded; expiry fails the test
    let (process, _) = rx
        .recv_timeout(EVENT_DEADLINE)
        .expect("spawn resolves")
        .expect("loaded");
    Arc::new(process)
}

fn plugin() -> PluginKey {
    PluginKey("process-plugin".to_string())
}

fn handler(id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin(),
        kind: PluginHandlerKind::Command,
        handler_id: id.to_string(),
    }
}

fn request(request_id: &str, handler_id: &str, timeout_ms: u64) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId(request_id.to_string()),
        handler: handler(handler_id),
        timeout_ms,
        context: PluginInvocationContext {
            client_id: None,
            session_id: None,
            subscription_id: None,
            surface_id: None,
            origin: None,
            metadata: None,
        },
        payload: BoundaryJson(json!({ "request": request_id })),
    }
}

/// Invoke on a helper thread; the receiver yields the result.
fn invoke(
    process: &Arc<PluginProcess>,
    request: PluginInvocationRequest,
    cancellation: &PluginCancellationToken,
) -> mpsc::Receiver<PluginInvocationResult> {
    let (tx, rx) = mpsc::channel();
    let process = process.clone();
    let cancellation = cancellation.clone();
    thread::spawn(move || {
        let _ = tx.send(PluginRuntime::invoke(&*process, request, cancellation));
    });
    rx
}

fn outcome(rx: &mpsc::Receiver<PluginInvocationResult>) -> PluginInvocationResult {
    // timer: deadline — the invocation settles by result, exit, or stop; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .expect("the invocation settles")
}

fn failure_kind(result: &PluginInvocationResult) -> PluginInvocationFailureKind {
    match result {
        PluginInvocationResult::Failed(failure) => failure.kind.clone(),
        PluginInvocationResult::Completed(_) => panic!("expected a failure, got {result:?}"),
    }
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

/// A FIFO that the `block_after_signal` handler writes when it starts.
struct Started {
    dir: PathBuf,
    path: PathBuf,
    rx: mpsc::Receiver<std::io::Result<String>>,
}

impl Started {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "botster-plugin-started-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let path = dir.join("started");
        mkfifo(&path);
        let (tx, rx) = mpsc::channel();
        let reader_path = path.clone();
        thread::spawn(move || {
            let mut line = String::new();
            let read = std::fs::File::open(&reader_path)
                .and_then(|mut fifo| fifo.read_to_string(&mut line));
            let _ = tx.send(read.map(|_| line));
        });
        Self { dir, path, rx }
    }

    fn env(&self) -> (OsString, OsString) {
        (
            OsString::from("PLUGIN_TEST_STARTED_FIFO"),
            self.path.clone().into_os_string(),
        )
    }

    /// Wait until the handler reports that it runs.
    fn wait(&self) {
        // timer: deadline — the handler reports that it runs; expiry fails the test
        let line = self
            .rx
            .recv_timeout(EVENT_DEADLINE)
            .expect("the handler started")
            .expect("read the started FIFO");
        assert_eq!(line.trim(), "started");
    }
}

impl Drop for Started {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn an_invocation_round_trips_through_the_child() {
    let process = start(config());
    let result = outcome(&invoke(
        &process,
        request("echo-1", "echo", 1_000),
        &PluginCancellationToken::new(),
    ));
    let PluginInvocationResult::Completed(success) = result else {
        panic!("expected completion, got {result:?}");
    };
    assert_eq!(success.request_id, RequestId("echo-1".to_string()));
    assert_eq!(
        success.payload,
        Some(BoundaryJson(json!({ "request": "echo-1" })))
    );
}

#[test]
fn a_queued_invocation_is_cancelled_at_once_without_a_kill() {
    let started = Started::new("queued");
    let mut config = config();
    config.env = vec![started.env()];
    let process = start(config);
    let blocked = invoke(
        &process,
        request("blocked", "block_after_signal", 1_000),
        &PluginCancellationToken::new(),
    );
    started.wait();
    // The child runs one invocation at a time, so this one waits behind the
    // running, blocked one.
    let queued_token = PluginCancellationToken::new();
    let queued = invoke(&process, request("queued", "echo", 1_000), &queued_token);
    queued_token.cancel();

    assert_eq!(
        failure_kind(&outcome(&queued)),
        PluginInvocationFailureKind::Cancelled
    );
    assert!(process.exit().is_none(), "a queued cancel needs no kill");

    process.kill();
    assert_eq!(
        failure_kind(&outcome(&blocked)),
        PluginInvocationFailureKind::WorkerKilled
    );
}

#[test]
fn a_cancelled_invocation_that_does_not_return_is_killed_after_the_grace() {
    let started = Started::new("grace");
    let mut config = config();
    config.cancel_grace = SHORT;
    config.env = vec![started.env()];
    let process = start(config);
    let token = PluginCancellationToken::new();
    let blocked = invoke(
        &process,
        request("stuck", "block_after_signal", 1_000),
        &token,
    );
    started.wait();
    // The handler is running and ignores the cancel, so only the grace kill
    // can end it.
    token.cancel();

    assert_eq!(
        failure_kind(&outcome(&blocked)),
        PluginInvocationFailureKind::WorkerKilled
    );
    assert_eq!(
        await_exit(&process).cause,
        PluginExitCause::Killed(PluginKillReason::Deadline)
    );
}

#[test]
fn a_handler_abort_is_a_crash_and_another_plugin_keeps_serving() {
    let crashing = start(config());
    let neighbor = start(config());

    let crashed = outcome(&invoke(
        &crashing,
        request("abort", "abort", 1_000),
        &PluginCancellationToken::new(),
    ));
    assert_eq!(
        failure_kind(&crashed),
        PluginInvocationFailureKind::WorkerCrashed
    );
    assert_eq!(
        await_exit(&crashing).cause,
        PluginExitCause::Crashed {
            signal: Some(libc::SIGABRT),
            code: None,
        }
    );
    // A later invocation of the dead plugin fails at once with the same kind.
    let after = outcome(&invoke(
        &crashing,
        request("after", "echo", 1_000),
        &PluginCancellationToken::new(),
    ));
    assert_eq!(
        failure_kind(&after),
        PluginInvocationFailureKind::WorkerCrashed
    );

    let served = outcome(&invoke(
        &neighbor,
        request("neighbor", "echo", 1_000),
        &PluginCancellationToken::new(),
    ));
    assert!(matches!(served, PluginInvocationResult::Completed(_)));
}

#[test]
fn a_handler_panic_reports_its_message() {
    let process = start(config());
    let result = outcome(&invoke(
        &process,
        request("panic", "panic", 1_000),
        &PluginCancellationToken::new(),
    ));
    assert_eq!(
        failure_kind(&result),
        PluginInvocationFailureKind::WorkerCrashed
    );
    let exit = await_exit(&process);
    assert_eq!(exit.cause, PluginExitCause::Panic);
    assert!(exit.fatal_message.contains("scripted panic in a handler"));
}

/// Plan 7.5 rule 1: a fatal cause that the child published before a later
/// parent kill wins, even though the process then died of that SIGKILL.
#[test]
fn a_published_fatal_cause_beats_a_later_violation_kill() {
    let process = start(config());
    let result = outcome(&invoke(
        &process,
        request("fatal", "fatal_then_violation", 1_000),
        &PluginCancellationToken::new(),
    ));
    assert_eq!(
        failure_kind(&result),
        PluginInvocationFailureKind::WorkerCrashed
    );
    let exit = await_exit(&process);
    assert_eq!(exit.cause, PluginExitCause::Panic);
    assert!(exit.fatal_message.contains("before a violation kill"));
}

#[test]
fn stop_fails_waiting_invocations_at_once_and_a_stopped_child_is_killed_at_the_deadline() {
    let started = Started::new("stop");
    let mut config = config();
    config.shutdown_deadline = SHORT;
    config.env = vec![started.env()];
    let process = start(config);
    let blocked = invoke(
        &process,
        request("blocked", "block_after_signal", 1_000),
        &PluginCancellationToken::new(),
    );
    // The invocation is in flight and running before the stop.
    started.wait();
    // Freeze the child, so it can neither read Shutdown nor exit.
    // SAFETY: kill only sends a signal to our own child process.
    unsafe { libc::kill(process.pid() as libc::pid_t, libc::SIGSTOP) };

    PluginRuntime::stop(&*process, &plugin());

    assert_eq!(
        failure_kind(&outcome(&blocked)),
        PluginInvocationFailureKind::WorkerStopped
    );
    assert_eq!(
        await_exit(&process).cause,
        PluginExitCause::Killed(PluginKillReason::ShutdownDeadline)
    );
}

#[test]
fn an_unknown_frame_type_from_the_child_is_a_violation_kill() {
    let process = start(config());
    let result = outcome(&invoke(
        &process,
        request("garbage", "garbage", 1_000),
        &PluginCancellationToken::new(),
    ));
    assert_eq!(
        failure_kind(&result),
        PluginInvocationFailureKind::WorkerKilled
    );
    assert!(matches!(
        await_exit(&process).cause,
        PluginExitCause::Killed(PluginKillReason::ProtocolViolation(_))
    ));
}

#[test]
fn a_result_for_a_request_not_in_flight_is_a_violation_kill() {
    let process = start(config());
    let result = outcome(&invoke(
        &process,
        request("foreign", "foreign_result", 1_000),
        &PluginCancellationToken::new(),
    ));
    assert_eq!(
        failure_kind(&result),
        PluginInvocationFailureKind::WorkerKilled
    );
    assert!(matches!(
        await_exit(&process).cause,
        PluginExitCause::Killed(PluginKillReason::ProtocolViolation(_))
    ));
}

fn mkfifo(path: &Path) {
    let path = CString::new(path.as_os_str().as_bytes()).expect("fifo path");
    // SAFETY: mkfifo with a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0, "mkfifo");
}

#[test]
fn a_kill_ends_the_whole_process_group() {
    let dir: PathBuf =
        std::env::temp_dir().join(format!("botster-plugin-group-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    let (descendant, alive, report) = (
        dir.join("descendant"),
        dir.join("alive"),
        dir.join("report"),
    );
    for fifo in [&descendant, &alive, &report] {
        mkfifo(fifo);
    }
    // The held input FIFO keeps the descendant alive until it is killed.
    let _hold = OpenOptions::new()
        .read(true)
        .write(true)
        .open(&descendant)
        .expect("hold the input FIFO");
    let alive_read = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK)
        .open(&alive)
        .expect("open the alive FIFO");
    // The report FIFO blocks the child until something reads it.
    let report_reader = thread::spawn(move || {
        let mut pid = String::new();
        std::fs::File::open(&report)
            .and_then(|mut report| report.read_to_string(&mut pid))
            .expect("read the descendant report");
        pid
    });
    let mut config = config();
    let env = |key: &str, path: &Path| (OsString::from(key), path.as_os_str().to_owned());
    config.env = vec![
        env("PLUGIN_TEST_DESCENDANT_FIFO", &descendant),
        env("PLUGIN_TEST_ALIVE_FIFO", &alive),
        env("PLUGIN_TEST_REPORT_FIFO", &dir.join("report")),
    ];
    let process = start(config);
    assert!(!report_reader.join().expect("report").trim().is_empty());

    process.kill();
    assert_eq!(
        await_exit(&process).cause,
        PluginExitCause::Killed(PluginKillReason::Requested)
    );
    // SAFETY: fcntl get/set on a descriptor this test owns.
    unsafe {
        let flags = libc::fcntl(alive_read.as_raw_fd(), libc::F_GETFL);
        libc::fcntl(
            alive_read.as_raw_fd(),
            libc::F_SETFL,
            flags & !libc::O_NONBLOCK,
        );
    }
    let (died, descendant_died) = mpsc::channel();
    thread::spawn(move || {
        let mut alive_read = alive_read;
        let mut sink = Vec::new();
        let _ = died.send(alive_read.read_to_end(&mut sink).is_ok());
    });
    // timer: deadline — EOF on the alive FIFO means the descendant died; expiry means only the leader died
    assert!(descendant_died
        .recv_timeout(EVENT_DEADLINE)
        .expect("the kill ended the whole group"));
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_engine_runs_a_process_plugin_and_its_deadline_kills_a_stuck_one() {
    let engine = PluginWorkerEngine::with_config(PluginWorkerEngineConfig {
        per_plugin_executor_concurrency: 2,
        reserved_request_response_executors: 1,
        ..PluginWorkerEngineConfig::default()
    });
    let mut config = config();
    config.cancel_grace = SHORT;
    let process = start(config);
    engine.load_plugin(PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin(),
            package: "process-plugin".to_string(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: Vec::new(),
            metadata: None,
        },
        manifest: serde_json::from_value(json!({
            "name": "process-plugin",
            "version": "0.1.0",
            "kind": "plugin",
            "botster": ">=0.1.0",
            "source": null,
            "capabilities": [],
            "entrypoints": [{ "runtime": "lua", "path": "plugin.lua", "bootstrap": false }],
            "dependencies": [],
            "features": [],
            "host_profile": null,
            "configuration": null,
            "runnable_entrypoints": []
        }))
        .expect("manifest"),
        runtime: process.clone(),
        handlers: ["echo", "block"]
            .into_iter()
            .map(|id| PluginHandlerRegistration {
                handler: handler(id),
                required_capability: None,
            })
            .collect(),
        resources: Vec::new(),
    });

    // The blocking request-response path takes its locks without try_lock,
    // so admission cannot be refused as busy; its timeout cancels the token.
    // The child runs one invocation at a time, so the calls go in sequence.
    let call = |id: &'static str, handler_id: &'static str, timeout_ms| {
        let engine = engine.clone();
        let (tx, rx) = mpsc::channel();
        thread::spawn(move || {
            let _ = tx.send(engine.invoke(request(id, handler_id, timeout_ms)).result);
        });
        outcome(&rx)
    };
    let settled = [call("fast", "echo", 10_000), call("stuck", "block", 100)];
    assert!(matches!(
        &settled[0],
        PluginInvocationResult::Completed(success) if success.request_id.0 == "fast"
    ));
    assert!(matches!(
        &settled[1],
        PluginInvocationResult::Failed(failure)
            if failure.request_id.0 == "stuck" && failure.kind == PluginInvocationFailureKind::TimedOut
    ));
    assert_eq!(
        await_exit(&process).cause,
        PluginExitCause::Killed(PluginKillReason::Deadline)
    );
}
