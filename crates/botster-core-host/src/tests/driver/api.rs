//! `CoreApi` through the driver: every call reaches the engine and returns what it holds (plan 2.1, Core 2).

use super::*;
use botster_core_contract::prelude::CoreApi;
use botster_route_codec::prelude::QueryKind;

fn rig_with(change: impl FnOnce(&mut crate::EngineConfig)) -> Rig {
    let mut cfg = config(CoreLimits::default());
    change(&mut cfg);
    Rig::with_config(cfg, Box::new(Production::new()))
}

/// Core 2, 9B, AD-4, A6-2: the constants of the engine come back through the driver: the features, the limits, the worker
/// protocol, the adoptable protocols and their compatibility, the shadow kinds and the terminal identity.
#[test]
fn the_constants_come_back_through_the_driver() {
    let rig = rig_with(|cfg| {
        cfg.worker_protocol = 3;
        cfg.limits.max_sessions = 17;
        cfg.shadow_answerable = vec![QueryKind::CellPixels];
        cfg.features.names.insert(Feature::SizePolicyOther);
        cfg.terminal_identity.term = "xterm-test".into();
    });
    let api = &rig.driver;
    assert_eq!(api.worker_protocol(), 3);
    assert_eq!(api.adoptable_worker_protocols(), BTreeSet::from([2, 3]));
    assert_eq!(api.limits().max_sessions, 17);
    assert!(api.features().names.contains(&Feature::SizePolicyOther));
    assert_eq!(api.shadow_answerable_kinds(), vec![QueryKind::CellPixels]);
    assert_eq!(api.terminal_identity().term, "xterm-test");
    assert_eq!(
        api.worker_protocol_compatibility(Some(2)),
        WorkerCompatibility::Compatible
    );
    assert_eq!(
        api.worker_protocol_compatibility(Some(1)),
        WorkerCompatibility::Incompatible
    );
    assert_eq!(
        api.worker_protocol_compatibility(None),
        WorkerCompatibility::Unknown
    );
    // T = 1 has no T - 1.
    let rig = rig_with(|cfg| cfg.worker_protocol = 1);
    assert_eq!(rig.driver.adoptable_worker_protocols(), BTreeSet::from([1]));
}

/// Core 2, LC-9: the reads see the sessions that exist; a service call has no service to find; the deadline shows when a
/// silence threshold is set.
#[test]
fn the_reads_and_the_service_calls_answer_through_the_driver() {
    let mut rig = Rig::new(limits(|l| l.max_sessions = 4));
    run_session_plain(&mut rig, "s1", LinkId(1));
    assert_eq!(rig.driver.list().len(), 1);
    assert_eq!(rig.driver.status().sessions.len(), 1);
    assert_eq!(rig.driver.status().sessions[0].state, SessionState::Running);
    assert_eq!(rig.driver.diagnostics()["sessions"], 1);
    assert_eq!(rig.driver.next_deadline(), None);
    rig.worker_says(
        LinkId(1),
        WorkerMsg::Observed {
            observation: Observation::Output {
                model_rev: ModelRev(2),
            },
        },
    );
    rig.pump();
    rig.driver
        .set_silence_threshold(&sid("s1"), Some(Duration::from_secs(4)))
        .unwrap();
    assert_eq!(
        rig.driver.next_deadline(),
        Some(rig.now + Duration::from_secs(4))
    );
    let id = ServiceId([0; 32]);
    assert_eq!(
        rig.driver
            .service_send(
                &id,
                0,
                &OutboundFrame {
                    frame_type: 0,
                    payload: botster_route_codec::prelude::HexBytes(vec![]),
                }
            )
            .unwrap_err(),
        SendError::UnknownService
    );
    assert_eq!(
        rig.driver.service_recv(&id, 0).unwrap_err(),
        RecvError::UnknownService
    );
    assert_eq!(
        rig.driver.service_report(&id).unwrap_err().code,
        ErrorCode::UnknownService
    );
    assert_eq!(
        rig.driver.service_log_tail(&id, 10).unwrap_err().code,
        ErrorCode::UnknownService
    );
}

fn run_session_plain(rig: &mut Rig, name: &str, link: LinkId) {
    start_session(rig, name, link);
    rig.drain_events();
}

/// Core ST-6, ST-6a: a capture is read in pages and released by its id, or with all the captures of its owner; the
/// formats that the worker reported are listed.
#[test]
fn captures_are_released_through_the_driver_and_formats_are_listed() {
    let mut rig = Rig::new(CoreLimits::default());
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    let capture = |rig: &mut Rig, owner: &str| {
        let op = rig
            .driver
            .begin(Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId(owner.into()),
            })
            .unwrap();
        rig.pump();
        let frames = rig.host_frames(LinkId(1));
        let req = frames
            .iter()
            .rev()
            .find_map(|(k, p)| match HostMsg::decode(p) {
                Ok(HostMsg::Op {
                    req,
                    op: Op::CaptureSnapshot { .. },
                }) if *k == FrameType::HOST_MSG => Some(req),
                _ => None,
            })
            .expect("the capture went to the worker");
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Pages {
                req,
                pages: vec![Page {
                    index: 0,
                    last: true,
                    bytes: botster_route_codec::prelude::HexBytes(vec![1, 2, 3]),
                }],
            },
        );
        rig.worker_says(
            LinkId(1),
            WorkerMsg::Done {
                req,
                result: OpResult::Ok(OpOutput::Capture(Capture {
                    capture: CaptureId(0),
                    page_count: 1,
                    total_bytes: 3,
                    model_rev: ModelRev(2),
                })),
            },
        );
        rig.pump();
        let events = rig.drain_events();
        events
            .into_iter()
            .find_map(|e| match e {
                Event::Completed {
                    op: o,
                    result: OpResult::Ok(OpOutput::Capture(c)),
                } if o == op => Some(c.capture),
                _ => None,
            })
            .expect("the capture completed")
    };
    let a = capture(&mut rig, "x");
    let b = capture(&mut rig, "x");
    let c = capture(&mut rig, "y");
    assert!(rig.driver.read_page(a, 0).is_ok());
    rig.driver.release(a);
    assert!(rig.driver.read_page(a, 0).is_err(), "released");
    assert!(rig.driver.read_page(b, 0).is_ok());
    rig.driver.release_owner(&ClientId("x".into()));
    assert!(rig.driver.read_page(b, 0).is_err(), "the owner released");
    assert!(
        rig.driver.read_page(c, 0).is_ok(),
        "another owner keeps its capture"
    );
    assert!(rig.driver.snapshot_formats(&sid("s1")).unwrap().is_empty());
    assert_eq!(
        rig.driver.snapshot_formats(&sid("nope")).unwrap_err().code,
        ErrorCode::UnknownSession
    );
}

/// Plan 2.5: an interrupted `send` or `recv` is tried again; a call that would block waits for the readiness event and keeps
/// the link; any other error ends the link.
#[test]
fn io_errors_are_told_apart_by_their_kind() {
    // An interrupted send is retried: the whole frame arrives and the link stays open.
    let mut rig = Rig::new(CoreLimits::default());
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    let sent_before = rig.mock.lock().unwrap().links[&LinkId(1)].from_host.len();
    rig.mock
        .lock()
        .unwrap()
        .links
        .get_mut(&LinkId(1))
        .unwrap()
        .fail_send
        .push(io::ErrorKind::Interrupted);
    rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    for _ in 0..4 {
        rig.pump();
    }
    {
        let mock = rig.mock.lock().unwrap();
        let link = &mock.links[&LinkId(1)];
        assert!(
            link.from_host.len() > sent_before,
            "the retry sent the frame"
        );
        assert!(!link.closed_by_host);
    }
    // A send that would block keeps the link, and write interest stays on.
    let mut rig = Rig::new(CoreLimits::default());
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    rig.mock
        .lock()
        .unwrap()
        .links
        .get_mut(&LinkId(1))
        .unwrap()
        .fail_send
        .push(io::ErrorKind::WouldBlock);
    rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    for _ in 0..4 {
        rig.pump();
    }
    {
        let mock = rig.mock.lock().unwrap();
        assert!(
            !mock.links[&LinkId(1)].closed_by_host,
            "a blocked send keeps the link"
        );
    }
    // Any other send error ends the link.
    let mut rig = Rig::new(CoreLimits::default());
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    rig.mock
        .lock()
        .unwrap()
        .links
        .get_mut(&LinkId(1))
        .unwrap()
        .fail_send
        .push(io::ErrorKind::BrokenPipe);
    rig.driver.begin(Op::Stop { id: sid("s1") }).unwrap();
    for _ in 0..4 {
        rig.pump();
    }
    assert!(rig.mock.lock().unwrap().links[&LinkId(1)].closed_by_host);
    // Receive: interrupted is retried, and the frame is read.
    let mut rig = Rig::new(CoreLimits::default());
    start_session(&mut rig, "s1", LinkId(1));
    rig.drain_events();
    rig.worker_says(
        LinkId(1),
        WorkerMsg::Observed {
            observation: Observation::Output {
                model_rev: ModelRev(2),
            },
        },
    );
    rig.mock
        .lock()
        .unwrap()
        .links
        .get_mut(&LinkId(1))
        .unwrap()
        .fail_recv
        .push(io::ErrorKind::Interrupted);
    rig.pump();
    assert!(rig
        .drain_events()
        .iter()
        .any(|e| matches!(e, Event::Activity { .. })));
    assert!(!rig.mock.lock().unwrap().links[&LinkId(1)].closed_by_host);
    // Receive: a call that would block keeps the link; any other error ends it.
    for (kind, closed) in [
        (io::ErrorKind::WouldBlock, false),
        (io::ErrorKind::ConnectionReset, true),
    ] {
        let mut rig = Rig::new(CoreLimits::default());
        start_session(&mut rig, "s1", LinkId(1));
        rig.drain_events();
        rig.mock
            .lock()
            .unwrap()
            .links
            .get_mut(&LinkId(1))
            .unwrap()
            .fail_recv
            .push(kind);
        rig.pump();
        assert_eq!(
            rig.mock.lock().unwrap().links[&LinkId(1)].closed_by_host,
            closed,
            "{kind:?}"
        );
    }
}
