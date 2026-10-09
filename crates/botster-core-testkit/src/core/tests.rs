//! Checks of the injected host edges (Core A5-1, A5-2, A5-3).

use super::*;
use std::time::Instant;

fn edges(seed: u64) -> SimEdges {
    let scheduler = SchedulerHandle::with_seed(seed);
    SimEdges {
        registry: Arc::default(),
        faults: Arc::default(),
        entropy: SeededEntropy::with_seed(seed),
        wake: Arc::new(SimHostWake::new(scheduler.clone())),
        scheduler: HandleScheduler(scheduler),
        spawner: None,
        pending: VecDeque::new(),
        links: BTreeMap::new(),
        next_link: 1,
        link_capacity: LINK_CAPACITY,
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

/// TM-6 and TH-2: the wake is a level flag with no descriptor, and an unset wake with no spurious wakes waits for its
/// deadline.
#[test]
fn the_wake_keeps_its_level_until_drained() {
    let scheduler = SchedulerHandle::with_seed(0);
    scheduler.set_overrides(|o| o.no_spurious_wakes = true);
    let wake = SimHostWake::new(scheduler);
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
    assert_eq!(
        edges
            .link_send_descriptor(link, b"frame", StreamEndpoint::new(71u64))
            .unwrap(),
        5
    );
    let descriptor = peer.recv_descriptor().unwrap();
    let transport = descriptor.downcast::<StreamEndpoint>().unwrap();
    assert_eq!(transport.downcast::<u64>().unwrap(), 71);
    let n = peer.recv(&mut bytes).unwrap();
    assert_eq!(&bytes[..n], b"frame", "the descriptor rode on these bytes");
    let (back, why) = edges
        .link_send_descriptor(LinkId(99), b"f", StreamEndpoint::new(0u64))
        .unwrap_err();
    assert_eq!(why, DescriptorSendError::Failed, "no such link");
    assert_eq!(back.downcast::<u64>().unwrap(), 0, "the endpoint comes back");
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
    spawns: Vec<WorkerSpawn>,
    signals: Vec<(ProcessIdentity, GroupSignal)>,
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
    unlinks: Vec<InstanceId>,
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
        log.spawns.push(_spec.clone());
        Ok(ProcessIdentity {
            pid: log.peers.len() as u32,
            start_time: 1,
        })
    }

    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        lock(&self.0).signals.push((identity, signal));
    }

    /// The log only records: no process of it ever ends by itself.
    fn identity_state(&self, _identity: ProcessIdentity) -> IdentityState {
        IdentityState::Matches
    }

    fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        lock(&self.0).exits.pop_front()
    }

    /// The log's processes bind no endpoint: no worker listens there.
    fn connect_worker(&mut self, _instance: &InstanceId, _end: LinkEnd) -> bool {
        false
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        lock(&self.0).unlinks.push(instance.clone());
    }
}

/// A5-1 and A5-3: the host edge forwards process events and assigns a distinct link to each spawn.
#[test]
fn each_spawn_has_its_own_link_and_process_events_reach_the_spawner() {
    let log = Arc::new(Mutex::new(ProcessLog::default()));
    let mut edges = edges(3).with_spawner(Box::new(RecordedSpawner(Arc::clone(&log))));
    let spec = WorkerSpawn {
        startup: CoreLimits::default().startup,
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
    // DESIGN.md parts 1 and 6: a connect where no worker listens is no link, and the removal of an endpoint reaches the
    // spawner.
    assert_eq!(edges.connect_worker(&spec.instance), None);
    edges.remove_endpoint(&spec.instance);
    assert_eq!(lock(&log).unlinks, vec![spec.instance.clone()]);
}

/// LC-2, LC-12, and DP-8: dropping the host releases the directory, retains rows, and advances the epoch.
#[test]
fn reopen_retains_rows_and_advances_the_host_epoch() {
    let run = || RunInputs {
        seed: 4,
        scheduler: SchedulerHandle::with_seed(4),
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
    let driver = HostDriver::open(cfg, edges).unwrap();
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

fn pump_core(core: &mut crate::worker::TestkitCore, start: Instant) -> Vec<Event> {
    let mut events = Vec::new();
    for _ in 0..128 {
        let report = core.pump(Now {
            monotonic: start,
            unix: 1_000_000,
        });
        events.extend(core.poll_events(64));
        if !report.more {
            return events;
        }
    }
    panic!("the host did not settle");
}

fn say(log: &Arc<Mutex<ProcessLog>>, msg: &botster_core_link::msg::WorkerMsg) {
    let mut payload = Vec::new();
    msg.encode(&mut payload);
    let mut bytes = Vec::new();
    botster_core_link::frame::encode_frame(
        botster_core_link::frame::FrameType::WORKER_MSG,
        &payload,
        65536,
        &mut bytes,
    )
    .unwrap();
    assert_eq!(lock(log).peers[0].send(&bytes).unwrap(), bytes.len());
}

fn host_messages(log: &Arc<Mutex<ProcessLog>>) -> Vec<botster_core_link::msg::HostMsg> {
    use botster_core_link::frame::{FrameDecoder, FrameType};
    let mut bytes = [0; 65536];
    let n = lock(log).peers[0].recv(&mut bytes).unwrap();
    let mut decoder = FrameDecoder::new(65536);
    let mut rest = &bytes[..n];
    let mut messages = Vec::new();
    while !rest.is_empty() {
        let took = decoder.push(rest);
        rest = &rest[took..];
        while let Some(frame) = decoder.next_frame().unwrap() {
            if frame.kind == FrameType::HOST_MSG {
                messages.push(botster_core_link::msg::HostMsg::decode(&frame.payload).unwrap());
            }
        }
    }
    messages
}

/// ST-6a: the facade releases live captures by id and owner. Libghostty supplies every snapshot byte.
#[test]
fn the_testkit_facade_releases_captures_by_id_and_owner() {
    use botster_core_link::frame::{encode_frame, FrameType};
    use botster_core_link::hello::Hello;
    use botster_core_link::msg::{HostMsg, PayloadId, WorkerMsg};
    use botster_core_link::proof::token_proof;
    use botster_terminal_ghostty::{History, Terminal};

    let start = Instant::now();
    let scheduler = SchedulerHandle::with_seed(7);
    scheduler.with(|s| {
        s.overrides_mut().defer_operations = Some(false);
        s.overrides_mut().work_limit = Some(1024);
        s.overrides_mut().poll_batch = Some(64);
        s.overrides_mut().no_spurious_wakes = true;
    });
    let workers = crate::worker::Workers::new(scheduler.clone(), start);
    let log = Arc::new(Mutex::new(ProcessLog::default()));
    let mut dirs = Directories::default();
    let opened = dirs
        .open(
            "captures",
            &OpenConfig {
                data_dir: "captures".into(),
                worker_path: Some("worker".into()),
                limits: CoreLimits::default(),
            },
            RunInputs { seed: 7, scheduler },
            core_features(),
            Some(Box::new(RecordedSpawner(log.clone()))),
        )
        .unwrap();
    let mut core = crate::worker::TestkitCore::new(opened.driver, opened.wake, workers);
    let session = SessionId("s".into());
    let size = Size {
        rows: 24,
        cols: 80,
        cell_px: None,
    };
    core.begin(Op::Create {
        session: session.clone(),
        request: SpawnRequest {
            argv: vec!["program".into()],
            env: BTreeMap::new(),
            cwd: "/".into(),
            size,
            labels: BTreeMap::new(),
            color_profile: None,
            notification_policy: None,
            size_policy: None,
        },
    })
    .unwrap();
    pump_core(&mut core, start);
    core.begin(Op::Start {
        id: session.clone(),
    })
    .unwrap();
    pump_core(&mut core, start);
    let spawn = lock(&log).spawns[0].clone();
    let hello = Hello {
        protocol: 1,
        instance: spawn.instance.clone(),
        host_epoch: spawn.host_epoch,
        proof: token_proof(&spawn.token, &spawn.instance, spawn.host_epoch),
    };
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    let mut bytes = Vec::new();
    encode_frame(FrameType::HELLO, &payload, 65536, &mut bytes).unwrap();
    assert_eq!(lock(&log).peers[0].send(&bytes).unwrap(), bytes.len());
    pump_core(&mut core, start);
    assert!(host_messages(&log)
        .iter()
        .any(|msg| matches!(msg, HostMsg::Launch(_))));
    let mut terminal = Terminal::new(&size, History::Off).unwrap();
    terminal.vt_write(b"capture");
    let snapshot = terminal.snapshot().unwrap();
    say(
        &log,
        &WorkerMsg::Launched {
            features: BTreeSet::new(),
            formats: vec![botster_terminal_ghostty::snapshot_format()],
            payload: PayloadId {
                pid: 900,
                start_time: 1,
            },
            terminal: TerminalState {
                size,
                modes: terminal.modes(),
                title: Some(terminal.title()),
                cwd: Some(terminal.cwd()),
                last_output_at: None,
                focused: None,
                model_rev: ModelRev(1),
                input_rev: InputRevs {
                    client: InputRev(0),
                    host: InputRev(0),
                },
            },
        },
    );
    pump_core(&mut core, start);
    assert_eq!(core.get(&session).unwrap().state, SessionState::Running);
    let owner = ClientId("owner".into());
    let mut captures = Vec::new();
    for _ in 0..2 {
        let op = core
            .begin(Op::CaptureSnapshot {
                session: session.clone(),
                owner: owner.clone(),
            })
            .unwrap();
        pump_core(&mut core, start);
        let req = host_messages(&log)
            .into_iter()
            .find_map(|msg| match msg {
                HostMsg::Op {
                    req,
                    op: Op::CaptureSnapshot { .. },
                } => Some(req),
                _ => None,
            })
            .expect("the host forwarded the capture");
        say(
            &log,
            &WorkerMsg::Pages {
                req,
                pages: vec![Page {
                    index: 0,
                    last: true,
                    bytes: botster_route_codec::prelude::HexBytes(snapshot.clone()),
                }],
            },
        );
        say(
            &log,
            &WorkerMsg::Done {
                req,
                result: OpResult::Ok(OpOutput::Capture(Capture {
                    capture: CaptureId(0),
                    page_count: 1,
                    total_bytes: snapshot.len() as u64,
                    model_rev: ModelRev(1),
                })),
            },
        );
        let events = pump_core(&mut core, start);
        let capture = events
            .into_iter()
            .find_map(|event| match event {
                Event::Completed {
                    op: completed,
                    result: OpResult::Ok(OpOutput::Capture(capture)),
                } if completed == op => Some(capture.capture),
                _ => None,
            })
            .expect("the capture completed");
        assert_eq!(core.read_page(capture, 0).unwrap().bytes.0, snapshot);
        captures.push(capture);
    }
    core.release(captures[0]);
    assert_eq!(
        core.read_page(captures[0], 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
    assert!(core.read_page(captures[1], 0).is_ok());
    core.release_owner(&owner);
    assert_eq!(
        core.read_page(captures[1], 0).unwrap_err().code,
        ErrorCode::UnknownCapture
    );
}

/// Plan 2.5 rule 7 and A5-2: queue capacity changes partial progress, not retained bytes or complete-frame order.
#[test]
fn spawned_links_retain_frames_at_each_queue_capacity() {
    use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
    for capacity in [65_536, 1_088, 1] {
        let log = Arc::new(Mutex::new(ProcessLog::default()));
        let mut edges = edges(9).with_spawner(Box::new(RecordedSpawner(log.clone())));
        edges.link_capacity = capacity;
        edges
            .spawn_worker(&WorkerSpawn {
                startup: CoreLimits::default().startup,
                program: "worker".into(),
                instance: InstanceId("1-1".into()),
                token: [9; 32],
                host_epoch: 1,
            })
            .unwrap();
        let link = edges.accept_link().unwrap();
        let payloads = [vec![7; 8192], vec![8; 2048], vec![9; 64]];
        let mut bytes = Vec::new();
        for payload in &payloads {
            encode_frame(FrameType::HOST_MSG, payload, 65536, &mut bytes).unwrap();
        }
        let mut sent = 0;
        let mut received = Vec::new();
        let mut buf = [0; 65536];
        while sent < bytes.len() {
            let n = edges.link_send(link, &bytes[sent..]).unwrap();
            assert!(n > 0);
            sent += n;
            let n = lock(&log).peers[0].recv(&mut buf).unwrap();
            assert!(n > 0);
            received.extend_from_slice(&buf[..n]);
        }
        assert_eq!(received, bytes);
        let mut decoder = FrameDecoder::new(65536);
        let mut rest = received.as_slice();
        let mut decoded = Vec::new();
        while !rest.is_empty() {
            let n = decoder.push(rest);
            rest = &rest[n..];
            while let Some(frame) = decoder.next_frame().unwrap() {
                assert_eq!(frame.kind, FrameType::HOST_MSG);
                decoded.push(frame.payload);
            }
        }
        assert_eq!(decoded, payloads);
    }
}

/// A zero queue capacity cannot make progress and is outside the internal parameter's range.
#[test]
#[should_panic(expected = "a link needs a positive queue capacity")]
fn a_spawn_refuses_zero_queue_capacity() {
    let mut edges = edges(9).with_spawner(Box::new(RecordedSpawner(Arc::default())));
    edges.link_capacity = 0;
    edges
        .spawn_worker(&WorkerSpawn {
            startup: CoreLimits::default().startup,
            program: "worker".into(),
            instance: InstanceId("1-1".into()),
            token: [9; 32],
            host_epoch: 1,
        })
        .unwrap();
}

/// A5-1, A5-2, LC-3, LC-6, and LC-7: the same worker completes the lifecycle at every legal buffer bound.
#[test]
fn the_worker_keeps_complete_operations_at_each_buffer_bound() {
    for bound in [65_536, 1_088, 1] {
        let start = Instant::now();
        let scheduler = SchedulerHandle::with_seed(11);
        scheduler.with(|s| {
            s.overrides_mut().no_spurious_wakes = true;
        });
        let workers = crate::worker::Workers::with_read_chunk(scheduler.clone(), start, bound);
        let mut dirs = Directories::default();
        let mut opened = dirs
            .open(
                "buffers",
                &OpenConfig {
                    data_dir: "buffers".into(),
                    worker_path: Some("worker".into()),
                    limits: CoreLimits::default(),
                },
                RunInputs {
                    seed: 11,
                    scheduler,
                },
                core_features(),
                Some(Box::new(workers.spawner("buffers"))),
            )
            .unwrap();
        opened.driver.edges().link_capacity = bound;
        let mut core = crate::worker::TestkitCore::new(opened.driver, opened.wake, workers);
        let session = SessionId("s".into());
        let create = core
            .begin(Op::Create {
                session: session.clone(),
                request: SpawnRequest {
                    argv: vec!["program".into()],
                    env: BTreeMap::from([("LARGE".into(), "x".repeat(8192))]),
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
        let mut events = settle_partial(&mut core, start);
        let launch = core
            .begin(Op::Start {
                id: session.clone(),
            })
            .unwrap();
        events.extend(settle_partial(&mut core, start));
        assert_eq!(core.get(&session).unwrap().state, SessionState::Running);
        let signal = core
            .begin(Op::Signal {
                id: session.clone(),
                sig: Signal::Term,
            })
            .unwrap();
        events.extend(settle_partial(&mut core, start));
        let remove = core
            .begin(Op::Remove {
                id: session.clone(),
            })
            .unwrap();
        events.extend(settle_partial(&mut core, start));
        assert!(core.list().is_empty());
        let completions: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                Event::Completed {
                    op,
                    result: OpResult::Ok(_),
                } => Some(*op),
                _ => None,
            })
            .collect();
        assert_eq!(completions, vec![create, launch, signal, remove]);
    }
}

fn settle_partial(core: &mut crate::worker::TestkitCore, start: Instant) -> Vec<Event> {
    let mut events = Vec::new();
    // Each pump advances ready in-process work. The bound detects a failure to retain or complete a frame.
    for _ in 0..65_536 {
        let report = core.pump(Now {
            monotonic: start,
            unix: 1_000_000,
        });
        events.extend(core.poll_events(64));
        if !report.more {
            return events;
        }
    }
    panic!("partial progress did not complete the operation");
}

/// LC-12, AD-6, LC-7 (integration finding K1): the handles of one run share one process table. After a drop and a reopen
/// the earlier handle's worker still runs in the `Sim`; the new handle's identity probe sees it (`Matches`, not a false
/// `Absent`). A cleaner removed its endpoint, so the adoption is `Lost(WorkerUnreachable)` (DESIGN.md part 1, AD-2), and the
/// new handle's `Remove` of that session kills the worker, and the remove completes.
#[test]
fn a_reopened_handle_sees_and_ends_the_worker_of_the_earlier_handle() {
    let start = Instant::now();
    let scheduler = SchedulerHandle::with_seed(13);
    scheduler.with(|s| {
        s.overrides_mut().no_spurious_wakes = true;
    });
    let workers = crate::worker::Workers::new(scheduler.clone(), start);
    let mut dirs = Directories::default();
    let config = OpenConfig {
        data_dir: "reopen".into(),
        worker_path: Some("worker".into()),
        limits: CoreLimits::default(),
    };
    let open = |dirs: &mut Directories| {
        let opened = dirs
            .open(
                "reopen",
                &config,
                RunInputs {
                    seed: 13,
                    scheduler: scheduler.clone(),
                },
                core_features(),
                Some(Box::new(workers.spawner("reopen"))),
            )
            .unwrap();
        crate::worker::TestkitCore::new(opened.driver, opened.wake, workers.clone())
    };
    let session = SessionId("s".into());
    let mut first = open(&mut dirs);
    first
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
    settle_partial(&mut first, start);
    first
        .begin(Op::Start {
            id: session.clone(),
        })
        .unwrap();
    settle_partial(&mut first, start);
    assert_eq!(first.get(&session).unwrap().state, SessionState::Running);
    drop(first);
    // The identity that the registry recorded for the worker (AD-6), read with Core's own decoder.
    let row = lock(&dirs.dirs["reopen"]).rows["session/s"].clone();
    let identity = botster_core_host::session::Row::decode(&session, &row)
        .and_then(|row| row.worker)
        .expect("the row names its worker")
        .identity();
    let probe = workers.spawner("reopen");
    assert_eq!(
        probe.identity_state(identity),
        IdentityState::Matches,
        "LC-12: the worker outlives its handle"
    );
    assert!(workers.unlink_endpoint(&key_of(&dirs, "reopen", &session)));
    let mut second = open(&mut dirs);
    let adopt = second.begin(Op::AdoptAll).unwrap();
    let mut events = settle_partial(&mut second, start);
    assert_eq!(
        second.get(&session).unwrap().state,
        SessionState::Lost(LostReason::WorkerUnreachable),
        "a missing endpoint is never repaired (DESIGN.md part 1)"
    );
    let remove = second
        .begin(Op::Remove {
            id: session.clone(),
        })
        .unwrap();
    events.extend(settle_partial(&mut second, start));
    // The kill is not an observed exit: after `stop_grace` the host checks the identity again (LC-7, AD-6).
    let grace = start + CoreLimits::default().stop_grace;
    for _ in 0..64 {
        let report = second.pump(Now {
            monotonic: grace,
            unix: 1_000_000,
        });
        events.extend(second.poll_events(64));
        if !report.more {
            break;
        }
    }
    for op in [adopt, remove] {
        assert!(
            events.iter().any(
                |e| matches!(e, Event::Completed { op: o, result: OpResult::Ok(_) } if *o == op)
            ),
            "{op:?}: {events:?}"
        );
    }
    assert_eq!(
        probe.identity_state(identity),
        IdentityState::Absent,
        "the remove ended the earlier worker"
    );
    assert!(second.list().is_empty());
}

/// The process edge of the real Core at its last step, with the worst corrupt row: the identity probe says that the corrupt
/// identity still matches (its pid and start time name a live process). Every signal passes the refusal of
/// `botster_core_sys::signal` first; an allowed one reaches the in-process worker that the row named before the corruption.
struct GuardedSpawner {
    inner: crate::worker::WorkerSpawner,
    corrupt: ProcessIdentity,
    real: ProcessIdentity,
    asked: Arc<Mutex<Vec<(u32, GroupSignal, bool)>>>,
}

impl Spawner for GuardedSpawner {
    fn spawn(
        &mut self,
        spec: &WorkerSpawn,
        connect: &mut dyn FnMut() -> LinkEnd,
    ) -> Result<ProcessIdentity, SpawnError> {
        self.inner.spawn(spec, connect)
    }

    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        let own = rustix::process::getpgrp()
            .as_raw_nonzero()
            .get()
            .unsigned_abs();
        let refused = botster_core_sys::signal::target(identity.pid, own).is_err();
        lock(&self.asked).push((identity.pid, signal, refused));
        if !refused {
            let to = if identity == self.corrupt {
                self.real
            } else {
                identity
            };
            self.inner.signal_group(to, signal);
        }
    }

    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState {
        if identity == self.corrupt {
            self.inner.identity_state(self.real)
        } else {
            self.inner.identity_state(identity)
        }
    }

    fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        self.inner.poll_exit()
    }

    /// This test checks the signal refusal only, so no adoption completes: `Stop` and `Remove` then ask for group signals.
    /// The adoption through the endpoint has its own tests.
    fn connect_worker(&mut self, _instance: &InstanceId, _end: LinkEnd) -> bool {
        false
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        self.inner.remove_endpoint(instance);
    }
}

/// A10-2, AD-6, the pattern rule: a row whose worker and payload pids were corrupted to 1 never signals anything. After the
/// reopen, `AdoptAll`, `Stop` and `Remove` of that session, every group signal that the host asks for names pid 1, and the
/// refusal stops each one: the worker that the row named before the corruption still runs. (The worker does not complete
/// the adoption, so the session is `Lost(WorkerUnreachable)` and its `Stop` ends at once; its `Remove` asks for the kills.)
#[test]
fn a_corrupt_row_with_pid_1_never_signals_anything() {
    let start = Instant::now();
    let scheduler = SchedulerHandle::with_seed(17);
    scheduler.with(|s| {
        s.overrides_mut().no_spurious_wakes = true;
    });
    let workers = crate::worker::Workers::new(scheduler.clone(), start);
    let mut dirs = Directories::default();
    let config = OpenConfig {
        data_dir: "corrupt".into(),
        worker_path: Some("worker".into()),
        limits: CoreLimits::default(),
    };
    let open = |dirs: &mut Directories, spawner: Box<dyn Spawner>| {
        let opened = dirs
            .open(
                "corrupt",
                &config,
                RunInputs {
                    seed: 17,
                    scheduler: scheduler.clone(),
                },
                core_features(),
                Some(spawner),
            )
            .unwrap();
        crate::worker::TestkitCore::new(opened.driver, opened.wake, workers.clone())
    };
    let session = SessionId("s".into());
    let mut first = open(&mut dirs, Box::new(workers.spawner("corrupt")));
    first
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
    settle_partial(&mut first, start);
    first
        .begin(Op::Start {
            id: session.clone(),
        })
        .unwrap();
    settle_partial(&mut first, start);
    assert_eq!(first.get(&session).unwrap().state, SessionState::Running);
    drop(first);
    // A10-2 at the storage edge: the row still decodes, and its worker and payload pids are 1.
    let registry = Arc::clone(&dirs.dirs["corrupt"]);
    let bytes = lock(&registry).rows["session/s"].clone();
    let real = botster_core_host::session::Row::decode(&session, &bytes)
        .and_then(|row| row.worker)
        .expect("the row names its worker")
        .identity();
    let mut row: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    for field in ["worker", "payload"] {
        if let Some(pid) = row
            .get_mut(field)
            .and_then(|identity| identity.get_mut("pid"))
        {
            *pid = serde_json::json!(1);
        }
    }
    lock(&registry)
        .rows
        .insert("session/s".into(), serde_json::to_vec(&row).unwrap());
    let corrupt = ProcessIdentity { pid: 1, ..real };
    let asked = Arc::new(Mutex::new(Vec::new()));
    let mut second = open(
        &mut dirs,
        Box::new(GuardedSpawner {
            inner: workers.spawner("corrupt"),
            corrupt,
            real,
            asked: Arc::clone(&asked),
        }),
    );
    second.begin(Op::AdoptAll).unwrap();
    let mut events = settle_partial(&mut second, start);
    for op in [
        Op::Stop {
            id: session.clone(),
        },
        Op::Remove {
            id: session.clone(),
        },
    ] {
        second.begin(op).unwrap();
        events.extend(settle_partial(&mut second, start));
    }
    for later in [
        CoreLimits::default().stop_grace,
        2 * CoreLimits::default().stop_grace,
    ] {
        for _ in 0..64 {
            let report = second.pump(Now {
                monotonic: start + later,
                unix: 1_000_000,
            });
            events.extend(second.poll_events(64));
            if !report.more {
                break;
            }
        }
    }
    let asked = lock(&asked).clone();
    assert!(!asked.is_empty(), "the host asked for a signal: {events:?}");
    for (pid, signal, refused) in &asked {
        assert!(*pid == 1 && *refused, "{pid} {signal:?}");
    }
    assert_eq!(
        workers.spawner("corrupt").identity_state(real),
        IdentityState::Matches,
        "no signal reached the worker"
    );
}

/// The endpoint key of the session `id` of the data directory `dir`: the instance of its stored row, read with Core's own
/// decoder.
fn key_of(dirs: &Directories, dir: &str, id: &SessionId) -> crate::worker::InstanceKey {
    let bytes = dirs
        .row(dir, &botster_core_host::session::row_key(id))
        .expect("the session has a row");
    let row = botster_core_host::session::Row::decode(id, &bytes).expect("the row decodes");
    crate::worker::InstanceKey {
        dir: dir.into(),
        instance: row.instance,
    }
}

/// The worker identity that the row of the session `id` of the data directory `dir` records (AD-6).
fn worker_of(dirs: &Directories, dir: &str, id: &SessionId) -> ProcessIdentity {
    let bytes = dirs
        .row(dir, &botster_core_host::session::row_key(id))
        .expect("the session has a row");
    botster_core_host::session::Row::decode(id, &bytes)
        .and_then(|row| row.worker)
        .expect("the row names its worker")
        .identity()
}

/// A handle over the data directory `dir` of a run, with the run's in-process workers.
fn handle_over(
    dirs: &mut Directories,
    dir: &str,
    workers: &crate::worker::Workers,
    scheduler: &SchedulerHandle,
) -> crate::worker::TestkitCore {
    handle_with(dirs, dir, workers, scheduler, CoreLimits::default())
}

/// [`handle_over`] with the limits `limits`.
fn handle_with(
    dirs: &mut Directories,
    dir: &str,
    workers: &crate::worker::Workers,
    scheduler: &SchedulerHandle,
    limits: CoreLimits,
) -> crate::worker::TestkitCore {
    let config = OpenConfig {
        data_dir: dir.into(),
        worker_path: Some("worker".into()),
        limits,
    };
    let opened = dirs
        .open(
            dir,
            &config,
            RunInputs {
                seed: 13,
                scheduler: scheduler.clone(),
            },
            core_features(),
            Some(Box::new(workers.spawner(dir))),
        )
        .unwrap();
    crate::worker::TestkitCore::new(opened.driver, opened.wake, workers.clone())
}

/// Creates the session `id` running `program` on `core`, and starts it.
fn create_and_start(core: &mut crate::worker::TestkitCore, id: &SessionId, start: Instant) {
    core.begin(Op::Create {
        session: id.clone(),
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
    settle_partial(core, start);
    core.begin(Op::Start { id: id.clone() }).unwrap();
    settle_partial(core, start);
    assert_eq!(core.get(id).unwrap().state, SessionState::Running);
}

/// The run's scheduler and in-process workers, with no spurious wakes.
fn adoption_run(start: Instant) -> (SchedulerHandle, crate::worker::Workers) {
    let scheduler = SchedulerHandle::with_seed(13);
    scheduler.with(|s| {
        s.overrides_mut().no_spurious_wakes = true;
    });
    let workers = crate::worker::Workers::new(scheduler.clone(), start);
    (scheduler, workers)
}

/// The completed `Stop` of `id`, which a host stop of the payload ends (AD-1: the stop reaches the adopted payload).
fn stopped_by_the_host(
    core: &mut crate::worker::TestkitCore,
    id: &SessionId,
    start: Instant,
) -> bool {
    let stop = core.begin(Op::Stop { id: id.clone() }).unwrap();
    settle_partial(core, start).iter().any(|e| {
        matches!(
            e,
            Event::Completed {
                op,
                result: OpResult::Ok(OpOutput::End(SessionEnd::Exited(Exit {
                    cause: ExitCause::HostStop,
                    ..
                })))
            } if *op == stop
        )
    })
}

/// Core AD-1, AD-6, DP-8, LC-12 (DESIGN.md "Adoption (P5)", parts 3, 4, 6): a new handle adopts the running session of a
/// dropped one through the worker endpoint, in memory. The session is `Running` with no second payload, and the new
/// handle's `Stop` reaches the same payload: the old link is fenced, and the new link carries the stop and the exit.
#[test]
fn a_reopened_handle_adopts_the_running_worker_through_its_endpoint() {
    let start = Instant::now();
    let (scheduler, workers) = adoption_run(start);
    let mut dirs = Directories::default();
    let session = SessionId("s".into());
    let mut first = handle_over(&mut dirs, "adopt", &workers, &scheduler);
    create_and_start(&mut first, &session, start);
    let worker = worker_of(&dirs, "adopt", &session);
    drop(first);

    let mut second = handle_over(&mut dirs, "adopt", &workers, &scheduler);
    let adopt = second.begin(Op::AdoptAll).unwrap();
    let events = settle_partial(&mut second, start);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == adopt)),
        "{events:?}"
    );
    let record = second.get(&session).unwrap();
    assert_eq!(record.state, SessionState::Running, "{events:?}");
    assert_eq!(
        record.worker_protocol,
        Some(botster_worker_core::WORKER_PROTOCOL)
    );
    assert_eq!(
        worker_of(&dirs, "adopt", &session),
        worker,
        "the adopted worker is the worker that the first handle spawned"
    );

    let stop = second
        .begin(Op::Stop {
            id: session.clone(),
        })
        .unwrap();
    let events = settle_partial(&mut second, start);
    assert!(
        events.iter().any(|e| matches!(
            e,
            Event::Completed {
                op,
                result: OpResult::Ok(OpOutput::End(SessionEnd::Exited(Exit {
                    signal: Some(15),
                    cause: ExitCause::HostStop,
                    ..
                })))
            } if *op == stop
        )),
        "{events:?}"
    );
}

/// AD-1, DESIGN.md parts 3 and 6: one `AdoptAll` adopts every running session, each over its own new link. Each stop
/// reaches its own payload.
#[test]
fn an_adopt_all_gives_each_adopted_worker_its_own_link() {
    let start = Instant::now();
    let (scheduler, workers) = adoption_run(start);
    let mut dirs = Directories::default();
    let sessions = [SessionId("a".into()), SessionId("b".into())];
    let mut first = handle_over(&mut dirs, "two", &workers, &scheduler);
    for id in &sessions {
        create_and_start(&mut first, id, start);
    }
    drop(first);

    let mut second = handle_over(&mut dirs, "two", &workers, &scheduler);
    second.begin(Op::AdoptAll).unwrap();
    let events = settle_partial(&mut second, start);
    for id in &sessions {
        assert_eq!(
            second.get(id).unwrap().state,
            SessionState::Running,
            "{id:?}: {events:?}"
        );
    }
    for id in &sessions {
        assert!(stopped_by_the_host(&mut second, id, start), "{id:?}");
    }
}

/// AD-6, DESIGN.md parts 3 and 7: the worker takes one candidate at a time and closes another one at once. A candidate
/// that closes frees the place: a host that connects later adopts the worker.
#[test]
fn a_second_candidate_is_closed_and_a_closed_candidate_frees_the_place() {
    let start = Instant::now();
    let (scheduler, workers) = adoption_run(start);
    let mut dirs = Directories::default();
    let session = SessionId("s".into());
    let mut first = handle_over(&mut dirs, "strangers", &workers, &scheduler);
    create_and_start(&mut first, &session, start);
    drop(first);

    let key = key_of(&dirs, "strangers", &session);
    let mut held = workers.connect_endpoint(&key).expect("the worker listens");
    let mut refused = workers.connect_endpoint(&key).expect("the worker listens");
    workers.run(start);
    let mut buf = [0u8; 1];
    assert_eq!(
        refused.recv(&mut buf).unwrap(),
        0,
        "the second candidate is closed"
    );
    assert_eq!(
        held.recv(&mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "the first candidate waits for its hello"
    );
    held.close();
    workers.run(start);

    let mut second = handle_over(&mut dirs, "strangers", &workers, &scheduler);
    second.begin(Op::AdoptAll).unwrap();
    let events = settle_partial(&mut second, start);
    assert_eq!(
        second.get(&session).unwrap().state,
        SessionState::Running,
        "{events:?}"
    );
    assert!(stopped_by_the_host(&mut second, &session, start));
}

/// DESIGN.md part 1: a killed worker cannot remove its endpoint, so the endpoint stays and no worker listens on it.
/// `Remove` (step 4) removes it; the endpoint of a session that is not removed stays.
#[test]
fn remove_unlinks_the_endpoint_that_a_killed_worker_left() {
    let start = Instant::now();
    let (scheduler, workers) = adoption_run(start);
    let mut dirs = Directories::default();
    let (removed, kept) = (SessionId("r".into()), SessionId("k".into()));
    let mut core = handle_over(&mut dirs, "killed", &workers, &scheduler);
    let mut killer = workers.spawner("killed");
    for id in [&removed, &kept] {
        create_and_start(&mut core, id, start);
        let worker = worker_of(&dirs, "killed", id);
        assert_eq!(killer.identity_state(worker), IdentityState::Matches);
        killer.signal_group(worker, GroupSignal::Kill);
    }
    let events = settle_partial(&mut core, start);
    for id in [&removed, &kept] {
        assert!(
            !matches!(core.get(id).unwrap().state, SessionState::Running),
            "{id:?}: {events:?}"
        );
        assert!(
            workers
                .connect_endpoint(&key_of(&dirs, "killed", id))
                .is_none(),
            "no worker listens on the endpoint of a killed worker"
        );
    }
    // The row goes with the session: its key is read before the `Remove`.
    let removed_key = key_of(&dirs, "killed", &removed);
    let remove = core
        .begin(Op::Remove {
            id: removed.clone(),
        })
        .unwrap();
    let events = settle_partial(&mut core, start);
    assert!(
        events.iter().any(
            |e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == remove)
        ),
        "{events:?}"
    );
    assert!(
        !workers.unlink_endpoint(&removed_key),
        "Remove removed the endpoint"
    );
    assert!(
        workers.unlink_endpoint(&key_of(&dirs, "killed", &kept)),
        "the killed worker left its endpoint"
    );
}

/// DESIGN.md part 3 (3.3): a candidate that sends no hello is closed at the `startup` of the handle that spawned the worker
/// (`WorkerSpawn.startup`), not at a default.
#[test]
fn a_silent_candidate_is_closed_at_the_spawning_handles_startup() {
    let start = Instant::now();
    let (scheduler, workers) = adoption_run(start);
    let mut dirs = Directories::default();
    let session = SessionId("s".into());
    let limits = CoreLimits {
        startup: CoreLimits::default().startup / 2,
        ..CoreLimits::default()
    };
    let mut core = handle_with(&mut dirs, "silent", &workers, &scheduler, limits.clone());
    create_and_start(&mut core, &session, start);
    let mut silent = workers
        .connect_endpoint(&key_of(&dirs, "silent", &session))
        .expect("the worker listens");
    workers.run(start);
    let mut buf = [0u8; 1];
    assert_eq!(
        silent.recv(&mut buf).unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock,
        "the candidate waits for its hello"
    );
    workers.run(start + limits.startup);
    assert_eq!(
        silent.recv(&mut buf).unwrap(),
        0,
        "the candidate is closed at startup"
    );
}

/// [`handle_over`], with the host's process table and wake object, as the harness keeps them.
fn handle_and_table(
    dirs: &mut Directories,
    dir: &str,
    workers: &crate::worker::Workers,
    scheduler: &SchedulerHandle,
) -> (
    crate::worker::TestkitCore,
    crate::worker::ProcessTable,
    Arc<dyn botster_core_host::driver::HostWake>,
) {
    let config = OpenConfig {
        data_dir: dir.into(),
        worker_path: Some("worker".into()),
        limits: CoreLimits::default(),
    };
    let spawner = workers.spawner(dir);
    let table = spawner.table();
    let opened = dirs
        .open(
            dir,
            &config,
            RunInputs {
                seed: 13,
                scheduler: scheduler.clone(),
            },
            core_features(),
            Some(Box::new(spawner)),
        )
        .unwrap();
    table.set_wake(Arc::clone(&opened.wake));
    let core =
        crate::worker::TestkitCore::new(opened.driver, Arc::clone(&opened.wake), workers.clone());
    (core, table, opened.wake)
}

/// True when `wake` is set. The run has no spurious wakes, so a set flag is the only `Woken`.
fn woken(wake: &Arc<dyn botster_core_host::driver::HostWake>) -> bool {
    matches!(
        botster_core_contract::prelude::WakeHandle::wait(&**wake, std::time::Duration::ZERO),
        Wake::Woken
    )
}

/// Clears the wake objects of `wakes`.
fn drain_all(wakes: &[&Arc<dyn botster_core_host::driver::HostWake>]) {
    for wake in wakes {
        WakeEdge::drain(&***wake);
    }
}

/// P5 #201 A1-F1, A1-F2 (TM-6, `edges_quiet`): the host that holds a worker's control link reads its reports and gets the
/// wakes of its controls. The spawning host keeps only the exit. A candidate that the worker refuses moves nothing; a
/// successful adoption moves the link, again at each adoption, and also when the earlier host's table is gone.
#[test]
fn the_adopting_host_takes_the_control_links_reports_and_wakes() {
    let start = Instant::now();
    let (scheduler, workers) = adoption_run(start);
    let mut dirs = Directories::default();
    let session = SessionId("s".into());
    let (mut a, table_a, wake_a) = handle_and_table(&mut dirs, "ctl", &workers, &scheduler);
    create_and_start(&mut a, &session, start);
    let worker = worker_of(&dirs, "ctl", &session);
    drop(a);

    // A stranger holds the one candidate place, so B's candidate is refused: the session is `Lost(WorkerUnreachable)`,
    // and the control link stays with A.
    let mut stranger = workers
        .connect_endpoint(&key_of(&dirs, "ctl", &session))
        .expect("the worker listens");
    workers.run(start);
    let (mut b, table_b, wake_b) = handle_and_table(&mut dirs, "ctl", &workers, &scheduler);
    b.begin(Op::AdoptAll).unwrap();
    let events = settle_partial(&mut b, start);
    assert_eq!(
        b.get(&session).unwrap().state,
        SessionState::Lost(LostReason::WorkerUnreachable),
        "{events:?}"
    );
    drain_all(&[&wake_a, &wake_b]);
    let (program, wake) = workers.program_edge(worker).unwrap();
    program.write(b"x");
    wake.expect("the control host has a wake").signal();
    assert!(woken(&wake_a), "a refused candidate moves nothing");
    assert!(!woken(&wake_b));

    // A's table is gone (its host is retired). The stranger leaves, and B's retry adopts the worker.
    drop((table_a, wake_a));
    stranger.close();
    workers.run(start);
    let adopt = b
        .begin(Op::Adopt {
            id: session.clone(),
        })
        .unwrap();
    let events = settle_partial(&mut b, start);
    assert!(
        events
            .iter()
            .any(|e| matches!(e, Event::Completed { op, result: OpResult::Ok(_) } if *op == adopt)),
        "{events:?}"
    );
    assert_eq!(b.get(&session).unwrap().state, SessionState::Running);
    assert!(workers.edges_quiet(&table_b));

    // A1-F1: the program edge's wake is B's. A1-F2: the worker's report waits on B's link, and B's edges are not quiet
    // until B reads it.
    drain_all(&[&wake_b]);
    let (program, wake) = workers.program_edge(worker).unwrap();
    program.write(b"y");
    wake.expect("the control host has a wake").signal();
    assert!(woken(&wake_b));
    workers.run(start);
    assert!(
        !workers.edges_quiet(&table_b),
        "B holds the worker's report"
    );
    settle_partial(&mut b, start);
    assert!(workers.edges_quiet(&table_b));

    // A second adoption moves the link again: `break_control` wakes C, not B.
    drop(b);
    let (mut c, table_c, wake_c) = handle_and_table(&mut dirs, "ctl", &workers, &scheduler);
    c.begin(Op::AdoptAll).unwrap();
    let events = settle_partial(&mut c, start);
    assert_eq!(
        c.get(&session).unwrap().state,
        SessionState::Running,
        "{events:?}"
    );
    drain_all(&[&wake_b, &wake_c]);
    workers.break_link(worker).unwrap();
    assert!(woken(&wake_c));
    assert!(!woken(&wake_b));
    workers.run(start);
    assert!(
        !workers.edges_quiet(&table_c),
        "C holds the end of the broken link"
    );
    assert!(
        workers.edges_quiet(&table_b),
        "B no longer holds the worker's link"
    );
}

/// P5 #201 A1-F2 (round 2): the end of file of a candidate that the worker refused is a report for the host that connected,
/// until that host reads it. The refusal moves no control link.
#[test]
fn a_refused_candidates_end_of_file_keeps_the_connecting_host_busy() {
    let start = Instant::now();
    let (scheduler, workers) = adoption_run(start);
    let mut dirs = Directories::default();
    let session = SessionId("s".into());
    let mut first = handle_over(&mut dirs, "eof", &workers, &scheduler);
    create_and_start(&mut first, &session, start);
    drop(first);
    let key = key_of(&dirs, "eof", &session);
    let _stranger = workers.connect_endpoint(&key).expect("the worker listens");
    workers.run(start);

    // A host's edge connects (`SimEdges::connect_worker` gives the spawner the worker's end of a new link).
    let mut spawner = workers.spawner("eof");
    let table = spawner.table();
    assert!(workers.edges_quiet(&table));
    let (mut host, worker) = crate::net::link_pair(1024);
    assert!(spawner.connect_worker(&key.instance, worker));
    workers.run(start);
    assert!(
        !workers.edges_quiet(&table),
        "the refused candidate's end of file waits for the host"
    );
    let mut buf = [0u8; 16];
    assert_eq!(host.recv(&mut buf).unwrap(), 0, "the candidate was closed");
    host.close();
    assert!(workers.edges_quiet(&table));
}
