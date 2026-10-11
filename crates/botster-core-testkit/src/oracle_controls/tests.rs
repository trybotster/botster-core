use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::hex_encode;
use std::time::Instant;

/// The most pumps that a test gives Core to reach an event: far above what these sessions need.
const PUMPS: usize = 50;

/// The size of the session `s1`: small, so that a short output wraps and scrolls.
const SIZE: Size = Size {
    rows: 3,
    cols: 10,
    cell_px: None,
};

fn spec() -> OpenSpec {
    OpenSpec {
        handle: "a".into(),
        data_dir: DataDirRef("d".into()),
        worker: Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: None,
        }),
        limits: json!({}),
    }
}

fn now(at: Instant) -> Now {
    Now {
        monotonic: at,
        unix: 1_000_000,
    }
}

/// Pumps until `op` completes, and returns its result.
fn complete(core: &mut dyn CoreApi, at: Instant, op: OpId) -> OpResult {
    for _ in 0..PUMPS {
        core.pump(now(at));
        for event in core.poll_events(64) {
            if let Event::Completed { op: o, result } = event {
                if o == op {
                    return result;
                }
            }
        }
    }
    panic!("the op {op:?} did not complete");
}

/// Pumps until Core has no runnable work (the fence of `await_quiet`).
fn settle(core: &mut dyn CoreApi, at: Instant) {
    for _ in 0..PUMPS {
        let more = core.pump(now(at)).more;
        if !more && core.poll_events(64).is_empty() {
            return;
        }
    }
    panic!("Core did not settle");
}

/// A Core on handle `a` with the session `s1` of [`SIZE`] running a payload that holds.
fn running(harness: &mut TestkitHarness) -> (Box<dyn CoreApi>, Instant) {
    let mut core = harness.open(&spec()).expect("open");
    let at = Instant::now();
    let mut create = json!({"Create": {"session": "s1", "request": {
        "program": [{"hold": {}}], "size": {"rows": SIZE.rows, "cols": SIZE.cols}}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    assert!(matches!(
        complete(core.as_mut(), at, create),
        OpResult::Ok(_)
    ));
    let start = core.begin(Op::Start { id: sid() }).unwrap();
    assert!(matches!(
        complete(core.as_mut(), at, start),
        OpResult::Ok(_)
    ));
    settle(core.as_mut(), at);
    (core, at)
}

fn sid() -> SessionId {
    SessionId("s1".into())
}

fn output(harness: &mut TestkitHarness, core: &mut dyn CoreApi, at: Instant, bytes: &[u8]) {
    harness
        .control(
            "a",
            "pty_output",
            &json!({"session": "s1", "bytes_hex": hex_encode(bytes)}),
        )
        .expect("pty_output");
    settle(core, at);
}

/// An independent oracle terminal of [`SIZE`] that applied `bytes`.
fn applied(bytes: &[u8]) -> Terminal {
    let mut terminal = Terminal::new(&SIZE, History::On).unwrap();
    terminal.vt_write(bytes);
    terminal
}

fn text(terminal: &Terminal, history: bool) -> String {
    terminal.screen_text(history).unwrap().text
}

fn bad(result: Result<Value, ControlError>) -> String {
    match result {
        Err(ControlError::Bad(message)) => message,
        other => panic!("Bad, got {other:?}"),
    }
}

/// The refusal of condition 3.
fn changed() -> Result<Value, ControlError> {
    Err(ControlError::UnsupportedFor(SIZE_OR_PROFILE_CHANGED.into()))
}

/// EV-8: the reply comes from a fresh terminal of the session's size after the prefix. Twelve bytes wrap at ten columns, so
/// the cursor position report (CPR, `CSI 6 n`) answers row 2, column 3, and only a terminal of the session's width gives it.
#[test]
fn the_query_reply_comes_from_a_fresh_terminal_of_the_sessions_size() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    // The session's own output never reaches the shadow: the fresh terminal sees only the prefix.
    output(&mut harness, core.as_mut(), at, b"session output");
    let query = |prefix: &[u8]| json!({"session": "s1", "request_hex": "1b5b366e", "prefix_hex": hex_encode(prefix)});
    assert_eq!(
        harness.control("a", "oracle_query_reply", &query(b"xxxxxxxxxxxx")),
        Ok(json!({"answerable": true, "reply_hex": hex_encode(b"\x1b[2;3R")}))
    );
    assert_eq!(
        harness.control(
            "a",
            "oracle_query_reply",
            &json!({"session": "s1", "request_hex": "1b5b366e"})
        ),
        Ok(json!({"answerable": true, "reply_hex": hex_encode(b"\x1b[1;1R")})),
        "no prefix: a fresh terminal's cursor is at the origin"
    );
    assert_eq!(
        harness.control(
            "a",
            "oracle_query_reply",
            &json!({"session": "s1", "request_hex": hex_encode(b"x")})
        ),
        Ok(json!({"answerable": false})),
        "a request that is not a query"
    );
}

/// EV-8 (d): the profile argument reaches the shadow, so a color query answers from it.
#[test]
fn the_query_reply_takes_the_profile_argument() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = running(&mut harness);
    let mut background = |profile: Option<Value>| {
        let mut args = json!({"session": "s1", "request_hex": hex_encode(b"\x1b]11;?\x1b\\")});
        if let Some(profile) = profile {
            args["profile"] = profile;
        }
        harness.control("a", "oracle_query_reply", &args).unwrap()
    };
    let profile = |b: u8| json!({"foreground": {"r": 1, "g": 2, "b": 3}, "background": {"r": 40, "g": 50, "b": b}});
    let first = background(Some(profile(60)));
    assert_eq!(first["answerable"], json!(true), "{first}");
    assert_ne!(first, background(Some(profile(61))));
    assert_eq!(first, background(Some(profile(60))));
}

/// A wrong argument is `Bad`, never a reply.
#[test]
fn a_query_with_a_wrong_argument_is_bad() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _at) = running(&mut harness);
    let mut control = |args: Value| harness.control("a", "oracle_query_reply", &args);
    assert!(bad(control(json!({"session": "s1", "request_hex": "1b5"}))).contains("request_hex"));
    assert!(bad(control(
        json!({"session": "s1", "request_hex": "1b", "prefix_hex": "zz"})
    ))
    .contains("prefix_hex"));
    assert!(bad(control(json!({"session": "s1", "request": "1b"}))).contains("arguments"));
    assert!(bad(control(json!({"session": "s2", "request_hex": "1b"}))).contains("s2"));
}

/// ST-2, A2-1: the screen is O_raw's, with the session's size; with history it holds the lines that scrolled off.
#[test]
fn the_screen_is_the_raw_oracles_with_and_without_history() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    let bytes = b"one\r\ntwo\r\nthree\r\nfour";
    output(&mut harness, core.as_mut(), at, bytes);
    let expected = applied(bytes);
    for history in [false, true] {
        assert_eq!(
            harness.control(
                "a",
                "oracle_screen",
                &json!({"session": "s1", "history": history})
            ),
            Ok(json!({"text": text(&expected, history), "rows": SIZE.rows, "cols": SIZE.cols})),
            "history {history}"
        );
    }
    assert!(text(&expected, true).contains("one"));
    assert!(!text(&expected, false).contains("one"));
}

/// Condition 3 (R-62), one test for every tier form: after a `Resize` or a `SetColorProfile` of the session, the controls
/// that replay its history refuse, and the controls that do not still answer. Before the shared check, the testkit's
/// `oracle_resume` gave a wrong `{"equal": false}` here.
#[test]
fn after_a_resize_or_a_profile_change_the_history_controls_refuse() {
    let profile = ColorProfile {
        foreground: Rgb { r: 1, g: 2, b: 3 },
        background: Rgb { r: 4, g: 5, b: 6 },
        cursor: None,
        palette: None,
    };
    let resize = Size {
        rows: 5,
        cols: 12,
        cell_px: None,
    };
    for change in [
        Op::Resize {
            session: sid(),
            size: resize,
        },
        Op::SetColorProfile {
            session: sid(),
            profile: profile.clone(),
        },
    ] {
        let mut harness = TestkitHarness::new(0);
        let (mut core, at) = running(&mut harness);
        output(&mut harness, core.as_mut(), at, b"abc");
        let capture = core
            .begin(Op::CaptureSnapshot {
                session: sid(),
                owner: ClientId("c1".into()),
            })
            .unwrap();
        let OpResult::Ok(OpOutput::Capture(capture)) = complete(core.as_mut(), at, capture) else {
            panic!("a capture");
        };
        let screen = json!({"session": "s1", "history": false});
        let resume = json!({"session": "s1", "capture": capture.capture});
        assert!(harness.control("a", "oracle_screen", &screen).is_ok());
        assert_eq!(
            harness.control("a", "oracle_resume", &resume),
            Ok(json!({"equal": true}))
        );

        // Core forwards the op to the started session's worker; its result does not matter here, only that it entered.
        let op = core.begin(change.clone()).unwrap();
        complete(core.as_mut(), at, op);
        assert_eq!(
            harness.control("a", "oracle_screen", &screen),
            changed(),
            "{change:?}"
        );
        assert_eq!(
            harness.control("a", "oracle_resume", &resume),
            changed(),
            "{change:?}"
        );
        assert!(
            harness
                .control(
                    "a",
                    "oracle_query_reply",
                    &json!({"session": "s1", "request_hex": "1b5b366e"})
                )
                .is_ok(),
            "{change:?}"
        );
    }
}

/// Only the session that the op names is changed, and an op that is not a `Resize` or a `SetColorProfile` changes nothing.
#[test]
fn the_record_names_only_the_resized_or_recolored_session() {
    let mut record = Changed::default();
    record.begun(&Op::Start { id: sid() });
    assert_eq!(record.check(&sid()), Ok(()));
    record.begun(&Op::Resize {
        session: SessionId("s2".into()),
        size: SIZE,
    });
    assert_eq!(record.check(&sid()), Ok(()));
    assert_eq!(record.check(&SessionId("s2".into())), changed().map(|_| ()));
}

/// OU-9, DP-3: a fresh oracle terminal loads the payload of a `screen` frame and reads its visible screen, with its size.
#[test]
fn the_screen_payload_loads_a_snapshot_in_a_fresh_terminal() {
    let source = applied(b"one\r\ntwo\r\nthree\r\nfour");
    let payload = json!({"payload": {"$bytes_hex": hex_encode(&source.snapshot().unwrap())}});
    assert_eq!(
        screen_payload(&payload),
        Ok(json!({"text": text(&source, false), "rows": SIZE.rows, "cols": SIZE.cols}))
    );
    let mut harness = TestkitHarness::new(0);
    assert_eq!(
        harness.control("h", "oracle_screen_payload", &payload),
        screen_payload(&payload),
        "the testkit serves the shared read with no open handle"
    );
    assert!(
        bad(screen_payload(&json!({"payload": {"$bytes_hex": "00"}}))).contains("does not load")
    );
    assert!(bad(screen_payload(&json!({"payload": "00"}))).contains("arguments"));
}
