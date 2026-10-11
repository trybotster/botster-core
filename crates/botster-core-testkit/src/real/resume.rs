//! The real form of `oracle_resume` (`docs/core-testkit-controls.md`, "The comparator and the real-tier form of
//! `oracle_resume`"; steward ruling R-62). The model is in the worker process, so the harness reads it through an
//! independent anchor and the shared comparator ([`crate::oracle_compare`]):
//!
//! - **O_raw** applies the session's output history: every `pty_output` that the session's probe acknowledged, in order
//!   ([`super::program`]). The harness never asks the subject for these bytes.
//! - **O_resume** loads the capture's pages, then applies every `pty_output` of the session that the transcript sent after the
//!   capture step: the outputs whose mark is not below the capture's.
//! - A **second capture**, taken when the control runs, is decoded into an oracle terminal: the session's model.
//!
//! `equal` is true only if O_resume equals O_raw and the second capture equals O_raw.
//!
//! The capture layer ([`crate::capture_layer`]) sits between the driver and Core's own composition. It records each
//! transcript capture with its cut and pages, and it takes the second capture.
//!
//! Conditions (R-62):
//! 1. The output history must be known. A session whose program is not only `hold` steps, or whose probe has a `fake`, gives
//!    `unsupported_control` `unknown_output_history`.
//! 2. The transcript fences each capture (R-57 `await_title`); a wrong cut gives only a false failure.
//! 3. A session that a `Resize` or a `SetColorProfile` reached during its history needs its own ruling: plain
//!    `unsupported_control`, never a pass.

use super::RealCoreHarness;
use crate::capture_layer::lock;
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
fn known_output_history(request: &SpawnRequest) -> bool {
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
        let mut shared = lock(&shared);
        if shared.changed.contains(&session) {
            return Err(ControlError::Unsupported);
        }
        let recorded = shared
            .captures
            .get(&capture)
            .filter(|recorded| recorded.session == session)
            .cloned()
            .ok_or_else(|| {
                ControlError::Bad(format!(
                    "no completed capture {} of the session {}",
                    capture.0, session.0
                ))
            })?;
        let pages = recorded
            .pages
            .map_err(|e| ControlError::Bad(format!("oracle_resume: {e}")))?;
        let worker = row.worker.ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no worker process", session.0))
        })?;
        let history = self.programs.output(worker.identity());
        let all: Vec<u8> = history
            .iter()
            .flat_map(|(_, bytes)| bytes.clone())
            .collect();
        let after: Vec<u8> = history
            .iter()
            .filter(|(mark, _)| *mark >= recorded.mark)
            .flat_map(|(_, bytes)| bytes.clone())
            .collect();
        let second = shared.capture_now(&session, botster_test_process::CLEANUP)?;
        #[cfg(test)]
        let second = shared.second.take().unwrap_or(second);
        drop(shared);
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

    fn titled(core: &mut dyn CoreApi, title: &str) {
        pump_until(core, |c| {
            c.terminal_state(&sid("s1")).unwrap().title.as_deref() == Some(title)
        });
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
        created_with(&harness, core.as_mut(), "s1", harness.probe_env());
        core.begin(Op::Start { id: sid("s1") }).expect("start");
        pump_until(core.as_mut(), |c| state(c, "s1") == SessionState::Running);
        let output = |h: &mut RealCoreHarness, bytes: &[u8]| {
            h.control(
                "a",
                "pty_output",
                &json!({"session": "s1", "bytes_hex": HexBytes(bytes.to_vec()).to_hex()}),
            )
        };
        assert_eq!(output(&mut harness, BEFORE), Ok(Value::Null));
        titled(core.as_mut(), "r62-st6b-dec-1");
        let shared = harness.handles["a"].resume.upgrade().unwrap();
        core.begin(Op::CaptureSnapshot {
            session: sid("s1"),
            owner: ClientId("c1".into()),
        })
        .unwrap();
        // The pumps take the events, and the layer records the completed capture.
        pump_until(core.as_mut(), |_| !lock(&shared).captures.is_empty());
        let capture = *lock(&shared).captures.keys().next().unwrap();
        assert_eq!(output(&mut harness, AFTER), Ok(Value::Null));
        titled(core.as_mut(), "r62-st6b-dec-2");

        let resume = |h: &mut RealCoreHarness, session: &str, capture: CaptureId| {
            h.control(
                "a",
                "oracle_resume",
                &json!({"session": session, "capture": capture}),
            )
        };
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Ok(json!({"equal": true}))
        );
        assert_eq!(
            resume(&mut harness, "s1", capture),
            Ok(json!({"equal": true})),
            "again"
        );

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
        created_with(&harness, core.as_mut(), "s2", harness.probe_env());
        assert!(
            bad(resume(&mut harness, "s2", capture)),
            "another session's capture"
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
            Err(ControlError::Unsupported)
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
