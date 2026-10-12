//! Steward ruling R-100: a call whose message to the worker can never fit the link frame is refused at the call with
//! `InvalidInput{field}` (no `OpId`, no route, no event: AM-4), and the link stays up.

use super::*;
use crate::frame_fit::{
    fit_within, launch_fields, op_fields, worst_launch_request, Field, FIXED_PART, MODIFIER_UPPER,
    WORST_SIZE,
};
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::msg::json_len;
use botster_route_codec::prelude::{HexBytes, Modifier};

/// A handle with the smallest link frame bound: one byte of paste and of snapshot, so the bound is 1 MiB and six bytes.
fn small() -> World {
    World::new(limits(|l| {
        l.max_paste_bytes = 1;
        l.max_snapshot_bytes = 1;
    }))
}

fn bound(w: &World) -> usize {
    w.engine.link_frame_bound() as usize
}

fn invalid(field: &str) -> ErrorCode {
    ErrorCode::InvalidInput {
        field: Some(field.into()),
    }
}

/// A running session whose worker offers the notification policy (A2-6), with the scripted worker answering after it.
fn running_with_policy(w: &mut World, name: &str) {
    w.autopilot = Autopilot::Silent;
    w.ok(create(name));
    let start = w.engine.begin(Op::Start { id: sid(name) }).unwrap();
    w.pump();
    let link = w.link_of_after_hello(name);
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Launched {
            features: BTreeSet::from([Feature::NotificationPolicy]),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    });
    w.complete(start);
    w.autopilot = Autopilot::Full;
}

fn refused(w: &mut World, op: Op) -> ErrorCode {
    w.engine.begin(op).expect_err("the call is refused").code
}

/// The link of `session` is up: no link was closed, and a read on it completes.
fn link_up(w: &mut World, session: &str) {
    assert!(w.closed.is_empty(), "no link closes: {:?}", w.closed);
    assert!(matches!(
        w.ok(Op::ReadModeFlags {
            session: sid(session)
        }),
        OpOutput::Modes(_)
    ));
}

/// R-100 test: `Create` with an argv over the bound is refused `InvalidInput{request.argv}` at the call; nothing is
/// created, and the running session's link stays up.
#[test]
fn a_create_whose_launch_cannot_fit_is_refused_at_the_call() {
    let mut w = small();
    w.running("s1");
    let mut request = request();
    request.argv.push("a".repeat(bound(&w)));
    let code = refused(
        &mut w,
        Op::Create {
            session: sid("s2"),
            request,
        },
    );
    assert_eq!(code, invalid("request.argv"));
    link_up(&mut w, "s1");
    // The id is free: the refusal created no session.
    w.ok(create("s2"));
}

/// R-100 test: `SetNotificationPolicy` with patterns over the bound is refused `InvalidInput{policy.patterns}` at the
/// call, and the link stays up.
#[test]
fn a_policy_whose_message_cannot_fit_is_refused_at_the_call() {
    let mut w = small();
    running_with_policy(&mut w, "s1");
    let patterns = vec!["p".repeat(bound(&w) / 2), "q".repeat(bound(&w) / 2)];
    let code = refused(
        &mut w,
        Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::Matching { patterns },
        },
    );
    assert_eq!(code, invalid("policy.patterns"));
    link_up(&mut w, "s1");
}

/// R-100 (lead, 2026-10-12): before the start, the policy travels in the `Launch`. Patterns that fit their own message but
/// not the `Launch` that carries them with the request are refused `InvalidInput{policy.patterns}`; the start then works.
#[test]
fn a_created_session_refuses_a_policy_that_its_launch_cannot_carry() {
    let mut w = small();
    let third = bound(&w) / 3;
    let mut request = request();
    request.env.insert("E".into(), "e".repeat(third));
    w.ok(Op::Create {
        session: sid("s1"),
        request,
    });
    let patterns = vec!["p".repeat(2 * third)];
    let op = Op::SetNotificationPolicy {
        session: sid("s1"),
        policy: NotificationPolicy::Matching {
            patterns: patterns.clone(),
        },
    };
    let alone = HostMsg::Op {
        req: u64::MAX,
        op: op.clone(),
    };
    assert!(
        alone.encoded_len() <= bound(&w) as u64,
        "the op's own message fits"
    );
    assert_eq!(refused(&mut w, op), invalid("policy.patterns"));
    w.ok(Op::Start { id: sid("s1") });
    link_up(&mut w, "s1");
}

/// R-100: `Start` checks its `Launch` too. No public call stores a request whose `Launch` cannot fit: the checks of
/// `Create` and of the setters prevent it, with the worst color profile and size policy. So this test sets one in the
/// row to prove the check itself, as a guard.
#[test]
fn a_start_whose_launch_cannot_fit_is_refused_at_the_call() {
    let mut w = small();
    w.ok(create("s1"));
    let huge = "a".repeat(bound(&w));
    w.engine
        .sessions
        .get_mut(&sid("s1"))
        .expect("created")
        .request
        .argv
        .push(huge);
    assert_eq!(
        refused(&mut w, Op::Start { id: sid("s1") }),
        invalid("request.argv")
    );
}

/// R-100 test: `attach` with a `file_directory` over the bound is refused `InvalidInput{file_directory}`; no route is
/// registered, and the link stays up.
#[test]
fn an_attach_whose_handoff_cannot_fit_is_refused_at_the_call() {
    let mut w = small();
    w.running("s1");
    w.engine.poll_events(usize::MAX);
    let sent = w.sent.len();
    let options = |file_directory: String| AttachOptions {
        file_directory,
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
        owner: None,
        query_deadline: Some(Duration::from_secs(1)),
        route_tag: None,
        route_limits: None,
        history: None,
        stall_deadline: None,
        answers_queries: true,
        input: true,
    };
    let directory = format!("/{}", "d".repeat(bound(&w)));
    let refusal = w
        .engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            options(directory),
        )
        .expect_err("refused");
    assert_eq!(refusal.error.code, invalid("file_directory"));
    w.pump();
    // No route: no `AttachRoute` went to the worker, and no event came.
    assert!(
        !w.sent[sent..]
            .iter()
            .any(|(_, m)| matches!(m, HostMsg::AttachRoute { .. })),
        "no handoff"
    );
    assert!(w.engine.poll_events(usize::MAX).is_empty(), "no event");
    link_up(&mut w, "s1");
    // The client can attach with a directory that fits.
    w.engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            options("/d".into()),
        )
        .expect("a fitting attach is admitted");
}

/// R-100: each field is named when it is the largest one of a message that cannot fit.
#[test]
fn each_field_is_named_when_it_is_the_largest() {
    let mut w = small();
    w.running("s1");
    let over = "a".repeat(bound(&w));
    let with = |change: &dyn Fn(&mut SpawnRequest)| {
        let mut request = request();
        change(&mut request);
        Op::Create {
            session: sid("s9"),
            request,
        }
    };
    let cases: Vec<(Op, &str)> = vec![
        (
            with(&|r| {
                r.env.insert("K".into(), over.clone());
            }),
            "request.env",
        ),
        (with(&|r| r.cwd = format!("/{over}")), "request.cwd"),
        (
            with(&|r| {
                r.notification_policy = Some(NotificationPolicy::Matching {
                    patterns: vec![over.clone()],
                })
            }),
            "request.notification_policy.patterns",
        ),
        (
            Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId(over.clone()),
            },
            "owner",
        ),
    ];
    for (op, field) in cases {
        assert_eq!(refused(&mut w, op), invalid(field), "{field}");
    }
    for (field, change) in [
        (
            "route_features",
            (|o: &mut AttachOptions, s: &str| o.route_features = vec![s.into()])
                as fn(&mut AttachOptions, &str),
        ),
        ("terminal_formats", |o, s| {
            o.terminal_formats = vec![s.into()]
        }),
    ] {
        let mut options = AttachOptions {
            file_directory: "/tmp".into(),
            file_permissions: None,
            route_features: vec![],
            terminal_formats: vec![],
            owner: None,
            query_deadline: Some(Duration::from_secs(1)),
            route_tag: None,
            route_limits: None,
            history: None,
            stall_deadline: None,
            answers_queries: true,
            input: true,
        };
        change(&mut options, &over);
        let refusal = w
            .engine
            .attach(
                ClientId("c".into()),
                sid("s1"),
                RouteTransport::Stream(StreamEndpoint::new(())),
                options,
            )
            .expect_err("refused");
        assert_eq!(refusal.error.code, invalid(field), "{field}");
    }
    link_up(&mut w, "s1");
}

/// R-100 boundary (lead, 2026-10-12): a message whose worst-case encoding is exactly the bound passes `begin` and reaches
/// the worker; one byte more is refused.
#[test]
fn a_message_of_exactly_the_bound_passes_and_one_byte_more_is_refused() {
    let mut w = small();
    w.running("s1");
    let capture = |owner: String| Op::CaptureSnapshot {
        session: sid("s1"),
        owner: ClientId(owner),
    };
    let base = HostMsg::Op {
        req: u64::MAX,
        op: capture(String::new()),
    }
    .encoded_len() as usize;
    let fits = "o".repeat(bound(&w) - base);
    assert_eq!(
        HostMsg::Op {
            req: u64::MAX,
            op: capture(fits.clone()),
        }
        .encoded_len() as usize,
        bound(&w),
        "the worst case is exactly the bound"
    );
    assert_eq!(
        refused(&mut w, capture(format!("{fits}o"))),
        invalid("owner")
    );
    w.engine.begin(capture(fits.clone())).expect("admitted");
    w.pump();
    assert!(
        w.sent.iter().any(|(_, msg)| matches!(
            msg,
            HostMsg::Op { op: Op::CaptureSnapshot { owner, .. }, .. } if owner.0 == fits
        )),
        "the message reaches the worker"
    );
    assert!(w.closed.is_empty(), "no link closes");
}

/// The 6x bound (lead, 2026-10-12): a `Text` write of `max_paste_bytes` control bytes, each a six-byte JSON escape, fits the
/// link frame, so no write that `max_paste_bytes` admits is refused for the frame.
#[test]
fn a_text_write_of_control_bytes_at_max_paste_fits() {
    let paste = 1u64 << 20;
    let mut w = World::new(limits(|l| {
        l.max_paste_bytes = paste;
        l.max_snapshot_bytes = 1;
        l.input_retained_bytes = 2 * paste;
    }));
    w.running("s1");
    let text = "\u{1}".repeat(paste as usize);
    let write = Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Text { text },
        guard: None,
    };
    let len = HostMsg::Op {
        req: u64::MAX,
        op: write.clone(),
    }
    .encoded_len();
    assert!(len > 6 * paste, "each byte takes six: {len}");
    assert!(len <= u64::from(w.engine.link_frame_bound()), "{len}");
    w.engine.begin(write).expect("admitted");
}

/// Where the cap of the bound binds (very large limits), a text that cannot fit is refused `InvalidInput{payload.text}`. The
/// cap is `u32::MAX / 2`; a text over it does not fit in a test's memory, so this proves the refusal at a small bound.
#[test]
fn a_text_over_a_bound_is_refused_with_its_field() {
    let op = Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Text {
            text: "\u{1}".repeat(1000),
        },
        guard: None,
    };
    let msg = || HostMsg::Op {
        req: u64::MAX,
        op: op.clone(),
    };
    let fields = op_fields(&op).expect("a write travels");
    let len = msg().encoded_len();
    assert!(
        fit_within(len, &fields, msg).is_ok(),
        "exactly the bound fits"
    );
    let error = fit_within(len - 1, &fields, msg).expect_err("one byte under");
    assert_eq!(error.code, invalid("payload.text"));
}

/// The fast path is sound: each field's upper bound is at least its JSON length, and every message without its fields is
/// under `FIXED_PART`, so a message whose upper bounds and `FIXED_PART` fit the bound always fits.
#[test]
fn the_fast_path_never_admits_a_message_that_cannot_fit() {
    let escaped = "\u{1}\"\\é\u{7f}a".repeat(50);
    let strings = vec![escaped.clone(), String::new(), "x".into()];
    let map = BTreeMap::from([
        (escaped.clone(), escaped.clone()),
        ("k".into(), String::new()),
    ]);
    let hex = HexBytes(vec![0xab; 77]);
    for field in [
        Field::Str("s", &escaped),
        Field::Str("e", ""),
        Field::Strs("l", &strings),
        Field::Strs("e", &[]),
        Field::Map("m", &map),
        Field::Map("e", &BTreeMap::new()),
        Field::Hex("h", &hex),
    ] {
        assert!(
            field.upper() >= field.exact(),
            "{} {}",
            field.upper(),
            field.exact()
        );
    }
    // The largest messages without their fields: a launch with every bounded part at its largest, a handoff with every
    // number at its largest, and the largest op.
    let w = World::new(CoreLimits::default());
    let mut request = request();
    request.argv.clear();
    request.cwd.clear();
    request.color_profile = Some(ColorProfile {
        palette: Some(vec![
            Rgb {
                r: 255,
                g: 255,
                b: 255
            };
            256
        ]),
        foreground: Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        background: Rgb {
            r: 255,
            g: 255,
            b: 255,
        },
        cursor: Some(Rgb {
            r: 255,
            g: 255,
            b: 255,
        }),
    });
    request.size_policy = Some(SizePolicy::Latest);
    let launch = HostMsg::Launch(Box::new(w.engine.launch_spec_for(&request, WORST_SIZE)));
    assert!(launch_fields(&request, "p").iter().all(|f| f.exact() <= 2));
    let handoff = HostMsg::AttachRoute {
        route: RouteId(u64::MAX),
        options: AttachOptions {
            file_directory: String::new(),
            file_permissions: Some(u32::MAX),
            route_features: vec![],
            terminal_formats: vec![],
            owner: None,
            query_deadline: Some(Duration::MAX),
            route_tag: None,
            route_limits: Some(RouteLimitChoices {
                max_frame_bytes: Some(u64::MAX),
                max_screen_frame_bytes: Some(u64::MAX),
                max_history_page_bytes: Some(u64::MAX),
            }),
            history: None,
            stall_deadline: Some(Duration::MAX),
            answers_queries: true,
            input: true,
        },
        limits: AppliedRouteLimits {
            max_frame_bytes: u64::MAX,
            max_screen_frame_bytes: u64::MAX,
            max_history_page_bytes: u64::MAX,
            max_paste_bytes: u64::MAX,
            max_query_bytes: u64::MAX,
            max_query_reply_bytes: u64::MAX,
            max_file_bytes: u64::MAX,
            query_deadline: Duration::MAX,
            stall_deadline: Duration::MAX,
            stall_close_after: Duration::MAX,
            route_input_queue_bytes: u64::MAX,
        },
    };
    let mouse = HostMsg::Op {
        req: u64::MAX,
        op: Op::WriteInput {
            session: sid(""),
            payload: InputPayload::Mouse(MouseInput {
                action: botster_route_codec::prelude::MouseAction::Wheel,
                button: botster_route_codec::prelude::MouseButton::WheelUp,
                mods: vec![],
                col: u32::MAX,
                row: u32::MAX,
                x: Some(u32::MAX),
                y: Some(u32::MAX),
                notches: Some(u32::MAX),
            }),
            guard: None,
        },
    };
    for msg in [launch, handoff, mouse] {
        assert!(msg.encoded_len() < FIXED_PART, "{}", msg.encoded_len());
    }
}

/// The upper bound is tight where every byte is an escape: over the JSON length by at most one separator for a string, a
/// list or a map, and by three bytes for hex (`2 * len + 17` encoded). So the fast path refuses only messages that are near
/// the bound.
#[test]
fn the_upper_bound_is_tight_on_escaped_text() {
    let escaped = "\u{1}".repeat(4);
    let strings = vec![escaped.clone(), escaped.clone()];
    let map = BTreeMap::from([
        ("\u{2}".repeat(3), escaped.clone()),
        (escaped.clone(), escaped.clone()),
    ]);
    let (none, one, many) = (
        HexBytes(vec![]),
        HexBytes(vec![0xab]),
        HexBytes(vec![0xab; 77]),
    );
    // `"caps_lock",` is the longest modifier with its separator.
    let mods = vec![Modifier::CapsLock; 5];
    for (i, (field, slack)) in [
        (Field::Str("s", &escaped), 1),
        (Field::Strs("l", &strings), 1),
        (Field::Map("m", &map), 1),
        (Field::Hex("h", &none), 3),
        (Field::Hex("h", &one), 3),
        (Field::Hex("h", &many), 3),
        (Field::Mods("m", &mods), 1),
    ]
    .into_iter()
    .enumerate()
    {
        let (upper, exact) = (field.upper(), field.exact());
        assert!(
            exact <= upper && upper <= exact + slack,
            "case {i}: upper {upper}, exact {exact}"
        );
    }
}

/// The fast path decides without encoding when the fields' upper bounds and `FIXED_PART` are exactly the bound.
#[test]
fn the_fast_path_encodes_nothing_at_its_bound() {
    let text = "t".repeat(10);
    let fields = [Field::Str("payload.text", &text)];
    let upper = fields[0].upper();
    assert!(fit_within(upper + FIXED_PART, &fields, || panic!("encoded")).is_ok());
}

/// A running session's policy travels in its own op only: patterns that a `Launch` with the request could not carry are
/// admitted once the session runs.
#[test]
fn a_running_session_does_not_check_a_launch_for_its_policy() {
    let mut w = small();
    let third = bound(&w) / 3;
    let mut request = request();
    request.env.insert("E".into(), "e".repeat(third));
    w.autopilot = Autopilot::Silent;
    w.ok(Op::Create {
        session: sid("s1"),
        request,
    });
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Launched {
            features: BTreeSet::from([Feature::NotificationPolicy]),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    });
    w.complete(start);
    w.engine
        .begin(Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::Matching {
                patterns: vec!["p".repeat(2 * third)],
            },
        })
        .expect("admitted");
}

/// The id of a session is a field of each op that names it, when `max_session_id_bytes` lets it be large.
#[test]
fn a_large_session_id_is_named() {
    let mut w = World::new(limits(|l| {
        l.max_paste_bytes = 1;
        l.max_snapshot_bytes = 1;
        l.max_session_id_bytes = u32::MAX;
    }));
    let id = "i".repeat(bound(&w));
    w.running(&id);
    assert_eq!(
        refused(&mut w, Op::ReadCursor { session: sid(&id) }),
        invalid("session")
    );
    assert_eq!(
        refused(
            &mut w,
            Op::Signal {
                id: sid(&id),
                sig: Signal::Term,
            }
        ),
        invalid("id")
    );
    assert!(w.closed.is_empty(), "no link closes");
}

/// The fields of a write: where the cap of the bound binds, each payload's field is named (a bound under the message).
#[test]
fn each_field_of_a_write_is_named() {
    use botster_route_codec::prelude::{Key, KeyEvent, NamedKey};
    let big = "k".repeat(1000);
    let key = |key: Key, text: Option<String>| KeyInput {
        key,
        shifted_key: None,
        base_layout_key: None,
        mods: vec![],
        event: KeyEvent::Press,
        text,
        repeat: None,
    };
    let cases = [
        (
            InputPayload::Bytes {
                bytes: HexBytes(vec![1; 1000]),
            },
            "payload.bytes",
        ),
        (
            InputPayload::Paste {
                bytes: HexBytes(vec![1; 1000]),
                require_bracketed: true,
            },
            "payload.bytes",
        ),
        (
            InputPayload::Key(key(Key::Named(NamedKey(big.clone())), None)),
            "payload.key",
        ),
        (
            InputPayload::Key(key(Key::Char(big.clone()), None)),
            "payload.key",
        ),
        (
            InputPayload::Key(key(Key::Char("a".into()), Some(big.clone()))),
            "payload.text",
        ),
        (
            InputPayload::Key(KeyInput {
                shifted_key: Some(Key::Char(big.clone())),
                ..key(Key::Char("a".into()), None)
            }),
            "payload.shifted_key",
        ),
        (
            InputPayload::Key(KeyInput {
                base_layout_key: Some(Key::Named(NamedKey(big.clone()))),
                ..key(Key::Char("a".into()), None)
            }),
            "payload.base_layout_key",
        ),
    ];
    for (payload, field) in cases {
        let op = Op::WriteInput {
            session: sid("s1"),
            payload,
            guard: None,
        };
        let msg = || HostMsg::Op {
            req: u64::MAX,
            op: op.clone(),
        };
        let fields = op_fields(&op).expect("a write travels");
        let error = fit_within(msg().encoded_len() - 1, &fields, msg).expect_err(field);
        assert_eq!(error.code, invalid(field), "{field}");
    }
}

/// The JSON length of the `Launch` that a request with `filler` bytes of argv gives at the worst case.
fn worst_launch_len(w: &World, filler: usize) -> u64 {
    let mut r = request();
    r.argv.push("a".repeat(filler));
    HostMsg::Launch(Box::new(
        w.engine
            .launch_spec_for(&worst_launch_request(&r), WORST_SIZE),
    ))
    .encoded_len()
}

/// RC-R100-I1 and RC-R100-3: a `Launch` is checked at the worker's bound before its launch, with the largest color profile
/// and size policy. A request whose worst `Launch` is exactly that bound is admitted. Setters that are queued on the
/// `Created` session before `Start` store the largest color profile and size policy, and the `Launch` that the host sends
/// still decodes in the shared frame decoder of a worker before its launch. One byte more is refused at `Create`.
#[test]
fn an_admitted_launch_decodes_in_the_workers_decoder_before_its_launch() {
    let mut w = World::offering(Feature::SizePolicyOther);
    assert!(
        w.engine.link_frame_bound() > DEFAULT_MAX_PAYLOAD,
        "the link bound is not the one that applies"
    );
    let filler = (u64::from(DEFAULT_MAX_PAYLOAD) - worst_launch_len(&w, 0)) as usize;
    assert_eq!(worst_launch_len(&w, filler), u64::from(DEFAULT_MAX_PAYLOAD));
    let with = |filler: usize| {
        let mut r = request();
        r.argv.push("a".repeat(filler));
        Op::Create {
            session: sid("s1"),
            request: r,
        }
    };
    assert_eq!(refused(&mut w, with(filler + 1)), invalid("request.argv"));
    w.ok(with(filler));
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let profile = ColorProfile {
        palette: Some(vec![white; 256]),
        foreground: white,
        background: white,
        cursor: Some(white),
    };
    let ops = [
        Op::SetColorProfile {
            session: sid("s1"),
            profile: profile.clone(),
        },
        Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Smallest,
        },
        Op::Start { id: sid("s1") },
    ]
    .map(|op| w.engine.begin(op).expect("admitted"));
    w.complete_all(&ops);
    let launch = w
        .sent
        .iter()
        .find_map(|(_, m)| match m {
            HostMsg::Launch(spec) => Some(spec.clone()),
            _ => None,
        })
        .expect("a launch was sent");
    assert_eq!(
        launch.color_profile,
        Some(profile),
        "the setters came first"
    );
    assert_eq!(launch.size_policy, SizePolicy::Smallest);
    let mut payload = Vec::new();
    HostMsg::Launch(launch).encode(&mut payload);
    let mut frame = Vec::new();
    encode_frame(
        FrameType::HOST_MSG,
        &payload,
        w.engine.link_frame_bound(),
        &mut frame,
    )
    .expect("the host encodes it");
    let mut decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
    decoder.push(&frame);
    let got = decoder
        .next_frame()
        .expect("the worker's decoder takes it")
        .expect("one frame");
    assert!(matches!(
        HostMsg::decode(&got.payload),
        Ok(HostMsg::Launch(_))
    ));
}

/// The worst case of a `Launch` has the longest size policy and color profile that a setter can store (RC-R100-3).
#[test]
fn the_worst_launch_request_has_the_longest_setter_values() {
    let worst = worst_launch_request(&request());
    for policy in [
        SizePolicy::Latest,
        SizePolicy::Smallest,
        SizePolicy::Largest,
    ] {
        assert!(json_len(&worst.size_policy) >= json_len(&Some(policy)));
    }
    let white = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let full = ColorProfile {
        palette: Some(vec![white; 256]),
        foreground: white,
        background: white,
        cursor: Some(white),
    };
    assert_eq!(worst.color_profile, Some(full));
}

/// RC-R100-1: the modifiers of a key or a mouse input are a field. A write with a modifier list over the bound is refused
/// `InvalidInput{payload.mods}` at the call, and the link stays up.
#[test]
fn a_write_with_too_many_modifiers_is_refused_with_its_field() {
    use botster_route_codec::prelude::{Key, KeyEvent, MouseAction, MouseButton};
    // Paste room for the PTY bytes of one key or mouse input, so `PayloadTooLarge` does not come first.
    let mut w = World::new(limits(|l| {
        l.max_paste_bytes = 64;
        l.max_snapshot_bytes = 1;
    }));
    w.running("s1");
    // `"shift",` is eight bytes.
    let mods = vec![Modifier::Shift; bound(&w) / 8 + 1];
    let payloads = [
        InputPayload::Key(KeyInput {
            key: Key::Char("a".into()),
            shifted_key: None,
            base_layout_key: None,
            mods: mods.clone(),
            event: KeyEvent::Press,
            text: None,
            repeat: None,
        }),
        InputPayload::Mouse(MouseInput {
            action: MouseAction::Press,
            button: MouseButton::Left,
            mods,
            col: 0,
            row: 0,
            x: None,
            y: None,
            notches: None,
        }),
    ];
    for payload in payloads {
        let op = Op::WriteInput {
            session: sid("s1"),
            payload,
            guard: None,
        };
        assert_eq!(refused(&mut w, op), invalid("payload.mods"));
        link_up(&mut w, "s1");
    }
}

/// [`MODIFIER_UPPER`] bounds each modifier with its separator, and the longest one meets it.
#[test]
fn the_modifier_bound_covers_every_modifier() {
    use Modifier::*;
    let all = [Shift, Alt, Ctrl, Meta, Super, Hyper, CapsLock, NumLock];
    let longest = all.iter().map(|m| json_len(m) + 1).max().unwrap();
    assert_eq!(longest, MODIFIER_UPPER);
}
