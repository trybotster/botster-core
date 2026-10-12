//! The real form of `oracle_resume` (`docs/core-testkit-controls.md`, "The comparator and the real-tier form of
//! `oracle_resume`"; steward ruling R-62). The model is in the worker process, so the harness reads it through an
//! independent anchor and the shared comparator ([`crate::oracle_compare`]):
//!
//! - **O_raw** applies the session's output history: every `pty_output` that the session's probe acknowledged, in order
//!   ([`super::program`]). The harness never asks the subject for these bytes.
//! - **O_resume** loads the capture's pages, then applies every `pty_output` of the session that the transcript sent after the
//!   capture step: the outputs whose mark is not below the capture's.
//! - A **second capture**, taken when the control runs, is decoded into an oracle terminal: the session's model. It runs
//!   only at a quiet point (R-69): a poll that returns any event other than its own `Completed` gives `unsupported_control`
//!   `events_pending`, never a pass.
//!
//! `equal` is true only if O_resume equals O_raw and the second capture equals O_raw.
//!
//! The capture layer ([`crate::capture_layer`]) sits between the driver and Core's own composition. It records each
//! transcript capture with its cut and pages, and it takes the second capture. It never holds or re-injects an event.
//!
//! Conditions (R-62):
//! 1. The output history must be known. A session whose program is not only `hold` steps, or whose probe has a `fake`, gives
//!    `unsupported_control` `unknown_output_history`.
//! 2. The transcript fences each capture (R-57 `await_title`); a wrong cut gives only a false failure.
//! 3. A session that a `Resize` or a `SetColorProfile` reached during its history needs its own ruling: plain
//!    `unsupported_control`, never a pass.

use super::RealCoreHarness;
use crate::capture_layer::{lock, Resumed};
use crate::controls::parse;
use crate::oracle_compare::{raw_oracle, resume_holds};
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::*;
use botster_terminal_ghostty::{History, Terminal};
use serde::Deserialize;
use serde_json::{json, Value};

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    session: SessionId,
    capture: CaptureId,
}

/// Condition 1: the program writes only its `pty_output` bytes. The driver puts the script in `argv[1]`
/// (`{"program": [...], "fake": {...}}`); only `hold` steps and no `fake` qualify.
pub(super) fn known_output_history(request: &SpawnRequest) -> bool {
    let Some(script) = request
        .argv
        .get(1)
        .and_then(|script| serde_json::from_str::<Value>(script).ok())
    else {
        return false;
    };
    let holds_only = script
        .get("program")
        .and_then(Value::as_array)
        .is_some_and(|steps| {
            steps.iter().all(|step| {
                step.as_object()
                    .is_some_and(|step| step.len() == 1 && step.contains_key("hold"))
            })
        });
    let no_fake = script
        .get("fake")
        .is_none_or(|fake| fake.as_object().is_some_and(serde_json::Map::is_empty));
    holds_only && no_fake
}

/// An oracle terminal that loads `pages`.
fn decoded(pages: &[u8], size: &Size, what: &str) -> Result<Terminal, ControlError> {
    Terminal::from_snapshot(pages, History::On, size.cell_px)
        .map_err(|e| ControlError::Bad(format!("oracle_resume: {what} does not decode: {e:?}")))
}

impl RealCoreHarness {
    /// `oracle_resume {session, capture}` → `{equal}` (Core ST-6b, R-62): see the module documentation.
    pub(super) fn oracle_resume(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let Resume { session, capture } = parse(args)?;
        let (_, row) = self.stored(handle, &session)?;
        if !known_output_history(&row.request) {
            return Err(ControlError::UnsupportedFor(
                "unknown_output_history".into(),
            ));
        }
        let shared = self
            .handles
            .get(handle)
            .and_then(|open| open.resume.upgrade())
            .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
        let worker = row.worker.ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no worker process", session.0))
        })?;
        let Resumed {
            mark,
            pages,
            second,
        } = lock(&shared).resume(&session, capture, botster_test_process::CLEANUP)?;
        let history = self.programs.output(worker.identity());
        let all: Vec<u8> = history
            .iter()
            .flat_map(|(_, bytes)| bytes.clone())
            .collect();
        let after: Vec<u8> = history
            .iter()
            .filter(|(at, _)| *at >= mark)
            .flat_map(|(_, bytes)| bytes.clone())
            .collect();
        let size = row.request.size;
        let profile = row.request.color_profile.as_ref();
        let mut resumed = decoded(&pages, &size, "the capture")?;
        resumed.vt_write(&after);
        let model = decoded(&second, &size, "the second capture")?;
        let equal = resume_holds(resumed, model, || raw_oracle(&size, profile, &all))
            .map_err(|e| ControlError::Bad(format!("oracle_resume: {e:?}")))?;
        Ok(json!({ "equal": equal }))
    }
}

#[cfg(test)]
mod slow_tests {
    use super::super::slow_controls::{created_with, pump_until, sid, state};
    use super::*;
    use crate::candidate::Candidate;
    use botster_core_conformance::{CoreHarness, OpenSpec, WorkerBuild};
    use botster_route_codec::prelude::HexBytes;
    use std::collections::BTreeMap;

    /// The output of `st_6b_saved_cursor_tab_stops_margins_rendition_and_charsets_survive_a_cut` at `contracts-v0.1.36`:
    /// steps 6 (before the capture) and 13 (after it).
    const BEFORE: &[u8] =
        b"\x1b[3;5H\x1b[1;31m\x1b7\x1b[1;10H\x1bH\x1b[2;4r\x1b(0\x1b[0m\x1b]2;r62-st6b-dec-1\x1b\\";
    const AFTER: &[u8] = b"\x1b8x\tlqk\x1bD\x1bD\x1bD\x1b(By\x1b]2;r62-st6b-dec-2\x1b\\";

    /// The pages of an oracle terminal of the session's size that applied `bytes` without the saved cursor (DECSC): a capture
    /// with one field removed.
    fn without_saved_cursor(bytes: &[u8]) -> Vec<u8> {
        let at = bytes.windows(2).position(|w| w == b"\x1b7").unwrap();
        let mut terminal = Terminal::new(
            &Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            History::On,
        )
        .unwrap();
        terminal.vt_write(&[&bytes[..at], &bytes[at + 2..]].concat());
        terminal.snapshot().unwrap()
    }

    /// Pumps Core with a later clock and never polls, until a pump posts an event (the `Activity` of a new output, Core
    /// ST-7); returns how many the pumps posted.
    fn pumped_without_poll(core: &mut dyn CoreApi) -> u32 {
        let deadline = botster_test_process::Deadline::cleanup();
        let wake = core.wake_handle();
        let mut posted = 0;
        loop {
            posted += core
                .pump(Now {
                    monotonic: std::time::Instant::now(),
                    unix: 1_000_100,
                })
                .events_posted;
            if posted > 0 {
                return posted;
            }
            assert!(!deadline.expired(), "Core never posted an event");
            // timer: deadline — the cleanup bound ends a Core that never posts
            wake.wait(deadline.remaining());
        }
    }

    /// Pumps and polls until Core is quiet, as the driver's `await_quiet`: a pass whose pump reports no `more`, and edges
    /// with nothing that arrived or is unread (`edges_quiet`). Each event that Core posted is polled, so the next capture
    /// meets no event that the test did not take.
    fn quiet(harness: &mut RealCoreHarness, core: &mut dyn CoreApi) {
        let deadline = botster_test_process::Deadline::cleanup();
        let wake = core.wake_handle();
        loop {
            let report = core.pump(Now {
                monotonic: std::time::Instant::now(),
                unix: 1_000_000,
            });
            core.poll_events(usize::MAX);
            if !report.more
                && harness.control("a", "edges_quiet", &json!({})) == Ok(json!({"quiet": true}))
            {
                return;
            }
            // Every pass checks the bound, also a pass whose pump reports `more` (R-89).
            assert!(!deadline.expired(), "Core never got quiet");
            if !report.more {
                // timer: deadline — the cleanup bound ends a Core that never gets quiet
                wake.wait(deadline.remaining());
            }
        }
    }

    fn titled(core: &mut dyn CoreApi, session: &str, title: &str) {
        pump_until(core, |c| {
            c.terminal_state(&sid(session)).unwrap().title.as_deref() == Some(title)
        });
    }

    /// Pumps and polls until Core completes `op`, and returns the result of its public `Completed`.
    fn completed(core: &mut dyn CoreApi, op: OpId) -> OpResult {
        let deadline = botster_test_process::Deadline::cleanup();
        let wake = core.wake_handle();
        loop {
            core.pump(Now {
                monotonic: std::time::Instant::now(),
                unix: 1_000_000,
            });
            for event in core.poll_events(usize::MAX) {
                if let Event::Completed { op: id, result } = event {
                    if id == op {
                        return result;
                    }
                }
            }
            assert!(!deadline.expired(), "Core never completed the op");
            // timer: deadline — the cleanup bound ends an op that never completes
            wake.wait(deadline.remaining());
        }
    }

    /// Pumps and polls until Core completes the capture `op`, and returns its id.
    fn completed_capture(core: &mut dyn CoreApi, op: OpId) -> CaptureId {
        match completed(core, op) {
            OpResult::Ok(OpOutput::Capture(done)) => done.capture,
            other => panic!("CaptureSnapshot: {other:?}"),
        }
    }

    /// A `ReadScreen` of `session`, to its `Completed`. Core completes it after it applied every earlier output of the
    /// session, so it fences an `Observation::Output` that is still in flight after a title (real-tier audit rows 10-11).
    fn screen_read(core: &mut dyn CoreApi, session: &str) {
        let read = core
            .begin(Op::ReadScreen {
                session: sid(session),
                history: false,
            })
            .unwrap();
        match completed(core, read) {
            OpResult::Ok(OpOutput::Screen(_)) => {}
            other => panic!("ReadScreen of {session}: {other:?}"),
        }
    }

    /// R-62 on a real session of the prebuilt candidate, with the bytes of the `saved_cursor` transcript:
    /// - the real form gives `{equal: true}`;
    /// - a capture with the saved cursor removed gives `{equal: false}` through comparison 1 (the transcript's capture) and
    ///   through comparison 2 (the second capture);
    /// - an unknown capture, a capture of another session and an unknown session are `Bad`;
    /// - after a `Resize` of the session the control is `unsupported_control` (condition 3);
    /// - a session whose program is not only `hold` is `unsupported_control` `unknown_output_history` (condition 1).
    #[test]
    fn the_real_form_compares_both_captures_with_the_raw_output() {
        let dir = Candidate::beside_test_binary().expect("the candidate directory");
        let candidate = Candidate::locate(&dir).unwrap_or_else(|error| panic!("{error}"));
        let mut harness = RealCoreHarness::new(candidate).unwrap();
        let spec = OpenSpec {
            handle: "a".into(),
            data_dir: harness.data_dir("a"),
            worker: harness.worker(WorkerBuild::Current),
            limits: serde_json::to_value(CoreLimits::default()).unwrap(),
        };
        let mut core = harness.open(&spec).expect("open");
        for session in ["s1", "s2"] {
            created_with(&harness, core.as_mut(), session, harness.probe_env());
            core.begin(Op::Start { id: sid(session) }).expect("start");
            pump_until(core.as_mut(), |c| {
                state(c, session) == SessionState::Running
            });
        }
        let output = |h: &mut RealCoreHarness, session: &str, bytes: &[u8]| {
            h.control(
                "a",
                "pty_output",
                &json!({"session": session, "bytes_hex": HexBytes(bytes.to_vec()).to_hex()}),
            )
        };
        // The output of s2 goes between the outputs of s1: the history of s1 has only its own outputs, and its cut counts
        // the outputs of every session. The first output after the capture is s1's: it takes the capture's own mark. The
        // title is the last bytes of the noise, so `titled` proves the whole output (real-tier audit row 10).
        let noise = |h: &mut RealCoreHarness, core: &mut dyn CoreApi, title: &str| {
            let bytes = format!("\x1b[1;31mother\x1b7\x1b]2;{title}\x07");
            assert_eq!(output(h, "s2", bytes.as_bytes()), Ok(Value::Null));
            titled(core, "s2", title);
        };
        noise(&mut harness, core.as_mut(), "s2-1");
        assert_eq!(output(&mut harness, "s1", BEFORE), Ok(Value::Null));
        titled(core.as_mut(), "s1", "r62-st6b-dec-1");
        let op = core
            .begin(Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId("c1".into()),
            })
            .unwrap();
        let capture = completed_capture(core.as_mut(), op);
        assert_eq!(output(&mut harness, "s1", AFTER), Ok(Value::Null));
        titled(core.as_mut(), "s1", "r62-st6b-dec-2");
        noise(&mut harness, core.as_mut(), "s2-2");
        // An `Observation::Output` can still be in flight after a title. A `ReadScreen` of each session fences it before the
        // oracle reads the outputs (real-tier audit row 11).
        screen_read(core.as_mut(), "s2");
        screen_read(core.as_mut(), "s1");

        let resume = |h: &mut RealCoreHarness, session: &str, capture: CaptureId| {
            h.control(
                "a",
                "oracle_resume",
                &json!({"session": session, "capture": capture}),
            )
        };
        // R-69 item 3: after a quiet second capture, Core's open captures, pending ops, queue and next deadline are as before.
        let public = |core: &dyn CoreApi| {
            let diagnostics = core.diagnostics();
            (
                [
                    diagnostics["captures"].clone(),
                    diagnostics["pending_ops"].clone(),
                    diagnostics["queued_events"].clone(),
                ],
                core.next_deadline(),
            )
        };
        // A late event of the outputs above (an output `Activity`) would reach the second capture as `events_pending`
        // (78922927's flake: the stage-2 baseline of gate 3f1fc94d). The test takes every event first.
        quiet(&mut harness, core.as_mut());
        let before = public(core.as_ref());
        assert_eq!(before.0[0], json!(1), "the transcript's capture");
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Ok(json!({"equal": true}))
        );
        assert_eq!(public(core.as_ref()), before);
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Ok(json!({"equal": true})),
            "again"
        );
        assert_eq!(public(core.as_ref()), before);

        // Fault injection: the layer's copy of a capture without the saved cursor, then a second capture without it.
        let shared = harness.handles["a"].resume.upgrade().unwrap();
        let pages = lock(&shared).captures[&capture].pages.clone();
        lock(&shared).captures.get_mut(&capture).unwrap().pages = Ok(without_saved_cursor(BEFORE));
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Ok(json!({"equal": false})),
            "comparison 1"
        );
        lock(&shared).captures.get_mut(&capture).unwrap().pages = pages;
        lock(&shared).second = Some(without_saved_cursor(&[BEFORE, AFTER].concat()));
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Ok(json!({"equal": false})),
            "comparison 2"
        );
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Ok(json!({"equal": true}))
        );

        let bad = |result: Result<Value, ControlError>| matches!(result, Err(ControlError::Bad(_)));
        assert!(bad(resume(&mut harness, "s1", CaptureId(capture.0 + 100))));
        assert!(bad(resume(&mut harness, "s9", capture)));
        assert!(
            bad(resume(&mut harness, "s2", capture)),
            "another session's capture"
        );

        // R-69: an `Activity` of the session's output that the driver has not polled.
        assert_eq!(output(&mut harness, "s1", b"x"), Ok(Value::Null));
        let posted = pumped_without_poll(core.as_mut());
        assert!(posted > 0);
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Err(ControlError::UnsupportedFor("events_pending".into())),
            "the pending events: {posted}"
        );

        core.begin(Op::Resize {
            session: sid("s1"),
            size: Size {
                rows: 20,
                cols: 70,
                cell_px: None,
            },
        })
        .unwrap();
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Err(ControlError::UnsupportedFor(
                "size_or_profile_changed".into()
            ))
        );

        let print = json!({ "program": [{ "print": "x" }, { "hold": {} }] }).to_string();
        core.begin(Op::Create {
            session: sid("s3"),
            request: SpawnRequest {
                argv: vec![harness.probe_binary(), print],
                ..request(json!({}))
            },
        })
        .unwrap();
        pump_until(core.as_mut(), |c| c.get(&sid("s3")).is_ok());
        assert_eq!(
            resume(&mut harness, "s3", capture),
            Err(ControlError::UnsupportedFor(
                "unknown_output_history".into()
            ))
        );
        drop(core);
    }

    fn request(script: Value) -> SpawnRequest {
        SpawnRequest {
            argv: vec!["probe".into(), script.to_string()],
            env: BTreeMap::new(),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            labels: BTreeMap::new(),
            color_profile: None,
            notification_policy: None,
            size_policy: None,
        }
    }

    #[test]
    fn only_a_hold_program_with_no_fake_has_a_known_output_history() {
        for (script, known) in [
            (json!({"program": [{"hold": {}}], "fake": {}}), true),
            (json!({"program": [{"hold": {}}]}), true),
            (json!({"program": [{"hold": {}}, {"hold": {}}]}), true),
            (json!({"program": [{"print": "x"}, {"hold": {}}]}), false),
            (json!({"program": [{"hold": {}, "print": "x"}]}), false),
            (json!({"program": ["hold"]}), false),
            (
                json!({"program": [{"hold": {}}], "fake": {"echo": true}}),
                false,
            ),
            (json!({"program": {"hold": {}}}), false),
            (json!({}), false),
        ] {
            assert_eq!(
                known_output_history(&request(script.clone())),
                known,
                "{script}"
            );
        }
        let mut no_script = request(json!({}));
        no_script.argv.truncate(1);
        assert!(!known_output_history(&no_script));
        let mut not_json = request(json!({}));
        not_json.argv[1] = "hold".into();
        assert!(!known_output_history(&not_json));
    }
}
