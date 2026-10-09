//! DP-2 (worker-core DESIGN.md "P4a: the stream route", "The handoff"): the driver sends `HostMsg::AttachRoute` in the link's
//! ordered output, with the route's stream riding on the frame's first byte, and the limits that `attach` returned.

use super::*;
use botster_core_link::msg::HostMsg;

fn options(route_limits: Option<RouteLimitChoices>) -> AttachOptions {
    AttachOptions {
        file_directory: "/tmp".into(),
        file_permissions: None,
        route_features: vec![],
        terminal_formats: vec![],
        connect_deadline: None,
        owner: None,
        query_deadline: None,
        route_tag: None,
        route_limits,
        history: None,
        stall_deadline: None,
        answers_queries: false,
        input: true,
    }
}

/// Attaches a route to `s1` whose endpoint holds `token`.
fn attach(rig: &mut Rig, token: Arc<u32>) -> AttachResult {
    attach_with(rig, token, None)
}

fn attach_with(rig: &mut Rig, token: Arc<u32>, limits: Option<RouteLimitChoices>) -> AttachResult {
    rig.driver
        .attach(
            ClientId("c".into()),
            sid("s1"),
            RouteTransport::Stream(StreamEndpoint::new(token)),
            options(limits),
        )
        .unwrap()
}

fn with_link<R>(rig: &Rig, f: impl FnOnce(&mut MockLink) -> R) -> R {
    f(rig.mock.lock().unwrap().links.get_mut(&LinkId(1)).unwrap())
}

/// The frames that the host wrote on the link, with the offset of each: every byte belongs to exactly one whole frame.
fn frames(bytes: &[u8]) -> Vec<(usize, FrameType, Vec<u8>)> {
    let mut out = Vec::new();
    let mut at = 0;
    while at < bytes.len() {
        let len = u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap()) as usize;
        let kind = FrameType(bytes[at + 4]);
        out.push((at, kind, bytes[at + 5..at + 5 + len].to_vec()));
        at += 5 + len;
    }
    assert_eq!(at, bytes.len(), "the framing is intact");
    out
}

/// The offsets and messages of the `AttachRoute` frames on the link.
fn attach_frames(rig: &Rig) -> Vec<(usize, RouteId, AppliedRouteLimits)> {
    let bytes = with_link(rig, |l| l.from_host.clone());
    frames(&bytes)
        .into_iter()
        .filter(|(_, kind, _)| *kind == FrameType::HOST_MSG)
        .filter_map(|(at, _, payload)| match HostMsg::decode(&payload) {
            Ok(HostMsg::AttachRoute { route, limits, .. }) => Some((at, route, limits)),
            _ => None,
        })
        .collect()
}

/// The offsets of the descriptors on the link, with the token of each endpoint.
fn descriptors(rig: &Rig) -> Vec<(usize, u32)> {
    with_link(rig, |l| {
        std::mem::take(&mut l.descriptors)
            .into_iter()
            .map(|(at, endpoint)| (at, *endpoint.downcast::<Arc<u32>>().expect("a test token")))
            .collect()
    })
}

fn running() -> Rig {
    let mut rig = Rig::new(CoreLimits::default());
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    rig
}

/// The stream rides on the first byte of its `AttachRoute` frame, and the frame carries the limits that `attach` returned:
/// a host choice (`max_frame_bytes: 1024`) and the defaults the host applied, not a value the worker would compute.
#[test]
fn a_handoff_sends_attach_route_with_the_stream_on_its_first_byte_and_the_returned_limits() {
    let mut rig = running();
    let result = attach_with(
        &mut rig,
        Arc::new(7),
        Some(RouteLimitChoices {
            max_frame_bytes: Some(1024),
            ..RouteLimitChoices::default()
        }),
    );
    assert_eq!(result.limits.max_frame_bytes, 1024);
    rig.pump();
    let sent = attach_frames(&rig);
    assert_eq!(sent.len(), 1, "one AttachRoute frame");
    let (at, route, limits) = sent[0];
    assert_eq!(route, result.route);
    assert_eq!(
        limits, result.limits,
        "the worker gets the values that attach returned (OU-1)"
    );
    assert_eq!(
        descriptors(&rig),
        vec![(at, 7)],
        "the stream rides on the frame's first byte"
    );
}

/// A link that takes no byte at the mark gives the stream back: the frame stays unstarted, and the next write readiness sends
/// both once.
#[test]
fn a_blocked_handoff_keeps_the_stream_and_sends_it_once_later() {
    let mut rig = running();
    // A full socket buffer: every send is `WouldBlock`, which the mock reports as `Blocked` at the mark.
    with_link(&rig, |l| l.send_budget = Some(0));
    let token = Arc::new(3);
    attach(&mut rig, Arc::clone(&token));
    rig.pump();
    assert!(attach_frames(&rig).is_empty(), "no byte of the frame left");
    assert!(descriptors(&rig).is_empty());
    assert_eq!(Arc::strong_count(&token), 2, "the mark keeps the stream");
    assert!(with_link(&rig, |l| l.write_interest), "bytes wait");
    with_link(&rig, |l| l.send_budget = None);
    rig.pump();
    let sent = attach_frames(&rig);
    assert_eq!(sent.len(), 1, "the frame went once");
    assert_eq!(
        descriptors(&rig),
        vec![(sent[0].0, 3)],
        "the stream went once"
    );
    assert!(rig
        .drain_events()
        .iter()
        .all(|e| !matches!(e, Event::RouteClosed { .. })));
}

/// A short write at the mark sends the stream with the first bytes; the rest of the frame goes as plain bytes, once.
#[test]
fn a_short_write_at_the_mark_sends_the_rest_as_plain_bytes() {
    let mut rig = running();
    with_link(&rig, |l| l.send_cap = Some(3));
    attach(&mut rig, Arc::new(5));
    rig.pump();
    let sent = attach_frames(&rig);
    assert_eq!(sent.len(), 1);
    assert_eq!(
        descriptors(&rig),
        vec![(sent[0].0, 5)],
        "one stream, on the first byte"
    );
}

/// A frame that was partly written when the handoff was queued is completed first: the stream rides on the byte after it.
#[test]
fn an_earlier_partial_frame_is_complete_before_the_stream_rides() {
    let mut rig = running();
    let before = with_link(&rig, |l| l.from_host.len());
    with_link(&rig, |l| l.send_budget = Some(3));
    rig.driver
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    rig.pump();
    assert_eq!(
        with_link(&rig, |l| l.from_host.len()),
        before + 3,
        "the op frame is partly written"
    );
    attach(&mut rig, Arc::new(9));
    rig.pump();
    with_link(&rig, |l| l.send_budget = None);
    rig.pump();
    let bytes = with_link(&rig, |l| l.from_host.clone());
    let all = frames(&bytes);
    let op_end = all
        .iter()
        .find(|(at, _, _)| *at == before)
        .map(|(at, _, payload)| at + 5 + payload.len())
        .expect("the op frame");
    assert_eq!(descriptors(&rig), vec![(op_end, 9)]);
    assert_eq!(attach_frames(&rig)[0].0, op_end);
}

/// Two routes on one link: each stream rides on the first byte of its own frame, in order.
#[test]
fn repeated_handoffs_ride_on_their_own_frames_in_order() {
    let mut rig = running();
    let first = attach(&mut rig, Arc::new(1));
    let second = attach(&mut rig, Arc::new(2));
    rig.pump();
    let sent = attach_frames(&rig);
    assert_eq!(
        sent.iter().map(|(_, r, _)| *r).collect::<Vec<_>>(),
        vec![first.route, second.route]
    );
    assert_eq!(descriptors(&rig), vec![(sent[0].0, 1), (sent[1].0, 2)]);
}

/// A failed descriptor send takes nothing: the frame is dropped whole (the framing stays intact), the stream closes, and the
/// route closes `HandoffFailed` (DP-2, OU-2b).
#[test]
fn a_failed_handoff_drops_the_unstarted_frame_and_closes_handoff_failed() {
    let mut rig = running();
    with_link(&rig, |l| {
        l.fail_descriptor = vec![DescriptorSendError::Failed]
    });
    let token = Arc::new(4);
    let result = attach(&mut rig, Arc::clone(&token));
    rig.pump();
    rig.driver
        .begin(Op::ReadModeFlags { session: sid("s1") })
        .unwrap();
    rig.pump();
    assert!(
        attach_frames(&rig).is_empty(),
        "no byte of the frame was sent"
    );
    assert_eq!(Arc::strong_count(&token), 1, "the stream closed");
    let events = rig.drain_events();
    assert!(
        events.iter().any(|e| matches!(e, Event::RouteClosed { route, reason: RouteCloseReason::HandoffFailed, .. } if *route == result.route)),
        "{events:?}"
    );
}

/// A link that closes with a handoff still queued closes the stream, and the route is closed once.
#[test]
fn a_link_closed_with_a_handoff_pending_closes_the_stream_and_the_route() {
    let mut rig = running();
    with_link(&rig, |l| l.send_budget = Some(0));
    let token = Arc::new(6);
    let result = attach(&mut rig, Arc::clone(&token));
    rig.pump();
    assert_eq!(Arc::strong_count(&token), 2, "the mark holds the stream");
    with_link(&rig, |l| l.peer_closed = true);
    for _ in 0..4 {
        rig.pump();
    }
    assert_eq!(
        Arc::strong_count(&token),
        1,
        "the stream closed with the link"
    );
    let closes = rig
        .drain_events()
        .into_iter()
        .filter(|e| matches!(e, Event::RouteClosed { route, .. } if *route == result.route))
        .count();
    assert_eq!(closes, 1, "one close of the route");
}
