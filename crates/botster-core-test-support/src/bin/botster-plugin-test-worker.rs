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
//!
//! A sandbox profile `{"fail": "<reason>"}` makes the sandbox hook fail.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use botster_core::runtime::plugin_process::worker::{run_worker, LoadedPlugin, WorkerHooks};
use botster_core::runtime::plugin_process::LoadFrame;
use botster_core::{
    BoundaryJson, PluginCancellationToken, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginRuntime,
};
use serde_json::{json, Value};

static SANDBOX_APPLIED: AtomicBool = AtomicBool::new(false);

fn main() {
    run_worker(WorkerHooks {
        apply_sandbox,
        load,
    });
}

fn apply_sandbox(profile: &BoundaryJson) -> Result<(), String> {
    if let Some(reason) = profile.0.get("fail").and_then(Value::as_str) {
        return Err(reason.to_string());
    }
    SANDBOX_APPLIED.store(true, Ordering::SeqCst);
    Ok(())
}

fn load(frame: LoadFrame) -> Result<LoadedPlugin, String> {
    let mode = frame.config.0.get("mode").and_then(Value::as_str);
    match mode {
        Some("report") => Ok(LoadedPlugin {
            runtime: Arc::new(TestRuntime),
            registration: BoundaryJson(report()),
        }),
        Some("fail") => Err("scripted load failure".to_string()),
        Some("spin") => loop {
            std::hint::spin_loop();
        },
        Some("close_ipc_then_wait") => {
            // SAFETY: closes the worker's IPC descriptor; nothing reads it
            // again before the parent kills this process.
            unsafe { libc::close(3) };
            loop {
                // SAFETY: pause only waits for a signal. Its lifetime ends at
                // the parent's kill, an event the test controls.
                unsafe { libc::pause() };
            }
        }
        Some("panic") => panic!("scripted panic during load"),
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

struct TestRuntime;

impl PluginRuntime for TestRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        _cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        PluginInvocationResult::Failed(PluginInvocationFailure {
            request_id: request.request_id,
            handler: request.handler,
            kind: PluginInvocationFailureKind::HandlerFailed,
            timeout_ms: None,
            reason: "the test worker has no handlers yet".to_string(),
        })
    }
}
