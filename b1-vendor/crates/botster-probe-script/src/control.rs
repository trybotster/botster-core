//! The probe's control socket: the real-tier side channel of the program controls `pty_output`, `pty_input`, `pty_blocked`,
//! `pty_accept` and `pty_chunk` (steward approval of 2026-10-10, through the Stage 1 Core lead), and `pty_queued` (steward ruling
//! R-74).
//!
//! A real harness sets [`CONTROL_ENV`] in the `env` of the `Create` request (the driver adds it through
//! `CoreHarness::probe_env`; no transcript names it, Core A5-4). The value is the path of a Unix socket that the harness listens
//! on. After its terminal setup (R-48) and before its first step, the probe connects to it and sends [`Report::Hello`]. A probe
//! that cannot connect exits with [`CONNECT_FAILED_EXIT`], which the harness reports as a setup failure, never a pass. Without
//! the variable, the probe opens no socket and behaves exactly as before.
//!
//! Each message is one line of JSON. The harness sends [`Request`]s; the probe applies each one in order and then sends
//! [`Report::Ack`], so an acknowledged request is in effect. The probe reports every stdin read with [`Report::Input`], and the
//! end of stdin with [`Report::InputEnd`]. The probe sends each of these reports before its script can act on the read, so the
//! harness sees every read even when the script then exits. Control traffic uses only the socket: stdout carries only the script's output and the
//! bytes of [`Request::Output`]. The probe holds the socket until it exits, so the end of the socket means that the probe exited;
//! a socket that fails ends the probe with [`CONTROL_FAILED_EXIT`].

use serde::{Deserialize, Serialize};

/// The environment variable that names the harness's control socket.
pub const CONTROL_ENV: &str = "BOTSTER_PROBE_CONTROL";

/// The exit code of a probe that could not connect to its control socket (`EX_UNAVAILABLE`). It is distinct from the probe's
/// other codes: 2 (the end of stdin in `print_after_input`) and 64 (a script that does not parse).
pub const CONNECT_FAILED_EXIT: i32 = 69;

/// The exit code of a connected probe whose control socket failed (`EX_SOFTWARE`): the harness closed its end, the socket or
/// the poll failed, a request did not parse, or a report could not be sent. The probe exits rather than drop the socket while it lives, so the end of the
/// socket always means that the probe exited.
pub const CONTROL_FAILED_EXIT: i32 = 70;

/// The exit code of a probe whose stdin is a terminal and whose R-48 terminal setup failed (`EX_OSERR`): `tcgetattr` or
/// `tcsetattr` returned an error. The probe never runs its script on a canonical terminal in silence, because a canonical
/// terminal holds input until a newline and, on macOS, drops it when full (steward ruling R-74). The harness reports it as a
/// setup failure, never a pass.
pub const TERMINAL_SETUP_FAILED_EXIT: i32 = 71;

/// A request from the harness to the probe.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    /// `pty_output`: the probe writes these bytes (hex) to stdout, after every earlier `Output`. The acknowledgement does not wait
    /// for the write, which source backpressure may block (Core OU-3d).
    Output(String),
    /// `pty_blocked`: `true` stops the probe's reads of stdin; `false` resumes them and removes an `Accept` limit.
    Blocked(bool),
    /// `pty_accept`: the probe reads at most this many more bytes of stdin, then stops as for `Blocked(true)`.
    Accept(usize),
    /// `pty_chunk`: each read of stdin takes at most this many bytes. Zero is refused by the harness, never sent.
    Chunk(usize),
    /// `pty_queued` (steward ruling R-74): the probe answers with [`Report::Queued`] instead of [`Report::Ack`]. It reads and
    /// changes nothing.
    Queued {},
}

/// A report from the probe to the harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Report {
    /// The first line: the probe's own process id (the payload's pid) and its parent's, which is the session's worker (the
    /// worker spawns the payload as its direct child). A harness finds a session's probe by the worker of Core's registry row.
    Hello { pid: u32, parent: u32 },
    /// The request before it is in effect.
    Ack {},
    /// The bytes (hex) of one read of stdin, in order.
    Input(String),
    /// Stdin ended (end of file or a read error). No `Input` follows.
    InputEnd {},
    /// The answer to [`Request::Queued`]: the bytes that wait in stdin's kernel buffer, which the probe has not read (`FIONREAD`
    /// on stdin; Linux counts only the line discipline's buffer). A transcript fences on it (`>= 1`) and never asserts its exact
    /// value, which is a kernel count (R-74).
    Queued { bytes: u64 },
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// The wire form is the one that the probe and the harness share: one externally tagged JSON object per line.
    #[test]
    fn the_wire_form_is_one_tagged_object() {
        let cases = [
            (Request::Output("6869".into()), json!({ "output": "6869" })),
            (Request::Blocked(true), json!({ "blocked": true })),
            (Request::Accept(2), json!({ "accept": 2 })),
            (Request::Chunk(1), json!({ "chunk": 1 })),
            (Request::Queued {}, json!({ "queued": {} })),
        ];
        for (request, wire) in cases {
            assert_eq!(serde_json::to_value(&request).unwrap(), wire);
            assert_eq!(serde_json::from_value::<Request>(wire).unwrap(), request);
        }
        let cases = [
            (
                Report::Hello { pid: 7, parent: 3 },
                json!({ "hello": { "pid": 7, "parent": 3 } }),
            ),
            (Report::Ack {}, json!({ "ack": {} })),
            (Report::Input("61".into()), json!({ "input": "61" })),
            (Report::InputEnd {}, json!({ "input_end": {} })),
            (
                Report::Queued { bytes: 3 },
                json!({ "queued": { "bytes": 3 } }),
            ),
        ];
        for (report, wire) in cases {
            assert_eq!(serde_json::to_value(&report).unwrap(), wire);
            assert_eq!(serde_json::from_value::<Report>(wire).unwrap(), report);
        }
    }

    /// An unknown request is refused, so a harness and a probe of different versions fail at once instead of guessing.
    #[test]
    fn an_unknown_request_is_refused() {
        assert!(serde_json::from_value::<Request>(json!({ "flush": {} })).is_err());
    }
}
