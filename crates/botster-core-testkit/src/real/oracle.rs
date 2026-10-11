//! The real tier's session lookup for the oracle controls ([`crate::oracle_controls`]). The oracle reads are shared with the
//! testkit; this tier supplies the size of the stored row and the session's known output history.
//!
//! `oracle_screen` reads O_raw of `oracle_resume` (R-62): a fresh oracle terminal that applies every `pty_output` that the
//! session's probe acknowledged, with the size and the profile of the stored request. Conditions 1 and 3 of `oracle_resume`
//! hold for it: a session whose program writes output that the transcript does not state gives `unsupported_control`
//! `unknown_output_history`, and a session that a `Resize` or a `SetColorProfile` reached gives `unsupported_control`
//! `size_or_profile_changed`. The transcript fences the read (condition 2): a read before the model applied the output gives
//! only a false failure.

use super::resume::known_output_history;
use super::RealCoreHarness;
use crate::capture_layer::lock;
use crate::controls::parse;
use crate::oracle_compare::raw_oracle;
use crate::oracle_controls::{screen, QueryReply, Screen};
use botster_core_conformance::ControlError;
use serde_json::Value;

impl RealCoreHarness {
    /// `oracle_query_reply` (Core EV-8): the size of the session's stored row.
    pub(super) fn oracle_query_reply(
        &self,
        handle: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        let args: QueryReply = parse(args)?;
        let (_, row) = self.stored(handle, &args.session)?;
        args.reply(&row.request.size)
    }

    /// `oracle_screen` (Core ST-2, A2-1): see the module documentation.
    pub(super) fn oracle_screen(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let Screen { session, history } = parse(args)?;
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
        lock(&shared).check(&session)?;
        let worker = row.worker.ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no worker process", session.0))
        })?;
        let all: Vec<u8> = self
            .programs
            .output(worker.identity())
            .into_iter()
            .flat_map(|(_, bytes)| bytes)
            .collect();
        let raw = raw_oracle(&row.request.size, row.request.color_profile.as_ref(), &all)
            .map_err(|e| ControlError::Bad(format!("oracle_screen: no oracle terminal: {e:?}")))?;
        screen(&raw, history)
    }
}

#[cfg(test)]
mod slow_tests {
    use super::super::slow_controls::{created_with, pump_until, sid, state};
    use super::*;
    use crate::candidate::Candidate;
    use botster_core_conformance::{CoreHarness, OpenSpec, WorkerBuild};
    use botster_core_contract::prelude::*;
    use botster_route_codec::prelude::hex_encode;
    use botster_terminal_ghostty::{History, Terminal};
    use serde_json::json;

    /// The size of `created_with`'s sessions.
    const SIZE: Size = Size {
        rows: 24,
        cols: 80,
        cell_px: None,
    };

    /// The bytes of `s1`: no `0a` (condition 4), and the sentinel title last (condition 2).
    const OUTPUT: &[u8] = b"one\x1b[2;1Htwo\x1b]2;oracle-1\x07";

    fn titled(core: &mut dyn CoreApi, session: &str, title: &str) {
        pump_until(core, |c| {
            c.terminal_state(&sid(session)).unwrap().title.as_deref() == Some(title)
        });
    }

    fn applied(bytes: &[u8]) -> Terminal {
        let mut terminal = Terminal::new(&SIZE, History::On).unwrap();
        terminal.vt_write(bytes);
        terminal
    }

    /// The oracle controls on a real session of the prebuilt candidate:
    /// - `oracle_screen` is O_raw of the session's own output history, not another session's output;
    /// - `oracle_query_reply` answers from a fresh terminal of the session's size;
    /// - `oracle_screen_payload` is the shared read;
    /// - after a `Resize`, `oracle_screen` refuses (condition 3) and `oracle_query_reply` still answers;
    /// - a session whose program prints is `unknown_output_history` (condition 1).
    #[test]
    fn the_real_tier_serves_the_oracle_controls() {
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
                &json!({"session": session, "bytes_hex": hex_encode(bytes)}),
            )
        };
        assert_eq!(
            output(&mut harness, "s2", b"other\x1b]2;noise\x07"),
            Ok(Value::Null)
        );
        titled(core.as_mut(), "s2", "noise");
        assert_eq!(output(&mut harness, "s1", OUTPUT), Ok(Value::Null));
        titled(core.as_mut(), "s1", "oracle-1");

        let screen = |h: &mut RealCoreHarness, history: bool| {
            h.control(
                "a",
                "oracle_screen",
                &json!({"session": "s1", "history": history}),
            )
        };
        let expected = applied(OUTPUT);
        for history in [false, true] {
            assert_eq!(
                screen(&mut harness, history),
                Ok(json!({
                    "text": expected.screen_text(history).unwrap().text,
                    "rows": SIZE.rows,
                    "cols": SIZE.cols,
                })),
                "history {history}"
            );
        }

        // The expected reply comes from an independent libghostty terminal (BUILD.md architecture rule 2); 82 prefix bytes
        // wrap at 80 columns, so a terminal of another width gives another reply.
        let request = b"\x1b[6n";
        let prefix = [b'x'; 82];
        let reply = crate::oracle_controls::tests::shadow(&SIZE, &prefix, request);
        assert_eq!(reply["answerable"], json!(true), "{reply}");
        assert_ne!(
            reply,
            crate::oracle_controls::tests::shadow(&Size { cols: 90, ..SIZE }, &prefix, request),
            "the width decides the reply"
        );
        let cpr = |h: &mut RealCoreHarness| {
            h.control(
                "a",
                "oracle_query_reply",
                &json!({"session": "s1", "request_hex": hex_encode(request), "prefix_hex": hex_encode(&prefix)}),
            )
        };
        assert_eq!(cpr(&mut harness), Ok(reply));

        let payload = json!({"payload": {"$bytes_hex": hex_encode(&expected.snapshot().unwrap())}});
        // The payload's screen is the visible screen of the terminal that made it.
        assert_eq!(
            harness.control("a", "oracle_screen_payload", &payload),
            Ok(json!({
                "text": expected.screen_text(false).unwrap().text,
                "rows": SIZE.rows,
                "cols": SIZE.cols,
            }))
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
            screen(&mut harness, false),
            Err(ControlError::UnsupportedFor(
                "size_or_profile_changed".into()
            ))
        );
        assert!(cpr(&mut harness).is_ok(), "no history, no refusal");

        let print = json!({ "program": [{ "print": "x" }, { "hold": {} }] }).to_string();
        let mut request = harness.stored("a", &sid("s2")).unwrap().1.request;
        request.argv = vec![harness.probe_binary(), print];
        core.begin(Op::Create {
            session: sid("s3"),
            request,
        })
        .unwrap();
        pump_until(core.as_mut(), |c| c.get(&sid("s3")).is_ok());
        assert_eq!(
            harness.control(
                "a",
                "oracle_screen",
                &json!({"session": "s3", "history": false})
            ),
            Err(ControlError::UnsupportedFor(
                "unknown_output_history".into()
            ))
        );
        drop(core);
    }
}
