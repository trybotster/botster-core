//! AM-1, AM-3, AM-4, ER-0, ID-1, 9B and the sync column of A2-1.

use super::*;

fn code(result: Result<OpId, CoreError>) -> ErrorCode {
    result.expect_err("the op is refused").code
}

/// Core AM-1: the admission table changes at `begin`, in `begin` order, before any `pump`.
#[test]
fn create_then_start_in_one_turn_is_admitted_and_a_second_start_is_refused() {
    let mut w = World::default();
    w.engine.begin(create("s1")).unwrap();
    w.engine.begin(Op::Start { id: sid("s1") }).unwrap();
    assert_eq!(
        code(w.engine.begin(Op::Start { id: sid("s1") })),
        ErrorCode::WrongState
    );
    assert_eq!(
        code(w.engine.begin(Op::Remove { id: sid("s1") })),
        ErrorCode::WrongState,
        "Start then Remove before a pump"
    );
}

/// Core ER-0, AM-4: a sync refusal allocates no `OpId`, posts no event and takes no slot.
#[test]
fn a_sync_refusal_allocates_no_op_posts_no_event_and_takes_no_slot() {
    let mut w = World::new(limits(|l| l.pending_ops = 1));
    let first = w.engine.begin(create("s1")).unwrap();
    assert_eq!(
        code(w.engine.begin(create("s2"))),
        ErrorCode::PendingLimit,
        "the table is full"
    );
    assert_eq!(
        code(w.engine.begin(Op::Start { id: sid("nope") })),
        ErrorCode::UnknownSession
    );
    assert!(
        w.engine.poll_events(64).is_empty(),
        "a refusal posts nothing"
    );
    w.complete(first);
    let next = w.engine.begin(create("s2")).unwrap();
    assert_eq!(next.0, first.0 + 1, "no OpId was spent on a refusal");
}

/// Core EV-5a, AM-4: the slot of an op is held until the host polls its `Completed`, and not until the op finishes.
#[test]
fn the_slot_is_held_until_the_completion_is_polled() {
    let mut w = World::new(limits(|l| l.pending_ops = 1));
    w.engine.begin(create("s1")).unwrap();
    w.pump();
    assert_eq!(
        code(w.engine.begin(create("s2"))),
        ErrorCode::PendingLimit,
        "the op finished, and its Completed is not polled"
    );
    w.engine.poll_events(64);
    assert!(w.engine.begin(create("s2")).is_ok());
}

/// Core ID-1, 9.3: an id over `max_session_id_bytes` is refused, and an id of exactly that length is not.
#[test]
fn an_id_over_the_limit_is_id_too_long() {
    let mut w = World::new(limits(|l| l.max_session_id_bytes = 8));
    assert_eq!(
        code(w.engine.begin(create("123456789"))),
        ErrorCode::IdTooLong
    );
    assert!(w.engine.begin(create("12345678")).is_ok());
}

/// Core LC-3: a duplicate id is `IdInUse`, whether the first create finished or not.
#[test]
fn a_duplicate_id_is_id_in_use() {
    let mut w = World::default();
    w.engine.begin(create("s1")).unwrap();
    assert_eq!(code(w.engine.begin(create("s1"))), ErrorCode::IdInUse);
    w.pump();
    assert_eq!(code(w.engine.begin(create("s1"))), ErrorCode::IdInUse);
}

/// Core ER-0, A2-1: `Create` refuses an empty argv, a zero size and a relative cwd with `InvalidInput`, and creates nothing.
#[test]
fn a_malformed_create_is_invalid_input_and_creates_nothing() {
    let mut w = World::default();
    let mut bad = request();
    bad.argv.clear();
    let mut zero = request();
    zero.size.rows = 0;
    let mut wide = request();
    wide.size.cols = 65_536;
    let mut relative = request();
    relative.cwd = "work".into();
    for request in [bad, zero, wide, relative] {
        assert_eq!(
            code(w.engine.begin(Op::Create {
                session: sid("s1"),
                request
            })),
            ErrorCode::InvalidInput { field: None }
        );
    }
    assert_eq!(
        w.engine.get(&sid("s1")).unwrap_err().code,
        ErrorCode::UnknownSession
    );
    assert!(w.engine.begin(create("s1")).is_ok(), "the id is free");
}

/// Core ER-0, 9B: `max_sessions` bounds the rows, and a `Released` that the host polls frees a place.
#[test]
fn the_session_limit_resumes_after_released_is_polled() {
    let mut w = World::new(limits(|l| {
        l.max_sessions = 1;
        l.mandatory_events = 8;
    }));
    w.ok(create("s1"));
    assert_eq!(code(w.engine.begin(create("s2"))), ErrorCode::SessionLimit);
    w.ok(Op::Remove { id: sid("s1") });
    w.ok(create("s2"));
}

/// Core A2-1: every row refuses an unknown session with `UnknownSession`, and a read of a `Created` session is `WrongState`.
#[test]
fn rows_refuse_unknown_sessions_and_states_that_they_are_not_admitted_in() {
    let mut w = World::default();
    let nope = sid("nope");
    let ops = [
        Op::Start { id: nope.clone() },
        Op::Stop { id: nope.clone() },
        Op::Signal {
            id: nope.clone(),
            sig: Signal::Term,
        },
        Op::Remove { id: nope.clone() },
        Op::UpdateMetadata {
            id: nope.clone(),
            labels: BTreeMap::new(),
        },
        Op::ReadScreen {
            session: nope.clone(),
            history: false,
        },
        Op::ReadCursor {
            session: nope.clone(),
        },
        Op::ReadModeFlags {
            session: nope.clone(),
        },
        Op::ReadFacts {
            session: nope.clone(),
            after: None,
        },
        Op::Resize {
            session: nope.clone(),
            size: size(),
        },
        Op::SetOutputTap {
            session: nope.clone(),
            enabled: true,
        },
        Op::SetNotificationPolicy {
            session: nope.clone(),
            policy: NotificationPolicy::All,
        },
        Op::CaptureSnapshot {
            session: nope.clone(),
            owner: ClientId("c".into()),
        },
    ];
    for op in ops {
        assert_eq!(
            code(w.engine.begin(op.clone())),
            ErrorCode::UnknownSession,
            "{op:?}"
        );
    }
    assert_eq!(
        code(w.engine.begin(Op::Detach {
            route: RouteId(9),
            reason: DetachReason::Detached
        })),
        ErrorCode::UnknownRoute
    );
    w.ok(create("c"));
    for op in [
        Op::ReadScreen {
            session: sid("c"),
            history: false,
        },
        Op::ReadModeFlags { session: sid("c") },
        Op::ReadFacts {
            session: sid("c"),
            after: None,
        },
        Op::Stop { id: sid("c") },
        Op::Signal {
            id: sid("c"),
            sig: Signal::Term,
        },
        Op::SetOutputTap {
            session: sid("c"),
            enabled: true,
        },
    ] {
        assert_eq!(
            code(w.engine.begin(op.clone())),
            ErrorCode::WrongState,
            "{op:?}"
        );
    }
}

/// Core A2-6, A2-1: a size policy other than `Latest` needs the feature `size_policy_other`.
#[test]
fn a_policy_that_needs_a_missing_feature_is_unsupported() {
    let mut w = World::default();
    w.ok(create("s1"));
    assert_eq!(
        code(w.engine.begin(Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Smallest
        })),
        ErrorCode::Unsupported { what: None }
    );
    assert!(w
        .engine
        .begin(Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Latest
        })
        .is_ok());
    let mut w = World::offering(Feature::SizePolicyOther);
    w.ok(create("s1"));
    assert!(w
        .engine
        .begin(Op::SetSizePolicy {
            session: sid("s1"),
            policy: SizePolicy::Largest
        })
        .is_ok());
}

/// Core A2-1 (`Signal`): an unsupported signal number is `Unsupported`, and a signal of a `Created` session is `WrongState`.
#[test]
fn an_unsupported_signal_is_refused_at_begin() {
    let mut w = World::default();
    w.running("s1");
    assert_eq!(
        code(w.engine.begin(Op::Signal {
            id: sid("s1"),
            sig: Signal::Other(0)
        })),
        ErrorCode::Unsupported { what: None }
    );
    assert_eq!(
        code(w.engine.begin(Op::Signal {
            id: sid("s1"),
            sig: Signal::Other(64)
        })),
        ErrorCode::Unsupported { what: None }
    );
    assert!(w
        .engine
        .begin(Op::Signal {
            id: sid("s1"),
            sig: Signal::Other(10)
        })
        .is_ok());
}

/// Core IN-5, AM-4: a write over a lane bound is refused at `begin` with `LaneFull`, and the lane returns when the host polls
/// the completion, not when the write finishes.
#[test]
fn a_session_lane_is_held_until_the_completion_is_polled() {
    let mut w = World::new(limits(|l| l.input_ops_per_session = 1));
    w.running("s1");
    let write = |b: u8| Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Bytes {
            bytes: botster_route_codec::prelude::HexBytes(vec![b]),
        },
        guard: None,
    };
    w.engine.begin(write(1)).unwrap();
    w.pump();
    assert_eq!(
        w.engine.begin(write(2)).unwrap_err().code,
        ErrorCode::LaneFull,
        "finished, not polled"
    );
    w.engine.poll_events(64);
    assert!(w.engine.begin(write(2)).is_ok());
}

/// Core IN-5: a write over `max_paste_bytes` is `PayloadTooLarge`, and a repeat of 0 is `InvalidInput`.
#[test]
fn a_payload_over_the_paste_bound_is_payload_too_large_and_a_zero_repeat_is_invalid() {
    let mut w = World::new(limits(|l| l.max_paste_bytes = 1024));
    w.running("s1");
    let bytes = |n: usize| Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Bytes {
            bytes: botster_route_codec::prelude::HexBytes(vec![0; n]),
        },
        guard: None,
    };
    assert_eq!(
        code(w.engine.begin(bytes(1025))),
        ErrorCode::PayloadTooLarge
    );
    assert!(w.engine.begin(bytes(1024)).is_ok());
    let key = |repeat| Op::WriteInput {
        session: sid("s1"),
        payload: InputPayload::Key(KeyInput {
            key: botster_route_codec::prelude::Key::Char('a'.into()),
            shifted_key: None,
            base_layout_key: None,
            mods: vec![],
            event: botster_route_codec::prelude::KeyEvent::Press,
            text: None,
            repeat,
        }),
        guard: None,
    };
    assert_eq!(
        code(w.engine.begin(key(Some(0)))),
        ErrorCode::InvalidInput { field: None }
    );
}

/// Core A2-1 (`cancel`): only a `WriteInput` can be cancelled; a completed op is `TooLate`; an op that this handle never
/// minted is `UnknownOp`.
#[test]
fn cancel_has_a_result_for_every_kind_of_op() {
    let mut w = World::default();
    let create = w.engine.begin(create("s1")).unwrap();
    assert_eq!(
        w.engine.cancel(create),
        CancelResult::Refused(CancelRefusal::NotCancellable)
    );
    w.complete(create);
    assert_eq!(w.engine.cancel(create), CancelResult::TooLate);
    assert_eq!(w.engine.cancel(OpId(999)), CancelResult::UnknownOp);
}

/// Core A2-1: `AdoptAll` runs once per handle.
#[test]
fn a_second_adopt_all_is_wrong_state() {
    let mut w = World::default();
    w.engine.begin(Op::AdoptAll).unwrap();
    assert_eq!(code(w.engine.begin(Op::AdoptAll)), ErrorCode::WrongState);
}

/// Core A2-1: a `Created` session takes a resize at once, and `Applied` reports the size that was stored.
#[test]
fn a_resize_of_a_created_session_stores_the_spawn_size() {
    let mut w = World::default();
    w.ok(create("s1"));
    let new = Size {
        rows: 30,
        cols: 100,
        cell_px: None,
    };
    let out = w.ok(Op::Resize {
        session: sid("s1"),
        size: new,
    });
    assert_eq!(out, OpOutput::Resize(ResizeResult::Applied { actual: new }));
    assert_eq!(w.engine.get(&sid("s1")).unwrap().size, new);
    assert_eq!(
        code(w.engine.begin(Op::Resize {
            session: sid("s1"),
            size: Size {
                rows: 0,
                cols: 5,
                cell_px: None
            }
        })),
        ErrorCode::InvalidInput { field: None }
    );
}
