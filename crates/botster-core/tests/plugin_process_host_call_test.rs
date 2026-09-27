//! Real-process tests of host calls, replies, and log lines (plan section
//! 5.1): the credits that bound them, local refusal on exhaustion, credit
//! returns, and the protocol violations that kill the process.
//!
//! Requires `script/prebuild-worker`.
#![cfg(all(feature = "local-runtime", unix))]

use std::ffi::{CString, OsString};
use std::io::Read;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::{mpsc, Arc};
use std::thread;
use std::time::Duration;

use botster_core::actor::{PluginHandlerKind, PluginHandlerRef, PluginInvocationContext};
use botster_core::engine::{
    CallId, DeliveryPool, PluginDeliveryQuota, PluginHandlerRegistration, PluginWorkerEngine,
    PluginWorkerRegistration,
};
use botster_core::runtime::plugin_process::{
    LoadFrame, PluginConfig, PluginExitCause, PluginHostCallKind, PluginIngress, PluginKillReason,
    PluginLogCredits, PluginProcess, PluginProcessConfig, PluginProcessError, PluginProcessExited,
    PluginProcessRlimits, PluginRegistration, PluginReplyCredits, PluginSources, SandboxProfile,
};
use botster_core::session::RequestId;
use botster_core::{
    BoundaryJson, PluginAdmissionResult, PluginCancellationToken, PluginInvocationRequest,
    PluginInvocationResult, PluginKey, PluginLoadSpec, PluginRuntime,
};
use botster_core_test_support::real_worker::WorkerBinary;
use serde_json::{json, Value};

/// Bound for every event a test waits for; expiry fails the test.
const EVENT_DEADLINE: Duration = Duration::from_secs(30);
/// A deadline that must not expire during a test.
const GENEROUS: Duration = Duration::from_secs(30);
const FRAME_HOST_CALL: u8 = 0x86;
const FRAME_LOG: u8 = 0x87;

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

fn spawn(
    config: &PluginProcessConfig,
    mode: &str,
) -> Result<(PluginProcess, PluginRegistration), PluginProcessError> {
    let load = LoadFrame {
        sources: PluginSources(BoundaryJson(json!({}))),
        config: PluginConfig(BoundaryJson(json!({ "mode": mode }))),
    };
    let (tx, rx) = mpsc::channel();
    let config = config.clone();
    thread::spawn(move || {
        let _ = tx.send(PluginProcess::spawn(&config, &load));
    });
    // timer: deadline — the worker reaches Loaded or fails; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE).expect("spawn resolves")
}

fn start(config: &PluginProcessConfig) -> (Arc<PluginProcess>, PluginRegistration) {
    let (process, registration) = spawn(config, "report").expect("loaded");
    (Arc::new(process), registration)
}

fn plugin() -> PluginKey {
    PluginKey("host-call-plugin".to_string())
}

fn handler(id: &str) -> PluginHandlerRef {
    PluginHandlerRef {
        plugin_key: plugin(),
        kind: PluginHandlerKind::Command,
        handler_id: id.to_string(),
    }
}

fn request(request_id: &str, handler_id: &str, payload: Value) -> PluginInvocationRequest {
    PluginInvocationRequest {
        request_id: RequestId(request_id.to_string()),
        handler: handler(handler_id),
        timeout_ms: 10_000,
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

/// Invoke on a helper thread; the receiver yields the result.
fn invoke_async(
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

fn settled(rx: &mpsc::Receiver<PluginInvocationResult>) -> PluginInvocationResult {
    // timer: deadline — the invocation settles by result, exit, or stop; expiry fails the test
    rx.recv_timeout(EVENT_DEADLINE)
        .expect("the invocation settles")
}

/// Invoke `handler_id` with `payload` and return the completed payload.
fn run(process: &Arc<PluginProcess>, request_id: &str, handler_id: &str, payload: Value) -> Value {
    let result = settled(&invoke_async(
        process,
        request(request_id, handler_id, payload),
        &PluginCancellationToken::new(),
    ));
    completed(&result)
}

fn completed(result: &PluginInvocationResult) -> Value {
    match result {
        PluginInvocationResult::Completed(success) => {
            success
                .payload
                .clone()
                .expect("the test handlers complete with a payload")
                .0
        }
        PluginInvocationResult::Failed(failure) => panic!("expected a completion, got {failure:?}"),
    }
}

fn host_calls(process: &Arc<PluginProcess>, request_id: &str, count: u64) -> Vec<Value> {
    let outcomes = run(
        process,
        request_id,
        "host_calls",
        json!({ "count": count, "max_result_bytes": 512 }),
    );
    outcomes.as_array().expect("one outcome per call").clone()
}

fn call_id(outcome: &Value) -> u64 {
    outcome["call_id"]
        .as_u64()
        .unwrap_or_else(|| panic!("expected a sent call, got {outcome}"))
}

fn refusal(outcome: &Value) -> String {
    outcome["refused"]
        .as_str()
        .unwrap_or_else(|| panic!("expected a refusal, got {outcome}"))
        .to_string()
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

fn assert_violation(process: &PluginProcess, expected: &str) {
    match await_exit(process).cause {
        PluginExitCause::Killed(PluginKillReason::ProtocolViolation(violation)) => assert!(
            violation.contains(expected),
            "violation {violation:?} does not name {expected:?}"
        ),
        other => panic!("expected a violation kill naming {expected:?}, got {other:?}"),
    }
}

/// Register `process` as the plugin's runtime in a fresh engine.
fn engine_with(process: &Arc<PluginProcess>) -> PluginWorkerEngine {
    let engine = PluginWorkerEngine::new();
    engine.load_plugin(PluginWorkerRegistration {
        load: PluginLoadSpec {
            plugin_key: plugin(),
            package: "host-call-plugin".to_string(),
            entrypoint: "plugin.lua".to_string(),
            descriptors: Vec::new(),
            metadata: None,
        },
        manifest: serde_json::from_value(json!({
            "name": "host-call-plugin",
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
        handlers: ["echo", "host_calls"]
            .into_iter()
            .map(|id| PluginHandlerRegistration {
                handler: handler(id),
                required_capability: None,
            })
            .collect(),
        resources: Vec::new(),
    });
    engine
}

fn reserve(engine: &PluginWorkerEngine, slots: usize) -> DeliveryPool {
    engine
        .try_reserve_delivery(
            &plugin(),
            PluginDeliveryQuota {
                call_result_slots: slots,
                call_result_request_bytes: slots * 1024,
                call_result_completion_bytes: 4096,
                ordinary_completion_entries: 1,
                ordinary_completion_bytes: 4096,
            },
        )
        .expect("the delivery pool")
}

fn attached(
    slots: usize,
    config: &PluginProcessConfig,
) -> (Arc<PluginProcess>, PluginWorkerEngine, DeliveryPool) {
    let (process, _) = start(config);
    let engine = engine_with(&process);
    let pool = reserve(&engine, slots);
    process
        .attach_delivery(pool.clone())
        .expect("the first attach");
    (process, engine, pool)
}

fn host_call_items(items: &[PluginIngress]) -> Vec<(PluginHostCallKind, u64, String)> {
    items
        .iter()
        .filter_map(|item| match item {
            PluginIngress::HostCall(call) => Some((
                call.kind,
                call.call_id.0,
                call.invocation_request_id.0.clone(),
            )),
            PluginIngress::Log(_) => None,
        })
        .collect()
}

#[test]
fn a_call_during_load_is_refused_as_outside_an_invocation() {
    let (_process, registration) = start(&config());
    assert_eq!(
        registration.0 .0["call_during_load"],
        json!("no invocation is running")
    );
}

#[test]
fn a_host_call_round_trips_and_its_unit_returns_when_its_result_drains() {
    let (process, engine, pool) = attached(1, &config());
    let (completion_tx, completion_rx) = mpsc::channel();
    engine.install_completion_notifier(Arc::new(move || {
        let _ = completion_tx.send(());
    }));

    let first = host_calls(&process, "root", 1);
    let call = call_id(&first[0]);
    let items = process.drain_ingress(16, 1024 * 1024);
    assert_eq!(
        host_call_items(&items),
        vec![(
            PluginHostCallKind::Call {
                max_result_bytes: 512
            },
            call,
            "root".to_string()
        )]
    );
    assert!(items[0].frame_bytes() > 0);
    // The only unit is in use until the result drains.
    assert!(refusal(&host_calls(&process, "busy", 1)[0]).contains("every delivery unit"));

    let admitted = pool.admit_result(CallId(call), request("result", "echo", json!(null)));
    assert!(
        matches!(admitted, PluginAdmissionResult::Queued { .. }),
        "{admitted:?}"
    );
    // timer: deadline — the result completes; expiry fails the test
    completion_rx
        .recv_timeout(EVENT_DEADLINE)
        .expect("the result completes");
    let drained = engine.drain_completions(16, 1024 * 1024);
    assert_eq!(drained.completions.len(), 1);

    // The drain returned the unit, and its credit reached the child before
    // the next invocation.
    let again = host_calls(&process, "again", 1);
    assert_eq!(call_id(&again[0]), call + 2);
    assert!(process.exit().is_none(), "no violation");
}

#[test]
fn exhausted_delivery_credit_is_a_local_refusal_and_release_restores_it() {
    let (process, _) = start(&config());
    // No pool is attached yet: the child has no delivery credit.
    assert!(refusal(&host_calls(&process, "early", 1)[0]).contains("every delivery unit"));

    let engine = engine_with(&process);
    let pool = reserve(&engine, 2);
    process.attach_delivery(pool.clone()).expect("attach");
    assert!(matches!(
        process.attach_delivery(pool.clone()),
        Err(PluginProcessError::InvalidConfig(_))
    ));
    let outcomes = host_calls(&process, "burst", 3);
    let first = call_id(&outcomes[0]);
    call_id(&outcomes[1]);
    assert!(refusal(&outcomes[2]).contains("every delivery unit"));
    assert_eq!(pool.free(), (0, 2 * 1024 - 2 * 512));

    assert!(pool.release_call(CallId(first)));
    assert!(!pool.release_call(CallId(first)), "a unit returns once");
    call_id(&host_calls(&process, "after", 1)[0]);
    assert!(process.exit().is_none(), "no violation");
}

#[test]
fn ingress_credit_bounds_undrained_calls_and_a_drain_returns_it() {
    let mut config = config();
    // Two call frames of 94 bytes fit; a third does not.
    config.ingress_bytes = 200;
    let (process, _engine, _pool) = attached(8, &config);

    let outcomes = host_calls(&process, "ingress", 3);
    call_id(&outcomes[0]);
    call_id(&outcomes[1]);
    assert!(refusal(&outcomes[2]).contains("ingress"));

    let items = process.drain_ingress(16, 1024 * 1024);
    assert_eq!(items.len(), 2);
    call_id(&host_calls(&process, "ingress", 1)[0]);
    assert!(process.exit().is_none(), "no violation");
}

#[test]
fn a_reply_holds_its_credit_until_released_and_a_waiting_reply_then_sends() {
    let mut config = config();
    config.reply_credits.count = 1;
    let (process, _) = start(&config);

    let first = call_id(&run(&process, "r1", "try_reply", json!({ "body": "one" })));
    let refused = run(&process, "r2", "try_reply", json!({ "body": "two" }));
    assert!(refusal(&refused).contains("reply credit"));

    let items = process.drain_ingress(16, 1024 * 1024);
    assert_eq!(
        host_call_items(&items),
        vec![(PluginHostCallKind::Reply, first, "r1".to_string())]
    );
    // Draining does not return reply credit: a waiting reply stays waiting
    // until the release.
    let waiting = invoke_async(
        &process,
        request("r3", "reply", json!({ "body": "three" })),
        &PluginCancellationToken::new(),
    );
    assert!(process.release_reply(CallId(first)));
    assert!(
        !process.release_reply(CallId(first)),
        "a reply releases once"
    );
    call_id(&completed(&settled(&waiting)));
    assert!(process.exit().is_none(), "no violation");
}

fn mkfifo(path: &Path) {
    let path = CString::new(path.as_os_str().as_bytes()).expect("fifo path");
    // SAFETY: mkfifo with a valid NUL-terminated path.
    assert_eq!(unsafe { libc::mkfifo(path.as_ptr(), 0o600) }, 0, "mkfifo");
}

/// A FIFO that a handler writes when it starts.
struct Started {
    dir: PathBuf,
    path: PathBuf,
    rx: mpsc::Receiver<std::io::Result<String>>,
}

impl Started {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "botster-plugin-host-call-{name}-{}",
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
fn a_cancel_ends_a_reply_that_waits_for_credit_without_taking_it() {
    let started = Started::new("reply-cancel");
    let mut config = config();
    config.reply_credits.count = 1;
    config.env.push(started.env());
    let (process, _) = start(&config);
    let first = call_id(&run(&process, "r1", "try_reply", json!({ "body": "one" })));

    let cancellation = PluginCancellationToken::new();
    let waiting = invoke_async(
        &process,
        request("r2", "reply", json!({ "body": "two", "signal": true })),
        &cancellation,
    );
    // The reply runs in the child; the cancel reaches it there, before or
    // while it waits for credit.
    started.wait();
    cancellation.cancel();
    let cancelled = completed(&settled(&waiting));
    assert_eq!(refusal(&cancelled), "the invocation was cancelled");

    assert!(process.release_reply(CallId(first)));
    call_id(&run(
        &process,
        "r3",
        "try_reply",
        json!({ "body": "three" }),
    ));
    assert!(process.exit().is_none(), "no violation");
}

#[test]
fn log_lines_without_credit_are_dropped_and_counted_once() {
    let mut config = config();
    config.log_credits.count = 2;
    let (process, _) = start(&config);

    let sent = run(
        &process,
        "l1",
        "logs",
        json!({ "count": 5, "body": "line" }),
    );
    assert_eq!(sent["sent"], json!(2));
    let first = process.drain_ingress(16, 1024 * 1024);
    let dropped: Vec<u64> = first
        .iter()
        .map(|item| match item {
            PluginIngress::Log(log) => log.dropped_since_last,
            PluginIngress::HostCall(call) => panic!("expected a log line, got {call:?}"),
        })
        .collect();
    assert_eq!(dropped, vec![0, 0]);

    let sent = run(
        &process,
        "l2",
        "logs",
        json!({ "count": 1, "body": "line" }),
    );
    assert_eq!(sent["sent"], json!(1));
    let second = process.drain_ingress(16, 1024 * 1024);
    assert!(matches!(
        second.as_slice(),
        [PluginIngress::Log(log)] if log.dropped_since_last == 3
    ));
}

#[test]
fn a_drain_takes_items_in_order_within_its_bounds() {
    let (process, _engine, _pool) = attached(4, &config());
    host_calls(&process, "order", 3);
    let first = process.drain_ingress(2, 1024 * 1024);
    let second = process.drain_ingress(16, 1);
    let third = process.drain_ingress(16, 1024 * 1024);
    assert_eq!(first.len(), 2);
    assert!(second.is_empty(), "a frame larger than max_bytes stays");
    assert_eq!(third.len(), 1);
    let ids: Vec<u64> = host_call_items(&first)
        .into_iter()
        .chain(host_call_items(&third))
        .map(|(_, id, _)| id)
        .collect();
    assert!(ids.windows(2).all(|pair| pair[0] < pair[1]), "{ids:?}");
}

/// Send one raw frame around the child's credit checks.
fn raw(process: &Arc<PluginProcess>, request_id: &str, frame_type: u8, frame: Value) {
    let _ = settled(&invoke_async(
        process,
        request(
            request_id,
            "raw",
            json!({ "type": frame_type, "frame": frame }),
        ),
        &PluginCancellationToken::new(),
    ));
}

fn raw_call(call_id: u64, max_result_bytes: usize, body: Value) -> Value {
    json!({
        "kind": "call",
        "max_result_bytes": max_result_bytes,
        "call_id": call_id,
        "body": body,
    })
}

fn raw_reply(call_id: u64, body: Value) -> Value {
    json!({ "kind": "reply", "call_id": call_id, "body": body })
}

#[test]
fn a_call_before_any_delivery_credit_is_a_violation_kill() {
    let (process, _) = start(&config());
    raw(&process, "v", FRAME_HOST_CALL, raw_call(7, 1, json!(null)));
    assert_violation(&process, "before any delivery credit");
}

#[test]
fn a_call_over_the_pool_bytes_is_a_violation_kill() {
    let (process, _engine, _pool) = attached(1, &config());
    raw(
        &process,
        "v",
        FRAME_HOST_CALL,
        raw_call(7, 4096, json!(null)),
    );
    assert_violation(&process, "declared result size");
}

#[test]
fn a_call_over_the_ingress_credit_is_a_violation_kill() {
    let (process, _engine, _pool) = attached(1, &config());
    let body = json!("x".repeat(70 * 1024));
    raw(&process, "v", FRAME_HOST_CALL, raw_call(7, 1, body));
    assert_violation(&process, "exceeds the ingress credit");
}

#[test]
fn a_duplicate_open_call_id_is_a_violation_kill() {
    let (process, _engine, _pool) = attached(2, &config());
    let open = call_id(&host_calls(&process, "first", 1)[0]);
    raw(
        &process,
        "v",
        FRAME_HOST_CALL,
        raw_call(open, 1, json!(null)),
    );
    assert_violation(&process, "already open");
}

#[test]
fn a_reply_on_an_open_call_id_is_a_violation_kill() {
    let (process, _engine, _pool) = attached(2, &config());
    let open = call_id(&host_calls(&process, "first", 1)[0]);
    raw(&process, "v", FRAME_HOST_CALL, raw_reply(open, json!(null)));
    assert_violation(&process, "already open");
}

#[test]
fn a_call_on_an_open_reply_id_is_a_violation_kill() {
    let (process, _engine, _pool) = attached(2, &config());
    raw(&process, "r", FRAME_HOST_CALL, raw_reply(100, json!(null)));
    assert!(process.exit().is_none(), "the reply fits its credit");
    raw(
        &process,
        "v",
        FRAME_HOST_CALL,
        raw_call(100, 1, json!(null)),
    );
    assert_violation(&process, "already open");
}

#[test]
fn a_reply_without_reply_credit_is_a_violation_kill() {
    let (process, _) = start(&config());
    raw(&process, "v1", FRAME_HOST_CALL, raw_reply(100, json!(null)));
    raw(&process, "v2", FRAME_HOST_CALL, raw_reply(101, json!(null)));
    assert!(process.exit().is_none(), "two replies fit two credits");
    raw(&process, "v3", FRAME_HOST_CALL, raw_reply(102, json!(null)));
    assert_violation(&process, "without reply credit");
}

#[test]
fn a_reply_over_its_allowance_is_a_violation_kill() {
    let (process, _) = start(&config());
    let body = json!("x".repeat(70 * 1024));
    raw(&process, "v", FRAME_HOST_CALL, raw_reply(100, body));
    assert_violation(&process, "exceeds the reply allowance");
}

#[test]
fn a_log_without_log_credit_is_a_violation_kill() {
    let (process, _) = start(&config());
    let body = json!("x".repeat(20 * 1024));
    raw(
        &process,
        "v",
        FRAME_LOG,
        json!({ "dropped_since_last": 0, "body": body }),
    );
    assert_violation(&process, "without log credit");
}

#[test]
fn a_host_call_for_a_request_not_in_flight_is_a_violation_kill() {
    let (process, _) = start(&config());
    let mut frame = raw_reply(100, json!(null));
    frame["invocation_request_id"] = json!("ghost");
    raw(&process, "v", FRAME_HOST_CALL, frame);
    assert_violation(&process, "not in flight");
}

#[test]
fn a_host_call_before_loaded_is_a_violation_kill() {
    match spawn(&config(), "host_call_before_loaded") {
        Err(PluginProcessError::Exited(exit)) => assert!(
            matches!(
                &exit.cause,
                PluginExitCause::Killed(PluginKillReason::ProtocolViolation(violation))
                    if violation.contains("not valid in this phase")
            ),
            "{exit:?}"
        ),
        other => panic!("expected a violation kill during startup, got {other:?}"),
    }
}

#[test]
fn spawn_refuses_a_frame_bound_below_the_reply_allowance() {
    let mut config = config();
    config.reply_credits.bytes = config.max_frame_bytes + 1;
    assert!(matches!(
        spawn(&config, "report"),
        Err(PluginProcessError::InvalidConfig(_))
    ));
}

/// Round-trip measurement (plan slice 4). It sets no target; run it with
/// `--ignored --nocapture` to report the numbers. All times are parent-side:
/// - publish: from the root invocation to the host call's arrival at the Hub
///   (the entity-publish path: one HostCall, no result);
/// - result leg: from the Hub's result admission to the publication of its
///   completion (the completion notifier, before the drain);
/// - capability call: from the root invocation to that publication
///   (HostCall, then the result Invoke).
#[test]
#[ignore = "measurement: run with --ignored --nocapture"]
fn measure_host_call_round_trips() {
    const ITERATIONS: usize = 500;
    let (process, engine, pool) = attached(1, &config());
    let (ingress_tx, ingress_rx) = mpsc::channel();
    process.install_ingress_notifier(Arc::new(move || {
        let _ = ingress_tx.send(std::time::Instant::now());
    }));
    let (completion_tx, completion_rx) = mpsc::channel();
    engine.install_completion_notifier(Arc::new(move || {
        let _ = completion_tx.send(std::time::Instant::now());
    }));

    let (mut publish, mut result_leg, mut capability) = (Vec::new(), Vec::new(), Vec::new());
    for index in 0..ITERATIONS {
        // timer: measurement-window — the round-trip probe reports elapsed times only
        let started = std::time::Instant::now();
        let root = invoke_async(
            &process,
            request(
                &format!("root-{index}"),
                "host_calls",
                json!({ "count": 1, "max_result_bytes": 512 }),
            ),
            &PluginCancellationToken::new(),
        );
        // timer: deadline — the host call arrives; expiry fails the measurement
        let arrived = ingress_rx
            .recv_timeout(EVENT_DEADLINE)
            .expect("the host call arrives");
        let call = match process.drain_ingress(1, 1024 * 1024).pop() {
            Some(PluginIngress::HostCall(call)) => call.call_id,
            other => panic!("expected a host call, got {other:?}"),
        };
        settled(&root);
        let admitted = std::time::Instant::now();
        let queued = pool.admit_result(
            call,
            request(&format!("result-{index}"), "echo", json!(null)),
        );
        assert!(
            matches!(queued, PluginAdmissionResult::Queued { .. }),
            "{queued:?}"
        );
        // timer: deadline — the result completes; expiry fails the measurement
        let completed_at = completion_rx
            .recv_timeout(EVENT_DEADLINE)
            .expect("the result completes");
        assert_eq!(
            engine.drain_completions(1, 1024 * 1024).completions.len(),
            1
        );
        publish.push(arrived - started);
        result_leg.push(completed_at - admitted);
        capability.push(completed_at - started);
    }
    for (name, samples) in [
        ("publish", &mut publish),
        ("result leg", &mut result_leg),
        ("capability call", &mut capability),
    ] {
        samples.sort();
        let at = |fraction: f64| samples[((samples.len() - 1) as f64 * fraction) as usize];
        println!(
            "{name}: p50 {:?} p90 {:?} p99 {:?} max {:?} (n={})",
            at(0.5),
            at(0.9),
            at(0.99),
            samples[samples.len() - 1],
            samples.len()
        );
    }
}
