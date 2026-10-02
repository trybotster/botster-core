//! The boundaries of the argument checks and of the capacity codes (Core A2-1, 9.3, IN-5, IN-9): each limit is tested at the
//! value that is allowed and at the next value that is not.

use super::*;
use botster_route_codec::prelude::{Key, KeyEvent, Modifier, MouseAction, MouseButton};

fn refused(w: &mut World, op: Op) -> ErrorCode {
    w.engine.begin(op).expect_err("the op is refused").code
}

fn accepted(w: &mut World, op: Op) {
    w.engine.begin(op).expect("the op is admitted");
}

fn with_size(rows: u32, cols: u32, cell: Option<(u32, u32)>) -> Size {
    Size {
        rows,
        cols,
        cell_px: cell.map(|(width, height)| CellPx { width, height }),
    }
}

/// Core A2-1 (`Resize`, `Create`): a size is from 1 to 65 535 in each dimension, and a cell is from 1 pixel.
#[test]
fn size_limits_are_exact() {
    for (rows, cols, cell, ok) in [
        (1, 1, None, true),
        (65_535, 65_535, None, true),
        (0, 80, None, false),
        (24, 0, None, false),
        (65_536, 80, None, false),
        (24, 65_536, None, false),
        (24, 80, Some((1, 1)), true),
        (24, 80, Some((0, 1)), false),
        (24, 80, Some((1, 0)), false),
    ] {
        let mut w = World::default();
        w.ok(create("s1"));
        let op = Op::Resize {
            session: sid("s1"),
            size: with_size(rows, cols, cell),
        };
        if ok {
            accepted(&mut w, op);
        } else {
            assert_eq!(
                refused(&mut w, op),
                ErrorCode::InvalidInput,
                "{rows}x{cols} {cell:?}"
            );
        }
        let mut request = request();
        request.size = with_size(rows, cols, cell);
        let create = Op::Create {
            session: sid("s2"),
            request,
        };
        if ok {
            accepted(&mut w, create);
        } else {
            assert_eq!(refused(&mut w, create), ErrorCode::InvalidInput);
        }
    }
}

fn profile(entries: Option<usize>) -> ColorProfile {
    let rgb = Rgb { r: 1, g: 2, b: 3 };
    ColorProfile {
        palette: entries.map(|n| vec![rgb; n]),
        foreground: rgb,
        background: rgb,
        cursor: None,
    }
}

/// Core A2-1 (`SetColorProfile`, `Create`): a palette has exactly 256 entries when it is present. The profile reaches the
/// launch of the worker.
#[test]
fn a_palette_has_exactly_256_entries() {
    for (entries, ok) in [
        (None, true),
        (Some(256), true),
        (Some(255), false),
        (Some(257), false),
    ] {
        let mut w = World::default();
        w.ok(create("s1"));
        let set = Op::SetColorProfile {
            session: sid("s1"),
            profile: profile(entries),
        };
        let mut request = request();
        request.color_profile = Some(profile(entries));
        let make = Op::Create {
            session: sid("s2"),
            request,
        };
        if ok {
            accepted(&mut w, set);
            accepted(&mut w, make);
        } else {
            assert_eq!(refused(&mut w, set), ErrorCode::InvalidInput, "{entries:?}");
            assert_eq!(refused(&mut w, make), ErrorCode::InvalidInput);
        }
    }
    // The state check comes first, and a profile of a `Created` session is stored and launched.
    let mut w = World::default();
    w.ok(create("s1"));
    w.ok(Op::SetColorProfile {
        session: sid("s1"),
        profile: profile(Some(256)),
    });
    w.ok(Op::Start { id: sid("s1") });
    let launched = w.sent.iter().find_map(|(_, m)| match m {
        HostMsg::Launch(spec) => Some(spec.color_profile.clone()),
        _ => None,
    });
    assert_eq!(launched, Some(Some(profile(Some(256)))));
}

/// Core A2-1 (`Create`): argv, env, cwd and the size are checked, and each check refuses alone.
#[test]
fn a_spawn_request_is_checked_part_by_part() {
    let bad: Vec<(&str, SpawnRequest)> = {
        let mut out = Vec::new();
        let mut r = request();
        r.argv = vec![];
        out.push(("empty argv", r));
        let mut r = request();
        r.argv = vec!["a".into(), "b\0".into()];
        out.push(("NUL in argv", r));
        let mut r = request();
        r.env.insert(String::new(), "v".into());
        out.push(("empty key", r));
        let mut r = request();
        r.env.insert("A=B".into(), "v".into());
        out.push(("key with =", r));
        let mut r = request();
        r.env.insert("A\0".into(), "v".into());
        out.push(("NUL in key", r));
        let mut r = request();
        r.env.insert("A".into(), "v\0".into());
        out.push(("NUL in value", r));
        let mut r = request();
        r.cwd = "relative".into();
        out.push(("relative cwd", r));
        let mut r = request();
        r.cwd = "/a\0".into();
        out.push(("NUL in cwd", r));
        out
    };
    for (what, request) in bad {
        let mut w = World::default();
        assert_eq!(
            refused(
                &mut w,
                Op::Create {
                    session: sid("s1"),
                    request
                }
            ),
            ErrorCode::InvalidInput,
            "{what}"
        );
    }
    let mut good = request();
    good.env.insert("A".into(), "v".into());
    let mut w = World::default();
    accepted(
        &mut w,
        Op::Create {
            session: sid("s1"),
            request: good,
        },
    );
}

fn hex(n: usize) -> botster_route_codec::prelude::HexBytes {
    botster_route_codec::prelude::HexBytes(vec![b'x'; n])
}

fn write(payload: InputPayload) -> Op {
    Op::WriteInput {
        session: sid("s1"),
        payload,
        guard: None,
    }
}

fn key(repeat: Option<u16>, event: KeyEvent, shifted: bool, mods: Vec<Modifier>) -> InputPayload {
    InputPayload::Key(KeyInput {
        key: Key::Char('a'.into()),
        shifted_key: shifted.then(|| Key::Char('A'.into())),
        base_layout_key: None,
        mods,
        event,
        text: None,
        repeat,
    })
}

fn mouse(action: MouseAction, button: MouseButton, notches: Option<u32>) -> InputPayload {
    InputPayload::Mouse(MouseInput {
        action,
        button,
        row: 1,
        col: 1,
        x: None,
        y: None,
        mods: vec![],
        notches,
    })
}

/// Core IN-5, IN-9, A2-1 (`WriteInput`): the payload bound is exact for every kind: the bytes of a byte, text or paste
/// payload, and 64 bytes for each repeat of a key and each notch of a wheel.
#[test]
fn the_payload_bound_is_exact_for_every_kind() {
    let mut w = World::new(limits(|l| {
        l.max_paste_bytes = 128;
        l.max_key_repeat = 3;
        l.input_ops_per_session = 64;
        l.input_retained_bytes = 1 << 20;
    }));
    w.running("s1");
    let paste = |n| InputPayload::Paste {
        bytes: hex(n),
        require_bracketed: false,
    };
    let text = |n: usize| InputPayload::Text {
        text: "x".repeat(n),
    };
    for (what, payload, ok) in [
        ("bytes 128", InputPayload::Bytes { bytes: hex(128) }, true),
        ("bytes 129", InputPayload::Bytes { bytes: hex(129) }, false),
        ("text 128", text(128), true),
        ("text 129", text(129), false),
        ("paste 128", paste(128), true),
        ("paste 129", paste(129), false),
        (
            "key repeat 2",
            key(Some(2), KeyEvent::Press, false, vec![]),
            true,
        ),
        (
            "key repeat 3",
            key(Some(3), KeyEvent::Press, false, vec![]),
            false,
        ),
        (
            "wheel 2",
            mouse(MouseAction::Wheel, MouseButton::WheelUp, Some(2)),
            true,
        ),
        (
            "wheel 3",
            mouse(MouseAction::Wheel, MouseButton::WheelUp, Some(3)),
            false,
        ),
    ] {
        let result = w.engine.begin(write(payload));
        if ok {
            assert!(result.is_ok(), "{what}");
        } else {
            assert_eq!(
                result.unwrap_err().code,
                ErrorCode::PayloadTooLarge,
                "{what}"
            );
        }
    }
}

/// Core IN-9, A2-1: a repeat is from 1 to `max_key_repeat` and only with a press; `shifted_key` needs shift; a wheel button
/// pairs with the wheel action; `notches` is from 1 and only with the wheel action.
#[test]
fn key_and_mouse_arguments_are_checked_one_by_one() {
    let mut w = World::new(limits(|l| {
        l.max_paste_bytes = 1 << 16;
        l.max_key_repeat = 5;
        l.input_ops_per_session = 1000;
        l.input_retained_bytes = 1 << 30;
    }));
    w.running("s1");
    let invalid = [
        ("repeat 0", key(Some(0), KeyEvent::Press, false, vec![])),
        ("repeat over", key(Some(6), KeyEvent::Press, false, vec![])),
        (
            "repeat on release",
            key(Some(2), KeyEvent::Release, false, vec![]),
        ),
        (
            "shifted without shift",
            key(None, KeyEvent::Press, true, vec![]),
        ),
        (
            "wheel button, press",
            mouse(MouseAction::Press, MouseButton::WheelUp, None),
        ),
        (
            "button left, wheel",
            mouse(MouseAction::Wheel, MouseButton::Left, None),
        ),
        (
            "notches with press",
            mouse(MouseAction::Press, MouseButton::Left, Some(1)),
        ),
        (
            "notches 0",
            mouse(MouseAction::Wheel, MouseButton::WheelDown, Some(0)),
        ),
    ];
    for (what, payload) in invalid {
        assert_eq!(
            w.engine.begin(write(payload)).unwrap_err().code,
            ErrorCode::InvalidInput,
            "{what}"
        );
    }
    for (what, payload) in [
        ("repeat 1", key(Some(1), KeyEvent::Press, false, vec![])),
        ("repeat max", key(Some(5), KeyEvent::Press, false, vec![])),
        ("no repeat", key(None, KeyEvent::Release, false, vec![])),
        (
            "shifted with shift",
            key(None, KeyEvent::Press, true, vec![Modifier::Shift]),
        ),
        (
            "wheel",
            mouse(MouseAction::Wheel, MouseButton::WheelLeft, Some(1)),
        ),
        ("press", mouse(MouseAction::Press, MouseButton::Left, None)),
        ("focus", InputPayload::Focus { focused: true }),
    ] {
        assert!(w.engine.begin(write(payload)).is_ok(), "{what}");
    }
}

/// Core IN-5, IN-9: the bytes that a write holds against the lane: the payload bytes, and 64 for each repeat or notch or
/// other semantic payload.
#[test]
fn held_bytes_counts_each_kind() {
    let held = HostEngine::held_bytes;
    assert_eq!(held(&InputPayload::Bytes { bytes: hex(7) }), 7);
    assert_eq!(held(&InputPayload::Text { text: "abc".into() }), 3);
    assert_eq!(
        held(&InputPayload::Paste {
            bytes: hex(5),
            require_bracketed: true
        }),
        5
    );
    assert_eq!(held(&key(None, KeyEvent::Press, false, vec![])), 64);
    assert_eq!(held(&key(Some(3), KeyEvent::Press, false, vec![])), 192);
    assert_eq!(
        held(&mouse(MouseAction::Wheel, MouseButton::WheelUp, None)),
        64
    );
    assert_eq!(
        held(&mouse(MouseAction::Wheel, MouseButton::WheelUp, Some(3))),
        192
    );
    assert_eq!(held(&InputPayload::Focus { focused: true }), 64);
}

/// Core IN-5, 9.3: the lane of a session is full at `input_ops_per_session` writes or at `input_retained_bytes` bytes, and
/// not before; a session limit and a capture limit are exact as well.
#[test]
fn capacity_limits_are_exact() {
    // The lane, by bytes: 10 bytes fit exactly; one more byte does not.
    let mut w = World::new(limits(|l| {
        l.input_retained_bytes = 10;
        l.input_ops_per_session = 100;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    accepted(&mut w, write(InputPayload::Bytes { bytes: hex(4) }));
    accepted(&mut w, write(InputPayload::Bytes { bytes: hex(6) }));
    assert_eq!(
        refused(&mut w, write(InputPayload::Bytes { bytes: hex(1) })),
        ErrorCode::LaneFull
    );
    // The lane, by count.
    let mut w = World::new(limits(|l| {
        l.input_ops_per_session = 2;
        l.input_retained_bytes = 1 << 20;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    accepted(&mut w, write(InputPayload::Bytes { bytes: hex(1) }));
    accepted(&mut w, write(InputPayload::Bytes { bytes: hex(1) }));
    assert_eq!(
        refused(&mut w, write(InputPayload::Bytes { bytes: hex(1) })),
        ErrorCode::LaneFull
    );
    // Sessions: the limit counts the sessions that exist.
    let mut w = World::new(limits(|l| l.max_sessions = 2));
    accepted(&mut w, create("a"));
    accepted(&mut w, create("b"));
    assert_eq!(refused(&mut w, create("c")), ErrorCode::SessionLimit);
}

/// Core ID-1, IN-6: every op that names a session is recorded in the op identity of that session, so that `cancel` can tell
/// an op of a removed instance.
#[test]
fn every_op_that_names_a_session_is_recorded_with_it() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.ok(create("s1"));
    let recorded = |w: &World, op: OpId| w.engine.sessions[&sid("s1")].ops.contains(op.0);
    let meta = w
        .engine
        .begin(Op::UpdateMetadata {
            id: sid("s1"),
            labels: Default::default(),
        })
        .unwrap();
    assert!(recorded(&w, meta));
    let resize = w
        .engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: with_size(10, 10, None),
        })
        .unwrap();
    assert!(recorded(&w, resize));
    let color = w
        .engine
        .begin(Op::SetColorProfile {
            session: sid("s1"),
            profile: profile(None),
        })
        .unwrap();
    assert!(recorded(&w, color));
    let policy = w
        .engine
        .begin(Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::All,
        })
        .unwrap();
    assert!(recorded(&w, policy));
    let size_policy = w
        .engine
        .begin(Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Latest,
        })
        .unwrap();
    assert!(recorded(&w, size_policy));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    assert!(recorded(&w, start));
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
    for (what, op) in [
        ("read", Op::ReadCursor { session: sid("s1") }),
        ("modes", Op::ReadModeFlags { session: sid("s1") }),
        (
            "screen",
            Op::ReadScreen {
                session: sid("s1"),
                history: false,
            },
        ),
        (
            "capture",
            Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId("c".into()),
            },
        ),
        ("write", write(InputPayload::Focus { focused: true })),
        (
            "signal",
            Op::Signal {
                id: sid("s1"),
                sig: Signal::Term,
            },
        ),
        ("stop", Op::Stop { id: sid("s1") }),
    ] {
        let id = w.engine.begin(op).unwrap();
        assert!(recorded(&w, id), "{what}");
    }
}

/// Core A2-1: an op that names no live session is refused by its own row: `UnknownSession`, `UnknownRoute`, `Unsupported`
/// (WebRTC, services, a live adopt) or `UnknownService`.
#[test]
fn the_rows_of_the_unbuilt_operations_have_their_own_codes() {
    let mut w = World::default();
    w.running("s1");
    let webrtc = |session: &str| Op::AttachWebRtc {
        client: ClientId("c".into()),
        session: sid(session),
        offer: String::new(),
        expected_fingerprint: String::new(),
        options: AttachOptions {
            file_directory: "/tmp".into(),
            file_permissions: None,
            route_features: vec![],
            terminal_formats: vec![],
            connect_deadline: None,
            owner: None,
            query_deadline: None,
            route_tag: None,
            route_limits: None,
            history: None,
            stall_deadline: None,
            answers_queries: false,
            input: true,
        },
    };
    assert_eq!(refused(&mut w, webrtc("nope")), ErrorCode::UnknownSession);
    assert!(matches!(
        refused(&mut w, webrtc("s1")),
        ErrorCode::Unsupported { .. }
    ));
    assert_eq!(
        refused(&mut w, Op::Adopt { id: sid("nope") }),
        ErrorCode::UnknownSession
    );
    assert_eq!(
        refused(&mut w, Op::Adopt { id: sid("s1") }),
        ErrorCode::WrongState,
        "a running session is not adoptable"
    );
    let service = w
        .engine
        .begin(Op::StopService {
            id: ServiceId([0; 32]),
        })
        .unwrap_err();
    assert_eq!(service.code, ErrorCode::UnknownService);
    let service = w
        .engine
        .begin(Op::RemoveService {
            id: ServiceId([0; 32]),
        })
        .unwrap_err();
    assert_eq!(service.code, ErrorCode::UnknownService);
    assert_eq!(
        refused(
            &mut w,
            Op::Detach {
                route: RouteId(9),
                reason: DetachReason::Detached
            }
        ),
        ErrorCode::UnknownRoute
    );
}

/// Core OR-1, AM-1: the setters of a `Created` session are applied in order before the launch, each of them, and a setter of a
/// running session goes to the worker instead.
#[test]
fn each_created_setter_reaches_the_launch_and_a_running_setter_goes_to_the_worker() {
    let mut w = World::default();
    w.engine.features.names.insert(Feature::SizePolicyOther);
    w.ok(create("s1"));
    let new = with_size(33, 101, None);
    w.engine
        .begin(Op::Resize {
            session: sid("s1"),
            size: new,
        })
        .unwrap();
    w.engine
        .begin(Op::SetColorProfile {
            session: sid("s1"),
            profile: profile(Some(256)),
        })
        .unwrap();
    w.engine
        .begin(Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Largest,
        })
        .unwrap();
    w.engine
        .begin(Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::None,
        })
        .unwrap();
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let spec = w
        .sent
        .iter()
        .find_map(|(_, m)| match m {
            HostMsg::Launch(spec) => Some(spec.clone()),
            _ => None,
        })
        .expect("the launch");
    assert_eq!(spec.size, new);
    assert_eq!(spec.color_profile, Some(profile(Some(256))));
    assert_eq!(spec.size_policy, SizePolicy::Largest);
    assert_eq!(spec.notification_policy, NotificationPolicy::None);
    // A running session: the same setters are sent to the worker as ops.
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.engine.features.names.insert(Feature::SizePolicyOther);
    w.ok(create("s1"));
    let start = w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    w.pump();
    let link = w.link_of_after_hello("s1");
    w.feed(Input::LinkMsg {
        link,
        msg: WorkerMsg::Launched {
            features: BTreeSet::from([Feature::NotificationPolicy, Feature::SizePolicyOther]),
            terminal: terminal_state(),
            formats: vec![],
            payload: botster_core_link::msg::PayloadId {
                pid: 900,
                start_time: 3,
            },
        },
    });
    w.complete(start);
    let before = w.sent.len();
    for op in [
        Op::SetColorProfile {
            session: sid("s1"),
            profile: profile(None),
        },
        Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Smallest,
        },
        Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::None,
        },
        Op::Resize {
            session: sid("s1"),
            size: with_size(12, 34, None),
        },
    ] {
        w.engine.begin(op).unwrap();
    }
    w.pump();
    let ops = w.sent[before..]
        .iter()
        .filter(|(_, m)| matches!(m, HostMsg::Op { .. }))
        .count();
    assert_eq!(
        ops, 4,
        "each setter of a running session goes to the worker"
    );
}

/// Core A2-1 (`cancel`): a write that was sent is cancelled by the worker (`HostMsg::Cancel`) and stays pending until the
/// worker answers; one that was not sent completes `Cancelled` at once.
#[test]
fn a_sent_write_is_cancelled_through_the_worker() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let op = w
        .engine
        .begin(write(InputPayload::Focus { focused: true }))
        .unwrap();
    w.pump();
    assert_eq!(w.engine.cancel(op), CancelResult::Admitted);
    w.pump();
    assert!(w
        .sent
        .iter()
        .any(|(_, m)| matches!(m, HostMsg::Cancel { .. })));
    w.pump();
    assert!(
        w.engine
            .poll_events(64)
            .iter()
            .all(|e| !matches!(e, Event::Completed { .. })),
        "the worker has not answered"
    );
}

/// Core A2-1 (`Stop` of an ended session): the end is `Lost(reason)` for a lost session and `Exited` for an exited one.
#[test]
fn stop_of_a_lost_session_reports_the_lost_reason() {
    let mut w = World::default();
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    // The worker process ends with no `Exited` report: the session is lost.
    w.exited("s1");
    w.pump();
    w.engine.poll_events(64);
    assert!(matches!(
        w.engine.get(&sid("s1")).unwrap().state,
        SessionState::Lost(LostReason::WorkerGone)
    ));
    let stop = w.engine.begin(Op::Stop { id: sid("s1") }).unwrap();
    assert!(matches!(
        w.complete(stop),
        OpResult::Ok(OpOutput::End(SessionEnd::Lost(LostReason::WorkerGone)))
    ));
}

fn attach_options() -> AttachOptions {
    AttachOptions {
        file_directory: "/tmp".into(),
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
        connect_deadline: None,
        owner: None,
        query_deadline: Some(Duration::from_secs(1)),
        route_tag: None,
        route_limits: None,
        history: None,
        stall_deadline: None,
        answers_queries: true,
        input: true,
    }
}

fn attach_with(
    w: &mut World,
    change: impl FnOnce(&mut AttachOptions),
) -> Result<AttachResult, ErrorCode> {
    let mut options = attach_options();
    change(&mut options);
    w.engine
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(())),
            options,
        )
        .map_err(|refused| refused.error.code)
}

/// Core OU-1, A9-1, A7-1: the route limits that a client chooses are checked at their exact bounds, and the limits that
/// apply are the choices, or the limits of the engine.
#[test]
fn attach_checks_each_route_limit_at_its_bound() {
    let mut w = World::new(limits(|l| {
        l.max_route_tag_bytes = 4;
        l.max_query_deadline = Duration::from_secs(5);
        l.routes_per_session = 64;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let cap = w.engine.limits().effective_max_route_frame_bytes();
    let floor = w.engine.limits().max_snapshot_bytes + 1;
    let invalid = ErrorCode::InvalidInput;
    // A tag and an owner of exactly the bound are valid; one byte more is not.
    assert!(attach_with(&mut w, |o| o.route_tag = Some("abcd".into())).is_ok());
    assert_eq!(
        attach_with(&mut w, |o| o.route_tag = Some("abcde".into())).unwrap_err(),
        invalid
    );
    assert!(attach_with(&mut w, |o| o.owner = Some("abcd".into())).is_ok());
    assert_eq!(
        attach_with(&mut w, |o| o.owner = Some("abcde".into())).unwrap_err(),
        invalid
    );
    // The frame cap: from 1 to the engine's cap.
    let choose = |frame: Option<u64>, screen: Option<u64>| {
        move |o: &mut AttachOptions| {
            o.route_limits = Some(RouteLimitChoices {
                max_frame_bytes: frame,
                max_screen_frame_bytes: screen,
                max_history_page_bytes: None,
                max_chunk_bytes: None,
            })
        }
    };
    assert!(attach_with(&mut w, choose(Some(cap), None)).is_ok());
    assert!(attach_with(&mut w, choose(Some(1), None)).is_ok());
    assert_eq!(
        attach_with(&mut w, choose(Some(cap + 1), None)).unwrap_err(),
        invalid
    );
    assert_eq!(
        attach_with(&mut w, choose(Some(0), None)).unwrap_err(),
        invalid
    );
    assert!(attach_with(&mut w, choose(None, Some(floor))).is_ok());
    assert_eq!(
        attach_with(&mut w, choose(None, Some(floor - 1))).unwrap_err(),
        invalid
    );
    // The query deadline: from 1 ms to the engine's bound, and required when the route answers queries.
    for (deadline, ok) in [
        (Some(Duration::from_millis(1)), true),
        (Some(Duration::from_secs(5)), true),
        (Some(Duration::from_micros(999)), false),
        (Some(Duration::from_millis(5001)), false),
        (None, false),
    ] {
        let result = attach_with(&mut w, |o| o.query_deadline = deadline);
        assert_eq!(result.is_ok(), ok, "{deadline:?}");
    }
    let result = attach_with(&mut w, |o| {
        o.answers_queries = false;
        o.query_deadline = None;
    })
    .unwrap();
    assert_eq!(result.limits.query_deadline, Duration::ZERO);
    // The applied limits: the choices, else the limits of the engine.
    let limits = w.engine.limits();
    let default = attach_with(&mut w, |_| {}).unwrap().limits;
    assert_eq!(default.max_frame_bytes, cap);
    assert_eq!(default.max_screen_frame_bytes, floor);
    assert_eq!(
        default.max_history_page_bytes,
        limits.max_history_page_bytes
    );
    assert_eq!(default.max_chunk_bytes, limits.default_route_chunk_bytes);
    assert_eq!(default.max_paste_bytes, limits.max_paste_bytes);
    assert_eq!(default.max_query_bytes, limits.max_query_bytes);
    assert_eq!(default.max_query_reply_bytes, limits.max_query_reply_bytes);
    assert_eq!(default.max_file_bytes, limits.max_file_bytes);
    assert_eq!(default.stall_deadline, limits.reader_progress_deadline);
    assert_eq!(default.stall_close_after, limits.stall_close_after);
    assert_eq!(
        default.route_input_queue_bytes,
        limits.route_input_queue_bytes
    );
    let chosen = attach_with(&mut w, |o| {
        o.stall_deadline = Some(Duration::from_secs(3));
        o.route_limits = Some(RouteLimitChoices {
            max_frame_bytes: Some(1000),
            max_screen_frame_bytes: Some(floor + 5),
            max_history_page_bytes: Some(7000),
            max_chunk_bytes: Some(2000),
        });
    })
    .unwrap()
    .limits;
    assert_eq!(chosen.max_frame_bytes, 1000);
    assert_eq!(chosen.max_screen_frame_bytes, floor + 5);
    assert_eq!(chosen.max_history_page_bytes, 7000);
    assert_eq!(chosen.max_chunk_bytes, 2000);
    assert_eq!(chosen.stall_deadline, Duration::from_secs(3));
}

/// Core ST-6, A8-1: the capture bounds are exact: the open captures of an owner, and the held bytes with `max_snapshot_bytes`
/// reserved for each capture that is not polled.
#[test]
fn capture_capacity_is_exact() {
    let mut w = World::new(limits(|l| {
        l.max_snapshot_bytes = 100;
        l.snapshot_retained_bytes = 200;
        l.open_captures_per_client = 8;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    let capture = |w: &mut World, owner: &str| {
        w.engine.begin(Op::CaptureSnapshot {
            session: sid("s1"),
            owner: ClientId(owner.into()),
        })
    };
    capture(&mut w, "a").unwrap();
    capture(&mut w, "a").unwrap();
    assert_eq!(
        capture(&mut w, "a").unwrap_err().code,
        ErrorCode::CaptureLimit,
        "2 x 100 fill 200"
    );
    // The owner bound.
    let mut w = World::new(limits(|l| {
        l.open_captures_per_client = 2;
        l.snapshot_retained_bytes = 1 << 30;
    }));
    w.autopilot = Autopilot::Silent;
    w.running("s1");
    capture(&mut w, "a").unwrap();
    capture(&mut w, "a").unwrap();
    assert_eq!(
        capture(&mut w, "a").unwrap_err().code,
        ErrorCode::CaptureLimit
    );
    assert!(capture(&mut w, "b").is_ok(), "the bound is per owner");
}
