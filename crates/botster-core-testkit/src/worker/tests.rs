//! Checks of the worker's injected edges (Core A5-1, A5-2, EV-4, and LC-5).

use super::*;
use botster_core_link::proof::TOKEN_LEN;

fn fixture(capacity: usize) -> (WorkerEdges, LinkEnd, SharedWorker, Instant) {
    let now = Instant::now();
    let scheduler = SchedulerHandle::with_seed(1);
    let (link, peer) = crate::net::link_pair(capacity);
    let id = ProcessIdentity {
        pid: 1001,
        start_time: 1,
    };
    let cell = Arc::new(Mutex::new(ProcessCell::default()));
    let mut processes = Processes::default();
    processes.cells.insert(id, Arc::clone(&cell));
    processes.links.insert(id, Arc::clone(&cell));
    let processes = Arc::new(Mutex::new(processes));
    lock(&cell).control = Some(Arc::downgrade(&processes));
    let edges = WorkerEdges {
        id,
        cell,
        processes,
        pids: Arc::new(Mutex::new(Pids { next: 1001 })),
        key: InstanceKey {
            dir: "d".into(),
            instance: InstanceId("1-1".into()),
        },
        held_starts: Arc::default(),
        held_spawn: None,
        endpoint: Endpoint::default(),
        endpoints: Arc::default(),
        candidates: BTreeMap::new(),
        next_candidate: 0,
        scheduler,
        link,
        link_open: true,
        outbound: VecDeque::new(),
        written: 0,
        payload: None,
        spawned: None,
        exit: None,
        drain: None,
        pty_write: None,
        wait_writable: false,
        ready: Vec::new(),
        read_chunk: READ_CHUNK,
        descriptors: BTreeMap::new(),
        next_descriptor: 0,
        routes: BTreeMap::new(),
        pty_budget: None,
    };
    let worker = SharedWorker(Arc::new(Mutex::new(Worker::new(WorkerConfig::new(
        InstanceId("1-1".into()),
        [1; TOKEN_LEN],
        1,
    )))));
    (edges, peer, worker, now)
}

/// Plan 2.5: writes keep their order and byte count across a full link, then detect a broken peer.
#[test]
fn partial_writes_keep_bytes_and_a_broken_link_discards_them() {
    let (mut edges, mut peer, worker, now) = fixture(4);
    edges.perform(now, Action::LinkSend(b"abcdef".to_vec()));
    assert_eq!(edges.ready(now, &worker), 1);
    assert!(edges.link.end().interest().write);
    assert_eq!(edges.take(now, &worker, 0), Input::LinkWritten { total: 4 });
    let mut bytes = [0; 4];
    assert_eq!(peer.recv(&mut bytes).unwrap(), 4);
    assert_eq!(&bytes, b"abcd");
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(edges.take(now, &worker, 0), Input::LinkWritten { total: 6 });
    assert_eq!(peer.recv(&mut bytes).unwrap(), 2);
    assert_eq!(&bytes[..2], b"ef");
    assert_eq!(edges.ready(now, &worker), 0);
    assert!(!edges.link.end().interest().write);
    peer.close();
    edges.perform(now, Action::LinkSend(b"lost".to_vec()));
    assert_eq!(edges.ready(now, &worker), 2);
    assert_eq!(edges.take(now, &worker, 1), Input::LinkClosed);
    assert_eq!(edges.ready(now, &worker), 0);
}

/// DP-8: a peer's EOF is LinkClosed, never an empty LinkBytes input.
#[test]
fn a_control_link_eof_closes_the_link() {
    let (mut edges, mut peer, worker, now) = fixture(4);
    peer.close();
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(edges.take(now, &worker, 0), Input::LinkClosed);
    assert_eq!(edges.ready(now, &worker), 0);
}

/// EV-4, `Action::DrainPty`: the edge offers no output before the spawn answer, and output precedes its drain. The edge's
/// drain is bounded (`Drain`): the count that the program held when it was asked, then one flushing read, then at most
/// the count measured once after that read. A writer that keeps writing cannot extend it, and `PtyDrained` answers
/// only an asked drain.
#[test]
fn the_edge_drain_is_bounded_by_the_asked_count() {
    let (mut edges, _peer, worker, now) = fixture(8);
    let script = serde_json::from_value(serde_json::json!({"program": [
        {"print": {"bytes_hex": "616263"}}, {"hold": {}}
    ]}))
    .unwrap();
    let program = ScriptedProgram::new(&script, true, &edges.scheduler).unwrap();
    let control = program.control();
    control.write_size(Some(1));
    edges.payload = Some(program);
    let id = PayloadId {
        pid: 1002,
        start_time: 1,
    };
    edges.spawned = Some(Ok(id));
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(edges.take(now, &worker, 0), Input::Spawned(Ok(id)));
    // No drain is asked: the edge reads output while it waits.
    assert_eq!(edges.ready(now, &worker), 1);
    assert!(matches!(edges.take(now, &worker, 0), Input::PtyOutput(_)));
    // The drain is asked while output waits. Output that comes before the flushing read is found by it; output that comes
    // after the count measured once after that read waits.
    let held = control.output_unread();
    edges.perform(now, Action::DrainPty);
    control.write_once(b"later");
    let mut drained = Vec::new();
    let mut after_flush = false;
    loop {
        assert_eq!(edges.ready(now, &worker), 1);
        match edges.take(now, &worker, 0) {
            Input::PtyOutput(bytes) => {
                drained.extend(bytes);
                if drained.len() > held && !after_flush {
                    after_flush = true;
                    control.write_once(b"more");
                }
            }
            Input::PtyDrained => break,
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(drained.len(), held + b"later".len());
    assert_eq!(
        control.output_unread(),
        b"more".len(),
        "a writer cannot extend the drain"
    );
    // The rest is read with no drain, and no `PtyDrained` follows it.
    while control.output_unread() > 0 {
        assert_eq!(edges.ready(now, &worker), 1);
        assert!(matches!(edges.take(now, &worker, 0), Input::PtyOutput(_)));
    }
    assert_eq!(edges.ready(now, &worker), 0);
}

/// A5-1: a worker exit closes the link, drops the program, and posts the process exit once.
#[test]
fn worker_exit_closes_the_link_and_posts_its_exit_once() {
    let (mut edges, mut peer, worker, now) = fixture(8);
    let workers = Workers::new(edges.scheduler.clone(), now);
    // The run's process table knows the worker of this fixture, as a spawn would have recorded it.
    lock(&workers.run_processes).insert(
        edges.id,
        (Arc::clone(&edges.cell), Arc::clone(&edges.processes)),
    );
    let mut spawner = WorkerSpawner {
        workers,
        dir: edges.key.dir.clone(),
        processes: Arc::clone(&edges.processes),
    };
    for signal in [GroupSignal::EndPayload, GroupSignal::Term] {
        spawner.signal_group(edges.id, signal);
        assert_eq!(edges.ready(now, &worker), 1);
        let expected = if signal == GroupSignal::Term {
            Input::Terminate
        } else {
            Input::EndPayload
        };
        assert_eq!(edges.take(now, &worker, 0), expected);
        assert_eq!(edges.ready(now, &worker), 0);
    }
    spawner.signal_group(edges.id, GroupSignal::Kill);
    spawner.signal_group(edges.id, GroupSignal::Kill);
    assert_eq!(spawner.poll_exit(), Some((edges.id, ExitStatus::Signal(9))));
    assert_eq!(spawner.poll_exit(), None);
    spawner.signal_group(edges.id, GroupSignal::EndPayload);
    spawner.signal_group(edges.id, GroupSignal::Term);
    assert!(!lock(&edges.cell).end_payload);
    assert!(!lock(&edges.cell).terminate);
    edges.perform(now, Action::LinkSend(b"lost".to_vec()));
    assert_eq!(edges.ready(now, &worker), 0);
    assert_eq!(peer.recv(&mut [0]).unwrap(), 0);
}

/// A5-1: worker identities remain unique across repeated spawns in the shared process table.
#[test]
fn worker_identities_do_not_repeat() {
    let workers = Workers::new(SchedulerHandle::with_seed(2), Instant::now());
    assert!(format!("{workers:?}").contains("Workers"));
    let mut spawner = workers.spawner("d");
    let spec = WorkerSpawn {
        startup: CoreLimits::default().startup,
        program: "worker".into(),
        instance: InstanceId("1-1".into()),
        token: [1; TOKEN_LEN],
        host_epoch: 1,
    };
    let mut peers = Vec::new();
    let mut connect = || {
        let (host, worker) = crate::net::link_pair(128);
        peers.push(host);
        worker
    };
    let mut identities = std::collections::BTreeSet::new();
    for _ in 0..1002 {
        let id = spawner.spawn(&spec, &mut connect).unwrap();
        assert!(id.pid > 0);
        assert!(identities.insert(id));
    }
}

/// LC-5: the workers expose the grace deadline set by the shared worker machine.
#[test]
fn workers_expose_the_payload_grace_deadline() {
    use botster_core_link::frame::{encode_frame, FrameType};
    use botster_core_link::hello::Hello;
    use botster_core_link::msg::{HostMsg, LaunchSpec};
    use botster_core_link::proof::host_proof;

    let (edges, _peer, mut worker, now) = fixture(1024);
    while worker.poll_action().is_some() {}
    let hello = Hello {
        protocol: 1,
        instance: InstanceId("1-1".into()),
        host_epoch: 1,
        proof: host_proof(&[1; TOKEN_LEN], &InstanceId("1-1".into()), 1),
    };
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    let mut bytes = Vec::new();
    encode_frame(FrameType::HELLO, &payload, u32::MAX, &mut bytes).unwrap();
    worker.handle(now, Input::LinkBytes(bytes));
    let launch = HostMsg::Launch(Box::new(LaunchSpec {
        argv: vec!["program".into()],
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
        link_frame_bound: 65536,
        stop_grace_ms: 250,
        limits: CoreLimits::default(),
    }));
    payload.clear();
    launch.encode(&mut payload);
    bytes = Vec::new();
    encode_frame(FrameType::HOST_MSG, &payload, u32::MAX, &mut bytes).unwrap();
    worker.handle(now, Input::LinkBytes(bytes));
    worker.handle(
        now,
        Input::Spawned(Ok(PayloadId {
            pid: 1002,
            start_time: 1,
        })),
    );
    worker.handle(now, Input::EndPayload);
    while worker.poll_action().is_some() {}
    let workers = Workers::new(edges.scheduler.clone(), now);
    lock(&workers.sim).add(Box::new(MachineNode::new(worker, edges)));
    assert_eq!(
        workers.next_deadline(),
        Some(now + Duration::from_millis(250))
    );
}

/// Plan 2.5 rule 7 and A5-2: positive read bounds retain control bytes and complete frames in order.
#[test]
fn control_reads_retain_frames_at_each_read_bound() {
    use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
    for bound in [65_536, 1_088, 1] {
        let (mut edges, mut peer, worker, now) = fixture(65536);
        edges.read_chunk = bound;
        let payloads = [vec![7; 8192], vec![8; 2048], vec![9; 64]];
        let mut bytes = Vec::new();
        for payload in &payloads {
            encode_frame(FrameType::HOST_MSG, payload, 65536, &mut bytes).unwrap();
        }
        let mut sent = 0;
        while sent < bytes.len() {
            sent += peer.send(&bytes[sent..]).unwrap();
        }
        let mut received = Vec::new();
        let mut decoder = FrameDecoder::new(65536);
        let mut decoded = Vec::new();
        while edges.ready(now, &worker) != 0 {
            let Input::LinkBytes(chunk) = edges.take(now, &worker, 0) else {
                panic!("the ready input must carry control bytes");
            };
            assert!(!chunk.is_empty());
            assert!(chunk.len() <= bound);
            received.extend_from_slice(&chunk);
            let mut rest = chunk.as_slice();
            while !rest.is_empty() {
                let n = decoder.push(rest);
                rest = &rest[n..];
                while let Some(frame) = decoder.next_frame().unwrap() {
                    assert_eq!(frame.kind, FrameType::HOST_MSG);
                    decoded.push(frame.payload);
                }
            }
        }
        assert_eq!(received, bytes);
        assert_eq!(decoded, payloads);
    }
}

/// A5-2 and EV-4: program bytes keep their order at the same positive read bounds.
#[test]
fn program_reads_retain_output_at_each_read_bound() {
    for bound in [65_536, 1_088, 1] {
        let (mut edges, _peer, worker, now) = fixture(8);
        edges.read_chunk = bound;
        let script = serde_json::from_value(serde_json::json!({"program": [
            {"print": {"bytes_hex": "61".repeat(8192)}}, {"hold": {}}
        ]}))
        .unwrap();
        edges.payload = Some(ScriptedProgram::new(&script, false, &edges.scheduler).unwrap());
        let mut received = Vec::new();
        while edges.ready(now, &worker) != 0 {
            let Input::PtyOutput(chunk) = edges.take(now, &worker, 0) else {
                panic!("the ready input must carry program output");
            };
            assert!(!chunk.is_empty());
            assert!(chunk.len() <= bound);
            received.extend_from_slice(&chunk);
        }
        assert_eq!(received, vec![b'a'; 8192]);
    }
}

/// A zero read bound cannot make progress and is outside the internal parameter's range.
#[test]
#[should_panic(expected = "a worker needs a positive read bound")]
fn a_worker_refuses_zero_read_bound() {
    Workers::with_read_chunk(SchedulerHandle::with_seed(1), Instant::now(), 0);
}

/// `edges_quiet` (Core A5-2): each report that the host has not consumed keeps the edges from quiet on its own: a worker's
/// ready work, a link report that the host has not read, and an exit that the host has not polled.
#[test]
fn each_unconsumed_report_alone_keeps_the_edges_from_quiet() {
    let now = Instant::now();
    let workers = Workers::new(SchedulerHandle::with_seed(0), now);
    let mut spawner = workers.spawner("d");
    let table = spawner.table();
    assert!(workers.edges_quiet(&table), "no worker, no report");
    let spec = WorkerSpawn {
        startup: CoreLimits::default().startup,
        program: "worker".into(),
        instance: InstanceId("1-1".into()),
        token: [1; TOKEN_LEN],
        host_epoch: 1,
    };
    let mut host = None;
    let id = spawner
        .spawn(&spec, &mut || {
            let (h, w) = crate::net::link_pair(1024);
            host = Some(h);
            w
        })
        .unwrap();
    let mut host = host.expect("the spawn connected");

    // The worker's hello waits in its own queue: ready work, and nothing on the link yet.
    assert!(workers.has_ready());
    assert!(!table.holds_reports());
    assert!(!workers.edges_quiet(&table));

    // The hello is on the link and the host has not read it.
    workers.run(now);
    assert!(!workers.has_ready());
    assert!(table.holds_reports());
    assert!(!workers.edges_quiet(&table));
    let mut buf = [0u8; 256];
    while matches!(host.recv(&mut buf), Ok(n) if n > 0) {}
    assert!(workers.edges_quiet(&table));

    // The host's end closes first, so the worker's end of file is no report for it; then the worker is killed, and only its
    // exit waits for the host.
    host.end().close();
    workers.run(now);
    assert!(workers.edges_quiet(&table));
    spawner.signal_group(id, GroupSignal::Kill);
    assert!(!workers.has_ready());
    assert!(table.holds_reports());
    assert!(!workers.edges_quiet(&table));
    assert_eq!(spawner.poll_exit(), Some((id, ExitStatus::Signal(9))));
    assert!(workers.edges_quiet(&table));
}

/// F63: a program-edge control (`pty_output`, `pty_blocked`) and the end of its process do not deadlock. `Processes::end`
/// locks the owner, then the cell, so `program_edge` must release the cell before it locks the owner. The test fixes the
/// order with events, not with delays:
/// 1. The control reads the cell and stops before it locks the owner.
/// 2. The end thread takes the owner and then ends the process, which needs the cell.
/// 3. The control goes on and locks the owner.
///
/// With the cell still held at step 3, the two threads wait for each other on every schedule. The deadlines only turn that
/// deadlock into a failure.
#[test]
fn a_program_edge_control_concurrent_with_the_process_end_does_not_deadlock() {
    use std::sync::mpsc;
    use std::thread;

    let (edges, _peer, _worker, now) = fixture(8);
    let workers = Workers::new(edges.scheduler.clone(), now);
    lock(&workers.run_processes).insert(
        edges.id,
        (Arc::clone(&edges.cell), Arc::clone(&edges.processes)),
    );
    let program = ScriptedProgram::from_argv(&["program".into()], &edges.scheduler).unwrap();
    lock(&edges.cell).program = Some(program.control());
    let id = edges.id;

    let (at_owner_tx, at_owner_rx) = mpsc::channel();
    let (go_tx, go_rx) = mpsc::channel::<()>();
    let (done_tx, done_rx) = mpsc::channel();
    let edge_workers = workers.clone();
    let control_done = done_tx.clone();
    let control = thread::spawn(move || {
        let result = edge_workers
            .program_edge_between(id, || {
                at_owner_tx.send(()).unwrap();
                go_rx.recv().unwrap();
            })
            .map(|_| ());
        control_done.send("control").unwrap();
        result
    });
    // timer: deadline — fails the test when the control never reaches the owner step.
    at_owner_rx.recv_timeout(Duration::from_secs(10)).unwrap();

    let (owner_held_tx, owner_held_rx) = mpsc::channel();
    let processes = Arc::clone(&edges.processes);
    let ender = thread::spawn(move || {
        let mut owner = lock(&processes);
        owner_held_tx.send(()).unwrap();
        owner.end(id, ExitStatus::Signal(9));
        drop(owner);
        done_tx.send("end").unwrap();
    });
    // timer: deadline — fails the test when the end thread never takes the owner.
    owner_held_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    go_tx.send(()).unwrap();

    let mut done = Vec::new();
    for _ in 0..2 {
        done.push(
            done_rx
                // timer: deadline — fails the test on a lock-order deadlock, so the test does not hang.
                .recv_timeout(Duration::from_secs(10))
                .expect("the control and the end finish: no lock-order deadlock"),
        );
    }
    done.sort_unstable();
    assert_eq!(done, ["control", "end"]);
    ender.join().unwrap();
    assert_eq!(
        control.join().unwrap(),
        Ok(()),
        "the control read the cell before the end"
    );
    assert_eq!(
        lock(&edges.processes).exits.pop_front(),
        Some((id, ExitStatus::Signal(9)))
    );
    assert!(workers.program_edge(id).is_err(), "the process has ended");
}

/// A payload spawn of the default program, which holds.
fn payload_spec() -> PayloadSpec {
    PayloadSpec {
        argv: vec!["program".into()],
        env: BTreeMap::new(),
        cwd: "/".into(),
        size: Size {
            rows: 24,
            cols: 80,
            cell_px: None,
        },
    }
}

/// `payload_alive`: the payload is alive from its spawn until its process ends. The end counts when the edge queues the exit
/// for the worker, before the worker takes it and before the reap.
#[test]
fn the_payload_is_alive_from_its_spawn_until_its_process_ends() {
    let (mut edges, _peer, worker, now) = fixture(1024);
    assert!(!lock(&edges.cell).payload_alive);
    edges.perform(now, Action::SpawnPayload(payload_spec()));
    assert!(lock(&edges.cell).payload_alive);
    assert_eq!(edges.ready(now, &worker), 1);
    assert!(matches!(edges.take(now, &worker, 0), Input::Spawned(Ok(_))));
    assert_eq!(edges.ready(now, &worker), 0, "the payload holds");
    assert!(lock(&edges.cell).payload_alive);
    edges.perform(now, Action::SignalPayload(9));
    assert!(
        !lock(&edges.cell).payload_alive,
        "the process ended; the worker has not taken its exit"
    );
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(
        edges.take(now, &worker, 0),
        Input::PayloadExited(ExitStatus::Signal(9))
    );
    assert!(!lock(&edges.cell).payload_alive);
}

/// `hold_start_at` (AD-7 step 4): while the start is held, the worker's spawn is kept and is not ready work. The release
/// makes it ready, and its answer is the spawn's. The end of the worker drops a kept spawn and its own hold only: the same
/// instance of another directory stays held.
#[test]
fn a_held_spawn_waits_for_the_release_and_goes_with_the_worker() {
    let (mut edges, _peer, worker, now) = fixture(1024);
    let key = edges.key.clone();
    lock(&edges.held_starts).insert(key.clone());
    edges.perform(now, Action::SpawnPayload(payload_spec()));
    assert_eq!(
        edges.ready(now, &worker),
        0,
        "a held spawn is not ready work"
    );
    assert!(!lock(&edges.cell).payload_alive);
    lock(&edges.held_starts).remove(&key);
    assert_eq!(edges.ready(now, &worker), 1);
    assert!(matches!(edges.take(now, &worker, 0), Input::Spawned(Ok(_))));
    assert!(lock(&edges.cell).payload_alive);

    let (mut edges, _peer, worker, now) = fixture(1024);
    let other = InstanceKey {
        dir: "other".into(),
        ..key.clone()
    };
    lock(&edges.held_starts).insert(key.clone());
    lock(&edges.held_starts).insert(other.clone());
    edges.perform(now, Action::SpawnPayload(payload_spec()));
    lock(&edges.cell).ended = true;
    assert_eq!(edges.ready(now, &worker), 0);
    assert!(
        edges.held_spawn.is_none(),
        "the kept spawn went with the worker"
    );
    assert!(
        !lock(&edges.held_starts).contains(&key),
        "its hold went too"
    );
    assert!(
        lock(&edges.held_starts).contains(&other),
        "the same instance of another directory stays held"
    );
    assert!(!lock(&edges.cell).payload_alive);
}

/// A route stream end in a descriptor, as the host's edge hands it over (DP-2).
fn route_descriptor(
    scheduler: &SchedulerHandle,
) -> (crate::net::Descriptor, crate::net::StreamEnd) {
    let (worker_end, client_end) = crate::net::stream_pair(scheduler, 4);
    let endpoint = StreamEndpoint::new(worker_end);
    (crate::net::Descriptor::new(endpoint), client_end)
}

/// DP-2: the edge gives the machine a descriptor before the bytes that it rides with, and `BindRoute` makes its stream the
/// route's transport.
#[test]
fn a_descriptor_comes_before_the_bytes_that_it_rides_with() {
    let (mut edges, mut peer, worker, now) = fixture(16);
    peer.send(b"ab").unwrap();
    let (descriptor, mut client) = route_descriptor(&edges.scheduler);
    assert_eq!(peer.send_with_descriptor(b"cd", descriptor).unwrap(), 2);
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(
        edges.take(now, &worker, 0),
        Input::LinkBytes(b"ab".to_vec())
    );
    assert_eq!(edges.ready(now, &worker), 1);
    let Input::Descriptor(id) = edges.take(now, &worker, 0) else {
        panic!("the descriptor comes before its bytes")
    };
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(
        edges.take(now, &worker, 0),
        Input::LinkBytes(b"cd".to_vec())
    );
    edges.perform(
        now,
        Action::BindRoute {
            descriptor: id,
            route: RouteId(3),
        },
    );
    edges.perform(
        now,
        Action::RouteWrite {
            route: RouteId(3),
            bytes: b"xy".to_vec(),
        },
    );
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(
        edges.take(now, &worker, 0),
        Input::RouteWritten {
            route: RouteId(3),
            result: Ok(2)
        }
    );
    let mut buf = [0u8; 4];
    let n = botster_core_edges::RouteTransport::read(&mut client, &mut buf).unwrap();
    assert_eq!(&buf[..n], &b"xy"[..n]);
    // A descriptor that no route took is closed: its client sees the end.
    let (descriptor, mut orphan) = route_descriptor(&edges.scheduler);
    peer.send_with_descriptor(b"e", descriptor).unwrap();
    edges.ready(now, &worker);
    let Input::Descriptor(id) = edges.take(now, &worker, 0) else {
        panic!("a descriptor")
    };
    edges.perform(now, Action::CloseDescriptor(id));
    assert_eq!(
        botster_core_edges::RouteTransport::read(&mut orphan, &mut buf).unwrap(),
        0
    );
}

/// OU-3a: the edge writes a route's bytes only when its stream takes bytes. A full or gated stream holds the write, and the
/// write goes when the client reads. `RouteClose` closes the stream.
#[test]
fn a_full_route_stream_holds_the_write_until_the_client_reads() {
    let (mut edges, mut peer, worker, now) = fixture(16);
    let (descriptor, mut client) = route_descriptor(&edges.scheduler);
    peer.send_with_descriptor(b"f", descriptor).unwrap();
    edges.ready(now, &worker);
    let Input::Descriptor(id) = edges.take(now, &worker, 0) else {
        panic!("a descriptor")
    };
    edges.ready(now, &worker);
    edges.take(now, &worker, 0);
    edges.perform(
        now,
        Action::BindRoute {
            descriptor: id,
            route: RouteId(1),
        },
    );
    edges.perform(
        now,
        Action::RouteWrite {
            route: RouteId(1),
            bytes: b"1234".to_vec(),
        },
    );
    edges.ready(now, &worker);
    assert_eq!(
        edges.take(now, &worker, 0),
        Input::RouteWritten {
            route: RouteId(1),
            result: Ok(4)
        }
    );
    edges.perform(
        now,
        Action::RouteWrite {
            route: RouteId(1),
            bytes: b"5".to_vec(),
        },
    );
    assert_eq!(edges.ready(now, &worker), 0, "a full queue holds the write");
    let control = edges
        .routes
        .get_mut(&RouteId(1))
        .unwrap()
        .end
        .end()
        .control();
    control.gate(true);
    let mut buf = [0u8; 4];
    let mut got = Vec::new();
    let n = botster_core_edges::RouteTransport::read(&mut client, &mut buf).unwrap();
    assert!(n > 0, "the client reads");
    got.extend_from_slice(&buf[..n]);
    assert_eq!(
        edges.ready(now, &worker),
        0,
        "a gated stream holds the write"
    );
    control.gate(false);
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(
        edges.take(now, &worker, 0),
        Input::RouteWritten {
            route: RouteId(1),
            result: Ok(1)
        }
    );
    edges.perform(now, Action::RouteClose { route: RouteId(1) });
    loop {
        let n = botster_core_edges::RouteTransport::read(&mut client, &mut buf).unwrap();
        if n == 0 {
            break;
        }
        got.extend_from_slice(&buf[..n]);
    }
    assert_eq!(got, b"12345", "every byte once, in order, then the end");
}

/// OU-3d: the edge reads at most the PTY budget in all, a budget of zero stops every read, and `None` lifts the limit.
#[test]
fn the_pty_read_budget_bounds_the_reads() {
    let (mut edges, _peer, worker, now) = fixture(8);
    let script = serde_json::from_value(serde_json::json!({"program": [
        {"print": {"bytes_hex": "6162636465666768"}}, {"hold": {}}
    ]}))
    .unwrap();
    let program = ScriptedProgram::new(&script, true, &edges.scheduler).unwrap();
    edges.payload = Some(program);
    let id = PayloadId {
        pid: 1003,
        start_time: 1,
    };
    edges.spawned = Some(Ok(id));
    edges.ready(now, &worker);
    edges.take(now, &worker, 0);
    edges.perform(now, Action::PtyReadBudget(Some(3)));
    let mut read = Vec::new();
    while edges.ready(now, &worker) == 1 {
        let Input::PtyOutput(bytes) = edges.take(now, &worker, 0) else {
            panic!("a read")
        };
        read.extend(bytes);
    }
    assert_eq!(read, b"abc", "three bytes in all, then none");
    edges.perform(now, Action::PtyReadBudget(None));
    while edges.ready(now, &worker) == 1 {
        let Input::PtyOutput(bytes) = edges.take(now, &worker, 0) else {
            panic!("a read")
        };
        read.extend(bytes);
    }
    assert_eq!(read, b"abcdefgh");
}
