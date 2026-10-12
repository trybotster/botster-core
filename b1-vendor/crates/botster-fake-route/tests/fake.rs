//! Tests of the fake itself: the behaviors that no codec transcript names (terminal queries, P9) and the reachability of every choice point (design 7.2, rule 3).

use botster_fake_route::*;
use botster_route_codec::prelude::*;
use serde_json::{json, Value};

fn frames(w: &mut FakeWorker, id: RouteId) -> Vec<ToClient> {
    let mut reader = StreamReader::new();
    let mut out = vec![];
    while let Read::Bytes(b) = w.read(id, usize::MAX) {
        reader.push(&b);
        while let Some(f) = reader.next_frame(&w.bounds()).unwrap() {
            if let Decoded::Frame(f) = ToClient::decode(&f, &w.bounds()).unwrap() {
                out.push(f);
            }
        }
    }
    out
}

fn input(w: &mut FakeWorker, id: RouteId, i: Input) {
    let frame = ToWorker::Input(i).encode();
    w.write(id, &botster_route_codec::prelude::stream_wrap(&frame));
}

fn hexs(b: &[u8]) -> String {
    botster_route_codec::prelude::hex_encode(b)
}

fn pty(w: &mut FakeWorker) -> Vec<u8> {
    let v = w.control("pty_input", &json!(null), None).unwrap();
    botster_route_codec::prelude::hex_decode(v["bytes"]["$bytes_hex"].as_str().unwrap()).unwrap()
}

fn reply(query_id: &str, reply: QueryReply) -> Input {
    Input::ReplyTerminalQuery {
        op: None,
        query_id: query_id.into(),
        reply,
    }
}

fn refusals(fs: &[ToClient]) -> Vec<RefusalReason> {
    fs.iter()
        .filter_map(|f| match f {
            ToClient::InputRefused(r) => Some(r.reason.clone()),
            _ => None,
        })
        .collect()
}

fn live_worker(seed: u64, cfg: WorkerConfig) -> (FakeWorker, RouteId) {
    let mut w = FakeWorker::new(seed, cfg);
    let id = w.attach(&AttachSpec::default());
    frames(&mut w, id);
    (w, id)
}

/// Codec TS-7 (P9 steps 2 to 5): a decline triggers the fallback, which writes the shadow answer once. A later client answer is already_replied.
#[test]
fn decline_runs_the_fallback_exactly_once() {
    let (mut w, r1) = live_worker(0, WorkerConfig::default());
    let q = w
        .control(
            "terminal_query",
            &json!({"request_hex": "5251", "shadow_hex": "5348"}),
            None,
        )
        .unwrap();
    let qid = q["query_id"].as_str().unwrap().to_string();
    let seen = frames(&mut w, r1);
    assert!(
        matches!(&seen[0], ToClient::TerminalQuery(t) if t.query_id == qid && t.shadow_answerable)
    );
    input(&mut w, r1, reply(&qid, QueryReply::Decline));
    assert_eq!(pty(&mut w), b"SH");
    input(
        &mut w,
        r1,
        reply(
            &qid,
            QueryReply::Encoded {
                bytes_base64: "eA==".into(),
            },
        ),
    );
    assert_eq!(
        refusals(&frames(&mut w, r1)),
        vec![RefusalReason::AlreadyReplied]
    );
    assert_eq!(pty(&mut w), b"SH");
}

/// Codec TS-7 (P9 step 2): the host deadline passes with no answer, so the fallback runs. A reply from another route is wrong_route.
#[test]
fn deadline_runs_the_fallback_and_binds_the_route() {
    let (mut w, r1) = live_worker(0, WorkerConfig::default());
    let r2 = w.attach(&AttachSpec::default());
    frames(&mut w, r2);
    input(
        &mut w,
        r1,
        Input::Focus {
            op: None,
            focused: false,
        },
    );
    let qid = w
        .control(
            "terminal_query",
            &json!({"request_hex": "00", "shadow_hex": "41"}),
            None,
        )
        .unwrap()["query_id"]
        .as_str()
        .unwrap()
        .to_string();
    input(&mut w, r2, reply(&qid, QueryReply::Decline));
    assert_eq!(
        refusals(&frames(&mut w, r2)),
        vec![RefusalReason::WrongRoute]
    );
    w.advance_clock(100);
    assert_eq!(pty(&mut w), b"A");
    input(&mut w, r1, reply(&qid, QueryReply::Decline));
    assert_eq!(
        refusals(&frames(&mut w, r1)),
        vec![RefusalReason::AlreadyReplied]
    );
    input(&mut w, r1, reply("nope", QueryReply::Decline));
    assert_eq!(
        refusals(&frames(&mut w, r1)),
        vec![RefusalReason::UnknownQuery]
    );
}

/// Codec TS-7: a resync retires an unstarted terminal_query frame and admits the fallback. The client answer path never reopens.
#[test]
fn resync_retires_an_unstarted_query() {
    let (mut w, r1) = live_worker(0, WorkerConfig::default());
    let qid = w
        .control(
            "terminal_query",
            &json!({"request_hex": "00", "shadow_hex": "42"}),
            None,
        )
        .unwrap()["query_id"]
        .as_str()
        .unwrap()
        .to_string();
    w.control("queue_overflow", &json!(null), Some(r1)).unwrap();
    assert_eq!(pty(&mut w), b"B");
    let seen = frames(&mut w, r1);
    assert!(!seen.iter().any(|f| matches!(f, ToClient::TerminalQuery(_))));
    input(&mut w, r1, reply(&qid, QueryReply::Decline));
    assert_eq!(
        refusals(&frames(&mut w, r1)),
        vec![RefusalReason::AlreadyReplied]
    );
}

/// Codec TS-7 (P9 step 5): a fallback refused at admission leaves the query pending, and a client answer is query_expired.
#[test]
fn refused_fallback_stays_pending() {
    let cfg = WorkerConfig {
        input_queue_bytes: 1,
        ..WorkerConfig::default()
    };
    let (mut w, r1) = live_worker(0, cfg);
    w.control("pty_blocked", &json!({"on": true}), None)
        .unwrap();
    let qid = w
        .control(
            "terminal_query",
            &json!({"request_hex": "00", "shadow_hex": "4344"}),
            None,
        )
        .unwrap()["query_id"]
        .as_str()
        .unwrap()
        .to_string();
    frames(&mut w, r1);
    input(&mut w, r1, reply(&qid, QueryReply::Decline));
    assert!(pty(&mut w).is_empty());
    input(&mut w, r1, reply(&qid, QueryReply::Decline));
    assert_eq!(
        refusals(&frames(&mut w, r1)),
        vec![RefusalReason::QueryExpired]
    );
    w.control("pty_blocked", &json!({"on": false}), None)
        .unwrap();
    assert_eq!(pty(&mut w), b"CD");
}

fn drain(w: &mut FakeWorker, id: RouteId) -> Vec<u8> {
    let mut all = vec![];
    while let Read::Bytes(b) = w.read(id, usize::MAX) {
        all.extend(b);
    }
    all
}

/// Codec TS-4c: output_dropped is offered only on a route whose frame bound can carry the worst-case frame (revisions 35 and 36).
#[test]
fn output_dropped_needs_a_frame_bound_that_fits() {
    let spec = AttachSpec {
        route_features: vec!["output_dropped".into()],
        ..AttachSpec::default()
    };
    let offered = |max: u64| {
        let cfg = WorkerConfig {
            max_frame_bytes: max,
            route_queue_bytes: 99_999,
            ..WorkerConfig::default()
        };
        let mut w = FakeWorker::new(0, cfg);
        let id = w.attach(&spec);
        let bytes = drain(&mut w, id);
        let length = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert_eq!(bytes[4], 0x01);
        let body: serde_json::Value = serde_json::from_slice(&bytes[5..4 + length]).unwrap();
        body["features"]
            .as_array()
            .unwrap()
            .iter()
            .any(|f| f == "output_dropped")
    };
    // The frame is the type byte and the body: 5 digits for route_queue_bytes 99999.
    let min =
        1 + r#"{"notifications":99999,"last":{"source":"osc777","truncated":true}}"#.len() as u64;
    assert!(offered(min));
    assert!(!offered(min - 1));
}

/// Codec TS-4c, revision 34: a long notification is shortened to fit the frame bound, body first, and marked truncated.
#[test]
fn output_dropped_is_shortened_to_fit() {
    let cfg = WorkerConfig {
        max_frame_bytes: 512,
        ..WorkerConfig::default()
    };
    let mut w = FakeWorker::new(0, cfg);
    let id = w.attach(&AttachSpec {
        route_features: vec!["output_dropped".into()],
        ..AttachSpec::default()
    });
    frames(&mut w, id);
    let long = "b".repeat(2000);
    w.control(
        "pty_output",
        &json!({"bytes_hex": "41", "scripted": {"notifications": [{"source": "osc777", "title": "T", "body": long}]}}),
        None,
    )
    .unwrap();
    w.control("queue_overflow", &json!(null), Some(id)).unwrap();
    let dropped = frames(&mut w, id).into_iter().find_map(|f| match f {
        ToClient::OutputDropped(d) => Some(d),
        _ => None,
    });
    let d = dropped.expect("output_dropped");
    assert_eq!(d.last.truncated, Some(true));
    assert!(
        serde_json::to_vec(&d).unwrap().len() < 512,
        "the frame is the type byte and the body"
    );
    assert_eq!(d.last.title.as_deref(), Some("T"));
}

/// Design 7.2, rule 3: every choice point is reachable. Across seeds, each one takes at least two outcomes.
#[test]
fn every_choice_point_is_reachable() {
    let mut seen: std::collections::BTreeMap<Choice, std::collections::BTreeSet<usize>> =
        Default::default();
    for seed in 0..32 {
        for framing in [Framing::Stream, Framing::Chunked] {
            let cfg = WorkerConfig {
                framing,
                max_chunk_bytes: 9,
                ..WorkerConfig::default()
            };
            let mut w = FakeWorker::new(seed, cfg);
            let id = w.attach(&AttachSpec {
                route_features: vec!["output_deflate".into()],
                ..AttachSpec::default()
            });
            drain(&mut w, id);
            w.control(
                "pty_output",
                &json!({"bytes_hex": hexs(b"hello world more output"), "scripted": {"modes": {"bracketed_paste": true}}}),
                None,
            )
            .unwrap();
            w.control("stall_reader", &json!({}), Some(id)).unwrap();
            w.advance_clock(1001);
            drain(&mut w, id);
            for (c, v) in w.choices_taken() {
                seen.entry(*c).or_default().extend(v.iter().copied());
            }
        }
    }
    for c in [
        Choice::OutputSplit,
        Choice::ReadSplit,
        Choice::ChunkSize,
        Choice::DeflatePerFrame,
        Choice::ModesOrder,
        Choice::StallDiscard,
    ] {
        assert!(
            seen.get(&c).is_some_and(|v| v.len() >= 2),
            "choice {c:?} took {:?}",
            seen.get(&c)
        );
    }
}
/// Codec TS-4: a screen over its frame bound cannot be produced, so the attach fails with snapshot_too_large.
#[test]
fn screen_bound_is_enforced() {
    let cfg = WorkerConfig {
        max_screen_frame_bytes: 100,
        script: script(json!({"screen": "00".repeat(100)})),
        ..WorkerConfig::default()
    };
    let mut w = FakeWorker::new(0, cfg);
    let id = w.attach(&AttachSpec::default());
    let seen = frames(&mut w, id);
    assert!(
        matches!(&seen[0], ToClient::RouteClosed(c) if c.reason == CloseReason::AttachFailed { reason: AttachFailedReason::SnapshotTooLarge })
    );
}

/// Codec TS-7: the outgoing queue never grows past route_queue_bytes because of exception frames, and none is lost.
#[test]
fn exception_frames_stay_within_the_queue_bound() {
    let cfg = WorkerConfig {
        route_queue_bytes: 700,
        file_name_limit: 16,
        file_directory: "/f".into(),
        ..WorkerConfig::default()
    };
    let mut w = FakeWorker::new(0, cfg);
    let id = w.attach(&AttachSpec {
        writable: false,
        ..AttachSpec::default()
    });
    frames(&mut w, id);
    for op in 1..=60u64 {
        let f = ToWorker::Bytes {
            op: Op(op),
            bytes: Payload::from(vec![b'x']),
        }
        .encode();
        w.write(id, &botster_route_codec::prelude::stream_wrap(&f));
        assert!(
            w.queued_bytes(id) <= 700,
            "queue {} after op {op}",
            w.queued_bytes(id)
        );
    }
    let mut got = vec![];
    for _ in 0..200 {
        got.extend(
            frames(&mut w, id)
                .into_iter()
                .filter(|f| matches!(f, ToClient::InputRefused(_))),
        );
        if got.len() == 60 {
            break;
        }
    }
    assert_eq!(got.len(), 60);
}
/// Codec TS-4: a scripted history page that does not fit a history page bound leaves the history unavailable (oversize), and the route stays live.
#[test]
fn history_page_bound_is_enforced() {
    let cfg = WorkerConfig {
        max_history_page_bytes: 64,
        script: script(json!({"history_pages": ["00".repeat(64)]})),
        ..WorkerConfig::default()
    };
    let mut w = FakeWorker::new(0, cfg);
    let id = w.attach(&AttachSpec {
        history: HistoryRequest::Full,
        ..AttachSpec::default()
    });
    let seen = frames(&mut w, id);
    let end = seen.iter().find_map(|f| match f {
        ToClient::BaselineEnd(e) => Some(e.history),
        _ => None,
    });
    assert_eq!(
        end,
        Some(HistoryState::Unavailable(HistoryUnavailable::Oversize))
    );
    assert!(seen.iter().any(|f| matches!(f, ToClient::Live)));
}

fn script(v: Value) -> botster_fake_route::script::SessionScript {
    serde_json::from_value(v).unwrap()
}

fn live_script(s: Value) -> (FakeWorker, RouteId) {
    live_worker(
        0,
        WorkerConfig {
            script: script(s),
            ..WorkerConfig::default()
        },
    )
}

/// Core IN-9, Codec TS-5b: the admission bound is the scripted worst case against max_paste_bytes. A bound equal to the cap passes, and one byte over is refused with zero effect.
#[test]
fn ts_5b_in9_scripted_admission_equal_passes_over_refused() {
    let key = |op| Input::Key {
        op: Some(Op(op)),
        key: Key::Char("a".into()),
        mods: vec![],
        event: KeyEvent::Press,
        text: None,
        shifted_key: None,
        base_layout_key: None,
    };
    for (bound, refused) in [(1024u64, false), (1025, true)] {
        let (mut w, id) =
            live_script(json!({"writes": {"key": "61"}, "admission": {"key": bound}}));
        input(&mut w, id, key(1));
        assert_eq!(refusals(&frames(&mut w, id)).len(), usize::from(refused));
        assert_eq!(
            pty(&mut w),
            if refused { b"".to_vec() } else { b"a".to_vec() }
        );
    }
}

/// Codec TS-5b: the script supplies the whole outcome of each semantic input. The fake writes the scripted bytes, writes nothing and
/// sends no frame for a not-reported input, and refuses an unsupported one. It never reads the modes to decide.
#[test]
fn ts_5b_not_written_outcomes_bytes_not_reported_unsupported() {
    let (mut w, id) = live_script(json!({"writes": {
        "mouse": "4d", "focus_in": "49", "key": {"not_written": {"unsupported": "legacy"}}}}));
    let mouse = Input::Mouse {
        op: None,
        action: MouseAction::Press,
        button: MouseButton::Left,
        row: 1,
        col: 1,
        x: None,
        y: None,
        mods: vec![],
        notches: None,
    };
    let focus_in = Input::Focus {
        op: None,
        focused: true,
    };
    let focus_out = Input::Focus {
        op: None,
        focused: false,
    };
    let key = Input::Key {
        op: Some(Op(7)),
        key: Key::Char("a".into()),
        mods: vec![],
        event: KeyEvent::Press,
        text: None,
        shifted_key: None,
        base_layout_key: None,
    };
    // Modes are all off, and the bytes are still written: the script decides.
    input(&mut w, id, mouse);
    input(&mut w, id, focus_in);
    input(&mut w, id, focus_out);
    input(&mut w, id, key);
    assert_eq!(pty(&mut w), b"MI");
    let seen = frames(&mut w, id);
    assert_eq!(
        seen.len(),
        1,
        "a not-reported outcome sends no frame: {seen:?}"
    );
    assert!(
        matches!(&seen[0], ToClient::InputRefused(r) if r.op == Some(Op(7)) && matches!(&r.reason, RefusalReason::Unsupported { what } if what == "legacy"))
    );
}

fn paste(w: &mut FakeWorker, id: RouteId, op: u64, require: bool) {
    input(
        w,
        id,
        Input::PasteBegin {
            op: Op(op),
            total_bytes: None,
            require_bracketed: Some(require),
        },
    );
    w.write(
        id,
        &stream_wrap(
            &ToWorker::PasteChunk {
                op: Op(op),
                bytes: Payload::from(&b"hi"[..]),
            }
            .encode(),
        ),
    );
    input(
        w,
        id,
        Input::PasteCommit {
            op: Op(op),
            want_result: Some(true),
        },
    );
}

fn written(fs: &[ToClient]) -> Option<u64> {
    fs.iter().find_map(|f| match f {
        ToClient::InputDone(d) => Some(d.written_bytes),
        _ => None,
    })
}

/// Codec TS-5d, TS-5e, TS-5b: a paste writes exactly the scripted bytes, whatever the body was, written_bytes is their length, and a
/// failed precondition against the scripted mode has zero effect. A paste with no scripted bytes writes nothing.
#[test]
fn ts_5d_ts_5e_paste_writes_scripted_bytes_and_checks_precondition() {
    let (mut w, id) = live_script(json!({"writes": {"paste": "5041535445"}}));
    paste(&mut w, id, 1, true);
    assert_eq!(
        refusals(&frames(&mut w, id)),
        vec![RefusalReason::ModePreconditionFailed]
    );
    assert!(pty(&mut w).is_empty());
    w.control(
        "pty_output",
        &json!({"bytes_hex": "41", "scripted": {"modes": {"bracketed_paste": true}}}),
        None,
    )
    .unwrap();
    frames(&mut w, id);
    paste(&mut w, id, 2, true);
    assert_eq!(written(&frames(&mut w, id)), Some(5));
    assert_eq!(pty(&mut w), b"PASTE");
    let (mut w, id) = live_script(json!({}));
    paste(&mut w, id, 3, false);
    assert_eq!(written(&frames(&mut w, id)), Some(0));
    assert!(pty(&mut w).is_empty());
}

/// Codec TS-5f: a committed file writes exactly the scripted path paste. The scripted size is compared with max_paste_bytes before the
/// file is created: a size equal to the bound passes, and one byte over is refused with no file and no write.
#[test]
fn ts_5f_file_writes_scripted_bytes_and_scripted_size_equal_passes_over_refused() {
    let run = |size: u64| {
        let (mut w, id) = live_worker(
            0,
            WorkerConfig {
                max_paste_bytes: 16,
                script: script(
                    json!({"writes": {"file": "46494c45"}, "admission": {"file": size}}),
                ),
                ..WorkerConfig::default()
            },
        );
        input(
            &mut w,
            id,
            Input::FileBegin {
                op: Op(1),
                name: "a".into(),
                mime: None,
                total_bytes: None,
            },
        );
        input(&mut w, id, Input::FileCommit { op: Op(1) });
        let fs = frames(&mut w, id);
        let files = w.control("fs_count", &json!(null), None).unwrap()["count"]
            .as_u64()
            .unwrap();
        (fs, files, pty(&mut w))
    };
    let (fs, files, bytes) = run(16);
    assert!(fs
        .iter()
        .any(|f| matches!(f, ToClient::InputDone(d) if d.written_bytes == 4 && d.path.is_some())));
    assert_eq!((files, bytes), (1, b"FILE".to_vec()));
    let (fs, files, bytes) = run(17);
    assert_eq!(refusals(&fs), vec![RefusalReason::TooLarge]);
    assert_eq!((files, bytes.len()), (0, 0));
}

/// Codec TS-5, TS-7: a waiting paste holds its scripted worst-case size in the input queue, and the worker stops reading input once the
/// queue holds the bound. A size under the bound leaves the next frame readable.
#[test]
fn ts_5_ts_7_waiting_paste_counts_scripted_size_in_input_queue() {
    let unread_after_second_begin = |size: u64| {
        let (mut w, id) = live_worker(
            0,
            WorkerConfig {
                input_queue_bytes: 8,
                script: script(json!({"writes": {"paste": "50"}, "admission": {"paste": size}})),
                ..WorkerConfig::default()
            },
        );
        w.control("pty_blocked", &json!({"on": true}), None)
            .unwrap();
        paste(&mut w, id, 1, false);
        input(
            &mut w,
            id,
            Input::PasteBegin {
                op: Op(2),
                total_bytes: None,
                require_bracketed: None,
            },
        );
        w.unread(id)
    };
    assert!(unread_after_second_begin(8) > 0);
    assert_eq!(unread_after_second_begin(7), 0);
}

/// Codec TS-10: when the last route whose latest focus input is true ends, and the scripted modes ask for focus reports, the scripted focus-out is written.
#[test]
fn ts_10_route_end_writes_scripted_focus_out() {
    let (mut w, id) = live_script(json!({"writes": {"focus_in": "49", "focus_out": "4f"}}));
    w.control(
        "pty_output",
        &json!({"bytes_hex": "41", "scripted": {"modes": {"focus_reporting": true}}}),
        None,
    )
    .unwrap();
    frames(&mut w, id);
    input(
        &mut w,
        id,
        Input::Focus {
            op: Some(Op(1)),
            focused: true,
        },
    );
    w.control("close_route", &json!({"reason": "detached"}), Some(id))
        .unwrap();
    assert_eq!(pty(&mut w), b"IO");
}

/// Codec TS-4c: a scripted notification that the policy blocks is not counted, and the count and the last one come from the script.
#[test]
fn ts_4c_output_dropped_counts_notifications_that_pass_policy() {
    let cfg = WorkerConfig {
        notification_policy: NotificationPolicy::Matching(vec!["keep".into()]),
        ..WorkerConfig::default()
    };
    let mut w = FakeWorker::new(0, cfg);
    let id = w.attach(&AttachSpec {
        route_features: vec!["output_dropped".into()],
        ..AttachSpec::default()
    });
    frames(&mut w, id);
    w.control("pty_output", &json!({"bytes_hex": "41424344", "scripted": {"notifications": [
        {"at": 0, "source": "bell"}, {"at": 1, "source": "osc9", "body": "keep a"},
        {"at": 2, "source": "osc9", "body": "drop"}, {"at": 3, "source": "osc777", "title": "keep", "body": "z"}]}}), None).unwrap();
    w.control("queue_overflow", &json!(null), Some(id)).unwrap();
    let d = frames(&mut w, id)
        .into_iter()
        .find_map(|f| match f {
            ToClient::OutputDropped(d) => Some(d),
            _ => None,
        })
        .expect("output_dropped");
    assert_eq!(
        (d.notifications, d.last.source, d.last.title.as_deref()),
        (2, NotificationSource::Osc777, Some("keep"))
    );
}

/// Codec TS-4c, HC SF-5c: the OSC 777 text is the title and the body joined by one newline, so a pattern may span the newline. An empty pattern list matches nothing.
#[test]
fn ts_4c_sf_5c_policy_joined_text_bell_and_empty_list() {
    let count = |patterns: Vec<String>, notification: Value| {
        let cfg = WorkerConfig {
            notification_policy: NotificationPolicy::Matching(patterns),
            ..WorkerConfig::default()
        };
        let mut w = FakeWorker::new(0, cfg);
        let id = w.attach(&AttachSpec {
            route_features: vec!["output_dropped".into()],
            ..AttachSpec::default()
        });
        frames(&mut w, id);
        w.control(
            "pty_output",
            &json!({"bytes_hex": "41", "scripted": {"notifications": [notification]}}),
            None,
        )
        .unwrap();
        w.control("queue_overflow", &json!(null), Some(id)).unwrap();
        frames(&mut w, id).into_iter().find_map(|f| match f {
            ToClient::OutputDropped(d) => Some(d.notifications),
            _ => None,
        })
    };
    let osc = || json!({"source": "osc777", "title": "Title", "body": "Body"});
    assert_eq!(count(vec!["Title\nBody".into()], osc()), Some(1));
    assert_eq!(count(vec!["Title Body".into()], osc()), None);
    assert_eq!(count(vec![], osc()), None);
    // A bell has no text, so even the empty pattern never matches it.
    assert_eq!(count(vec![String::new()], json!({"source": "bell"})), None);
    assert_eq!(count(vec![String::new()], osc()), Some(1));
}

/// Codec TS-1, TS-3: `output_deflate` frames go only to a route that negotiated the feature, whatever the seed. With the feature, some seed deflates.
#[test]
fn output_deflate_only_on_a_route_that_negotiated_it() {
    let deflated = |seed: u64, features: Vec<String>| {
        let mut w = FakeWorker::new(seed, WorkerConfig::default());
        let id = w.attach(&AttachSpec {
            route_features: features,
            ..AttachSpec::default()
        });
        drain(&mut w, id);
        for chunk in ["one", "two", "three", "four"] {
            w.control(
                "pty_output",
                &json!({"bytes_hex": hexs(chunk.as_bytes())}),
                None,
            )
            .unwrap();
        }
        frames(&mut w, id)
            .iter()
            .any(|f| matches!(f, ToClient::OutputDeflate { .. }))
    };
    let mut with = false;
    for seed in 0..32 {
        assert!(
            !deflated(seed, vec![]),
            "seed {seed} deflated without the feature"
        );
        with |= deflated(seed, vec!["output_deflate".into()]);
    }
    assert!(with, "no seed deflated with the feature");
}
