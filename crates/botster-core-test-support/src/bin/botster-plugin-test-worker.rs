//! Scripted plugin worker for Core's real-process plugin host tests.
//!
//! It links the Core worker library with a Rust test runtime instead of Lua.
//! The `Load` config selects a behavior:
//! - `{"mode": "report"}`: load and report the environment, open descriptors,
//!   working directory, `RLIMIT_NOFILE`, and whether the sandbox hook ran first.
//! - `{"mode": "fail"}`: refuse the load.
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
use std::sync::Arc;

use botster_core::runtime::plugin_process::worker::{run_worker, LoadedPlugin, WorkerHooks};
use botster_core::runtime::plugin_process::{LoadFrame, PluginRegistration, SandboxProfile};
use botster_core::session::RequestId;
use botster_core::{
    BoundaryJson, PluginCancellationToken, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginInvocationSuccess, PluginRuntime,
};
use serde_json::{json, Value};

static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);

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

fn load(frame: LoadFrame) -> Result<LoadedPlugin, String> {
    let mode = frame.config.0 .0.get("mode").and_then(Value::as_str);
    match mode {
        Some("report") => Ok(LoadedPlugin {
            runtime: Arc::new(TestRuntime),
            registration: PluginRegistration(BoundaryJson(report())),
        }),
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
/// - `forged_identity`: return the live request id with another plugin's handler.
struct TestRuntime;

impl PluginRuntime for TestRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        let handler_id = request.handler.handler_id.clone();
        match handler_id.as_str() {
            "echo" => PluginInvocationResult::Completed(PluginInvocationSuccess {
                request_id: request.request_id,
                handler: request.handler,
                payload: Some(request.payload),
            }),
            "block" => wait_for_kill(),
            "block_after_signal" => {
                // Tell the test that this handler is running, then block.
                let started =
                    std::env::var_os("PLUGIN_TEST_STARTED_FIFO").expect("PLUGIN_TEST_STARTED_FIFO");
                let mut started = OpenOptions::new()
                    .write(true)
                    .open(started)
                    .expect("open the started FIFO");
                writeln!(started, "started").expect("signal the start");
                drop(started);
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
