use super::*;
use botster_core_contract::prelude::{InputRev, RouteCloseReason};
use botster_core_link::frame::{encode_frame, DEFAULT_MAX_PAYLOAD};
use botster_core_link::proof::{token_proof, TOKEN_LEN};

const LINK: LinkId = LinkId(1);
const R2: RouteId = RouteId(2);

fn instance() -> InstanceId {
    InstanceId("1-1".into())
}

fn frame(kind: FrameType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut out).unwrap();
    out
}

fn hello() -> Vec<u8> {
    let instance = instance();
    let hello = Hello {
        protocol: 1,
        proof: token_proof(&[3u8; TOKEN_LEN], &instance, 1),
        instance,
        host_epoch: 1,
    };
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    frame(FrameType::HELLO, &payload)
}

fn worker(msg: &WorkerMsg) -> Vec<u8> {
    let mut payload = Vec::new();
    msg.encode(&mut payload);
    frame(FrameType::WORKER_MSG, &payload)
}

fn activity(route: RouteId) -> Vec<u8> {
    worker(&WorkerMsg::Observed {
        observation: Observation::ClientInput {
            route,
            input_rev: InputRev(1),
        },
    })
}

fn close(route: RouteId) -> Vec<u8> {
    worker(&WorkerMsg::RouteClosed {
        route,
        reason: RouteCloseReason::Detached,
        route_tag: None,
    })
}

fn progress(route: RouteId, mark: u64, waiting: bool) -> Vec<u8> {
    worker(&WorkerMsg::RouteProgress {
        route,
        mark,
        waiting,
    })
}

fn other() -> Vec<u8> {
    worker(&WorkerMsg::RouteReceived { route: R2 })
}

/// The driver's calls on `LINK`: each call gives the bytes of one entry, after the call itself consumed what came before.
fn calls(reports: &mut RouteReports, chunks: &[Vec<u8>]) {
    for chunk in chunks {
        reports.call(LINK);
        reports.received(LINK, chunk);
    }
}

const NONE: Seen = Seen {
    activity_before_close: false,
    closed: false,
    progressed: false,
};
const BOTH: Seen = Seen {
    activity_before_close: true,
    closed: true,
    progressed: false,
};

/// The control's answer, and its wakes: one for each answer while a received report waits.
fn answer(reports: &RouteReports, wakes: &mut usize) -> Seen {
    reports.answer(&instance(), R2, || *wakes += 1)
}

/// R-66: a report is consumed at the driver's next call on its link, never at the call that received it. While it waits for
/// that call, the control wakes the host; after it, the control wakes nothing.
#[test]
fn a_report_is_consumed_at_the_next_call_on_its_link() {
    let mut reports = RouteReports::default();
    let mut wakes = 0;
    assert_eq!(answer(&reports, &mut wakes), NONE);
    assert_eq!(wakes, 0, "nothing received");
    calls(&mut reports, &[hello(), [activity(R2), close(R2)].concat()]);
    assert_eq!(answer(&reports, &mut wakes), NONE);
    assert_eq!(wakes, 1);
    assert!(reports.waits());
    reports.call(LinkId(9));
    assert_eq!(reports.seen(&instance(), R2), NONE, "another link's call");
    reports.call(LINK);
    assert_eq!(answer(&reports, &mut wakes), BOTH);
    assert_eq!(wakes, 1, "nothing waits");
    assert!(!reports.waits());
}

/// R-66: an activity after the close is not before it; a close alone has no activity.
#[test]
fn only_an_activity_consumed_before_the_close_counts() {
    let mut reports = RouteReports::default();
    calls(&mut reports, &[hello(), close(R2), activity(R2), other()]);
    reports.call(LINK);
    assert_eq!(
        reports.seen(&instance(), R2),
        Seen {
            activity_before_close: false,
            closed: true,
            progressed: false,
        }
    );
    let mut reports = RouteReports::default();
    calls(&mut reports, &[hello(), activity(R2), other()]);
    reports.call(LINK);
    assert_eq!(
        reports.seen(&instance(), R2),
        Seen {
            activity_before_close: true,
            closed: false,
            progressed: false,
        }
    );
    assert_eq!(reports.seen(&instance(), RouteId(3)), NONE);
    assert_eq!(reports.seen(&InstanceId("2-1".into()), R2), NONE);
}

/// A frame split over two reads is a report only when its last byte arrives.
#[test]
fn a_split_frame_is_received_with_its_last_byte() {
    let close = close(R2);
    let (head, tail) = close.split_at(close.len() - 1);
    let mut reports = RouteReports::default();
    calls(&mut reports, &[hello(), head.to_vec()]);
    assert!(!reports.waits());
    calls(&mut reports, &[tail.to_vec()]);
    assert!(reports.waits());
    assert_eq!(reports.seen(&instance(), R2), NONE);
    reports.call(LINK);
    assert!(reports.seen(&instance(), R2).closed);
}

/// The driver closes the link at a frame that it cannot read, so no later call consumes the reports before that frame.
#[test]
fn the_record_stops_where_the_driver_closes_the_link() {
    let not_hello = [close(R2)];
    let not_worker = [
        hello(),
        [close(R2), frame(FrameType::HELLO, b"{}")].concat(),
    ];
    let bad_msg = [
        hello(),
        [close(R2), frame(FrameType::WORKER_MSG, b"x")].concat(),
    ];
    for chunks in [&not_hello[..], &not_worker, &bad_msg] {
        let mut reports = RouteReports::default();
        calls(&mut reports, chunks);
        assert!(!reports.waits());
        calls(&mut reports, &[close(R2)]);
        reports.call(LINK);
        assert_eq!(reports.seen(&instance(), R2), NONE);
    }
}

/// A link that the driver closed consumes nothing more.
#[test]
fn a_closed_link_consumes_nothing() {
    let mut reports = RouteReports::default();
    calls(&mut reports, &[hello(), close(R2)]);
    reports.closed(LINK);
    assert!(!reports.waits());
    reports.call(LINK);
    assert_eq!(reports.seen(&instance(), R2), NONE);
}

fn progressed(reports: &RouteReports) -> bool {
    reports.seen(&instance(), R2).progressed
}

/// R-90 follow-up, Q2: `progressed` is a consumed progress report with a mark above every mark that the record received
/// before the route's latest `route_accept`. A report of the same mark (the route still waits, or newly waits) is not
/// progress; a mark that was received but not consumed at the accept is in the base.
#[test]
fn progressed_is_a_consumed_mark_above_the_marks_before_the_accept() {
    let mut reports = RouteReports::default();
    calls(&mut reports, &[hello(), progress(R2, 4, true)]);
    reports.call(LINK);
    assert!(!progressed(&reports), "no accept yet");
    // A received report that waits for the next call at the accept is in the base.
    calls(&mut reports, &[progress(R2, 5, true)]);
    reports.accepted(R2);
    reports.call(LINK);
    assert!(!progressed(&reports), "mark 5 came before the accept");
    calls(
        &mut reports,
        &[progress(R2, 5, false), progress(R2, 5, true)],
    );
    reports.call(LINK);
    assert!(!progressed(&reports), "the same mark is no progress");
    calls(&mut reports, &[progress(R2, 6, true)]);
    assert!(!progressed(&reports), "received, not consumed");
    reports.call(LINK);
    assert!(progressed(&reports));
    // A new accept starts again from the highest mark.
    reports.accepted(R2);
    assert!(!progressed(&reports));
    calls(
        &mut reports,
        &[worker(&WorkerMsg::RouteResumed { route: R2, mark: 7 })],
    );
    reports.call(LINK);
    assert!(progressed(&reports), "a resume reports progress");
    assert!(!reports.seen(&instance(), RouteId(3)).progressed);
}

/// With no mark of the route before the accept, no report proves progress after it.
#[test]
fn progressed_needs_a_mark_before_the_accept() {
    let mut reports = RouteReports::default();
    calls(&mut reports, &[hello()]);
    reports.accepted(R2);
    calls(&mut reports, &[progress(R2, 9, true)]);
    reports.call(LINK);
    assert!(!progressed(&reports));
}

/// The base of a route's accept holds only that route's marks: a higher mark of another route, received and not consumed at
/// the accept, is not in it.
#[test]
fn the_base_holds_only_the_routes_own_marks() {
    let mut reports = RouteReports::default();
    calls(&mut reports, &[hello(), progress(R2, 4, true)]);
    calls(&mut reports, &[progress(RouteId(3), 9, true)]);
    reports.accepted(R2);
    calls(&mut reports, &[progress(R2, 5, true)]);
    reports.call(LINK);
    assert!(progressed(&reports), "mark 5 is above the route's mark 4");
}
