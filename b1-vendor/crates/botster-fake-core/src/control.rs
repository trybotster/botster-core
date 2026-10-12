//! The test controls of FakeCore (design 6.3): inputs that only a test can cause. Each cites the clauses that need it.
//!
//! A control is a hook of this fake. It is not a contract obligation. A real Core gets the same effect from injected parts in its own
//! harness, never from a test branch in production code.

use crate::state::*;
use crate::FakeCore;
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::hex_encode;
use serde_json::{json, Value};

/// The control names, each with the clauses that need it.
pub const CONTROLS: &[&str] = &[
    "fail_next",   // Core ER-0, AD-7: the next op of a kind completes with an injected error
    "lose_worker", // Core IN-7, AD-2: the session's worker is gone; pending writes end as IN-7 says
    "break_control", // Core LC-5, A2-1: the control link of a session is broken
    "descendants", // Core LC-6: the child processes of the payload that are still alive
    "pty_chunk",   // Core AM-2, IN-6: at most n bytes of a host write reach the PTY per step
    "pty_blocked", // Core AM-2, DP-9: the PTY stops taking writes
    "pty_input",   // Core AM-2, IN-2: the bytes that reached the PTY, in admission order
    "pty_output",  // Core ST-1, OU-7: the program writes to the terminal
    "withhold_feature", // Core A2-6: the subject does not offer an optional feature
    "unavailable_bound", // Core SV-4: the platform cannot apply an OS bound
    "control_queue", // Core IN-6: the control queue of a session has no room, or has room again
    "revisions", // Core IN-10, ST-1: the worker's revisions (the oracle for the lag in `terminal_state`)
];

impl FakeCore {
    pub fn has_control(op: &str) -> bool {
        CONTROLS.contains(&op)
    }

    /// Runs a control. `Err` means that the control is unknown or its arguments are wrong.
    pub fn control(&self, op: &str, args: &Value) -> Result<Value, String> {
        let mut inner = self.lock();
        let session = || -> Result<SessionId, String> {
            args.get("session")
                .and_then(Value::as_str)
                .map(|s| SessionId(s.to_string()))
                .ok_or_else(|| format!("control '{op}' needs 'session'"))
        };
        let flag = args.get("on").and_then(Value::as_bool).unwrap_or(true);
        match op {
            "fail_next" => {
                let kind = args
                    .get("target")
                    .and_then(Value::as_str)
                    .ok_or("needs 'target': the op kind")?;
                let kind = crate::pump::KINDS
                    .iter()
                    .find(|k| **k == kind)
                    .ok_or("unknown op kind")?;
                let error: ErrorCode =
                    serde_json::from_value(args.get("error").cloned().ok_or("needs 'error'")?)
                        .map_err(|e| e.to_string())?;
                inner
                    .injected
                    .push((kind, CoreError::new(error, "injected")));
            }
            "lose_worker" => {
                let sid = session()?;
                let reason = match args.get("reason").and_then(Value::as_str) {
                    Some("worker_unreachable") => LostReason::WorkerUnreachable,
                    _ => LostReason::WorkerGone,
                };
                let s = inner.sessions.get_mut(&sid).ok_or("no such session")?;
                if let Some(w) = s.worker.as_mut() {
                    w.link_lost();
                }
                s.state = SessionState::Lost(reason);
                s.payload = Payload::None;
                let instance = s.instance.clone();
                inner.adm.get_mut(&sid).expect("row").state = Adm::Lost;
                inner.post(Event::SessionState {
                    id: sid,
                    instance,
                    state: SessionState::Lost(reason),
                });
                inner.signal();
            }
            "break_control" => {
                let sid = session()?;
                inner
                    .sessions
                    .get_mut(&sid)
                    .ok_or("no such session")?
                    .control_broken = flag;
            }
            "descendants" => {
                let sid = session()?;
                let s = inner.sessions.get(&sid).ok_or("no such session")?;
                return Ok(json!({ "alive": s.children }));
            }
            "pty_chunk" => {
                let sid = session()?;
                let n = args
                    .get("bytes")
                    .and_then(Value::as_u64)
                    .map(|n| n as usize);
                inner.worker_mut(&sid)?.set_pty_chunk(n);
            }
            "pty_blocked" => {
                let sid = session()?;
                inner
                    .worker_mut(&sid)?
                    .control("pty_blocked", args, None)
                    .map_err(|e| e.0)?;
            }
            "pty_input" => {
                let sid = session()?;
                let w = inner.worker_mut(&sid)?;
                let segments: Vec<Value> = w
                    .pty_bytes()
                    .into_iter()
                    .map(|(op, b)| json!({ "op": op.map(|o| o.0), "bytes": { "$bytes_hex": hex_encode(&b) } }))
                    .collect();
                return Ok(
                    json!({ "bytes": { "$bytes_hex": hex_encode(&w.pty_stream()) }, "segments": segments }),
                );
            }
            "pty_output" => {
                let sid = session()?;
                inner
                    .worker_mut(&sid)?
                    .control("pty_output", args, None)
                    .map_err(|e| e.0)?;
            }
            "withhold_feature" => {
                let name = args.get("name").cloned().ok_or("needs 'name'")?;
                let feature: Feature = serde_json::from_value(name).map_err(|e| e.to_string())?;
                inner.withheld.insert(feature);
            }
            "unavailable_bound" => {
                let bound: Bound =
                    serde_json::from_value(args.get("bound").cloned().ok_or("needs 'bound'")?)
                        .map_err(|e| e.to_string())?;
                inner.unavailable.insert(bound);
            }
            "control_queue" => {
                let sid = session()?;
                if flag {
                    inner.full.insert(sid);
                } else {
                    inner.full.remove(&sid);
                    // Core IN-6: a real wake follows when the control queue has room again (`SessionWritable`, class K).
                    if inner.refused_full.remove(&sid) {
                        let instance = inner.sess(&sid).map_err(|e| e.detail)?.instance.clone();
                        inner.post(Event::SessionWritable { id: sid, instance });
                    }
                }
            }
            "revisions" => {
                let sid = session()?;
                let w = inner.worker_mut(&sid)?;
                return Ok(
                    json!({ "client": w.client_rev(), "host": w.host_rev(), "model": w.model_rev() }),
                );
            }
            other => return Err(format!("unknown control '{other}'")),
        }
        Ok(Value::Null)
    }
}

impl crate::Inner {
    fn worker_mut(
        &mut self,
        sid: &SessionId,
    ) -> Result<&mut botster_fake_route::FakeWorker, String> {
        self.sessions
            .get_mut(sid)
            .and_then(|s| s.worker.as_mut())
            .ok_or_else(|| "the session has no worker".to_string())
    }
}
