//! Controls for the scripted program edge and the worker process edge (Core A5-1, A5-3).

use crate::controls::ControlRegistry;
use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use botster_route_codec::prelude::{hex_decode, HexBytes};
use serde_json::{json, Value};

/// Each control acts on the edge of the session that belongs to the specified handle.
pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("pty_input", |harness, handle, args| {
        program_control(harness, handle, "pty_input", args)
    });
    registry.register("pty_output", |harness, handle, args| {
        program_control(harness, handle, "pty_output", args)
    });
    registry.register("pty_output_unread", |harness, handle, args| {
        program_control(harness, handle, "pty_output_unread", args)
    });
    registry.register("pty_blocked", |harness, handle, args| {
        program_control(harness, handle, "pty_blocked", args)
    });
    registry.register("pty_chunk", |harness, handle, args| {
        program_control(harness, handle, "pty_chunk", args)
    });
    registry.register("pty_accept", |harness, handle, args| {
        program_control(harness, handle, "pty_accept", args)
    });
    registry.register("pty_fail_after", |harness, handle, args| {
        program_control(harness, handle, "pty_fail_after", args)
    });
    registry.register("program_write_size", |harness, handle, args| {
        program_control(harness, handle, "program_write_size", args)
    });
    registry.register("program_write_once", |harness, handle, args| {
        program_control(harness, handle, "program_write_once", args)
    });
    registry.register("process_end_worker", |harness, handle, args| {
        worker_control(harness, handle, "process_end_worker", args)
    });
    registry.register("lose_worker", |harness, handle, args| {
        worker_control(harness, handle, "lose_worker", args)
    });
    registry.register("break_control", |harness, handle, args| {
        worker_control(harness, handle, "break_control", args)
    });
}

fn program_control(
    harness: &mut TestkitHarness,
    handle: &str,
    op: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let bad = |why: String| ControlError::Bad(why);
    let session = args
        .get("session")
        .and_then(Value::as_str)
        .map(|s| SessionId(s.to_string()))
        .ok_or_else(|| bad(format!("{op} needs 'session'")))?;
    let control = harness
        .workers()
        .program_control(handle, &session)
        .ok_or_else(|| bad(format!("{op}: session {} has no payload", session.0)))?;
    let count = |name: &str| {
        args.get(name)
            .and_then(Value::as_u64)
            .and_then(|n| usize::try_from(n).ok())
            .ok_or_else(|| bad(format!("{op} needs '{name}', a count")))
    };
    let hex = |name: &str| {
        args.get(name)
            .and_then(Value::as_str)
            .and_then(|text| hex_decode(text).ok())
            .ok_or_else(|| bad(format!("{op} needs '{name}', hex bytes")))
    };
    match op {
        "pty_input" => {
            return Ok(
                json!({ "bytes": { "$bytes_hex": HexBytes(control.input_log()).to_hex() } }),
            )
        }
        "pty_output_unread" => return Ok(json!({ "bytes": control.output_unread() })),
        // `scripted` holds facts for a fake that has no terminal; the real model derives them (R-7), so it is not read.
        "pty_output" => control.write_plain(&hex("bytes_hex")?),
        "program_write_once" => control.write_once(&hex("bytes_hex")?),
        "pty_blocked" => {
            control.set_blocked(args.get("on").and_then(Value::as_bool).unwrap_or(true))
        }
        "pty_chunk" => control.input_chunk(Some(count("bytes")?)),
        "pty_accept" => control.accept_at_most(count("bytes")?),
        "pty_fail_after" => control.fail_after(count("bytes")?),
        "program_write_size" => control.write_size(Some(count("bytes")?)),
        _ => return Err(ControlError::Unsupported),
    }
    Ok(Value::Null)
}

/// The process controls of a session's worker (Core A5-1, A5-3): `process_end_worker` and FakeCore's `lose_worker` (reason
/// `worker_gone`, its default) end the worker process at this point; `break_control` breaks its control link while it
/// lives.
fn worker_control(
    harness: &mut TestkitHarness,
    handle: &str,
    op: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let session = args
        .get("session")
        .and_then(Value::as_str)
        .map(|s| SessionId(s.to_string()))
        .ok_or_else(|| ControlError::Bad(format!("{op} needs 'session'")))?;
    let done = match (op, args.get("reason").and_then(Value::as_str)) {
        ("break_control", _) => harness.workers().break_link(handle, &session),
        // FakeCore's default reason: the worker process ends (`Lost(WorkerGone)`).
        ("process_end_worker", _) | ("lose_worker", None | Some("worker_gone")) => {
            harness.workers().end_worker(handle, &session)
        }
        // A live worker whose link is withheld (`Lost(WorkerUnreachable)`) is the process edge's `withhold_control_link`
        // (P6); ending the worker would give the other state, so it is not offered here.
        ("lose_worker", Some("worker_unreachable")) => return Err(ControlError::Unsupported),
        (_, Some(other)) => {
            return Err(ControlError::Bad(format!("{op}: unknown reason {other}")))
        }
        _ => return Err(ControlError::Unsupported),
    };
    if done {
        Ok(Value::Null)
    } else {
        Err(ControlError::Bad(format!(
            "{op}: session {} has no worker",
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
