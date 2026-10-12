use super::*;
use botster_core_contract::prelude::*;
use botster_core_link::frame::{encode_frame, DEFAULT_MAX_PAYLOAD};
use botster_core_link::msg::PayloadId;
use botster_core_link::proof::{token_proof, TOKEN_LEN};
use std::collections::BTreeSet;

const LINK: LinkId = LinkId(1);
const WORKER: ProcessIdentity = ProcessIdentity {
    pid: 40,
    start_time: 7,
};

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

fn other(route: u64) -> Vec<u8> {
    worker(&WorkerMsg::RouteReceived {
        route: RouteId(route),
    })
}

fn exited() -> Vec<u8> {
    worker(&WorkerMsg::Exited {
        code: Some(0),
        signal: None,
    })
}

fn launched() -> Vec<u8> {
    worker(&WorkerMsg::Launched {
        features: BTreeSet::new(),
        terminal: TerminalState {
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            modes: ModeFlags::default(),
            title: None,
            cwd: None,
            last_output_at: None,
            focused: Some(false),
            model_rev: ModelRev(1),
            input_rev: InputRevs {
                client: InputRev(0),
                host: InputRev(0),
            },
        },
        formats: Vec::new(),
        payload: PayloadId {
            pid: 41,
            start_time: 8,
        },
    })
}

/// The inner edge of one link: the bytes that the worker wrote, read in chunks of at most `chunk`; then `fail` once, if set;
/// then `WouldBlock`, or `Ok(0)` once `closed`.
struct Inner {
    bytes: VecDeque<u8>,
    chunk: usize,
    closed: bool,
    fail: Option<io::ErrorKind>,
}

impl Inner {
    fn new(chunk: usize) -> Inner {
        Inner {
            bytes: VecDeque::new(),
            chunk,
            closed: false,
            fail: None,
        }
    }

    fn write(&mut self, bytes: &[u8]) {
        self.bytes.extend(bytes);
    }

    fn recv(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.bytes.is_empty() {
            if let Some(kind) = self.fail.take() {
                return Err(kind.into());
            }
            return if self.closed {
                Ok(0)
            } else {
                Err(io::ErrorKind::WouldBlock.into())
            };
        }
        let n = buf.len().min(self.chunk).min(self.bytes.len());
        for (slot, byte) in buf.iter_mut().zip(self.bytes.drain(..n)) {
            *slot = byte;
        }
        Ok(n)
    }
}

/// What the driver reads on `LINK` now: every byte up to `WouldBlock`, in reads of at most `ask` bytes. `Ok(0)` ends it too.
fn drain(
    holds: &mut ReportHolds,
    inner: &mut Inner,
    ask: usize,
) -> (Vec<u8>, Option<io::ErrorKind>) {
    let mut got = Vec::new();
    loop {
        let mut buf = vec![0u8; ask];
        match holds.recv(LINK, &mut buf, |b| inner.recv(b)) {
            Ok(0) => return (got, None),
            Ok(n) => got.extend_from_slice(&buf[..n]),
            Err(e) => return (got, Some(e.kind())),
        }
    }
}

fn would_block() -> Option<io::ErrorKind> {
    Some(io::ErrorKind::WouldBlock)
}

/// With no arm, every byte passes through as the inner edge gives it, and a read asks the inner edge for no more than the
/// driver asked for: the edge keeps nothing.
#[test]
fn with_no_arm_bytes_pass_through_and_nothing_is_kept() {
    let mut holds = ReportHolds::default();
    let mut inner = Inner::new(usize::MAX);
    let all = [hello(), other(1), exited()].concat();
    inner.write(&all);
    let mut buf = [0u8; 5];
    assert_eq!(holds.recv(LINK, &mut buf, |b| inner.recv(b)).unwrap(), 5);
    assert_eq!(buf[..], all[..5]);
    assert_eq!(
        inner.bytes.len(),
        all.len() - 5,
        "the edge read only what the driver asked for"
    );
    assert!(!holds.gives());
    let (rest, end) = drain(&mut holds, &mut inner, 3);
    assert_eq!([&all[..5], &rest[..]].concat(), all);
    assert_eq!(end, would_block());
    assert_eq!(holds.held_reports(&instance()), 0);
}

/// `hold_worker_report` (Core A12-1a): the worker's next report and every later byte wait, in order. The count is 0 while
/// armed and 1 while the report is held. The release gives the bytes out unchanged, and the next report is not held.
#[test]
fn a_held_report_waits_with_every_later_byte_until_the_release() {
    let mut holds = ReportHolds::default();
    let mut inner = Inner::new(7);
    inner.write(&hello());
    assert_eq!(drain(&mut holds, &mut inner, 64).0, hello());
    holds.hold_worker_report(instance()).unwrap();
    assert_eq!(
        holds.held_reports(&instance()),
        0,
        "armed, nothing held yet"
    );
    let later = [other(1), other(2), exited()].concat();
    inner.write(&later);
    assert_eq!(
        drain(&mut holds, &mut inner, 64),
        (Vec::new(), would_block()),
        "the report waits, and nothing after it goes out"
    );
    assert_eq!(holds.held_reports(&instance()), 1);
    assert!(
        inner.bytes.is_empty(),
        "the edge took the later bytes ahead"
    );
    assert!(!holds.gives(), "held bytes are not given");
    assert_eq!(holds.release_worker_report(&instance()), Ok(true));
    assert_eq!(holds.held_reports(&instance()), 0);
    assert_eq!(drain(&mut holds, &mut inner, 4).0, later);
    inner.write(&other(3));
    assert_eq!(
        drain(&mut holds, &mut inner, 64).0,
        other(3),
        "the hold ended"
    );
}

/// `hold_worker_report` before the hello: the link's instance is not known yet, so the edge reads frame by frame, and the
/// hello goes out. A partial frame goes out only once it is complete.
#[test]
fn an_arm_before_the_hello_holds_the_first_report_of_that_instance() {
    let mut holds = ReportHolds::default();
    holds.hold_worker_report(instance()).unwrap();
    let mut inner = Inner::new(3);
    let hello = hello();
    inner.write(&hello[..4]);
    assert_eq!(
        drain(&mut holds, &mut inner, 64),
        (Vec::new(), would_block())
    );
    inner.write(&[&hello[4..], &other(1)[..]].concat());
    assert_eq!(drain(&mut holds, &mut inner, 64).0, hello);
    assert_eq!(holds.held_reports(&instance()), 1);
}

/// A frame whose first byte the driver took before the arm is never held: the arm takes the next one.
#[test]
fn a_frame_already_begun_is_not_held() {
    let mut holds = ReportHolds::default();
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    let first = other(1);
    inner.write(&first);
    let mut buf = [0u8; 2];
    holds.recv(LINK, &mut buf, |b| inner.recv(b)).unwrap();
    holds.hold_worker_report(instance()).unwrap();
    inner.write(&other(2));
    assert_eq!(drain(&mut holds, &mut inner, 64).0, first[2..]);
    assert_eq!(holds.held_reports(&instance()), 1);
}

/// `scheduler_delay {point: exited, pumps: 2}` (Core A5-2, R-88 (E)): only `Exited` is held; the pump that holds it is pump
/// 1, and the report goes out at the start of pump 3.
#[test]
fn a_delay_holds_only_its_point_and_gives_it_out_at_the_start_of_pump_pumps_plus_one() {
    let mut holds = ReportHolds::default();
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    holds.delay(instance(), Point::Exited, 2).unwrap();
    inner.write(&[launched(), other(1), exited(), other(2)].concat());
    assert_eq!(
        drain(&mut holds, &mut inner, 64).0,
        [launched(), other(1)].concat()
    );
    assert_eq!(holds.held_reports(&instance()), 1);
    holds.pump_started();
    assert_eq!(
        drain(&mut holds, &mut inner, 64),
        (Vec::new(), would_block()),
        "pump 2"
    );
    holds.pump_started();
    assert_eq!(
        drain(&mut holds, &mut inner, 64).0,
        [exited(), other(2)].concat(),
        "pump 3"
    );
    assert_eq!(holds.held_reports(&instance()), 0);
}

/// `scheduler_delay {point: running}` holds the ready report, `Launched`.
#[test]
fn a_running_delay_holds_the_ready_report() {
    let mut holds = ReportHolds::default();
    holds.delay(instance(), Point::Running, 1).unwrap();
    let mut inner = Inner::new(usize::MAX);
    inner.write(&[hello(), other(1), launched()].concat());
    assert_eq!(
        drain(&mut holds, &mut inner, 64).0,
        [hello(), other(1)].concat()
    );
    assert_eq!(holds.held_reports(&instance()), 1);
    holds.pump_started();
    assert_eq!(drain(&mut holds, &mut inner, 64).0, launched());
}

/// Two delays of one session hold their reports one after the other: the second waits behind the first.
#[test]
fn two_delays_of_one_session_hold_in_turn() {
    let mut holds = ReportHolds::default();
    holds.delay(instance(), Point::Running, 1).unwrap();
    holds.delay(instance(), Point::Exited, 1).unwrap();
    let mut inner = Inner::new(usize::MAX);
    inner.write(&[hello(), launched(), exited()].concat());
    assert_eq!(drain(&mut holds, &mut inner, 64).0, hello());
    holds.pump_started();
    assert_eq!(drain(&mut holds, &mut inner, 64).0, launched());
    assert_eq!(
        holds.held_reports(&instance()),
        1,
        "the exit report is held now"
    );
    holds.pump_started();
    assert_eq!(drain(&mut holds, &mut inner, 64).0, exited());
}

/// One session never has holds of both controls (R-88 (E)): the second is refused `mixed_holds`, armed or holding; a second
/// `hold_worker_report` is `Bad`.
#[test]
fn holds_of_both_controls_on_one_session_are_refused() {
    let mixed = Err(ControlError::Refused(json!({"reason": "mixed_holds"})));
    let mut holds = ReportHolds::default();
    holds.hold_worker_report(instance()).unwrap();
    assert_eq!(holds.delay(instance(), Point::Exited, 1), mixed);
    assert!(matches!(
        holds.hold_worker_report(instance()),
        Err(ControlError::Bad(_))
    ));
    let other_session = InstanceId("2-1".into());
    assert_eq!(holds.delay(other_session.clone(), Point::Exited, 1), Ok(()));
    assert_eq!(holds.hold_worker_report(other_session), mixed);
    let mut inner = Inner::new(usize::MAX);
    inner.write(&[hello(), other(1)].concat());
    drain(&mut holds, &mut inner, 64);
    assert_eq!(holds.held_reports(&instance()), 1);
    assert_eq!(
        holds.delay(instance(), Point::Exited, 1),
        mixed,
        "a holding hold counts too"
    );
}

/// `release_worker_report` before the report came disarms; a release with no hold is `Bad` and gives nothing.
#[test]
fn a_release_disarms_and_a_release_with_no_hold_is_bad() {
    let mut holds = ReportHolds::default();
    assert!(matches!(
        holds.release_worker_report(&instance()),
        Err(ControlError::Bad(_))
    ));
    holds.hold_worker_report(instance()).unwrap();
    assert_eq!(holds.release_worker_report(&instance()), Ok(false));
    let mut inner = Inner::new(usize::MAX);
    inner.write(&[hello(), other(1)].concat());
    assert_eq!(
        drain(&mut holds, &mut inner, 64).0,
        [hello(), other(1)].concat()
    );
    holds.delay(instance(), Point::Exited, 1).unwrap();
    assert!(
        matches!(
            holds.release_worker_report(&instance()),
            Err(ControlError::Bad(_))
        ),
        "a delay is not released by release_worker_report"
    );
}

/// The end of the link waits behind a held report, and goes out after it; with no hold, a partial frame goes out before the
/// end.
#[test]
fn the_end_waits_behind_the_hold_and_a_partial_frame_goes_out_before_it() {
    let mut holds = ReportHolds::default();
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    holds.hold_worker_report(instance()).unwrap();
    inner.write(&other(1));
    inner.closed = true;
    assert_eq!(
        drain(&mut holds, &mut inner, 64),
        (Vec::new(), would_block())
    );
    holds.release_worker_report(&instance()).unwrap();
    assert_eq!(drain(&mut holds, &mut inner, 64), (other(1), None));

    let mut holds = ReportHolds::default();
    holds.hold_worker_report(InstanceId("9-9".into())).unwrap();
    let mut inner = Inner::new(usize::MAX);
    let partial = hello()[..5].to_vec();
    inner.write(&partial);
    inner.closed = true;
    assert_eq!(drain(&mut holds, &mut inner, 64), (partial, None));
}

/// The exit of a worker whose session has a hold waits until the hold ends, so Core never drains the link past a held
/// report; other exits pass, in the order of their reaps.
#[test]
fn a_held_sessions_worker_exit_waits_for_the_release() {
    let mut holds = ReportHolds::default();
    let stranger = ProcessIdentity {
        pid: 50,
        start_time: 1,
    };
    holds.spawned(WORKER, instance());
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    holds.hold_worker_report(instance()).unwrap();
    let mut reaped = VecDeque::from([
        (WORKER, ExitStatus::Code(0)),
        (stranger, ExitStatus::Code(1)),
    ]);
    assert_eq!(
        holds.poll_exit(|| reaped.pop_front()),
        Some((stranger, ExitStatus::Code(1)))
    );
    assert_eq!(holds.poll_exit(|| reaped.pop_front()), None);
    holds.release_worker_report(&instance()).unwrap();
    assert!(holds.gives());
    assert_eq!(
        holds.poll_exit(|| None),
        Some((WORKER, ExitStatus::Code(0)))
    );
}

/// `break_control` on a held session (Core A12-1a): the report stays unread; every byte the edge took and every later byte
/// is dropped, the end goes out, and the worker's exit is given.
#[test]
fn a_break_drops_the_held_report_and_every_later_byte() {
    let mut holds = ReportHolds::default();
    holds.spawned(WORKER, instance());
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    assert!(
        !holds.broken(&instance()),
        "no hold: the break is not the edge's"
    );
    holds.hold_worker_report(instance()).unwrap();
    inner.write(&[other(1), other(2)].concat());
    drain(&mut holds, &mut inner, 64);
    let mut reaped = VecDeque::from([(WORKER, ExitStatus::Code(0))]);
    assert_eq!(holds.poll_exit(|| reaped.pop_front()), None);
    assert!(holds.broken(&instance()));
    assert_eq!(holds.held_reports(&instance()), 0);
    inner.write(&other(3));
    inner.closed = true;
    assert_eq!(drain(&mut holds, &mut inner, 64), (Vec::new(), None));
    assert_eq!(
        holds.poll_exit(|| None),
        Some((WORKER, ExitStatus::Code(0)))
    );
}

/// The driver closed the link: its record goes, and a held session's exit is given.
#[test]
fn a_closed_link_frees_the_exit() {
    let mut holds = ReportHolds::default();
    holds.spawned(WORKER, instance());
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    holds.hold_worker_report(instance()).unwrap();
    let mut reaped = VecDeque::from([(WORKER, ExitStatus::Signal(9))]);
    assert_eq!(holds.poll_exit(|| reaped.pop_front()), None);
    holds.closed(LINK);
    assert_eq!(
        holds.poll_exit(|| None),
        Some((WORKER, ExitStatus::Signal(9)))
    );
}

/// With an arm for the link, one read of the driver takes every byte that the inner edge has, in chunks: the edge holds them
/// ahead, and gives the driver only what it asked for.
#[test]
fn an_armed_link_is_taken_ahead_in_one_call() {
    let mut holds = ReportHolds::default();
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    holds.delay(instance(), Point::Exited, 1).unwrap();
    let frames = [other(1), other(2)].concat();
    inner.write(&frames);
    let mut buf = [0u8; 2];
    assert_eq!(holds.recv(LINK, &mut buf, |b| inner.recv(b)).unwrap(), 2);
    assert!(inner.bytes.is_empty(), "the edge took every byte ahead");
    assert!(holds.gives());
    assert_eq!(
        [&buf[..], &drain(&mut holds, &mut inner, 64).0[..]].concat(),
        frames
    );
}

/// A read error of the inner edge keeps its place in the link's bytes: a partial frame taken ahead goes out first, then the
/// error.
#[test]
fn a_read_error_comes_after_the_bytes_before_it() {
    let mut holds = ReportHolds::default();
    holds.hold_worker_report(InstanceId("9-9".into())).unwrap();
    let mut inner = Inner::new(usize::MAX);
    let partial = hello()[..5].to_vec();
    inner.write(&partial);
    inner.fail = Some(io::ErrorKind::ConnectionReset);
    assert_eq!(
        drain(&mut holds, &mut inner, 64),
        (partial, Some(io::ErrorKind::ConnectionReset))
    );
}

/// An interrupted read of the inner edge is no end: the driver gets `Interrupted` at once, the partial frame stays ahead, and
/// the next call gives the frame whole.
#[test]
fn an_interrupted_read_returns_at_once_and_keeps_the_partial_frame() {
    let mut holds = ReportHolds::default();
    holds.hold_worker_report(InstanceId("9-9".into())).unwrap();
    let mut inner = Inner::new(usize::MAX);
    let hello = hello();
    inner.write(&hello[..5]);
    inner.fail = Some(io::ErrorKind::Interrupted);
    let mut buf = [0u8; 64];
    assert_eq!(
        holds
            .recv(LINK, &mut buf, |b| inner.recv(b))
            .map_err(|e| e.kind()),
        Err(io::ErrorKind::Interrupted)
    );
    inner.write(&hello[5..]);
    assert_eq!(drain(&mut holds, &mut inner, 64), (hello, would_block()));
}

/// A worker's exit waits only while its own session's link is open: an arm of a session with no link yet, beside another
/// session's open link, holds no exit.
#[test]
fn an_exit_waits_only_for_its_own_sessions_open_link() {
    let mut holds = ReportHolds::default();
    holds.spawned(WORKER, InstanceId("2-1".into()));
    holds.hold_worker_report(InstanceId("2-1".into())).unwrap();
    let mut inner = Inner::new(usize::MAX);
    inner.write(&hello());
    drain(&mut holds, &mut inner, 64);
    let mut reaped = VecDeque::from([(WORKER, ExitStatus::Code(0))]);
    assert_eq!(
        holds.poll_exit(|| reaped.pop_front()),
        Some((WORKER, ExitStatus::Code(0)))
    );
}
