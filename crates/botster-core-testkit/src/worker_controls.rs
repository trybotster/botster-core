//! Controls for the scripted program edge and the worker process edge (Core A5-1, A5-3). Each control acts on the edge of
//! the session `session` of the handle that it names.

use crate::controls::ControlRegistry;
use crate::harness::TestkitHarness;
use crate::program::ProgramControl;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use botster_route_codec::prelude::{hex_decode, HexBytes};
use serde_json::{json, Value};

/// Registers the controls that the worker module owns.
pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("pty_input", pty_input);
    registry.register("pty_output", pty_output);
    registry.register("pty_output_unread", pty_output_unread);
    registry.register("pty_blocked", pty_blocked);
    registry.register("pty_chunk", pty_chunk);
    registry.register("pty_accept", pty_accept);
    registry.register("pty_fail_after", pty_fail_after);
    registry.register("program_write_size", program_write_size);
    registry.register("program_write_once", program_write_once);
    registry.register("process_end_worker", process_end_worker);
    registry.register("lose_worker", lose_worker);
    registry.register("break_control", break_control);
}

/// Every byte of PTY input that the program edge took, in order.
fn pty_input(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let control = program(harness, handle, args)?;
    Ok(json!({ "bytes": { "$bytes_hex": HexBytes(control.input_log()).to_hex() } }))
}

/// The program writes plain bytes. `scripted` holds facts for a fake that has no terminal; the real model derives them
/// (R-7), so it is not read.
fn pty_output(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    program(harness, handle, args)?.write_plain(&hex(args, "bytes_hex")?);
    Ok(Value::Null)
}

/// The bytes that the program wrote and the worker has not read.
fn pty_output_unread(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let control = program(harness, handle, args)?;
    Ok(json!({ "bytes": control.output_unread() }))
}

/// The PTY stops taking input, or takes it again with `on: false` (`on` defaults to true, as FakeCore's control does).
fn pty_blocked(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let on = args.get("on").and_then(Value::as_bool).unwrap_or(true);
    program(harness, handle, args)?.set_blocked(on);
    Ok(Value::Null)
}

fn pty_chunk(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    program(harness, handle, args)?.input_chunk(Some(count(args, "bytes")?));
    Ok(Value::Null)
}

fn pty_accept(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    program(harness, handle, args)?.accept_at_most(count(args, "bytes")?);
    Ok(Value::Null)
}

fn pty_fail_after(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    program(harness, handle, args)?.fail_after(count(args, "bytes")?);
    Ok(Value::Null)
}

fn program_write_size(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    program(harness, handle, args)?.write_size(Some(count(args, "bytes")?));
    Ok(Value::Null)
}

fn program_write_once(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    program(harness, handle, args)?.write_once(&hex(args, "bytes_hex")?);
    Ok(Value::Null)
}

/// The process edge ends the session's worker at this point, as a kill does (`Lost(WorkerGone)`).
fn process_end_worker(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let session = session(args)?;
    acted(&session, harness.workers().end_worker(handle, &session))
}

/// FakeCore's `lose_worker`. Its default reason, `worker_gone`, ends the worker process. A live worker whose link is
/// withheld (`worker_unreachable`) is the process edge's `withhold_control_link` (P6); ending the worker would give the
/// other state, so that reason is not offered here.
fn lose_worker(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let session = session(args)?;
    match args.get("reason").and_then(Value::as_str) {
        None | Some("worker_gone") => {
            acted(&session, harness.workers().end_worker(handle, &session))
        }
        Some("worker_unreachable") => Err(ControlError::Unsupported),
        Some(other) => Err(ControlError::Bad(format!("unknown reason {other}"))),
    }
}

/// The control link of the session's live worker breaks.
fn break_control(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let session = session(args)?;
    acted(&session, harness.workers().break_link(handle, &session))
}

fn session(args: &Value) -> Result<SessionId, ControlError> {
    args.get("session")
        .and_then(Value::as_str)
        .map(|s| SessionId(s.to_string()))
        .ok_or_else(|| ControlError::Bad("needs 'session'".into()))
}

/// The program edge of the session's payload.
fn program(
    harness: &TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<ProgramControl, ControlError> {
    let session = session(args)?;
    harness
        .workers()
        .program_control(handle, &session)
        .ok_or_else(|| ControlError::Bad(format!("session {} has no payload", session.0)))
}

fn count(args: &Value, name: &str) -> Result<usize, ControlError> {
    args.get(name)
        .and_then(Value::as_u64)
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| ControlError::Bad(format!("needs '{name}', a count")))
}

fn hex(args: &Value, name: &str) -> Result<Vec<u8>, ControlError> {
    args.get(name)
        .and_then(Value::as_str)
        .and_then(|text| hex_decode(text).ok())
        .ok_or_else(|| ControlError::Bad(format!("needs '{name}', hex bytes")))
}

/// A process control that found the session's worker succeeded.
fn acted(session: &SessionId, found: bool) -> Result<Value, ControlError> {
    if found {
        Ok(Value::Null)
    } else {
        Err(ControlError::Bad(format!(
            "session {} has no worker",
            session.0
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_core_conformance::{CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef};
    use botster_core_contract::prelude::*;

    fn spec() -> OpenSpec {
        OpenSpec {
            handle: "h".into(),
            data_dir: DataDirRef("d".into()),
            worker: Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: None,
            }),
            limits: json!({}),
        }
    }

    /// Starts session `s1` on `core` (Create, then Start), pumping until the Start completes. The program holds.
    fn start_s1(core: &mut dyn CoreApi) {
        let request = SpawnRequest {
            argv: vec![
                botster_probe_script::PROBE_BINARY.to_string(),
                r#"{"program":[{"hold":{}}]}"#.to_string(),
            ],
            env: std::collections::BTreeMap::new(),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            labels: std::collections::BTreeMap::new(),
            color_profile: None,
            notification_policy: None,
            size_policy: None,
        };
        let s1 = SessionId("s1".into());
        core.begin(Op::Create {
            session: s1.clone(),
            request,
        })
        .unwrap();
        let start = std::time::Instant::now();
        let now = |i: u64| Now {
            monotonic: start + std::time::Duration::from_millis(i),
            unix: 1,
        };
        let mut started = None;
        for i in 0..200 {
            core.pump(now(i));
            for event in core.poll_events(64) {
                if matches!(event, Event::Completed { op, .. } if Some(op) == started) {
                    return;
                }
                if matches!(&event, Event::Completed { .. }) && started.is_none() {
                    started = Some(core.begin(Op::Start { id: s1.clone() }).unwrap());
                }
            }
        }
        panic!("s1 did not start");
    }

    /// F9: two data directories each mint the instance `1-1`; a program control on one handle reaches that handle's payload
    /// only.
    #[test]
    fn program_controls_reach_the_handle_of_their_own_directory() {
        let mut harness = TestkitHarness::new(0);
        let mut a = harness.open(&spec()).expect("a Core");
        let mut b = harness
            .open(&OpenSpec {
                handle: "h2".into(),
                data_dir: DataDirRef("d2".into()),
                ..spec()
            })
            .expect("a second Core");
        start_s1(a.as_mut());
        start_s1(b.as_mut());
        harness
            .control(
                "h",
                "pty_output",
                &json!({"session": "s1", "bytes_hex": "6162"}),
            )
            .unwrap();
        let unread = |harness: &mut TestkitHarness, handle| {
            harness
                .control(handle, "pty_output_unread", &json!({"session": "s1"}))
                .unwrap()["bytes"]
                .as_u64()
                .unwrap()
        };
        assert_eq!(unread(&mut harness, "h"), 2);
        assert_eq!(
            unread(&mut harness, "h2"),
            0,
            "the other directory's payload is untouched"
        );
    }
}
