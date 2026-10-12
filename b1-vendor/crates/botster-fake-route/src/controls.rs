//! The test controls of the fake: inputs that only a test can cause. Each cites its clauses.
//!
//! A control is a test hook of this fake. It is not a contract obligation (design 6.3).

use crate::*;
use botster_route_codec::prelude::{hex_decode, hex_encode};
use serde_json::{json, Value};

/// The control names, each with the clauses that need it.
pub const CONTROLS: &[&str] = &[
    "pty_output", // Codec BP-2, TS-3, TS-4c, TS-4d, TS-8: the program writes to the terminal; `scripted` holds the script facts
    "pty_exit",   // Codec TS-8: the process exits
    "pty_blocked", // Codec TS-5, TS-7: the PTY stops taking writes
    "pty_fail_after", // Codec TS-5b: the next PTY write fails after n bytes
    "pty_input",  // Codec BP-2, TS-5, TS-5b, TS-5d, TS-5f: the bytes that reached the PTY
    "stall_reader", // Codec TS-7: the reader stops making progress
    "queue_overflow", // Codec TS-4c, TS-7: the route's queued output reaches its byte bound
    "fail_writes", // Codec TS-4c, TS-9: a started frame can no longer be written
    "drop_transport", // Codec TS-9: the transport dies
    "client_close", // Codec TS-3, TS-10: the client closes its end
    "advance_clock", // Codec TS-7: the injected clock moves
    "terminal_query", // Codec TS-7: the program sends a query that needs a reply
    "close_route", // Codec TS-8, TS-10, BP-5: the Hub closes the route
    "fail_file_write", // Codec TS-5f: the file cannot be written on the host
    "fail_next_attach", // Codec TS-4: the screen cannot be produced
    "history_capture", // Codec TS-4: what the history capture gives at the next attach
    "fs_list",    // Codec TS-5f: the files on the host
    "fs_count",   // Codec TS-5f: how many files are on the host
    "effective_size", // Codec TS-4d, TS-6: the size applied to the PTY
    "queue_within_bound", // Codec TS-7: the route's outgoing queue is within its byte bound
    "input_blocked", // Codec TS-5, TS-7: the worker has input waiting on the route and does not read it
];

/// A control that the fake does not know, or whose arguments are wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ControlError(pub String);

impl FakeWorker {
    /// Runs a control. `route` is the route id for a control that names one.
    pub fn control(
        &mut self,
        op: &str,
        args: &Value,
        route: Option<RouteId>,
    ) -> Result<Value, ControlError> {
        let bad = |what: &str| ControlError(format!("control '{op}': {what}"));
        let hex_arg = |name: &str| -> Result<Vec<u8>, ControlError> {
            let text = args
                .get(name)
                .and_then(Value::as_str)
                .ok_or_else(|| bad(&format!("needs '{name}'")))?;
            hex_decode(text).map_err(|e| bad(&e))
        };
        let flag = args.get("on").and_then(Value::as_bool).unwrap_or(true);
        let need = || {
            route
                .filter(|r| self.routes.contains_key(r))
                .ok_or_else(|| bad("needs a route"))
        };
        match op {
            "pty_output" => {
                let scripted: crate::script::ScriptedOutput = args
                    .get("scripted")
                    .map(|v| serde_json::from_value(v.clone()))
                    .transpose()
                    .map_err(|e| bad(&e.to_string()))?
                    .unwrap_or_default();
                let modes = scripted
                    .modes
                    .as_ref()
                    .map(|m| m.apply(&self.modes))
                    .transpose()
                    .map_err(|e| bad(&e))?;
                self.pty_output(&hex_arg("bytes_hex")?, scripted.notifications, modes)
            }
            "pty_exit" => {
                let exit = Exit {
                    code: args.get("code").and_then(Value::as_i64).map(|c| c as i32),
                    signal: None,
                };
                self.exited = Some(exit);
                // Codec TS-5b: input still waiting for the PTY is refused session_ended.
                for w in std::mem::take(&mut self.pty_pending) {
                    if !w.silent {
                        self.refuse(w.route, w.op, RefusalReason::SessionEnded);
                    }
                }
                // Codec TS-8: the routes close with session_ended and the exit, after the final tail is delivered.
                self.exit_pending = true;
                self.pump_source();
            }
            "pty_blocked" => {
                self.pty_blocked = flag;
                self.flush_pty();
            }
            "pty_fail_after" => {
                self.pty_fail_after =
                    Some(args.get("bytes").and_then(Value::as_u64).unwrap_or(0) as usize)
            }
            "pty_input" => {
                // One entry per write, so a transcript can name "the bytes written for op N" (Codec TS-5b).
                let segments: Vec<Value> = self
                    .pty_log
                    .iter()
                    .map(|(op, b)| json!({ "op": op, "bytes": { "$bytes_hex": hex_encode(b) } }))
                    .collect();
                let all: Vec<u8> = self.pty_log.iter().flat_map(|(_, b)| b.clone()).collect();
                return Ok(
                    json!({ "bytes": { "$bytes_hex": hex_encode(&all) }, "segments": segments }),
                );
            }
            "stall_reader" => {
                let id = need()?;
                self.routes.get_mut(&id).expect("route").stalled = flag;
                self.pump_source();
            }
            "queue_overflow" => {
                let id = need()?;
                self.resync(id, ResyncReason::Overflow);
            }
            "fail_writes" => {
                let id = need()?;
                self.routes.get_mut(&id).expect("route").fail_writes = flag;
            }
            "drop_transport" => {
                let id = need()?;
                self.end_route(id, RouteEndReason::TransportLost);
            }
            "client_close" => {
                let id = need()?;
                // Codec TR-3: a frame that is incomplete when the channel closes is discarded. Codec TS-10: only this route ends.
                self.routes
                    .get_mut(&id)
                    .expect("route")
                    .chunks
                    .discard_incomplete();
                self.end_route(id, RouteEndReason::TransportLost);
            }
            "advance_clock" => self.advance_clock(
                args.get("ms")
                    .and_then(Value::as_u64)
                    .ok_or_else(|| bad("needs 'ms'"))?,
            ),
            "terminal_query" => {
                let kind = args
                    .get("kind")
                    .map(|k| serde_json::from_value::<QueryKind>(k.clone()))
                    .transpose()
                    .map_err(|e| bad(&e.to_string()))?;
                let shadow = if args.get("shadow_hex").is_some() {
                    Some(hex_arg("shadow_hex")?)
                } else {
                    None
                };
                let qid = self.terminal_query(&hex_arg("request_hex")?, kind, shadow);
                return Ok(json!({ "query_id": qid }));
            }
            "close_route" => {
                let id = need()?;
                let reason = serde_json::from_value::<CloseReason>(
                    args.get("reason").cloned().unwrap_or(json!("detached")),
                )
                .map_err(|e| bad(&e.to_string()))?;
                self.close_route(id, reason, None);
            }
            "fail_file_write" => self.fail_file_write = flag,
            "fail_next_attach" => {
                let reason = serde_json::from_value::<AttachFailedReason>(
                    args.get("reason")
                        .cloned()
                        .unwrap_or(json!("screen_unavailable")),
                )
                .map_err(|e| bad(&e.to_string()))?;
                self.fail_next_attach = Some(reason);
            }
            "history_capture" => {
                self.cfg.history_capture =
                    serde_json::from_value(args.get("state").cloned().unwrap_or(json!("complete")))
                        .map_err(|e| bad(&e.to_string()))?;
            }
            "fs_list" => {
                let files: Vec<Value> = self
                    .files
                    .iter()
                    .map(|(p, c)| json!({ "path": p, "content": { "$bytes_hex": hex_encode(c) } }))
                    .collect();
                return Ok(json!({ "files": files }));
            }
            "fs_count" => return Ok(json!({ "count": self.files.len() })),
            "effective_size" => return Ok(json!({ "rows": self.rows, "cols": self.cols })),
            "queue_within_bound" => {
                let within = self.queued_bytes(need()?) <= self.cfg.route_queue_bytes;
                return Ok(json!({ "within": within }));
            }
            "input_blocked" => return Ok(json!({ "blocked": self.input_blocked(need()?) })),
            other => return Err(ControlError(format!("unknown control '{other}'"))),
        }
        Ok(Value::Null)
    }
}
