//! The control queue's term for one host message with no input payload (IN-6, steward rulings R-91 and R-100; DESIGN.md "The
//! control queue"): the largest frame of the link. A message kind whose strings `CoreLimits` bounds has a worst case under
//! the limits that is at most the term. A kind with a string that no `CoreLimits` value bounds (R-100) is refused over the
//! frame bound, so it is never queued larger than the term either.

use super::*;
use crate::driver::largest_frame;
use botster_core_link::msg::LaunchSpec;

/// The frame of `msg` under `limits`, or `None` when the frame bound refuses it.
fn frame(limits: &CoreLimits, msg: &HostMsg) -> (Option<usize>, usize) {
    let rig = Rig::new(limits.clone());
    let bound = rig.driver.engine().link_frame_bound();
    let mut payload = Vec::new();
    msg.encode(&mut payload);
    let mut out = Vec::new();
    let len = encode_frame(FrameType::HOST_MSG, &payload, bound, &mut out)
        .ok()
        .map(|()| out.len());
    (len, largest_frame(bound))
}

/// `msg` at its worst case under the default limits is within the term.
fn within_term(msg: &HostMsg) {
    let (len, term) = frame(&CoreLimits::default(), msg);
    let len = len.expect("within the frame bound");
    assert!(len <= term, "{len} > {term}: {msg:?}");
}

/// Limits with a small frame bound (1 MiB + 4 × 1024 bytes), so a string over it is quick to build.
fn small() -> CoreLimits {
    limits(|l| {
        l.max_paste_bytes = 1024;
        l.max_snapshot_bytes = 1024;
    })
}

/// `msg` with a string over the frame bound of [`small`] is refused, and the term bounds every frame that is not.
fn refused_over_the_bound(msg: impl Fn(String) -> HostMsg) {
    let limits = small();
    let (len, term) = frame(&limits, &msg(String::new()));
    assert!(len.is_some_and(|n| n <= term));
    let (len, _) = frame(&limits, &msg("a".repeat(term)));
    assert_eq!(len, None, "a frame over the bound is never queued");
}

/// The longest session id that admission takes, of the byte that JSON escapes to the most bytes (`\u0001`, six).
fn worst_id() -> SessionId {
    SessionId("\u{1}".repeat(CoreLimits::default().max_session_id_bytes as usize))
}

fn op(op: Op) -> HostMsg {
    HostMsg::Op { req: u64::MAX, op }
}

#[test]
fn a_stop_frame_is_within_the_term() {
    within_term(&HostMsg::Stop);
}

#[test]
fn a_kill_frame_is_within_the_term() {
    within_term(&HostMsg::Kill);
}

#[test]
fn a_remove_frame_is_within_the_term() {
    within_term(&HostMsg::Remove);
}

#[test]
fn a_cancel_frame_is_within_the_term() {
    within_term(&HostMsg::Cancel { req: u64::MAX });
}

#[test]
fn a_detach_frame_is_within_the_term() {
    within_term(&HostMsg::Detach {
        route: RouteId(u64::MAX),
        reason: DetachReason::Revoked,
    });
}

/// Every op that the host forwards with no input payload and no unbounded string: its session id is bounded by
/// `max_session_id_bytes`, and its other fields are numbers, enums and a 256-colour palette.
#[test]
fn a_forwarded_op_frame_is_within_the_term() {
    let session = worst_id();
    let rgb = Rgb {
        r: 255,
        g: 255,
        b: 255,
    };
    let size = Size {
        rows: u32::MAX,
        cols: u32::MAX,
        cell_px: Some(CellPx {
            width: u32::MAX,
            height: u32::MAX,
        }),
    };
    for o in [
        Op::Signal {
            id: session.clone(),
            sig: Signal::Other(i32::MIN),
        },
        Op::SetSizePolicy {
            session: session.clone(),
            policy: SizePolicy::Smallest,
        },
        Op::SetColorProfile {
            session: session.clone(),
            profile: ColorProfile {
                palette: Some(vec![rgb; 256]),
                foreground: rgb,
                background: rgb,
                cursor: Some(rgb),
            },
        },
        Op::Resize {
            session: session.clone(),
            size,
        },
        Op::ReadScreen {
            session: session.clone(),
            history: true,
        },
        Op::ReadCursor {
            session: session.clone(),
        },
        Op::ReadModeFlags {
            session: session.clone(),
        },
        Op::ReadFacts {
            session: session.clone(),
            after: Some(InputCursor(u64::MAX)),
        },
        Op::SetOutputTap {
            session: session.clone(),
            enabled: true,
        },
    ] {
        within_term(&op(o));
    }
}

/// R-100: a launch's argv, env and cwd, and its notification patterns, have no `CoreLimits` bound.
#[test]
fn a_launch_over_the_frame_bound_is_refused() {
    refused_over_the_bound(|s| {
        HostMsg::Launch(Box::new(LaunchSpec {
            argv: vec![s],
            env: BTreeMap::new(),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            color_profile: None,
            notification_policy: NotificationPolicy::All,
            size_policy: SizePolicy::Latest,
            link_frame_bound: 0,
            stop_grace_ms: 0,
            limits: CoreLimits::default(),
        }))
    });
}

/// R-100: `SetNotificationPolicy`'s patterns have no `CoreLimits` bound.
#[test]
fn a_notification_policy_over_the_frame_bound_is_refused() {
    refused_over_the_bound(|s| {
        op(Op::SetNotificationPolicy {
            session: sid("s1"),
            policy: NotificationPolicy::Matching { patterns: vec![s] },
        })
    });
}

/// R-100: `CaptureSnapshot`'s owner has no `CoreLimits` bound.
#[test]
fn a_capture_over_the_frame_bound_is_refused() {
    refused_over_the_bound(|s| {
        op(Op::CaptureSnapshot {
            session: sid("s1"),
            owner: ClientId(s),
        })
    });
}

/// R-100: an attach's `file_directory`, `route_features` and `terminal_formats` have no `CoreLimits` bound.
#[test]
fn an_attach_route_over_the_frame_bound_is_refused() {
    refused_over_the_bound(|s| {
        let options: AttachOptions =
            serde_json::from_value(serde_json::json!({ "file_directory": format!("/{s}") }))
                .expect("attach options");
        HostMsg::AttachRoute {
            route: RouteId(1),
            options,
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
        }
    });
}
