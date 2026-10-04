//! Checks of the worker's injected edges (Core A5-1, A5-2, EV-4, and LC-5).

use super::*;
use botster_core_link::proof::TOKEN_LEN;

#[allow(clippy::disallowed_methods)] // The test initializes the injected clock once.
fn fixture(capacity: usize) -> (WorkerEdges, LinkEnd, Worker, Instant) {
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
    let edges = WorkerEdges {
        id,
        cell,
        processes: Arc::new(Mutex::new(processes)),
        pids: Arc::new(Mutex::new(Pids { next: 1001 })),
        scheduler,
        link,
        link_open: true,
        outbound: VecDeque::new(),
        written: 0,
        payload: None,
        spawned: None,
        exit: None,
        output_ended: false,
        drain: false,
        ready: Vec::new(),
    };
    let worker = Worker::new(WorkerConfig::new(
        InstanceId("1-1".into()),
        [1; TOKEN_LEN],
        1,
    ));
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

/// EV-4: output precedes its drain; the edge offers no output before the spawn answer.
#[test]
fn program_output_waits_for_spawn_and_ends_with_one_drain() {
    let (mut edges, _peer, worker, now) = fixture(8);
    let script = serde_json::from_value(serde_json::json!({"program": [
        {"print": {"bytes_hex": "616263"}}, {"exit": {"code": 7}}
    ]}))
    .unwrap();
    let program = ScriptedProgram::new(&script, true, &edges.scheduler).unwrap();
    program.control().write_size(Some(3));
    edges.payload = Some(program);
    let id = PayloadId {
        pid: 1002,
        start_time: 1,
    };
    edges.spawned = Some(Ok(id));
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(edges.take(now, &worker, 0), Input::Spawned(Ok(id)));
    assert_eq!(edges.ready(now, &worker), 2);
    assert_eq!(
        edges.take(now, &worker, 0),
        Input::PtyOutput(b"abc".to_vec())
    );
    assert_eq!(
        edges.take(now, &worker, 1),
        Input::PayloadExited(ExitStatus::Code(7))
    );
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(edges.take(now, &worker, 0), Input::PtyDrained);
    assert_eq!(edges.ready(now, &worker), 0);
    edges.perform(now, Action::DrainPty);
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(edges.take(now, &worker, 0), Input::PtyDrained);
    assert_eq!(edges.ready(now, &worker), 0);
}

/// A5-1: a worker exit closes the link, drops the program, and posts the process exit once.
#[test]
fn worker_exit_closes_the_link_and_posts_its_exit_once() {
    let (mut edges, mut peer, worker, now) = fixture(8);
    let mut spawner = WorkerSpawner {
        workers: Workers::new(edges.scheduler.clone(), now),
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
#[allow(clippy::disallowed_methods)] // The test initializes the injected clock once.
fn worker_identities_do_not_repeat() {
    let workers = Workers::new(SchedulerHandle::with_seed(2), Instant::now());
    assert!(format!("{workers:?}").contains("Workers"));
    let mut spawner = workers.spawner();
    let spec = WorkerSpawn {
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
    use botster_core_link::proof::token_proof;

    let (edges, _peer, mut worker, now) = fixture(1024);
    while worker.poll_action().is_some() {}
    let hello = Hello {
        protocol: 1,
        instance: InstanceId("1-1".into()),
        host_epoch: 1,
        proof: token_proof(&[1; TOKEN_LEN], &InstanceId("1-1".into()), 1),
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

/// Plan 2.5: a ready read takes a control frame within the edge's read bound in one input.
#[test]
fn a_large_control_frame_is_one_ready_input() {
    use botster_core_link::frame::{encode_frame, FrameType};
    let (mut edges, mut peer, worker, now) = fixture(65536);
    let payload = vec![7; 8192];
    let mut bytes = Vec::new();
    encode_frame(FrameType::HOST_MSG, &payload, 65536, &mut bytes).unwrap();
    assert_eq!(peer.send(&bytes).unwrap(), bytes.len());
    assert_eq!(edges.ready(now, &worker), 1);
    assert_eq!(edges.take(now, &worker, 0), Input::LinkBytes(bytes));
    assert_eq!(edges.ready(now, &worker), 0);
}
