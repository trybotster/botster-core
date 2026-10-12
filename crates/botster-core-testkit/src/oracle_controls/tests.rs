use super::*;
use botster_core_conformance::{
    normalize_op, CoreHarness, DataDirRef, OpenSpec, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::{hex_encode, MouseEncoding, MouseTracking};
use botster_terminal_ghostty::MAX_BUFFERED_EVENTS;
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

/// Creates and starts a second session `session` on the Core of [`running`], with the same request.
fn started(harness: &TestkitHarness, core: &mut dyn CoreApi, at: Instant, session: &str) {
    let mut create = json!({"Create": {"session": session, "request": {
        "program": [{"hold": {}}], "size": {"rows": SIZE.rows, "cols": SIZE.cols}}}});
    normalize_op(&mut create, &harness.probe_binary());
    let create = core.begin(serde_json::from_value(create).unwrap()).unwrap();
    assert!(matches!(complete(core, at, create), OpResult::Ok(_)));
    let start = core
        .begin(Op::Start {
            id: SessionId(session.into()),
        })
        .unwrap();
    assert!(matches!(complete(core, at, start), OpResult::Ok(_)));
    settle(core, at);
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

/// The cursor position report request (CPR, `CSI 6 n`): a stimulus whose reply depends on the terminal's width.
const CPR: &[u8] = b"\x1b[6n";

/// The expected reply, from an independent libghostty terminal of `size` that applied `prefix` and then `request` (R-7,
/// BUILD.md architecture rule 2: never a hand-written terminal byte).
pub(crate) fn shadow(size: &Size, prefix: &[u8], request: &[u8]) -> Value {
    let mut terminal = Terminal::new(size, History::Off).unwrap();
    terminal.vt_write(prefix);
    terminal.drain_events();
    let reply = terminal
        .vt_write_until_query(request)
        .unwrap()
        .query
        .map(|query| query.shadow_reply)
        .unwrap_or_default();
    if reply.is_empty() {
        json!({"answerable": false})
    } else {
        json!({"answerable": true, "reply_hex": hex_encode(&reply)})
    }
}

/// EV-8: the reply comes from a fresh terminal of the session's size after the prefix. Twelve bytes wrap at ten columns, so
/// the expected CPR differs from the one of a wider terminal: only a terminal of the session's width gives it.
#[test]
fn the_query_reply_comes_from_a_fresh_terminal_of_the_sessions_size() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    // The session's own output never reaches the shadow: the fresh terminal sees only the prefix.
    output(&mut harness, core.as_mut(), at, b"session output");
    let prefix = b"xxxxxxxxxxxx";
    let wide = Size {
        cols: SIZE.cols + 10,
        ..SIZE
    };
    let expected = shadow(&SIZE, prefix, CPR);
    assert_eq!(expected["answerable"], json!(true), "{expected}");
    assert_ne!(
        expected,
        shadow(&wide, prefix, CPR),
        "the width decides the reply"
    );
    assert_ne!(
        expected,
        shadow(&SIZE, b"", CPR),
        "the prefix decides the reply"
    );
    let query = |prefix: &[u8]| json!({"session": "s1", "request_hex": hex_encode(CPR), "prefix_hex": hex_encode(prefix)});
    assert_eq!(
        harness.control("a", "oracle_query_reply", &query(prefix)),
        Ok(expected)
    );
    assert_eq!(
        harness.control(
            "a",
            "oracle_query_reply",
            &json!({"session": "s1", "request_hex": hex_encode(CPR)})
        ),
        Ok(shadow(&SIZE, b"", CPR)),
        "no prefix"
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
        let session = json!({"session": "s1"});
        assert!(harness.control("a", "oracle_screen", &screen).is_ok());
        assert!(harness.control("a", "oracle_modes", &session).is_ok());
        assert!(harness.control("a", "oracle_state", &session).is_ok());
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
        for control in ["oracle_modes", "oracle_state"] {
            assert_eq!(
                harness.control("a", control, &session),
                changed(),
                "{control} {change:?}"
            );
        }
        assert!(
            harness
                .control(
                    "a",
                    "oracle_query_reply",
                    &json!({"session": "s1", "request_hex": hex_encode(CPR)})
                )
                .is_ok(),
            "{change:?}"
        );
    }
}

/// Condition 3 (R-62), Core DP-6: a route's `resize` changes the size with no host `Resize`. The session is recorded at its
/// `SizeChanged`, so the controls that replay its history refuse after it, as after a host `Resize`.
#[test]
fn after_a_route_resize_the_history_controls_refuse() {
    use botster_hub_conformance::route::RouteRead;
    use botster_route_codec::prelude::{stream_wrap, TYPE_INPUT, TYPE_LIVE};
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    output(&mut harness, core.as_mut(), at, b"abc");
    let options: AttachOptions = serde_json::from_value(
        json!({"file_directory": "/tmp", "answers_queries": false, "input": true}),
    )
    .unwrap();
    let (_, mut route) = harness
        .attach_stream("a", core.as_mut(), ClientId("c1".into()), &sid(), options)
        .expect("attached");
    settle(core.as_mut(), at);
    // DP-5: input before `live` is refused, so the client reads the baseline through `live` first.
    let deadline = botster_conformance::Deadline::after(None);
    let mut bytes = Vec::new();
    let mut frames = Vec::new();
    while !frames
        .iter()
        .any(|f: &Vec<u8>| f.first() == Some(&TYPE_LIVE))
    {
        match route.read(1 << 20, &deadline) {
            RouteRead::Bytes(more) => bytes.extend(more),
            RouteRead::Empty => settle(core.as_mut(), at),
            other => panic!("{other:?}"),
        }
        while bytes.len() >= 4 {
            let len = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
            if bytes.len() < 4 + len {
                break;
            }
            frames.push(bytes[4..4 + len].to_vec());
            bytes.drain(..4 + len);
        }
    }
    let screen = json!({"session": "s1", "history": false});
    let session = json!({"session": "s1"});
    assert!(harness.control("a", "oracle_screen", &screen).is_ok());
    route.write(&stream_wrap(
        &[
            &[TYPE_INPUT][..],
            br#"{"kind":"resize","rows":5,"cols":12}"#,
        ]
        .concat(),
    ));
    route.read(1 << 20, &deadline);
    settle(core.as_mut(), at);
    assert_eq!(
        core.get(&sid())
            .map(|record| (record.size.rows, record.size.cols)),
        Ok((5, 12)),
        "the route's size applied"
    );
    assert_eq!(harness.control("a", "oracle_screen", &screen), changed());
    for control in ["oracle_modes", "oracle_state"] {
        assert_eq!(
            harness.control("a", control, &session),
            changed(),
            "{control}"
        );
    }
}

/// Condition 3 names only the session that the op reached, through the controls: a `Resize` of `s2` refuses `s2`'s history
/// controls and leaves `s1`'s answering, and an op that is not a `Resize` or a `SetColorProfile` (a capture of each
/// session) refuses neither.
#[test]
fn only_the_resized_or_recolored_session_refuses() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    started(&harness, core.as_mut(), at, "s2");
    output(&mut harness, core.as_mut(), at, b"abc");
    let screen = |h: &mut TestkitHarness, session: &str| {
        h.control(
            "a",
            "oracle_screen",
            &json!({"session": session, "history": false}),
        )
    };
    for session in ["s1", "s2"] {
        let op = core
            .begin(Op::CaptureSnapshot {
                session: SessionId(session.into()),
                owner: ClientId("c1".into()),
            })
            .unwrap();
        complete(core.as_mut(), at, op);
    }
    assert!(screen(&mut harness, "s1").is_ok());
    assert!(
        screen(&mut harness, "s2").is_ok(),
        "a capture changes nothing"
    );

    let op = core
        .begin(Op::Resize {
            session: SessionId("s2".into()),
            size: SIZE,
        })
        .unwrap();
    complete(core.as_mut(), at, op);
    assert_eq!(screen(&mut harness, "s2"), changed());
    assert_eq!(
        screen(&mut harness, "s1"),
        Ok(json!({"text": text(&applied(b"abc"), false), "rows": SIZE.rows, "cols": SIZE.cols})),
        "another session's resize"
    );
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

/// `oracle_encode {session: s1, kind, input, modes}` on handle `a`.
fn encoded(
    harness: &mut TestkitHarness,
    kind: &str,
    input: Value,
    modes: Value,
) -> Result<Value, ControlError> {
    harness.control(
        "a",
        "oracle_encode",
        &json!({"session": "s1", "kind": kind, "input": input, "modes": modes}),
    )
}

/// The hex result of a reported encoding.
fn hex(bytes: &[u8]) -> Result<Value, ControlError> {
    Ok(Value::String(hex_encode(bytes)))
}

fn arrow_up() -> Value {
    json!({"key": {"named": "arrow_up"}, "mods": [], "event": "press"})
}

fn press(x_y: Option<(u32, u32)>) -> Value {
    let mut mouse = json!({"action": "press", "button": "left", "row": 1, "col": 2, "mods": []});
    if let Some((x, y)) = x_y {
        mouse["x"] = json!(x);
        mouse["y"] = json!(y);
    }
    json!({"mouse": mouse})
}

/// The modes of a fresh terminal of the pinned libghostty: the base of a partial `modes` (R-84 (1)).
fn fresh() -> ModeFlags {
    Terminal::new(&SIZE, History::Off).unwrap().modes()
}

/// The bytes of `input` with `modes`, from the pinned binding.
fn key_bytes(modes: &ModeFlags, input: Value) -> Result<Value, ControlError> {
    let key: KeyInput = serde_json::from_value(input).unwrap();
    hex(&encode_key_with_modes(modes, &key).unwrap())
}

/// R-84 (1), as corrected: a partial `modes` is laid over the modes of a fresh terminal. The expected bytes come from the
/// pinned binding with the modes built as a struct, not through the control's merge; the two results differ, so the merge
/// decides the bytes.
#[test]
fn oracle_encode_lays_a_partial_modes_over_a_fresh_terminal() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _) = running(&mut harness);
    let application = ModeFlags {
        application_cursor_keys: true,
        ..fresh()
    };
    assert_ne!(
        key_bytes(&fresh(), arrow_up()),
        key_bytes(&application, arrow_up())
    );
    assert_eq!(
        encoded(&mut harness, "key", arrow_up(), json!({})),
        key_bytes(&fresh(), arrow_up())
    );
    assert_eq!(
        encoded(
            &mut harness,
            "key",
            arrow_up(),
            json!({"application_cursor_keys": true})
        ),
        key_bytes(&application, arrow_up())
    );
}

/// The base is the fresh terminal's, not `ModeFlags::default()` (every mode off): a fresh terminal has `dec_1036` (Alt sends
/// an ESC prefix) and `dec_1035` on, so `{}` gives other bytes for Alt+q than the all-off modes. A partial `other_modes`
/// changes only its own keys.
#[test]
fn oracle_encode_keeps_the_fresh_other_modes_that_the_call_does_not_name() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _) = running(&mut harness);
    let alt = json!({"key": {"char": "q"}, "mods": ["alt"], "event": "press", "text": "\u{e4}"});
    let keypad = json!({"key": {"named": "kp_0"}, "mods": [], "event": "press"});
    let fresh = fresh();
    assert_ne!(
        key_bytes(&fresh, alt.clone()),
        key_bytes(&ModeFlags::default(), alt.clone())
    );
    assert_eq!(
        encoded(&mut harness, "key", alt.clone(), json!({})),
        key_bytes(&fresh, alt.clone())
    );

    let mut no_esc = fresh.clone();
    no_esc.other_modes.insert("dec_1036".into(), false);
    let mut application_keypad = fresh.clone();
    application_keypad.application_keypad = true;
    let no_esc_modes = json!({"other_modes": {"dec_1036": false}});
    assert_ne!(
        key_bytes(&no_esc, alt.clone()),
        key_bytes(&fresh, alt.clone())
    );
    assert_eq!(
        encoded(&mut harness, "key", alt.clone(), no_esc_modes),
        key_bytes(&no_esc, alt)
    );
    // `dec_1035` stays on when the call names only `dec_1036`.
    let mut keypad_off = application_keypad.clone();
    keypad_off.other_modes.insert("dec_1035".into(), false);
    assert_ne!(
        key_bytes(&application_keypad, keypad.clone()),
        key_bytes(&keypad_off, keypad.clone())
    );
    assert_eq!(
        encoded(
            &mut harness,
            "key",
            keypad.clone(),
            json!({"application_keypad": true, "other_modes": {"dec_1036": false}})
        ),
        key_bytes(&application_keypad, keypad)
    );
    assert!(bad(encoded(
        &mut harness,
        "key",
        arrow_up(),
        json!({"other_modes": {"dec_99999": true}})
    ))
    .contains("dec_99999"));
}

/// R-84 (2): `NotReported` (a mouse event with tracking off) gives `""`; `Unsupported{what}` (SGR pixels with no pixel
/// position) is refused with its `what`; a reported mouse event uses the session's stored size.
#[test]
fn oracle_encode_gives_empty_for_not_reported_and_refuses_unsupported() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _) = running(&mut harness);
    assert_eq!(
        encoded(&mut harness, "mouse", press(None), json!({})),
        hex(&[])
    );
    let pixels = json!({"mouse_tracking": "normal", "mouse_encoding": "sgr_pixels"});
    assert_eq!(
        encoded(&mut harness, "mouse", press(None), pixels.clone()),
        Err(ControlError::Refused(
            json!({"reason": "unsupported", "what": UnsupportedWhat::PixelPosition})
        ))
    );
    let modes = ModeFlags {
        mouse_tracking: MouseTracking::Normal,
        mouse_encoding: MouseEncoding::SgrPixels,
        ..fresh()
    };
    let MouseArg { mouse } = serde_json::from_value(press(Some((100, 200)))).unwrap();
    let expected = encode_mouse_with_modes(&modes, &SIZE, &mouse).unwrap();
    assert!(!expected.is_empty());
    assert_eq!(
        encoded(&mut harness, "mouse", press(Some((100, 200))), pixels),
        hex(&expected)
    );
}

/// The R-84 follow-up: an explicit `size` wins over the session's stored size for a mouse event. An SGR pixels press at
/// (500, 300): the stored size has no cell size, so the binding reports the position as given (R-84 (4)); with 8x16 cells the
/// 10x3 screen is 80x48 pixels, so the binding does not report it. Each expected value comes from the binding.
#[test]
fn oracle_encode_takes_an_explicit_size_over_the_stored_size() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _) = running(&mut harness);
    let sized = Size {
        cell_px: Some(CellPx {
            width: 8,
            height: 16,
        }),
        ..SIZE
    };
    let modes = ModeFlags {
        mouse_tracking: MouseTracking::Normal,
        mouse_encoding: MouseEncoding::SgrPixels,
        ..fresh()
    };
    let MouseArg { mouse } = serde_json::from_value(press(Some((500, 300)))).unwrap();
    let stored = encode_mouse_with_modes(&modes, &SIZE, &mouse).unwrap();
    assert!(!stored.is_empty());
    assert_eq!(
        encode_mouse_with_modes(&modes, &sized, &mouse),
        Err(EncodeError::NotReported)
    );
    let mut args = json!({"session": "s1", "kind": "mouse", "input": press(Some((500, 300))),
        "modes": {"mouse_tracking": "normal", "mouse_encoding": "sgr_pixels"}});
    assert_eq!(harness.control("a", "oracle_encode", &args), hex(&stored));
    args["size"] = serde_json::to_value(sized).unwrap();
    assert_eq!(harness.control("a", "oracle_encode", &args), hex(&[]));
}

/// R-84 (2), (3): focus with reporting off gives `""`, on gives the binding's report; a paste with bracketed paste off is
/// the text alone (Core IN-8), on is the binding's markers around the text.
#[test]
fn oracle_encode_focus_and_paste_follow_their_modes() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _) = running(&mut harness);
    let on = ModeFlags {
        focus_reporting: true,
        bracketed_paste: true,
        ..fresh()
    };
    let focus = json!({"focused": false});
    assert_eq!(
        encoded(&mut harness, "focus", focus.clone(), json!({})),
        hex(&[])
    );
    let report = encode_focus_with_modes(&on, false).unwrap();
    assert_eq!(
        encoded(
            &mut harness,
            "focus",
            focus,
            json!({"focus_reporting": true})
        ),
        hex(&report)
    );
    let paste = json!({"text": "a b"});
    assert_eq!(
        encoded(&mut harness, "paste", paste.clone(), json!({})),
        hex(b"a b")
    );
    let (start, end) = paste_frame_with_modes(&on).unwrap();
    assert_eq!(
        encoded(
            &mut harness,
            "paste",
            paste,
            json!({"bracketed_paste": true})
        ),
        hex(&[start, b"a b".to_vec(), end].concat())
    );
}

/// The wiring cross-check: Core's worker writes, for a host key with the model's application cursor mode on, exactly the
/// bytes of `oracle_encode` with that mode. Both sides call the same pinned encoder (R-7), so this proves the control's
/// parse, merge and dispatch, not the encoder.
#[test]
fn oracle_encode_equals_the_bytes_that_the_worker_writes() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    output(&mut harness, core.as_mut(), at, b"\x1b[?1h");
    let key: KeyInput = serde_json::from_value(arrow_up()).unwrap();
    let op = core
        .begin(Op::WriteInput {
            session: sid(),
            payload: InputPayload::Key(key),
            guard: None,
        })
        .unwrap();
    assert!(matches!(complete(core.as_mut(), at, op), OpResult::Ok(_)));
    let written = harness
        .control("a", "pty_input", &json!({"session": "s1"}))
        .unwrap()["bytes"]["$bytes_hex"]
        .clone();
    assert_eq!(
        Ok(written),
        encoded(
            &mut harness,
            "key",
            arrow_up(),
            json!({"application_cursor_keys": true})
        )
    );
}

/// A missing `modes` is `unsupported_control` (no session model here); a key that is not a mode, a key input with `repeat`,
/// an input in another kind's form and an unknown kind are `Bad`.
#[test]
fn oracle_encode_refuses_a_missing_modes_and_bad_arguments() {
    let mut harness = TestkitHarness::new(0);
    let (_core, _) = running(&mut harness);
    assert_eq!(
        harness.control(
            "a",
            "oracle_encode",
            &json!({"session": "s1", "kind": "key", "input": arrow_up()})
        ),
        Err(ControlError::UnsupportedFor(
            "oracle_encode_without_modes".into()
        ))
    );
    assert!(bad(encoded(
        &mut harness,
        "key",
        arrow_up(),
        json!({"cursor_keys": true})
    ))
    .contains("cursor_keys"));
    let mut repeated = arrow_up();
    repeated["repeat"] = json!(2);
    assert!(bad(encoded(&mut harness, "key", repeated, json!({}))).contains("repeat"));
    assert!(bad(encoded(&mut harness, "mouse", arrow_up(), json!({}))).contains("mouse input"));
    assert!(bad(encoded(&mut harness, "scroll", arrow_up(), json!({}))).contains("scroll"));
}

/// The binding fact that R-84 (5) needs: libghostty reports an explicitly empty title or cwd as an event, with the BEL and
/// the ST terminator. Its getters give `""` both before and after, so only the event tells the two apart.
#[test]
fn libghostty_reports_an_empty_title_or_cwd_as_an_event() {
    let mut terminal = Terminal::new(&SIZE, History::Off).unwrap();
    let title = |text: &str| TerminalEvent::Title(text.into());
    let cwd = |text: &str| TerminalEvent::Cwd(text.into());
    assert_eq!(terminal.drain_events().events, vec![]);
    assert_eq!(
        (terminal.title(), terminal.cwd()),
        (String::new(), String::new()),
        "unset"
    );
    for (bytes, event) in [
        (&b"\x1b]2;abc\x07"[..], title("abc")),
        (b"\x1b]2;\x07", title("")),
        (b"\x1b]0;abc\x1b\\", title("abc")),
        (b"\x1b]0;\x1b\\", title("")),
        (b"\x1b]2;\x1b\\", title("")),
        (b"\x1b]7;file://h/x\x07", cwd("file://h/x")),
        (b"\x1b]7;\x07", cwd("")),
        (b"\x1b]7;\x1b\\", cwd("")),
    ] {
        terminal.vt_write(bytes);
        assert_eq!(
            terminal.drain_events().events,
            vec![event],
            "{}",
            String::from_utf8_lossy(bytes)
        );
    }
    assert_eq!(
        (terminal.title(), terminal.cwd()),
        (String::new(), String::new()),
        "empty"
    );
}

/// The modes of O_raw (note (1)), with each mode that the output set, and the `{flags}` form.
#[test]
fn oracle_modes_are_the_raw_oracles() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    let modes = |h: &mut TestkitHarness| h.control("a", "oracle_modes", &json!({"session": "s1"}));
    assert_eq!(
        modes(&mut harness),
        Ok(json!({"flags": applied(b"").modes()}))
    );
    let bytes = b"\x1b[?2004h\x1b[?1h\x1b[?25l\x1b[?1004h";
    output(&mut harness, core.as_mut(), at, bytes);
    let expected = applied(bytes).modes();
    assert_ne!(expected, applied(b"").modes(), "the output sets modes");
    assert_eq!(modes(&mut harness), Ok(json!({"flags": expected})));
    assert!(bad(harness.control("a", "oracle_modes", &json!({"session": "s2"}))).contains("s2"));
}

/// R-84 (5): `oracle_state` keeps the last `Title` and `Cwd` events of the O_raw replay. A field is absent until an event
/// comes; an empty value that the program set is `""` (the ST-4 reading), equal to Core's `terminal_state`.
#[test]
fn oracle_state_keeps_the_last_title_and_cwd_events_of_the_raw_replay() {
    let mut harness = TestkitHarness::new(0);
    let (mut core, at) = running(&mut harness);
    let state = |h: &mut TestkitHarness| {
        h.control("a", "oracle_state", &json!({"session": "s1"}))
            .unwrap()
    };
    let core_state = |core: &dyn CoreApi| {
        let TerminalState {
            title, cwd, modes, ..
        } = core.terminal_state(&sid()).unwrap();
        (title, cwd, modes)
    };
    assert_eq!(
        state(&mut harness),
        json!({"modes": applied(b"").modes()}),
        "no event: absent"
    );
    assert_eq!(core_state(core.as_ref()).0, None);

    let first = b"\x1b]2;one\x07\x1b]2;two\x07\x1b]7;file://h/x\x07\x1b[?2004h";
    output(&mut harness, core.as_mut(), at, first);
    let expected = json!({"modes": applied(first).modes(), "title": "two", "cwd": "file://h/x"});
    assert_eq!(state(&mut harness), expected, "the last title wins");

    let all = [&first[..], b"\x1b]2;\x07\x1b]7;\x1b\\"].concat();
    output(&mut harness, core.as_mut(), at, b"\x1b]2;\x07\x1b]7;\x1b\\");
    let state = state(&mut harness);
    assert_eq!(
        state,
        json!({"modes": applied(&all).modes(), "title": "", "cwd": ""}),
        "set empty: \"\""
    );
    // The oracle and Core keep the last event independently (R-84 (5)); they agree here.
    let (title, cwd, modes) = core_state(core.as_ref());
    assert_eq!(
        json!({"modes": modes, "title": title, "cwd": cwd}),
        state,
        "Core's terminal_state"
    );
}

/// A replay that dropped events can have dropped the last title, so `oracle_state` is `Bad`, never a wrong value.
#[test]
fn oracle_state_refuses_a_replay_that_dropped_events() {
    let mut terminal = Terminal::new(&SIZE, History::Off).unwrap();
    terminal.vt_write(&vec![0x07; MAX_BUFFERED_EVENTS]);
    terminal.vt_write(b"\x1b]2;lost\x07");
    assert!(bad(state(&mut terminal)).contains("dropped 1 events"));

    let mut terminal = Terminal::new(&SIZE, History::Off).unwrap();
    terminal.vt_write(&vec![0x07; MAX_BUFFERED_EVENTS - 1]);
    terminal.vt_write(b"\x1b]2;kept\x07");
    assert_eq!(
        state(&mut terminal).unwrap()["title"],
        json!("kept"),
        "a full buffer, nothing dropped"
    );
}
