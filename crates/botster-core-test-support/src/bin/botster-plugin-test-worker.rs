//! Scripted plugin worker for Core's real-process plugin host tests.
//!
//! It links the Core worker library with a Rust test runtime instead of Lua.
//! The `Load` config selects a behavior:
//! - `{"mode": "report"}`: load and report the environment, open descriptors,
//!   working directory, `RLIMIT_NOFILE`, whether the sandbox hook ran first,
//!   and how the host port answered a call made during the load.
//! - `{"mode": "fail"}`: refuse the load.
//! - `{"mode": "host_call_before_loaded"}`: send a host call frame during the
//!   load, around the host port.
//! - `{"mode": "spin"}`: never finish the load.
//! - `{"mode": "close_ipc_then_wait"}`: close the IPC channel and stay alive
//!   until killed.
//! - `{"mode": "panic"}` / `{"mode": "abort"}`: die during the load.
//! - `{"mode": "panic_with_full_stderr"}`: point fd 2 at a pipe that is full
//!   and never drained, then panic; any write to stderr would block forever.
//!
//! A sandbox profile `{"fail": "<reason>"}` makes the sandbox hook fail.
//!
//! Before the worker library starts, two opt-in environment switches apply:
//! - `PLUGIN_TEST_DESCENDANT_FIFO`, `PLUGIN_TEST_ALIVE_FIFO`, and
//!   `PLUGIN_TEST_REPORT_FIFO`: start a descendant in this process group
//!   (`/bin/cat` reading the first FIFO, which the test holds open, with its
//!   stdout on the second, which only it holds for writing), then write its
//!   pid to the third. EOF on the alive FIFO means the descendant died.
//! - `PLUGIN_TEST_BLOCKING_PRIOR_HOOK`: install a panic hook that never
//!   returns, which the worker library must not chain to.

use std::collections::BTreeMap;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use botster_core::runtime::plugin_process::worker::{
    run_worker, CappedAllocator, HostPort, LoadedPlugin, WorkerHooks,
};
use botster_core::runtime::plugin_process::{
    LoadFrame, PluginMessageBody, PluginRegistration, SandboxProfile,
};
use botster_core::session::RequestId;
use botster_core::{
    BoundaryJson, PluginCancellationToken, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginInvocationSuccess, PluginRuntime,
};
use serde_json::{json, Value};

/// The worker counts every allocation, so a Bootstrap memory cap holds.
#[global_allocator]
static ALLOCATOR: CappedAllocator = CappedAllocator::new();

static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);
static PORT: OnceLock<HostPort> = OnceLock::new();

fn main() {
    if let (Some(descendant), Some(alive), Some(report)) = (
        std::env::var_os("PLUGIN_TEST_DESCENDANT_FIFO"),
        std::env::var_os("PLUGIN_TEST_ALIVE_FIFO"),
        std::env::var_os("PLUGIN_TEST_REPORT_FIFO"),
    ) {
        start_descendant(&descendant, &alive, &report);
    }
    if std::env::var_os("PLUGIN_TEST_BLOCKING_PRIOR_HOOK").is_some() {
        std::panic::set_hook(Box::new(|_| wait_for_kill()));
    }
    run_worker(WorkerHooks {
        apply_sandbox,
        load,
    });
}

/// A descendant in this process group whose lifetime is the test's FIFO:
/// it exits only when the test closes its end, or when it is killed.
fn start_descendant(
    descendant: &std::ffi::OsStr,
    alive: &std::ffi::OsStr,
    report: &std::ffi::OsStr,
) {
    let stdin = File::open(descendant).expect("open the descendant FIFO");
    let stdout = OpenOptions::new()
        .write(true)
        .open(alive)
        .expect("open the alive FIFO");
    let mut command = Command::new("/bin/cat");
    command.stdin(stdin).stdout(stdout).stderr(Stdio::null());
    // SAFETY: close only calls an async-signal-safe function before exec.
    unsafe {
        command.pre_exec(|| {
            // The descendant must not hold the worker's IPC or fatal pipe.
            libc::close(3);
            libc::close(4);
            Ok(())
        });
    }
    #[expect(
        clippy::zombie_processes,
        reason = "the descendant's end is the group kill under test; this worker dies first"
    )]
    let child = command.spawn().expect("start the descendant");
    let mut report = OpenOptions::new()
        .write(true)
        .open(report)
        .expect("open the report FIFO");
    writeln!(report, "{}", child.id()).expect("report the descendant pid");
}

/// Replace fd 2 with a full pipe whose read end stays open and undrained, so
/// the next stderr write blocks forever.
fn fill_stderr() {
    let mut fds = [0 as libc::c_int; 2];
    // SAFETY: pipe, dup2, fcntl, and write on descriptors this process owns;
    // the read end is deliberately leaked so the pipe never drains.
    unsafe {
        assert_eq!(libc::pipe(fds.as_mut_ptr()), 0, "pipe");
        assert!(libc::dup2(fds[1], 2) >= 0, "dup2");
        let flags = libc::fcntl(2, libc::F_GETFL);
        libc::fcntl(2, libc::F_SETFL, flags | libc::O_NONBLOCK);
        let chunk = [b'x'; 4096];
        while libc::write(2, chunk.as_ptr().cast(), chunk.len()) > 0 {}
        libc::fcntl(2, libc::F_SETFL, flags & !libc::O_NONBLOCK);
    }
}

fn wait_for_kill() -> ! {
    loop {
        // SAFETY: pause only waits for a signal. Its lifetime ends at the
        // parent's kill, an event the test controls.
        unsafe { libc::pause() };
    }
}

fn apply_sandbox(profile: &SandboxProfile) -> Result<(), String> {
    if let Some(reason) = profile.0 .0.get("fail").and_then(Value::as_str) {
        return Err(reason.to_string());
    }
    SANDBOX_APPLIED.store(true, Ordering::SeqCst);
    Ok(())
}

fn load(frame: LoadFrame, port: HostPort) -> Result<LoadedPlugin, String> {
    let during_load = match port.call(1, body(json!("during load"))) {
        Ok(call) => format!("sent {}", call.0),
        Err(refusal) => refusal.to_string(),
    };
    let _ = PORT.set(port);
    let mode = frame.config.0 .0.get("mode").and_then(Value::as_str);
    match mode {
        Some("report") => {
            let mut report = report();
            report["call_during_load"] = json!(during_load);
            Ok(LoadedPlugin {
                runtime: Arc::new(TestRuntime),
                registration: PluginRegistration(BoundaryJson(report)),
            })
        }
        Some("host_call_before_loaded") => {
            // A host call is valid only after Loaded; the parent must kill.
            let frame = json!({
                "kind": "reply",
                "call_id": 0,
                "invocation_request_id": "none",
                "body": null,
            });
            write_raw_frame(0x86, &serde_json::to_vec(&frame).expect("encode"));
            wait_for_kill()
        }
        Some("fail") => Err("scripted load failure".to_string()),
        Some("spin") => loop {
            std::hint::spin_loop();
        },
        Some("close_ipc_then_wait") => {
            // SAFETY: closes the worker's IPC descriptor; nothing reads it
            // again before the parent kills this process.
            unsafe { libc::close(3) };
            wait_for_kill()
        }
        Some("panic") => panic!("scripted panic during load"),
        Some("panic_with_full_stderr") => {
            fill_stderr();
            panic!("scripted panic with a full stderr")
        }
        Some("abort") => std::process::abort(),
        other => Err(format!("unknown test worker mode {other:?}")),
    }
}

fn report() -> Value {
    let env: BTreeMap<String, String> = std::env::vars().collect();
    let fds: Vec<i32> = (0..1024)
        // SAFETY: F_GETFD only queries the descriptor table.
        .filter(|fd| unsafe { libc::fcntl(*fd, libc::F_GETFD) } >= 0)
        .collect();
    let mut nofile = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: getrlimit writes into a valid rlimit.
    unsafe { libc::getrlimit(libc::RLIMIT_NOFILE, &mut nofile) };
    json!({
        "env": env,
        "fds": fds,
        "cwd": std::env::current_dir().ok(),
        "nofile": nofile.rlim_cur,
        "sandbox_applied_before_load": SANDBOX_APPLIED.load(Ordering::SeqCst),
    })
}

/// Handlers, chosen by handler id:
/// - `echo`: complete with the request payload;
/// - `block`: never return, ignoring cancellation, until killed;
/// - `block_after_signal`: write to `PLUGIN_TEST_STARTED_FIFO`, then block;
/// - `abort` / `panic`: die inside the handler;
/// - `fatal_then_violation`: publish a panic cause, then break the protocol;
/// - `garbage`: write a frame of an unknown type to the parent;
/// - `foreign_result`: write a result for a request that is not in flight;
/// - `forged_identity`: return the live request id with another plugin's handler;
/// - `host_calls`: `{"count", "max_result_bytes"}`: make that many host calls
///   and complete with each outcome (`{"call_id"}` or `{"refused"}`);
/// - `reply` / `try_reply`: `{"body"}`: send a reply (waiting for credit, or
///   not) and complete with the outcome; `reply` with `"signal": true` first
///   writes to `PLUGIN_TEST_STARTED_FIFO`;
/// - `logs`: `{"count", "body"}`: send that many log lines and complete with
///   the number sent;
/// - `raw`: `{"type", "frame"}`: write `frame` (with this invocation's request
///   id added as `invocation_request_id` when absent) as a raw frame of
///   `type`, around the credit checks, then complete; with `"wait": true` it
///   first reads `PLUGIN_TEST_GO_FIFO` to its end;
/// - `allocate`: `{"step", "steps"}`: allocate and touch `steps` blocks of
///   `step` bytes, keep them, and complete with the total;
/// - `logs_then_allocate`: `{"logs", "log_bytes", "step"}`: send that many log
///   lines of that size, then allocate `step`-byte blocks without end;
/// - `spin`: burn CPU until killed;
/// - `open_files`: open `/dev/null` until refused, then complete with the
///   count and the errno.
struct TestRuntime;

/// Allocate and touch `steps` blocks of `step` bytes, keeping them all.
fn allocate(step: usize, steps: Option<u64>) -> usize {
    let mut kept: Vec<Vec<u8>> = Vec::new();
    let mut index = 0u64;
    while steps.is_none_or(|steps| index < steps) {
        kept.push(vec![1u8; step]);
        index += 1;
    }
    kept.iter().map(Vec::len).sum()
}

fn port() -> &'static HostPort {
    PORT.get().expect("the host port from load")
}

fn body(value: Value) -> PluginMessageBody {
    PluginMessageBody(BoundaryJson(value))
}

fn outcome(sent: Result<botster_core::engine::CallId, impl std::fmt::Display>) -> Value {
    match sent {
        Ok(call) => json!({ "call_id": call.0 }),
        Err(refusal) => json!({ "refused": refusal.to_string() }),
    }
}

impl PluginRuntime for TestRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        let handler_id = request.handler.handler_id.clone();
        let args = request.payload.0.clone();
        let completed = |payload: Value| {
            PluginInvocationResult::Completed(PluginInvocationSuccess {
                request_id: request.request_id.clone(),
                handler: request.handler.clone(),
                payload: Some(BoundaryJson(payload)),
            })
        };
        match handler_id.as_str() {
            "allocate" => {
                let step = args["step"].as_u64().expect("step") as usize;
                let total = allocate(step, args["steps"].as_u64());
                completed(json!({ "allocated": total }))
            }
            "logs_then_allocate" => {
                let line = "x".repeat(args["log_bytes"].as_u64().expect("log_bytes") as usize);
                for _ in 0..args["logs"].as_u64().expect("logs") {
                    port().log(body(json!(line)));
                }
                let step = args["step"].as_u64().expect("step") as usize;
                allocate(step, None);
                unreachable!("the memory cap ends an unbounded allocation")
            }
            "spin" => loop {
                std::hint::spin_loop();
            },
            "open_files" => {
                let mut opened = Vec::new();
                let errno = loop {
                    match File::open("/dev/null") {
                        Ok(file) => opened.push(file),
                        Err(error) => break error.raw_os_error(),
                    }
                };
                completed(json!({ "opened": opened.len(), "errno": errno }))
            }
            "host_calls" => {
                let count = args["count"].as_u64().unwrap_or(1);
                let max_result_bytes = args["max_result_bytes"].as_u64().unwrap_or(0) as usize;
                let outcomes: Vec<Value> = (0..count)
                    .map(|index| outcome(port().call(max_result_bytes, body(json!(index)))))
                    .collect();
                completed(json!(outcomes))
            }
            "reply" => {
                if args["signal"] == json!(true) {
                    signal_started();
                }
                completed(outcome(
                    port().reply(body(args["body"].clone()), &cancellation),
                ))
            }
            "try_reply" => completed(outcome(port().try_reply(body(args["body"].clone())))),
            "logs" => {
                let count = args["count"].as_u64().unwrap_or(1);
                let sent = (0..count)
                    .filter(|_| port().log(body(args["body"].clone())))
                    .count();
                completed(json!({ "sent": sent }))
            }
            "raw" => {
                if args["wait"] == json!(true) {
                    // Hold until the test writes a line to the go FIFO.
                    let go = std::env::var_os("PLUGIN_TEST_GO_FIFO").expect("PLUGIN_TEST_GO_FIFO");
                    let mut line = String::new();
                    std::io::Read::read_to_string(
                        &mut File::open(go).expect("open the go FIFO"),
                        &mut line,
                    )
                    .expect("read the go FIFO");
                }
                let mut frame = args["frame"].clone();
                if frame.get("invocation_request_id").is_none() {
                    frame["invocation_request_id"] = json!(request.request_id.0);
                }
                let frame_type = args["type"].as_u64().expect("raw frame type") as u8;
                write_raw_frame(frame_type, &serde_json::to_vec(&frame).expect("encode"));
                completed(json!({ "raw": true }))
            }
            "echo" => PluginInvocationResult::Completed(PluginInvocationSuccess {
                request_id: request.request_id,
                handler: request.handler,
                payload: Some(request.payload),
            }),
            "block" => wait_for_kill(),
            "block_after_signal" => {
                signal_started();
                wait_for_kill()
            }
            "abort" => std::process::abort(),
            "panic" => panic!("scripted panic in a handler"),
            "fatal_then_violation" => {
                // Publish a panic cause, then break the protocol and stay
                // alive: the host's violation kill lands after the cause.
                let fatal = b"\x02scripted fatal cause before a violation kill";
                // SAFETY: a write from a live buffer to the fatal pipe.
                unsafe { libc::write(4, fatal.as_ptr().cast(), fatal.len()) };
                write_raw_frame(0x7f, b"a frame the parent does not know");
                wait_for_kill()
            }
            "garbage" => {
                write_raw_frame(0x7f, b"not a frame the parent knows");
                wait_for_kill()
            }
            "forged_identity" => {
                // The live request id, but another plugin's handler.
                let mut handler = request.handler;
                handler.plugin_key = botster_core::PluginKey("someone-else".to_string());
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler,
                    payload: Some(BoundaryJson(serde_json::json!({ "forged": true }))),
                })
            }
            "foreign_result" => {
                let foreign = PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: RequestId("not-in-flight".to_string()),
                    handler: request.handler,
                    payload: None,
                });
                let payload = serde_json::to_vec(&foreign).expect("encode");
                write_raw_frame(0x85, &payload);
                wait_for_kill()
            }
            other => PluginInvocationResult::Failed(PluginInvocationFailure {
                request_id: request.request_id,
                handler: request.handler,
                kind: PluginInvocationFailureKind::HandlerFailed,
                timeout_ms: None,
                reason: format!("unknown test handler {other}"),
            }),
        }
    }
}

/// Tell the test that this handler is running.
fn signal_started() {
    let started = std::env::var_os("PLUGIN_TEST_STARTED_FIFO").expect("PLUGIN_TEST_STARTED_FIFO");
    let mut started = OpenOptions::new()
        .write(true)
        .open(started)
        .expect("open the started FIFO");
    writeln!(started, "started").expect("signal the start");
}

/// Write one frame straight to the IPC descriptor, around the library.
fn write_raw_frame(frame_type: u8, payload: &[u8]) {
    let mut frame = Vec::with_capacity(5 + payload.len());
    frame.extend_from_slice(&((payload.len() + 1) as u32).to_le_bytes());
    frame.push(frame_type);
    frame.extend_from_slice(payload);
    // SAFETY: a write from a live buffer to the worker's IPC descriptor.
    let written = unsafe { libc::write(3, frame.as_ptr().cast(), frame.len()) };
    assert_eq!(written, frame.len() as isize, "raw frame write");
}
