//! Checks of the injected host edges (Core A5-1, A5-2, A5-3).

use super::*;

fn edges(seed: u64) -> SimEdges {
    SimEdges {
        registry: Arc::default(),
        faults: Arc::default(),
        entropy: SeededEntropy::with_seed(seed),
        scheduler: HandleScheduler(SchedulerHandle::with_seed(seed)),
        wake: Arc::default(),
        spawner: None,
        pending: VecDeque::new(),
        links: BTreeMap::new(),
        next_link: 1,
    }
}

/// AD-1 and A5-3: rows persist, prefix reads are ordered, and a failed write has no effect.
#[test]
fn rows_keep_their_bytes_and_failures_have_no_effect() {
    let mut edges = edges(1);
    edges.write_row("session/b", b"second").unwrap();
    edges.write_row("session/a", b"first").unwrap();
    edges.write_row("meta/a", b"other").unwrap();
    assert_eq!(
        edges.read_rows("session/").unwrap(),
        vec![
            ("session/a".into(), b"first".to_vec()),
            ("session/b".into(), b"second".to_vec())
        ]
    );
    let faults = edges.faults();
    let failure = StorageError::Failed { errno: 28 };
    lock(&faults).next_write = Some(failure);
    assert_eq!(edges.write_row("session/a", b"replacement"), Err(failure));
    assert_eq!(edges.read_rows("session/a").unwrap()[0].1, b"first");
    lock(&faults).next_write = Some(failure);
    assert_eq!(edges.delete_row("session/a"), Err(failure));
    assert_eq!(edges.read_rows("session/a").unwrap().len(), 1);
    edges.delete_row("session/a").unwrap();
    edges.delete_row("session/a").unwrap();
    assert!(edges.read_rows("session/a").unwrap().is_empty());
    edges.write_row("session/b", b"replacement").unwrap();
    assert_eq!(edges.read_rows("session/b").unwrap()[0].1, b"replacement");
}

/// A5-2: host entropy and scheduling use their seeded streams through the edge.
#[test]
fn entropy_and_choices_follow_the_seeded_streams() {
    let mut edges = edges(19);
    let mut entropy = SeededEntropy::with_seed(19);
    let mut expected = [0; 32];
    let mut actual = [0; 32];
    entropy.fill(&mut expected);
    edges.fill_random(&mut actual);
    assert_eq!(actual, expected);
    assert_ne!(actual, [0; 32]);
    let mut scheduler = crate::scheduler::SeededScheduler::with_seed(19);
    let mut picks = Vec::new();
    let mut bounds = Vec::new();
    for _ in 0..16 {
        let pick = edges.scheduler().pick(ChoicePoint::ReadyWork, 7);
        assert_eq!(pick, scheduler.pick(ChoicePoint::ReadyWork, 7));
        picks.push(pick);
        let bound = edges.scheduler().bound(ChoicePoint::PumpBound, 11);
        assert_eq!(bound, scheduler.bound(ChoicePoint::PumpBound, 11));
        bounds.push(bound);
    }
    assert!(picks.iter().any(|&n| n != 0));
    assert!(bounds.iter().any(|&n| n != 1));
}

/// TM-6 and TH-2: the wake is a level flag with no descriptor, and an unset wake waits for its deadline.
#[test]
#[allow(clippy::disallowed_methods)] // The real wake timeout needs the real clock.
fn the_wake_keeps_its_level_until_drained() {
    let wake = SimHostWake::default();
    assert_eq!(wake.fd(), -1);
    wake.signal();
    assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
    assert_eq!(wake.wait(Duration::ZERO), Wake::Woken);
    wake.drain();
    let timeout = Duration::from_millis(5);
    let start = Instant::now();
    // timer: deadline — an unset wake must wait until the caller's timeout expires.
    assert_eq!(wake.wait(timeout), Wake::TimedOut);
    assert!(start.elapsed() >= timeout);
}

/// DP-2 and plan 2.5: links forward bytes and descriptors, keep interests, and close at the peer.
#[test]
fn links_forward_bytes_descriptors_interests_and_close() {
    let mut edges = edges(2);
    let (host, mut peer) = crate::net::link_pair(32);
    edges.pending.push_back((LinkId(1), host));
    let link = edges.accept_link().unwrap();
    assert_eq!(edges.accept_link(), None);
    edges.set_read_interest(link, true);
    edges.set_write_interest(link, true);
    assert_eq!(
        edges.links.get_mut(&link).unwrap().end().interest(),
        crate::net::Interest {
            read: true,
            write: true
        }
    );
    edges.set_read_interest(link, false);
    edges.set_write_interest(link, false);
    assert_eq!(
        edges.links.get_mut(&link).unwrap().end().interest(),
        crate::net::Interest::default()
    );
    peer.send(b"request").unwrap();
    let mut bytes = [0; 32];
    let n = edges.link_recv(link, &mut bytes).unwrap();
    assert_eq!(&bytes[..n], b"request");
    edges.link_send(link, b"answer").unwrap();
    let n = peer.recv(&mut bytes).unwrap();
    assert_eq!(&bytes[..n], b"answer");
    let options = AttachOptions::default();
    edges
        .handoff_route(link, RouteId(1), StreamEndpoint::new(71u64), &options)
        .unwrap();
    let descriptor = peer.recv_descriptor().unwrap();
    let transport = descriptor.downcast::<StreamEndpoint>().unwrap();
    assert_eq!(transport.downcast::<u64>().unwrap(), 71);
    assert!(edges
        .handoff_route(LinkId(99), RouteId(1), StreamEndpoint::new(0u64), &options)
        .is_err());
    edges.link_close(link);
    assert_eq!(peer.recv(&mut bytes).unwrap(), 0);
    assert_eq!(
        peer.send(b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
    assert_eq!(edges.link_recv(link, &mut bytes).unwrap(), 0);
    assert_eq!(
        edges.link_send(link, b"x").unwrap_err().kind(),
        io::ErrorKind::BrokenPipe
    );
}

#[derive(Default)]
struct ProcessLog {
    peers: Vec<LinkEnd>,
    signals: Vec<(ProcessIdentity, GroupSignal)>,
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
}

struct RecordedSpawner(Arc<Mutex<ProcessLog>>);

impl Spawner for RecordedSpawner {
    fn spawn(
        &mut self,
        _spec: &WorkerSpawn,
        connect: &mut dyn FnMut() -> LinkEnd,
    ) -> Result<ProcessIdentity, SpawnError> {
        let mut log = lock(&self.0);
        log.peers.push(connect());
        Ok(ProcessIdentity {
            pid: log.peers.len() as u32,
            start_time: 1,
        })
    }

    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        lock(&self.0).signals.push((identity, signal));
    }

    fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        lock(&self.0).exits.pop_front()
    }
}

/// A5-1 and A5-3: the host edge forwards process events and assigns a distinct link to each spawn.
#[test]
fn each_spawn_has_its_own_link_and_process_events_reach_the_spawner() {
    let log = Arc::new(Mutex::new(ProcessLog::default()));
    let mut edges = edges(3).with_spawner(Box::new(RecordedSpawner(Arc::clone(&log))));
    let spec = WorkerSpawn {
        program: "worker".into(),
        instance: InstanceId("1-1".into()),
        token: [3; 32],
        host_epoch: 1,
    };
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(edges.spawn_worker(&spec).unwrap());
    }
    let links: Vec<_> = std::iter::from_fn(|| edges.accept_link()).collect();
    assert_eq!(links, vec![LinkId(1), LinkId(2), LinkId(3)]);
    for (index, &link) in links.iter().enumerate() {
        edges.link_send(link, &[index as u8]).unwrap();
        let mut byte = [0];
        assert_eq!(lock(&log).peers[index].recv(&mut byte).unwrap(), 1);
        assert_eq!(byte, [index as u8]);
    }
    edges.signal_group(ids[1], GroupSignal::EndPayload);
    assert_eq!(lock(&log).signals, vec![(ids[1], GroupSignal::EndPayload)]);
    lock(&log).exits.push_back((ids[1], ExitStatus::Signal(9)));
    assert_eq!(
        edges.poll_process_exit(),
        Some((ids[1], ExitStatus::Signal(9)))
    );
    assert_eq!(edges.poll_process_exit(), None);
}

/// LC-2, LC-12, and DP-8: dropping the host releases the directory, retains rows, and advances the epoch.
#[test]
#[allow(clippy::disallowed_methods)] // The test initializes the injected clock once.
fn reopen_retains_rows_and_advances_the_host_epoch() {
    let start = Instant::now();
    let run = || RunInputs {
        seed: 4,
        scheduler: SchedulerHandle::with_seed(4),
        start,
    };
    let config = OpenConfig {
        data_dir: "memory".into(),
        worker_path: Some("worker".into()),
        limits: CoreLimits::default(),
    };
    let mut dirs = Directories::default();
    let mut first = dirs
        .open("d", &config, run(), core_features(), None)
        .unwrap();
    first
        .driver
        .edges()
        .write_row("test/row", b"saved")
        .unwrap();
    assert_eq!(
        first.driver.edges().read_rows("meta/host-epoch").unwrap(),
        vec![("meta/host-epoch".into(), b"1".to_vec())]
    );
    assert_eq!(
        dirs.open("d", &config, run(), core_features(), None)
            .err()
            .unwrap()
            .code,
        ErrorCode::DataDirInUse
    );
    drop(first);
    let mut second = dirs
        .open("d", &config, run(), core_features(), None)
        .unwrap();
    assert_eq!(
        second.driver.edges().read_rows("test/").unwrap(),
        vec![("test/row".into(), b"saved".to_vec())]
    );
    assert_eq!(
        second.driver.edges().read_rows("meta/host-epoch").unwrap(),
        vec![("meta/host-epoch".into(), b"2".to_vec())]
    );
}

/// Core 2 and 9B: the testkit facade forwards constants and typed failures to the host driver.
#[test]
#[allow(clippy::disallowed_methods)] // The test initializes the injected clock once.
fn the_testkit_facade_forwards_configuration_and_typed_failures() {
    let start = Instant::now();
    let scheduler = SchedulerHandle::with_seed(5);
    let workers = crate::worker::Workers::new(scheduler, start);
    let mut limits = CoreLimits::default();
    limits.pump_events += 1;
    let features = core_features();
    assert_eq!(
        features.names,
        BTreeSet::from([Feature::Silence, Feature::NotificationPolicy])
    );
    assert_eq!(features.service_preamble_versions, vec![1]);
    let cfg = EngineConfig {
        limits: limits.clone(),
        features: features.clone(),
        host_epoch: 1,
        worker_path: "worker".into(),
        worker_protocol: 2,
        shadow_answerable: vec![botster_route_codec::prelude::QueryKind::WindowTitle],
        terminal_identity: TerminalIdentity {
            term: "configured".into(),
            terminfo_source: "source".into(),
        },
    };
    let edges = edges(5);
    let wake = edges.wake();
    let driver = HostDriver::new(cfg, edges, start);
    let mut core = crate::worker::TestkitCore::new(driver, wake, workers);
    assert_eq!(core.limits(), limits);
    assert_eq!(core.features(), features);
    assert_eq!(core.worker_protocol(), 2);
    assert_eq!(
        core.shadow_answerable_kinds(),
        vec![botster_route_codec::prelude::QueryKind::WindowTitle]
    );
    assert_eq!(core.diagnostics()["sessions"], 0);
    let session = SessionId("missing".into());
    assert_eq!(
        core.snapshot_formats(&session).unwrap_err().code,
        ErrorCode::UnknownSession
    );
    assert_eq!(
        core.set_silence_threshold(&session, None).unwrap_err().code,
        ErrorCode::UnknownSession
    );
    let service = ServiceId([0; 32]);
    let frame = OutboundFrame {
        frame_type: 0,
        payload: botster_route_codec::prelude::HexBytes(vec![]),
    };
    assert_eq!(
        core.service_send(&service, 0, &frame).unwrap_err(),
        SendError::UnknownService
    );
    assert_eq!(
        core.service_recv(&service, 0).unwrap_err(),
        RecvError::UnknownService
    );
    assert_eq!(
        core.service_report(&service).unwrap_err().code,
        ErrorCode::UnknownService
    );
    assert_eq!(
        core.service_log_tail(&service, 8).unwrap_err().code,
        ErrorCode::UnknownService
    );
    let session = SessionId("created".into());
    let op = core
        .begin(Op::Create {
            session: session.clone(),
            request: SpawnRequest {
                argv: vec!["program".into()],
                env: BTreeMap::new(),
                cwd: "/".into(),
                size: Size {
                    rows: 24,
                    cols: 80,
                    cell_px: None,
                },
                labels: BTreeMap::new(),
                color_profile: None,
                notification_policy: None,
                size_policy: None,
            },
        })
        .unwrap();
    let mut events = Vec::new();
    for _ in 0..128 {
        let report = core.pump(Now {
            monotonic: start,
            unix: 1_000_000,
        });
        events.extend(core.poll_events(64));
        if !report.more {
            break;
        }
    }
    assert!(events.iter().any(|event| matches!(event, Event::Completed { op: completed, result: OpResult::Ok(_) } if *completed == op)));
    assert_eq!(core.list().len(), 1);
    assert_eq!(core.list()[0].id, session);
    assert_eq!(core.status().sessions.len(), 1);
    assert_eq!(core.diagnostics()["sessions"], 1);
}
