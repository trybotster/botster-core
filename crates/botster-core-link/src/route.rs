//! What the host and the worker of a route agree on without a message (Core OU-1).

use botster_core_contract::prelude::{AppliedRouteLimits, RouteCloseReason, SnapshotFormat};
use botster_route_codec::prelude::{
    bound_of, stream_wrap, AttachFailedReason, CloseReason, Exit as WireExit, FrameBounds,
    RouteClosed, RouteEndReason, ToClient,
};

/// The `terminal_format` name of a snapshot format (OU-1, Codec TS-1): the lowercase format name, its version, and `+raw`
/// for raw `output` bytes, as in the codec's example `ghostsnp/2+raw`. The host refuses an attach with no common name, and
/// the worker announces the name that it picked, so both use this one function.
pub fn terminal_format(format: &SnapshotFormat) -> String {
    format!(
        "{}/{}+raw",
        format.name.to_ascii_lowercase(),
        format.version
    )
}

/// The `route_closed` frame that a client receives for a healthy close reason (Core A2-3, OU-2b), or `None` for a failed
/// reason, which has no frame (its cause is `route_ended_cause`). The worker writes this frame for a route that it holds;
/// the host writes it for a route that it still holds before the hand-over (steward ruling R-50). `SessionRemoved` and
/// `Other` have no wire reason in A2-3's table.
pub fn route_closed_frame(reason: RouteCloseReason) -> Option<RouteClosed> {
    let (reason, exit) = match reason {
        RouteCloseReason::Detached => (CloseReason::Detached, None),
        RouteCloseReason::Replaced => (CloseReason::Replaced, None),
        RouteCloseReason::Revoked => (CloseReason::Revoked, None),
        RouteCloseReason::SessionEnded { exit } => (
            CloseReason::SessionEnded,
            Some(WireExit {
                code: exit.code,
                signal: exit.signal,
            }),
        ),
        RouteCloseReason::BadFrame { code } => (CloseReason::ProtocolError { code }, None),
        RouteCloseReason::SnapshotTooLarge => (
            CloseReason::AttachFailed {
                reason: AttachFailedReason::SnapshotTooLarge,
            },
            None,
        ),
        _ => return None,
    };
    Some(RouteClosed { reason, exit })
}

/// The codec bounds of a route's frames under its applied limits (DP-3).
pub fn frame_bounds(limits: &AppliedRouteLimits) -> FrameBounds {
    FrameBounds {
        max_frame: limits.max_frame_bytes,
        max_screen: limits.max_screen_frame_bytes,
        max_history: limits.max_history_page_bytes,
    }
}

/// True when the encoded frame is within its bound on a route with these limits (DP-3, `bound_of`).
pub fn frame_fits(frame: &[u8], limits: &AppliedRouteLimits) -> bool {
    frame
        .first()
        .is_some_and(|&kind| frame.len() as u64 <= bound_of(kind, &frame_bounds(limits)))
}

/// The stream bytes of the `route_closed` frame for `reason` on a route with these limits: the frame of a healthy reason,
/// encoded and stream-wrapped (TB-L2), when it fits the route's bound. `None` for a failed reason, or when not even
/// `route_closed` fits; the transport then closes with no frame (OU-2b).
pub fn route_closed_bytes(
    reason: RouteCloseReason,
    limits: &AppliedRouteLimits,
) -> Option<Vec<u8>> {
    let frame = ToClient::RouteClosed(route_closed_frame(reason)?).encode();
    frame_fits(&frame, limits).then(|| stream_wrap(&frame))
}

/// The route-ended cause of a failed close reason (Core A2-3, OU-2b; Codec TS-9), or `None` for a healthy reason. A failed
/// route ends with no `route_closed` frame; a Hub tells its client this cause.
pub fn route_ended_cause(reason: RouteCloseReason) -> Option<RouteEndReason> {
    match reason {
        RouteCloseReason::HandoffFailed => Some(RouteEndReason::HandoffFailed),
        RouteCloseReason::WriteFailed => Some(RouteEndReason::WriteFailed),
        RouteCloseReason::StallTimeout => Some(RouteEndReason::Stalled),
        RouteCloseReason::SessionLost => Some(RouteEndReason::SessionLost),
        RouteCloseReason::PeerClosed => Some(RouteEndReason::TransportLost),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_format_name_is_lowercase_with_its_version_and_raw_output() {
        let format = SnapshotFormat {
            name: "GHOSTSNP".into(),
            version: 2,
        };
        assert_eq!(terminal_format(&format), "ghostsnp/2+raw");
    }

    /// The wire reasons of `conf::a2_3_every_route_close_reason_maps_to_one_wire_reason`: a healthy reason has its frame and
    /// no cause, a failed reason has its cause and no frame, and `SessionRemoved` has neither.
    #[test]
    fn each_close_reason_has_its_frame_or_its_cause() {
        use botster_core_contract::prelude::{Exit, ExitCause};
        use botster_route_codec::prelude::ProtocolErrorCode;
        let wire = |reason| serde_json::to_value(route_closed_frame(reason)).unwrap();
        let cause = |reason| serde_json::to_value(route_ended_cause(reason)).unwrap();
        let healthy = [
            (
                RouteCloseReason::Detached,
                serde_json::json!({"reason": "detached"}),
            ),
            (
                RouteCloseReason::Replaced,
                serde_json::json!({"reason": "replaced"}),
            ),
            (
                RouteCloseReason::Revoked,
                serde_json::json!({"reason": "revoked"}),
            ),
            (
                RouteCloseReason::BadFrame {
                    code: ProtocolErrorCode::UnknownFrame,
                },
                serde_json::json!({"reason": {"protocol_error": {"code": "unknown_frame"}}}),
            ),
            (
                RouteCloseReason::SessionEnded {
                    exit: Exit {
                        code: Some(3),
                        signal: None,
                        cause: ExitCause::Other,
                    },
                },
                serde_json::json!({"reason": "session_ended", "exit": {"code": 3}}),
            ),
            (
                RouteCloseReason::SnapshotTooLarge,
                serde_json::json!({"reason": {"attach_failed": {"reason": "snapshot_too_large"}}}),
            ),
        ];
        for (reason, frame) in healthy {
            assert_eq!(wire(reason), frame, "{reason:?}");
            assert_eq!(cause(reason), serde_json::Value::Null, "{reason:?}");
        }
        let failed = [
            (RouteCloseReason::HandoffFailed, "handoff_failed"),
            (RouteCloseReason::PeerClosed, "transport_lost"),
            (RouteCloseReason::WriteFailed, "write_failed"),
            (RouteCloseReason::StallTimeout, "stalled"),
            (RouteCloseReason::SessionLost, "session_lost"),
        ];
        for (reason, ended) in failed {
            assert_eq!(cause(reason), serde_json::json!(ended), "{reason:?}");
            assert_eq!(wire(reason), serde_json::Value::Null, "{reason:?}");
        }
        assert_eq!(
            wire(RouteCloseReason::SessionRemoved),
            serde_json::Value::Null
        );
        assert_eq!(
            cause(RouteCloseReason::SessionRemoved),
            serde_json::Value::Null
        );
    }

    fn limits(max_frame_bytes: u64) -> AppliedRouteLimits {
        use std::time::Duration;
        AppliedRouteLimits {
            max_frame_bytes,
            max_screen_frame_bytes: 1 << 20,
            max_history_page_bytes: 1 << 20,
            max_paste_bytes: 1,
            max_query_bytes: 1,
            max_query_reply_bytes: 1,
            max_file_bytes: 1,
            query_deadline: Duration::from_millis(1),
            stall_deadline: Duration::from_millis(1),
            stall_close_after: Duration::from_millis(1),
            route_input_queue_bytes: 1,
        }
    }

    /// R-50, DP-3, TB-L2: the bytes of a held route's close are one stream-wrapped `route_closed` frame of the healthy
    /// reason, which the codec's own reader reads back. A frame of exactly the bound fits; one byte less of bound gives no
    /// bytes, and so does a failed reason. An empty frame never fits.
    #[test]
    fn a_held_routes_close_bytes_are_its_wrapped_frame_within_the_bound() {
        use botster_route_codec::prelude::{Decoded, StreamReader};
        let reason = RouteCloseReason::Revoked;
        let frame = ToClient::RouteClosed(route_closed_frame(reason).unwrap()).encode();
        let exact = limits(frame.len() as u64);
        assert!(frame_fits(&frame, &exact));
        assert!(!frame_fits(&frame, &limits(frame.len() as u64 - 1)));
        assert!(!frame_fits(&[], &exact));
        let bytes = route_closed_bytes(reason, &exact).expect("the frame fits");
        let mut reader = StreamReader::new();
        reader.push(&bytes);
        let read = reader
            .next_frame(&frame_bounds(&exact))
            .unwrap()
            .expect("one whole frame");
        assert_eq!(reader.pending(), 0, "nothing after the frame");
        match ToClient::decode(&read, &frame_bounds(&exact)).unwrap() {
            Decoded::Frame(ToClient::RouteClosed(closed)) => assert_eq!(
                serde_json::to_value(closed).unwrap(),
                serde_json::json!({"reason": "revoked"})
            ),
            _ => panic!("not route_closed: {read:?}"),
        }
        assert_eq!(
            route_closed_bytes(reason, &limits(frame.len() as u64 - 1)),
            None
        );
        assert_eq!(
            route_closed_bytes(RouteCloseReason::SessionLost, &limits(1 << 20)),
            None
        );
    }
}
