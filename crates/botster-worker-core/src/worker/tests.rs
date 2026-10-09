//! Tests of the `Worker` machine through its inputs and actions only.

use super::*;
use botster_core_link::frame::Frame;
use std::time::Duration;

const TOKEN: [u8; TOKEN_LEN] = [7; TOKEN_LEN];
const EPOCH: u64 = 4;

fn instance() -> InstanceId {
    InstanceId("4-1".into())
}

fn cfg() -> WorkerConfig {
    WorkerConfig::new(instance(), TOKEN, EPOCH)
}

fn size() -> Size {
    Size {
        rows: 24,
        cols: 80,
        cell_px: None,
    }
}

fn spec() -> LaunchSpec {
    LaunchSpec {
        argv: vec!["/bin/prog".into(), "-x".into()],
        env: BTreeMap::from([("A".to_string(), "1".to_string())]),
        cwd: "/tmp".into(),
        size: size(),
        color_profile: None,
        notification_policy: NotificationPolicy::All,
        size_policy: SizePolicy::Latest,
        link_frame_bound: 1 << 16,
        stop_grace_ms: 250,
        limits: CoreLimits::default(),
    }
}

const PAYLOAD: PayloadId = PayloadId {
    pid: 4242,
    start_time: 99,
};

/// The host side of the test: it encodes frames and decodes what the worker sent.
struct World {
    worker: Worker,
    now: Instant,
    decoder: FrameDecoder,
    /// The link takes every byte at once: after each input the driver reports every byte sent so far as written.
    instant_link: bool,
    /// The bytes of every `LinkSend` that the test has collected.
    sent: u64,
}

/// The real instant that a test gives its injected clock as the start. It is the one call of the crate's tests that reads the
/// real clock, so its allowance covers that one call (the machine crates read no clock: plan 2.3c).
#[allow(clippy::disallowed_methods)]
fn real_now() -> std::time::Instant {
    std::time::Instant::now()
}

impl World {
    fn new() -> World {
        World::with(cfg())
    }

    fn with(cfg: WorkerConfig) -> World {
        let now = real_now();
        World {
            worker: Worker::new(cfg),
            now,
            decoder: FrameDecoder::new(DEFAULT_MAX_PAYLOAD),
            instant_link: true,
            sent: 0,
        }
    }

    fn feed(&mut self, input: Input) -> Vec<Action> {
        self.worker.handle(self.now, input);
        let mut actions = self.collect();
        if self.instant_link {
            self.worker
                .handle(self.now, Input::LinkWritten { total: self.sent });
            actions.extend(self.collect());
        }
        actions
    }

    /// The machine's actions, with the bytes of its sends counted.
    fn collect(&mut self) -> Vec<Action> {
        let actions: Vec<Action> = std::iter::from_fn(|| self.worker.poll_action()).collect();
        for action in &actions {
            match action {
                Action::LinkSend(bytes) => self.sent += bytes.len() as u64,
                // The new link counts its written bytes from zero, and its stream starts with a new frame.
                Action::AdoptLink(_) => {
                    self.sent = 0;
                    self.decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
                }
                _ => {}
            }
        }
        actions
    }

    /// The actions of the construction (the hello).
    fn initial(&mut self) -> Vec<Action> {
        self.collect()
    }

    fn frame(kind: FrameType, payload: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        encode_frame(kind, payload, u32::MAX, &mut out).unwrap();
        out
    }

    fn host_hello(instance: InstanceId, epoch: u64, token: [u8; TOKEN_LEN]) -> Vec<u8> {
        let hello = Hello {
            protocol: WORKER_PROTOCOL,
            proof: host_proof(&token, &instance, epoch),
            instance,
            host_epoch: epoch,
        };
        let mut payload = Vec::new();
        hello.encode(&mut payload).unwrap();
        World::frame(FrameType::HELLO, &payload)
    }

    fn msg(msg: &HostMsg) -> Vec<u8> {
        let mut payload = Vec::new();
        msg.encode(&mut payload);
        World::frame(FrameType::HOST_MSG, &payload)
    }

    fn send(&mut self, msg: &HostMsg) -> Vec<Action> {
        self.feed(Input::LinkBytes(World::msg(msg)))
    }

    /// The frames of every `LinkSend` among `actions`.
    fn frames(&mut self, actions: &[Action]) -> Vec<Frame> {
        let mut out = Vec::new();
        for action in actions {
            if let Action::LinkSend(bytes) = action {
                let mut rest = bytes.as_slice();
                while !rest.is_empty() {
                    let took = self.decoder.push(rest);
                    rest = &rest[took..];
                    if let Some(frame) = self.decoder.next_frame().unwrap() {
                        out.push(frame);
                    }
                }
            }
        }
        out
    }

    /// The reports among `actions`, decoded.
    fn reports(&mut self, actions: &[Action]) -> Vec<WorkerMsg> {
        self.frames(actions)
            .into_iter()
            .filter(|f| f.kind == FrameType::WORKER_MSG)
            .map(|f| WorkerMsg::decode(&f.payload).unwrap())
            .collect()
    }

    /// The worker's hello, then the host's: the link is ready.
    fn linked() -> World {
        let mut w = World::new();
        let first = w.initial();
        assert_eq!(w.frames(&first).len(), 1);
        let actions = w.feed(Input::LinkBytes(World::host_hello(
            instance(),
            EPOCH,
            TOKEN,
        )));
        assert!(actions.is_empty(), "{actions:?}");
        w
    }

    /// A linked worker whose payload runs.
    fn running() -> World {
        let mut w = World::linked();
        w.send(&HostMsg::Launch(Box::new(spec())));
        w.feed(Input::Spawned(Ok(PAYLOAD)));
        w
    }

    /// A running payload whose leader ended with `status`, after the drain.
    fn exited(status: ExitStatus) -> (World, Vec<WorkerMsg>) {
        let mut w = World::running();
        let mut actions = w.feed(Input::PayloadExited(status));
        actions.extend(w.feed(Input::PtyDrained));
        let reports = w.reports(&actions);
        (w, reports)
    }
}

fn signals(actions: &[Action]) -> Vec<i32> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::SignalPayload(n) => Some(*n),
            _ => None,
        })
        .collect()
}

fn reaps(actions: &[Action]) -> usize {
    actions
        .iter()
        .filter(|a| **a == Action::ReapPayload)
        .count()
}

/// AD-6, DP-8, A6-2: the first action is the hello: protocol 1, the instance, the host epoch, and the token proof bound to
/// both. The token itself is never sent.
#[test]
fn the_first_action_is_the_hello_with_the_token_proof() {
    let mut w = World::new();
    let actions = w.initial();
    let frames = w.frames(&actions);
    assert_eq!(frames.len(), 1);
    assert_eq!(frames[0].kind, FrameType::HELLO);
    let hello = Hello::decode(&frames[0].payload).unwrap();
    assert_eq!(hello.protocol, 1);
    assert_eq!(hello.instance, instance());
    assert_eq!(hello.host_epoch, EPOCH);
    assert_eq!(hello.proof, token_proof(&TOKEN, &instance(), EPOCH));
    let sent: Vec<u8> = actions
        .iter()
        .flat_map(|a| match a {
            Action::LinkSend(b) => b.clone(),
            _ => Vec::new(),
        })
        .collect();
    assert!(
        !sent.windows(TOKEN_LEN).any(|w| w == TOKEN),
        "the token never travels"
    );
}

/// Plan section 3, A6-2: the announced protocol is a construction input of the worker.
#[test]
fn the_hello_announces_the_configured_protocol() {
    let mut w = World::with(WorkerConfig {
        protocol: 9,
        ..cfg()
    });
    let actions = w.initial();
    assert_eq!(
        Hello::decode(&w.frames(&actions)[0].payload)
            .unwrap()
            .protocol,
        9
    );
}

/// AD-6: a host that does not prove the token for this instance and epoch is refused: the link closes, and a launch that
/// follows on it is never obeyed.
#[test]
fn a_host_hello_that_does_not_prove_the_token_closes_the_link() {
    let wrong = [
        World::host_hello(instance(), EPOCH, [8; TOKEN_LEN]),
        World::host_hello(InstanceId("4-2".into()), EPOCH, TOKEN),
        World::host_hello(instance(), EPOCH + 1, TOKEN),
        World::msg(&HostMsg::Stop),
        World::frame(FrameType::HELLO, b"not a hello"),
    ];
    for bytes in wrong {
        let mut w = World::new();
        w.initial();
        let mut input = bytes;
        input.extend(World::msg(&HostMsg::Launch(Box::new(spec()))));
        let actions = w.feed(Input::LinkBytes(input));
        assert_eq!(actions, [Action::LinkClose]);
        assert_eq!(w.feed(Input::LinkBytes(World::msg(&HostMsg::Stop))), []);
    }
}

/// AD-7 step 4: the payload is launched only on the host's `Launch`, with exactly the requested program, environment,
/// directory and size (A2-1).
#[test]
fn the_payload_launches_only_on_the_host_launch() {
    let mut w = World::linked();
    assert_eq!(w.feed(Input::PtyDrained), []);
    assert_eq!(w.feed(Input::EndPayload), []);
    let actions = w.send(&HostMsg::Launch(Box::new(spec())));
    assert_eq!(
        actions,
        [Action::SpawnPayload(PayloadSpec {
            argv: vec!["/bin/prog".into(), "-x".into()],
            env: BTreeMap::from([("A".to_string(), "1".to_string())]),
            cwd: "/tmp".into(),
            size: size(),
        })]
    );
    assert_eq!(
        w.send(&HostMsg::Launch(Box::new(spec()))),
        [],
        "one payload per session"
    );
}

/// The launch and the host's hello may arrive in one read: the frames are handled in order.
#[test]
fn the_hello_and_the_launch_in_one_read_are_both_handled() {
    let mut w = World::new();
    w.initial();
    let mut bytes = World::host_hello(instance(), EPOCH, TOKEN);
    bytes.extend(World::msg(&HostMsg::Launch(Box::new(spec()))));
    let actions = w.feed(Input::LinkBytes(bytes));
    assert!(
        matches!(actions.as_slice(), [Action::SpawnPayload(_)]),
        "{actions:?}"
    );
}

/// A frame split across reads is handled when it is complete.
#[test]
fn a_launch_split_across_reads_is_handled_when_complete() {
    let mut w = World::linked();
    let bytes = World::msg(&HostMsg::Launch(Box::new(spec())));
    for byte in &bytes[..bytes.len() - 1] {
        assert_eq!(w.feed(Input::LinkBytes(vec![*byte])), []);
    }
    let actions = w.feed(Input::LinkBytes(vec![bytes[bytes.len() - 1]]));
    assert!(matches!(actions.as_slice(), [Action::SpawnPayload(_)]));
}

/// LC-3, A2-6, P1 interface: a started payload is reported with the worker's features and the payload's identity.
#[test]
fn a_started_payload_is_reported_launched_with_its_identity() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
    let reports = w.reports(&actions);
    let [WorkerMsg::Launched {
        features,
        terminal,
        payload,
        ..
    }] = reports.as_slice()
    else {
        panic!("{reports:?}");
    };
    assert_eq!(*payload, PAYLOAD);
    assert_eq!(*features, BTreeSet::from([Feature::FocusReport]));
    assert_eq!(terminal.size, size());
}

/// AD-7, R-35: a worker accepts at most one `Launch` in its life, so an adoption retry that sends a `Launch` again can never
/// spawn a second payload. A `Launch` while the first spawn is in flight (R-35 (b)), after the payload runs, after it exited,
/// and after its spawn failed starts nothing.
#[test]
fn a_worker_accepts_one_launch_in_its_life() {
    let launch = || HostMsg::Launch(Box::new(spec()));
    let mut spawning = World::linked();
    assert_ne!(spawning.send(&launch()), [], "the first Launch spawns");
    assert_eq!(spawning.send(&launch()), [], "the spawn is in flight");
    let actions = spawning.feed(Input::Spawned(Ok(PAYLOAD)));
    assert_eq!(
        spawning.reports(&actions).len(),
        1,
        "one spawn answers one Launch"
    );
    let mut running = World::running();
    assert_eq!(running.send(&launch()), [], "running");
    let (mut exited, _) = World::exited(ExitStatus::Code(0));
    assert_eq!(exited.send(&launch()), [], "exited");
    let mut failed = World::linked();
    failed.send(&launch());
    failed.feed(Input::Spawned(Err(SpawnFailure::CwdMissing)));
    assert_eq!(failed.send(&launch()), [], "the spawn failed");
}

/// LC-4, A2-1: a payload that does not start is a typed failure.
#[test]
fn a_payload_that_does_not_start_is_reported_with_its_reason() {
    for (failure, reason) in [
        (SpawnFailure::CwdMissing, StartFailReason::CwdMissing),
        (
            SpawnFailure::Exec { errno: 2 },
            StartFailReason::ExecFailed { errno: 2 },
        ),
    ] {
        let mut w = World::linked();
        w.send(&HostMsg::Launch(Box::new(spec())));
        let actions = w.feed(Input::Spawned(Err(failure)));
        assert_eq!(w.reports(&actions), [WorkerMsg::LaunchFailed { reason }]);
        assert_eq!(w.feed(Input::EndPayload), [], "no group to signal");
    }
}

/// EV-4: an exit by a code carries the code and no signal; an exit by a signal carries the signal and no code.
#[test]
fn the_exit_carries_the_code_or_the_signal() {
    let (_, reports) = World::exited(ExitStatus::Code(3));
    assert_eq!(
        reports,
        [WorkerMsg::Exited {
            code: Some(3),
            signal: None
        }]
    );
    let (_, reports) = World::exited(ExitStatus::Signal(15));
    assert_eq!(
        reports,
        [WorkerMsg::Exited {
            code: None,
            signal: Some(15)
        }]
    );
}

/// EV-4, ST-5: the exit is reported only after the PTY was read to its current end, so the output written before the exit
/// is read first. The exit is reported once.
#[test]
fn the_exit_waits_for_a_drain_of_the_pty() {
    let mut w = World::running();
    assert_eq!(
        w.feed(Input::PayloadExited(ExitStatus::Code(0))),
        [Action::DrainPty]
    );
    // The tail is reported as output before the exit (ST-5, 6.2).
    let actions = w.feed(Input::PtyOutput(b"tail".to_vec()));
    assert_eq!(w.reports(&actions), [output(1)]);
    let actions = w.feed(Input::PtyDrained);
    assert_eq!(w.reports(&actions).len(), 1);
    assert_eq!(w.feed(Input::PtyDrained), []);
    assert_eq!(w.feed(Input::PayloadExited(ExitStatus::Code(0))), []);
}

/// EV-4: a drain before the exit cannot satisfy the drain required by that exit.
#[test]
fn a_drain_before_the_exit_does_not_allow_an_early_reap() {
    let mut w = World::running();
    assert_eq!(w.feed(Input::PtyDrained), []);
    assert_eq!(
        w.feed(Input::PayloadExited(ExitStatus::Code(0))),
        [Action::DrainPty]
    );
    let actions = w.send(&HostMsg::Kill);
    assert_eq!(signals(&actions), [SIGKILL]);
    assert_eq!(reaps(&actions), 0);
    assert!(w.reports(&actions).is_empty());
    let actions = w.feed(Input::PtyDrained);
    assert_eq!(reaps(&actions), 1);
    assert_eq!(
        w.reports(&actions),
        [WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        }]
    );
}

/// A frame of another kind cannot execute bytes that decode as a host message.
#[test]
fn a_host_message_in_another_frame_kind_is_ignored() {
    let mut w = World::running();
    let mut payload = Vec::new();
    HostMsg::Kill.encode(&mut payload);
    let actions = w.feed(Input::LinkBytes(World::frame(FrameType::HELLO, &payload)));
    assert_eq!(actions, []);
    assert_eq!(signals(&w.send(&HostMsg::Kill)), [SIGKILL]);
}

/// LC-5: a group kill already in progress cannot start a graceful end or a timer.
#[test]
fn end_payload_after_a_group_kill_changes_nothing() {
    let mut w = World::running();
    assert_eq!(signals(&w.send(&HostMsg::Kill)), [SIGKILL]);
    assert_eq!(w.feed(Input::EndPayload), []);
    assert_eq!(w.worker.next_deadline(), None);
}

/// LC-6 and F7: an explicit kill reaps a leader whose exit was already drained.
#[test]
fn an_explicit_kill_after_the_exit_reaps_the_leader() {
    let (mut w, _) = World::exited(ExitStatus::Code(0));
    let actions = w.send(&HostMsg::Op {
        req: 7,
        op: Op::Signal {
            id: SessionId("s".into()),
            sig: Signal::Kill,
        },
    });
    assert_eq!(signals(&actions), [SIGKILL]);
    assert_eq!(reaps(&actions), 1);
    assert_eq!(
        w.reports(&actions),
        [WorkerMsg::Done {
            req: 7,
            result: OpResult::Ok(OpOutput::Unit),
        }]
    );
}

/// LC-5 with the link: `Stop` is the graceful request to the group, and `Kill` the group kill.
#[test]
fn stop_and_kill_signal_the_payload_group() {
    let mut w = World::running();
    assert_eq!(signals(&w.send(&HostMsg::Stop)), [SIGTERM]);
    let actions = w.send(&HostMsg::Kill);
    assert_eq!(signals(&actions), [SIGKILL]);
    assert_eq!(reaps(&actions), 0, "the leader has not ended");
}

/// Lead ruling on P1 F7: the leader stays unreaped until its group kill is complete. After `Kill`, the leader is reaped once
/// it has ended and its exit was reported, and never signalled again.
#[test]
fn the_leader_is_reaped_only_after_the_group_kill() {
    let mut w = World::running();
    w.send(&HostMsg::Kill);
    let mut actions = w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    actions.extend(w.feed(Input::PtyDrained));
    assert_eq!(reaps(&actions), 1);
    let position = |wanted: &Action| actions.iter().position(|a| a == wanted).unwrap();
    let exit_report = actions
        .iter()
        .position(|a| matches!(a, Action::LinkSend(_)))
        .unwrap();
    assert!(exit_report < position(&Action::ReapPayload));
    assert!(
        signals(&w.send(&HostMsg::Stop)).is_empty(),
        "a reaped group is never signalled"
    );
}

/// F7: a leader that ended by itself keeps its group id reserved: it is not reaped until a group kill.
#[test]
fn a_leader_that_ended_alone_stays_unreaped() {
    let (mut w, _) = World::exited(ExitStatus::Code(0));
    assert_eq!(w.feed(Input::PtyDrained), []);
    let actions = w.send(&HostMsg::Kill);
    assert_eq!(signals(&actions), [SIGKILL]);
    assert_eq!(reaps(&actions), 1);
}

/// LC-5 without the link (`EndPayload`): the graceful request at once, the group kill at `stop_grace`. A repeated signal
/// changes nothing.
#[test]
fn end_payload_asks_then_kills_after_stop_grace() {
    let mut w = World::running();
    assert_eq!(w.feed(Input::LinkClosed), []);
    assert_eq!(signals(&w.feed(Input::EndPayload)), [SIGTERM]);
    let at = w.now + Duration::from_millis(250);
    assert_eq!(w.worker.next_deadline(), Some(at));
    assert_eq!(w.feed(Input::EndPayload), [], "idempotent");
    assert_eq!(
        w.worker.next_deadline(),
        Some(at),
        "a repeat does not move the grace"
    );
    w.now = at - Duration::from_millis(1);
    assert_eq!(w.feed(Input::Timer), []);
    w.now = at;
    assert_eq!(signals(&w.feed(Input::Timer)), [SIGKILL]);
    assert_eq!(w.worker.next_deadline(), None);
    let mut actions = w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    actions.extend(w.feed(Input::PtyDrained));
    assert_eq!(reaps(&actions), 1);
    assert!(w.reports(&actions).is_empty(), "no link: the exit waits");
}

/// LC-5: a `Stop` that came on the link before it broke still gets its group kill when the host repeats the stop by signal.
#[test]
fn end_payload_after_a_linked_stop_still_kills_at_the_grace() {
    let mut w = World::running();
    w.send(&HostMsg::Stop);
    w.feed(Input::LinkClosed);
    assert!(
        signals(&w.feed(Input::EndPayload)).is_empty(),
        "already asked"
    );
    w.now += Duration::from_millis(250);
    assert_eq!(signals(&w.feed(Input::Timer)), [SIGKILL]);
}

/// LC-5: when the leader already ended, `EndPayload` kills what remains of its group at once and reaps the leader.
#[test]
fn end_payload_after_the_leader_ended_kills_the_group_at_once() {
    let (mut w, _) = World::exited(ExitStatus::Code(0));
    let actions = w.feed(Input::EndPayload);
    assert_eq!(signals(&actions), [SIGKILL]);
    assert_eq!(reaps(&actions), 1);
    assert_eq!(w.worker.next_deadline(), None);
}

/// The grace set by the launch is the one that `EndPayload` uses; an older host's launch has the table default.
#[test]
fn the_grace_comes_from_the_launch() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(LaunchSpec {
        stop_grace_ms: 7000,
        ..spec()
    })));
    w.feed(Input::Spawned(Ok(PAYLOAD)));
    w.feed(Input::EndPayload);
    assert_eq!(
        w.worker.next_deadline(),
        Some(w.now + Duration::from_secs(7))
    );
}

/// LC-6, A2-1: `Signal` reaches the payload group and completes `Ok` after it was sent.
#[test]
fn a_signal_reaches_the_group_and_is_confirmed() {
    for (sig, number) in [
        (Signal::Term, 15),
        (Signal::Kill, 9),
        (Signal::Int, 2),
        (Signal::Hup, 1),
        (Signal::Other(10), 10),
    ] {
        let mut w = World::running();
        let actions = w.send(&HostMsg::Op {
            req: 5,
            op: Op::Signal {
                id: SessionId("s".into()),
                sig,
            },
        });
        assert_eq!(signals(&actions), [number]);
        assert_eq!(
            w.reports(&actions),
            [WorkerMsg::Done {
                req: 5,
                result: OpResult::Ok(OpOutput::Unit)
            }]
        );
    }
}

/// LC-6: with no live group there is nothing to signal; the answer is `SessionEnded`, and no signal is sent.
#[test]
fn a_signal_without_a_live_group_is_session_ended() {
    let mut w = World::linked();
    let actions = w.send(&HostMsg::Op {
        req: 1,
        op: Op::Signal {
            id: SessionId("s".into()),
            sig: Signal::Term,
        },
    });
    assert!(signals(&actions).is_empty());
    let reports = w.reports(&actions);
    let [WorkerMsg::Done {
        req: 1,
        result: OpResult::Err(error),
    }] = reports.as_slice()
    else {
        panic!("{reports:?}");
    };
    assert_eq!(error.code, ErrorCode::SessionEnded);
}

/// An operation of a later milestone is answered, never left without a `Done`.
#[test]
fn an_operation_of_a_later_milestone_is_answered_internal() {
    let mut w = World::running();
    let actions = w.send(&HostMsg::Op {
        req: 2,
        op: Op::ReadFacts {
            session: SessionId("s".into()),
            after: None,
        },
    });
    let reports = w.reports(&actions);
    let [WorkerMsg::Done {
        req: 2,
        result: OpResult::Err(error),
    }] = reports.as_slice()
    else {
        panic!("{reports:?}");
    };
    assert_eq!(error.code, ErrorCode::Internal);
    assert!(error.detail.contains("ReadFacts"), "{}", error.detail);
}

/// LC-7 step 3, A6-3: `Remove` of a running payload kills the group, reaps the leader once it ended, sends the complete
/// result (`Deleted`: this milestone has no uploads), closes the link and ends the worker, in this order.
#[test]
fn remove_kills_reaps_reports_and_ends_in_order() {
    let mut w = World::running();
    let actions = w.send(&HostMsg::Remove);
    assert_eq!(signals(&actions), [SIGKILL]);
    assert!(!actions.contains(&Action::Exit));
    let mut actions = w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    actions.extend(w.feed(Input::PtyDrained));
    let tail: Vec<&Action> = actions
        .iter()
        .filter(|a| !matches!(a, Action::LinkSend(_)))
        .collect();
    assert_eq!(
        tail,
        [
            &Action::DrainPty,
            &Action::ReapPayload,
            &Action::LinkClose,
            &Action::Exit
        ]
    );
    assert_eq!(
        w.reports(&actions),
        [
            WorkerMsg::Exited {
                code: None,
                signal: Some(9)
            },
            WorkerMsg::RemoveResult {
                uploads: UploadsOutcome::Deleted
            }
        ]
    );
}

/// LC-7: a worker with no payload (never launched, or failed) ends at once on `Remove`.
#[test]
fn remove_without_a_payload_ends_at_once() {
    let mut w = World::linked();
    let actions = w.send(&HostMsg::Remove);
    assert_eq!(
        w.reports(&actions),
        [WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted
        }]
    );
    assert_eq!(
        &actions[actions.len() - 2..],
        [Action::LinkClose, Action::Exit]
    );
}

/// LC-7: a `Remove` that comes while the spawn is out waits for its answer, then ends the payload.
#[test]
fn remove_during_the_spawn_waits_for_it() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    assert_eq!(w.send(&HostMsg::Remove), []);
    let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
    assert_eq!(signals(&actions), [SIGKILL]);
    let mut actions = w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    actions.extend(w.feed(Input::PtyDrained));
    assert!(actions.ends_with(&[Action::LinkClose, Action::Exit]));
}

/// A leader that ended alone is killed and reaped at `Remove`.
#[test]
fn remove_after_the_exit_kills_and_reaps() {
    let (mut w, _) = World::exited(ExitStatus::Code(1));
    let actions = w.send(&HostMsg::Remove);
    assert_eq!(signals(&actions), [SIGKILL]);
    assert_eq!(reaps(&actions), 1);
    assert!(actions.ends_with(&[Action::LinkClose, Action::Exit]));
}

/// DP-8: a closed link leaves the payload running; the exit is kept, not lost and not sent on a dead link.
#[test]
fn a_closed_link_leaves_the_payload_running() {
    let mut w = World::running();
    assert_eq!(w.feed(Input::LinkClosed), []);
    let mut actions = w.feed(Input::PayloadExited(ExitStatus::Code(0)));
    actions.extend(w.feed(Input::PtyDrained));
    assert_eq!(actions, [Action::DrainPty]);
    assert_eq!(w.feed(Input::LinkBytes(World::msg(&HostMsg::Kill))), []);
}

/// Plan section 3: a frame above the bound ends the link; the launch sets the bound of later frames.
#[test]
fn a_frame_above_the_launch_bound_closes_the_link() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(LaunchSpec {
        link_frame_bound: 64,
        ..spec()
    })));
    let mut header = Vec::new();
    header.extend_from_slice(&65u32.to_le_bytes());
    header.push(FrameType::HOST_MSG.0);
    assert_eq!(w.feed(Input::LinkBytes(header)), [Action::LinkClose]);
}

/// Plan section 3: a message that this worker cannot decode is a later host's and is ignored; the link stays.
#[test]
fn an_unknown_host_message_is_ignored() {
    let mut w = World::running();
    let bytes = World::frame(FrameType::HOST_MSG, br#"{"t":"later","x":1}"#);
    assert_eq!(w.feed(Input::LinkBytes(bytes)), []);
    assert_eq!(signals(&w.send(&HostMsg::Stop)), [SIGTERM]);
}

/// A token is a credential: the configuration never prints it.
#[test]
fn the_configuration_never_prints_the_token() {
    let shown = format!("{:?}", cfg());
    assert!(shown.contains("4-1"), "{shown}");
    assert!(!shown.contains("7, 7"), "{shown}");
}

/// F1 (EV-4, A5-2): an exit that the edge reports before the spawn's answer is kept, and reported after `Launched`.
#[test]
fn an_exit_before_the_spawn_answer_is_reported_after_the_launch() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    assert_eq!(w.feed(Input::PayloadExited(ExitStatus::Code(3))), []);
    let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
    assert!(actions.ends_with(&[Action::DrainPty]), "{actions:?}");
    let mut actions = actions;
    actions.extend(w.feed(Input::PtyDrained));
    let reports = w.reports(&actions);
    assert!(
        matches!(reports[0], WorkerMsg::Launched { .. }),
        "{reports:?}"
    );
    assert_eq!(
        reports[1..],
        [WorkerMsg::Exited {
            code: Some(3),
            signal: None
        }]
    );
}

/// F2 (LC-5): `EndPayload` that comes while the spawn is out applies when the spawn succeeds: the graceful request and the
/// grace. When the spawn fails, nothing is signalled (no proven group).
#[test]
fn end_payload_during_the_spawn_applies_after_it() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    assert_eq!(w.feed(Input::EndPayload), []);
    let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
    assert_eq!(signals(&actions), [SIGTERM]);
    assert_eq!(
        w.worker.next_deadline(),
        Some(w.now + Duration::from_millis(250))
    );
    let mut failed = World::linked();
    failed.send(&HostMsg::Launch(Box::new(spec())));
    failed.feed(Input::EndPayload);
    let actions = failed.feed(Input::Spawned(Err(SpawnFailure::CwdMissing)));
    assert!(signals(&actions).is_empty());
    assert_eq!(failed.worker.next_deadline(), None);
}

/// F2 (LC-5): `Stop` and `Kill` that come while the spawn is out apply when it succeeds; a kill wins over the request.
#[test]
fn stop_and_kill_during_the_spawn_apply_after_it() {
    for (msgs, expected) in [
        (vec![HostMsg::Stop], vec![SIGTERM]),
        (vec![HostMsg::Kill], vec![SIGKILL]),
        (vec![HostMsg::Stop, HostMsg::Kill], vec![SIGKILL]),
    ] {
        let mut w = World::linked();
        w.send(&HostMsg::Launch(Box::new(spec())));
        for msg in &msgs {
            assert!(signals(&w.send(msg)).is_empty());
        }
        assert_eq!(signals(&w.feed(Input::Spawned(Ok(PAYLOAD)))), expected);
    }
}

/// F4 (LC-7, A6-3): the close and the end wait until every byte sent before them is written. A report that covers only
/// earlier bytes (here: the exit report written, the removal result not) releases nothing; a stale report changes
/// nothing; a link that fails releases them with no close action.
#[test]
fn the_close_and_the_end_wait_for_every_byte_sent_before_them() {
    let (mut w, _) = World::exited(ExitStatus::Code(0));
    w.instant_link = false;
    let written = w.sent;
    let actions = w.send(&HostMsg::Remove);
    let sends: Vec<usize> = actions
        .iter()
        .filter_map(|a| match a {
            Action::LinkSend(b) => Some(b.len()),
            _ => None,
        })
        .collect();
    assert_eq!(sends.len(), 1, "{actions:?}");
    assert!(!actions.contains(&Action::LinkClose));
    assert_eq!(w.feed(Input::LinkWritten { total: written }), []);
    assert_eq!(w.feed(Input::LinkWritten { total: w.sent - 1 }), []);
    assert_eq!(
        w.feed(Input::LinkWritten { total: w.sent }),
        [Action::LinkClose, Action::Exit]
    );
    let mut failed = World::linked();
    failed.instant_link = false;
    failed.send(&HostMsg::Remove);
    assert_eq!(failed.feed(Input::LinkClosed), [Action::Exit]);
}

/// F4: two reports are queued (the exit report, then the removal result) and the driver reports only the first one as
/// written: the close waits for the second.
#[test]
fn a_report_of_the_first_send_does_not_release_a_close_over_the_second() {
    let mut w = World::running();
    w.instant_link = false;
    let before = w.sent;
    w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    w.feed(Input::PtyDrained);
    let first = w.sent;
    assert!(first > before, "the exit report is queued");
    let actions = w.send(&HostMsg::Kill);
    assert!(actions.contains(&Action::ReapPayload), "{actions:?}");
    let actions = w.send(&HostMsg::Remove);
    assert!(
        matches!(actions.as_slice(), [Action::LinkSend(_)]),
        "{actions:?}"
    );
    assert_eq!(w.feed(Input::LinkWritten { total: first }), []);
    assert_eq!(
        w.feed(Input::LinkWritten { total: w.sent }),
        [Action::LinkClose, Action::Exit]
    );
}

/// F4: a staged close obeys nothing more and sends nothing more.
#[test]
fn a_closing_link_obeys_and_sends_nothing() {
    let mut w = World::new();
    w.instant_link = false;
    w.initial();
    let mut bytes = World::host_hello(instance(), EPOCH, [8; TOKEN_LEN]);
    bytes.extend(World::msg(&HostMsg::Launch(Box::new(spec()))));
    assert_eq!(w.feed(Input::LinkBytes(bytes)), []);
    assert_eq!(
        w.feed(Input::LinkWritten { total: w.sent }),
        [Action::LinkClose]
    );
}

/// F7 (plan R12): `SIGTERM` kills the group of a leader that the worker holds, reaps it, and ends the worker without
/// waiting for a host that does not read.
#[test]
fn terminate_kills_the_held_group_reaps_and_ends() {
    let mut w = World::running();
    w.instant_link = false;
    assert_eq!(signals(&w.feed(Input::Terminate)), [SIGKILL]);
    let mut actions = w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    actions.extend(w.feed(Input::PtyDrained));
    assert!(
        actions.ends_with(&[Action::ReapPayload, Action::LinkClose, Action::Exit]),
        "{actions:?}"
    );
}

/// F7: a worker that holds no leader (none launched, or reaped) signals nothing at `SIGTERM` and ends at once.
#[test]
fn terminate_without_a_held_leader_signals_nothing() {
    let mut w = World::linked();
    assert_eq!(w.feed(Input::Terminate), [Action::LinkClose, Action::Exit]);
    let (mut w, _) = World::exited(ExitStatus::Code(0));
    w.send(&HostMsg::Kill);
    let actions = w.feed(Input::Terminate);
    assert!(signals(&actions).is_empty(), "{actions:?}");
    assert_eq!(actions, [Action::LinkClose, Action::Exit]);
}

fn output(rev: u64) -> WorkerMsg {
    WorkerMsg::Observed {
        observation: Observation::Output {
            model_rev: ModelRev(rev),
        },
    }
}

/// Core ST-1, 6.2: each read of the payload's output advances the session's read-visible revision and is reported, so the
/// host posts `Activity{source: Output}`. An empty read changes nothing, and two reads give two different tokens.
#[test]
fn each_output_read_advances_model_rev_and_is_reported() {
    let mut w = World::running();
    let actions = w.feed(Input::PtyOutput(b"a".to_vec()));
    assert_eq!(w.reports(&actions), [output(1)]);
    assert_eq!(w.feed(Input::PtyOutput(Vec::new())), []);
    let actions = w.feed(Input::PtyOutput(b"bc".to_vec()));
    assert_eq!(w.reports(&actions), [output(2)]);
}

/// A host that reads slowly does not grow the worker's queue: while the last `Output` report is not written, reads give no
/// report of their own. When it is written, one report of the latest revision follows; another report first sends the
/// waiting `Output`, so the reports keep the order of the events.
#[test]
fn output_reports_wait_for_the_link_and_keep_the_order() {
    let mut w = World::running();
    w.instant_link = false;
    let actions = w.feed(Input::PtyOutput(b"a".to_vec()));
    assert_eq!(w.reports(&actions), [output(1)]);
    let first = w.sent;
    for bytes in [&b"b"[..], b"c", b"d"] {
        assert_eq!(w.feed(Input::PtyOutput(bytes.to_vec())), []);
    }
    assert_eq!(w.feed(Input::LinkWritten { total: first - 1 }), []);
    let actions = w.feed(Input::LinkWritten { total: first });
    assert_eq!(w.reports(&actions), [output(4)]);
    assert_eq!(w.feed(Input::LinkWritten { total: w.sent }), []);
    let actions = w.feed(Input::PtyOutput(b"e".to_vec()));
    assert_eq!(w.reports(&actions), [output(5)]);
    assert_eq!(w.feed(Input::PtyOutput(b"f".to_vec())), []);
    w.feed(Input::PayloadExited(ExitStatus::Code(3)));
    let actions = w.feed(Input::PtyDrained);
    assert_eq!(
        w.reports(&actions),
        [
            output(6),
            WorkerMsg::Exited {
                code: Some(3),
                signal: None
            }
        ]
    );
}

/// A worker whose link is gone reports nothing (DP-8): it still reads the output, so the payload never blocks.
#[test]
fn output_after_the_link_closed_is_read_and_not_sent() {
    let mut w = World::running();
    assert!(w
        .feed(Input::LinkClosed)
        .iter()
        .all(|a| !matches!(a, Action::LinkSend(_))));
    assert_eq!(w.feed(Input::PtyOutput(b"a".to_vec())), []);
}

use botster_route_codec::prelude::HexBytes;

// ---- the admission point and the host's writes (AM-2, IN-1 to IN-10) ----

fn write(req: u64, bytes: &[u8], guard: Option<Guard>) -> HostMsg {
    HostMsg::Op {
        req,
        op: Op::WriteInput {
            session: SessionId("s".into()),
            payload: InputPayload::Bytes {
                bytes: HexBytes(bytes.to_vec()),
            },
            guard,
        },
    }
}

fn pty_writes(actions: &[Action]) -> Vec<Vec<u8>> {
    actions
        .iter()
        .filter_map(|a| match a {
            Action::PtyWrite(b) => Some(b.clone()),
            _ => None,
        })
        .collect()
}

/// The `InputResult` of the `Done` of `req` among the reports of `actions`.
fn input_result(w: &mut World, actions: &[Action], req: u64) -> Option<InputResult> {
    w.reports(actions).into_iter().find_map(|m| match m {
        WorkerMsg::Done {
            req: r,
            result: OpResult::Ok(OpOutput::Input(result)),
        } if r == req => Some(result),
        _ => None,
    })
}

fn outcome(result: &InputResult) -> (WriteOutcome, u64, u64) {
    (
        result.outcome,
        result.payload_bytes_written,
        result.pty_bytes_written,
    )
}

/// AM-2, IN-2, IN-3: a write is one transaction across short writes: the rest of it goes to the PTY after each count, and
/// its `Done` comes when the last byte is taken, with exact counts.
#[test]
fn a_write_is_one_transaction_across_short_writes() {
    let mut w = World::running();
    let actions = w.send(&write(1, b"abcde", None));
    assert_eq!(pty_writes(&actions), [b"abcde".to_vec()]);
    let actions = w.feed(Input::PtyWritten(Ok(2)));
    assert_eq!(pty_writes(&actions), [b"cde".to_vec()]);
    assert!(input_result(&mut w, &actions, 1).is_none());
    let actions = w.feed(Input::PtyWritten(Ok(3)));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Written, 5, 5));
}

/// AM-2: the next write starts only after the previous one completed; host writes are admitted in their order, and each
/// admission advances the host's revision and reports it (IN-10, IN-4).
#[test]
fn writes_are_admitted_one_at_a_time_in_order() {
    let mut w = World::running();
    let first = w.send(&write(1, b"ab", None));
    let second = w.send(&write(2, b"cd", None));
    assert_eq!(pty_writes(&first), [b"ab".to_vec()]);
    assert!(
        pty_writes(&second).is_empty(),
        "the first owns the PTY input"
    );
    let actions = w.feed(Input::PtyWritten(Ok(2)));
    assert_eq!(pty_writes(&actions), [b"cd".to_vec()]);
    let reports = w.reports(&actions);
    assert!(
        matches!(reports[0], WorkerMsg::Done { req: 1, .. }),
        "{reports:?}"
    );
    assert_eq!(
        reports[1],
        WorkerMsg::Observed {
            observation: Observation::HostInput {
                input_rev: InputRev(2)
            }
        }
    );
}

/// The PTY takes nothing now: the rest waits for `PtyWritable`, with no spin.
#[test]
fn a_full_pty_waits_for_writability() {
    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    assert!(pty_writes(&w.feed(Input::PtyWritten(Ok(0)))).is_empty());
    assert!(pty_writes(&w.feed(Input::PtyOutput(b"x".to_vec()))).is_empty());
    assert_eq!(pty_writes(&w.feed(Input::PtyWritable)), [b"abc".to_vec()]);
}

/// AM-2: one PTY write is out at a time: a `PtyWritable` while a write is out hands the PTY nothing more.
#[test]
fn a_writable_pty_gets_no_second_write_while_one_is_out() {
    let mut w = World::running();
    assert_eq!(
        pty_writes(&w.send(&write(1, b"abc", None))),
        [b"abc".to_vec()]
    );
    assert!(pty_writes(&w.feed(Input::PtyWritable)).is_empty());
    let actions = w.feed(Input::PtyWritten(Ok(3)));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Written, 3, 3));
}

/// IN-6: a queued write is cancelled with exact zero and never reaches the PTY; the active one keeps its order.
#[test]
fn a_cancel_of_a_queued_write_is_an_exact_zero() {
    let mut w = World::running();
    w.send(&write(1, b"ab", None));
    w.send(&write(2, b"cd", None));
    let actions = w.send(&HostMsg::Cancel { req: 2 });
    let result = input_result(&mut w, &actions, 2).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Cancelled, 0, 0));
    let actions = w.feed(Input::PtyWritten(Ok(2)));
    assert!(
        pty_writes(&actions).is_empty(),
        "the cancelled write never starts"
    );
}

/// IN-6, IN-2: a cancel during a write waits for the count of the write that is out, and reports exactly what was written;
/// a write that finished first reports its real outcome.
#[test]
fn a_cancel_during_a_write_reports_exact_progress() {
    let mut w = World::running();
    w.send(&write(1, b"abcde", None));
    let actions = w.send(&HostMsg::Cancel { req: 1 });
    assert!(
        input_result(&mut w, &actions, 1).is_none(),
        "a write is out"
    );
    let actions = w.feed(Input::PtyWritten(Ok(2)));
    assert!(pty_writes(&actions).is_empty());
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Cancelled, 2, 2));

    let mut w = World::running();
    w.send(&write(1, b"ab", None));
    w.send(&HostMsg::Cancel { req: 1 });
    let actions = w.feed(Input::PtyWritten(Ok(2)));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Written, 2, 2));

    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    w.feed(Input::PtyWritten(Ok(1)));
    w.feed(Input::PtyWritten(Ok(0)));
    let actions = w.send(&HostMsg::Cancel { req: 1 });
    let result = input_result(&mut w, &actions, 1).expect("no write is out");
    assert_eq!(outcome(&result), (WriteOutcome::Cancelled, 1, 1));
}

/// IN-2: a PTY write that fails reports `Failed` with the exact counts up to the failure.
#[test]
fn a_failed_pty_write_is_failed_with_exact_counts() {
    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    w.feed(Input::PtyWritten(Ok(1)));
    let actions = w.feed(Input::PtyWritten(Err(5)));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Failed, 1, 1));
}

/// IN-2, IN-7: the payload's end makes a started write `Partial` with exact counts, a write that wrote nothing a certain
/// zero, and a write after it `NotWritten(SessionEnded)`.
#[test]
fn the_payload_end_ends_writes_with_exact_counts() {
    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    w.feed(Input::PtyWritten(Ok(1)));
    w.feed(Input::PtyWritten(Ok(0)));
    let actions = w.feed(Input::PayloadExited(ExitStatus::Code(0)));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Partial, 1, 1));
    let actions = w.send(&write(2, b"x", None));
    let result = input_result(&mut w, &actions, 2).expect("done");
    assert_eq!(
        outcome(&result),
        (
            WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
            0,
            0
        )
    );

    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    w.feed(Input::PayloadExited(ExitStatus::Code(0)));
    let actions = w.feed(Input::PtyWritten(Err(5)));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(
        outcome(&result),
        (
            WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
            0,
            0
        )
    );
}

/// IN-2: a write that starts while the payload is being stopped is `NotWritten(Stopping)`.
#[test]
fn a_write_during_a_stop_is_not_written_stopping() {
    let mut w = World::running();
    w.send(&HostMsg::Stop);
    let actions = w.send(&write(1, b"x", None));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(
        outcome(&result),
        (WriteOutcome::NotWritten(NotWrittenReason::Stopping), 0, 0)
    );
    assert!(pty_writes(&actions).is_empty());
}

/// IN-10: a write after a kill is `Stopping` too: a killed payload takes no input, with or without a grace before it.
#[test]
fn a_write_after_a_kill_is_not_written_stopping() {
    let mut w = World::running();
    w.send(&HostMsg::Kill);
    let actions = w.send(&write(1, b"x", None));
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(
        outcome(&result),
        (WriteOutcome::NotWritten(NotWrittenReason::Stopping), 0, 0)
    );
    assert!(pty_writes(&actions).is_empty());
}

/// IN-10: a guard of the client class compares the client revision: it passes at that revision, and is `Stale` at
/// another, whatever the host wrote.
#[test]
fn a_client_guard_compares_the_client_revision() {
    let mut w = World::running();
    let client = |rev| Guard {
        input: Some(InputGuard {
            source_class: SourceClass::Client,
            rev: InputRev(rev),
        }),
        model_rev: None,
    };
    w.send(&write(1, b"a", None));
    w.feed(Input::PtyWritten(Ok(1)));
    let actions = w.send(&write(2, b"b", Some(client(0))));
    assert_eq!(
        pty_writes(&actions),
        [b"b".to_vec()],
        "no client input came"
    );
    w.feed(Input::PtyWritten(Ok(1)));
    let actions = w.send(&write(3, b"c", Some(client(1))));
    let result = input_result(&mut w, &actions, 3).expect("done");
    assert_eq!(
        outcome(&result),
        (WriteOutcome::NotWritten(NotWrittenReason::Stale), 0, 0)
    );
}

/// IN-10: the input guard passes iff no input of its class was admitted after its revision; the guard of a queued write is
/// checked at its start, so a write admitted before it makes it `Stale` with exact zero.
#[test]
fn the_input_guard_is_checked_at_the_start() {
    let mut w = World::running();
    let guard = |rev| Guard {
        input: Some(InputGuard {
            source_class: SourceClass::Host,
            rev: InputRev(rev),
        }),
        model_rev: None,
    };
    w.send(&write(1, b"a", None));
    // Queued behind write 1, with the revision from before it.
    let actions = w.send(&write(2, b"b", Some(guard(0))));
    assert!(input_result(&mut w, &actions, 2).is_none(), "queued");
    let actions = w.feed(Input::PtyWritten(Ok(1)));
    let result = input_result(&mut w, &actions, 2).expect("stale at its start");
    assert_eq!(
        outcome(&result),
        (WriteOutcome::NotWritten(NotWrittenReason::Stale), 0, 0)
    );
    assert!(pty_writes(&actions).is_empty());
    let actions = w.send(&write(3, b"c", Some(guard(1))));
    assert_eq!(
        pty_writes(&actions),
        [b"c".to_vec()],
        "unchanged: it passes"
    );
}

/// IN-10: the terminal guard passes iff the model's revision is unchanged; output moves it.
#[test]
fn the_terminal_guard_refuses_after_output() {
    let mut w = World::running();
    let guard = |rev| Guard {
        input: None,
        model_rev: Some(ModelRev(rev)),
    };
    let actions = w.send(&write(1, b"a", Some(guard(0))));
    assert_eq!(pty_writes(&actions), [b"a".to_vec()]);
    w.feed(Input::PtyWritten(Ok(1)));
    w.feed(Input::PtyOutput(b"redraw".to_vec()));
    let actions = w.send(&write(2, b"b", Some(guard(0))));
    let result = input_result(&mut w, &actions, 2).expect("done");
    assert_eq!(
        outcome(&result),
        (WriteOutcome::NotWritten(NotWrittenReason::Stale), 0, 0)
    );
}

/// IN-1, IN-9: `Text` is written as its UTF-8 bytes; an empty payload completes `Written` with zero counts.
#[test]
fn text_is_its_utf8_and_an_empty_write_is_written() {
    let mut w = World::running();
    let actions = w.send(&HostMsg::Op {
        req: 1,
        op: Op::WriteInput {
            session: SessionId("s".into()),
            payload: InputPayload::Text { text: "é!".into() },
            guard: None,
        },
    });
    assert_eq!(pty_writes(&actions), ["é!".as_bytes().to_vec()]);
    w.feed(Input::PtyWritten(Ok(3)));
    let actions = w.send(&write(2, b"", None));
    let result = input_result(&mut w, &actions, 2).expect("done");
    assert_eq!(outcome(&result), (WriteOutcome::Written, 0, 0));
    assert!(pty_writes(&actions).is_empty());
}

/// A write before the spawn's answer waits for it, then starts on the live payload.
#[test]
fn a_write_waits_for_the_spawn() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    assert!(pty_writes(&w.send(&write(1, b"a", None))).is_empty());
    assert_eq!(
        pty_writes(&w.feed(Input::Spawned(Ok(PAYLOAD)))),
        [b"a".to_vec()]
    );
}

// ---- the terminal model (libghostty) ----
// Expected terminal values come from an independent libghostty terminal (the oracle, R-7) or from the clause's relations;
// no expected terminal byte or state is written by hand. Only the program's output bytes are inputs.

use botster_terminal_ghostty::{History, Terminal};

fn oracle() -> Terminal {
    Terminal::new(&size(), History::On).expect("a terminal")
}

fn observations(w: &mut World, actions: &[Action]) -> Vec<Observation> {
    w.reports(actions)
        .into_iter()
        .filter_map(|m| match m {
            WorkerMsg::Observed { observation } => Some(observation),
            _ => None,
        })
        .collect()
}

fn op(w: &mut World, req: u64, op: Op) -> OpResult {
    let actions = w.send(&HostMsg::Op { req, op });
    w.reports(&actions)
        .into_iter()
        .find_map(|m| match m {
            WorkerMsg::Done { req: r, result } if r == req => Some(result),
            _ => None,
        })
        .expect("a Done")
}

fn sid() -> SessionId {
    SessionId("s".into())
}

/// ST-4, ST-6: the launch carries the model's state (a fresh model's modes, and no title or cwd) and the binding's snapshot
/// format.
#[test]
fn the_launch_carries_the_models_state() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
    let reports = w.reports(&actions);
    let [WorkerMsg::Launched {
        terminal, formats, ..
    }] = reports.as_slice()
    else {
        panic!("{reports:?}");
    };
    assert_eq!(terminal.modes, oracle().modes());
    assert_eq!(
        (terminal.title.as_ref(), terminal.cwd.as_ref()),
        (None, None)
    );
    assert_eq!(formats, &[botster_terminal_ghostty::snapshot_format()]);
}

/// E2-1, EV-1, EV-7: OSC 2 gives a title; OSC 1 gives none (and stays in the output, the model's concern). A BEL is a bell.
/// Each step's output advances model_rev, and an `Output` is reported.
#[test]
fn title_bell_and_output_reach_the_host() {
    let mut w = World::running();
    let actions = w.feed(Input::PtyOutput(b"\x1b]2;hello\x07".to_vec()));
    let obs = observations(&mut w, &actions);
    assert!(
        obs.iter()
            .any(|o| matches!(o, Observation::Title { title, .. } if title == "hello")),
        "{obs:?}"
    );
    assert!(obs.iter().any(|o| matches!(o, Observation::Output { .. })));
    let actions = w.feed(Input::PtyOutput(b"\x1b]1;icon\x07".to_vec()));
    let obs = observations(&mut w, &actions);
    assert!(
        !obs.iter().any(|o| matches!(o, Observation::Title { .. })),
        "{obs:?}"
    );
    let actions = w.feed(Input::PtyOutput(b"\x07".to_vec()));
    assert!(observations(&mut w, &actions)
        .iter()
        .any(|o| matches!(o, Observation::Bell)));
}

/// Plan 2.4, rule 1: the model keeps an ESC that may start the string terminator of an unfinished string. A read of only
/// that ESC completes no step and changes nothing yet: no revision and no report. The next byte completes it, in order. The
/// oracle decides which bytes the model keeps.
#[test]
fn a_read_that_completes_no_step_waits_for_the_rest() {
    let (open, esc, rest) = (b"\x1b]2;t", b"\x1b", b"\\");
    let mut expected = oracle();
    assert_eq!(
        expected.vt_write_until_query(open).unwrap().consumed,
        open.len()
    );
    assert_eq!(
        expected.vt_write_until_query(esc).unwrap().consumed,
        0,
        "the oracle keeps the ESC"
    );
    let mut whole = esc.to_vec();
    whole.extend_from_slice(rest);
    expected.vt_write(&whole);
    let mut w = World::running();
    let rev = |result: OpResult| match result {
        OpResult::Ok(OpOutput::Modes(modes)) => modes.model_rev,
        other => panic!("{other:?}"),
    };
    w.feed(Input::PtyOutput(open.to_vec()));
    let before = rev(op(&mut w, 1, Op::ReadModeFlags { session: sid() }));
    let actions = w.feed(Input::PtyOutput(esc.to_vec()));
    assert_eq!(w.reports(&actions), []);
    let after = rev(op(&mut w, 2, Op::ReadModeFlags { session: sid() }));
    assert_eq!(before, after, "no step, no change");
    let actions = w.feed(Input::PtyOutput(rest.to_vec()));
    let title = expected.title();
    assert!(!title.is_empty());
    assert!(observations(&mut w, &actions)
        .iter()
        .any(|o| matches!(o, Observation::Title { title: t, .. } if *t == title)));
}

/// E2-3: a step whose final flags differ posts one `ModesChanged` with the model's flags; a step that sets and resets the
/// same mode posts none.
#[test]
fn modes_changed_compares_the_final_flags_of_a_step() {
    let mut w = World::running();
    let set = b"\x1b[?2004h";
    let actions = w.feed(Input::PtyOutput(set.to_vec()));
    let modes: Vec<ModeFlags> = observations(&mut w, &actions)
        .into_iter()
        .filter_map(|o| match o {
            Observation::Modes { flags, .. } => Some(flags),
            _ => None,
        })
        .collect();
    let mut expected = oracle();
    expected.vt_write(set);
    assert_eq!(modes, [expected.modes()]);
    assert_ne!(modes[0], oracle().modes(), "the mode changed");
    let actions = w.feed(Input::PtyOutput(b"\x1b[?2004l\x1b[?2004h".to_vec()));
    assert!(!observations(&mut w, &actions)
        .iter()
        .any(|o| matches!(o, Observation::Modes { .. })));
}

/// ST-1, ST-2, ST-3: the reads come from the model, with the model's revision; the cursor's coordinates are cells; the row
/// text is trimmed and the text before the cursor is not.
#[test]
fn reads_come_from_the_model() {
    let mut w = World::running();
    w.feed(Input::PtyOutput(b"ab  ".to_vec()));
    let mut expected = oracle();
    expected.vt_write(b"ab  ");
    let OpResult::Ok(OpOutput::Screen(screen)) = op(
        &mut w,
        1,
        Op::ReadScreen {
            session: sid(),
            history: false,
        },
    ) else {
        panic!("a screen");
    };
    assert_eq!(screen.text, expected.screen_text(false).unwrap().text);
    assert_eq!(
        (screen.rows, screen.cols),
        (u32::from(expected.rows()), u32::from(expected.cols()))
    );
    let OpResult::Ok(OpOutput::Cursor(cursor)) = op(&mut w, 2, Op::ReadCursor { session: sid() })
    else {
        panic!("a cursor");
    };
    let at = expected.cursor();
    assert_eq!(
        (cursor.row, cursor.col, cursor.visible),
        (at.row, at.col, at.visible)
    );
    assert_eq!(cursor.row_text, "ab", "trailing spaces are trimmed");
    assert_eq!(
        cursor.text_before_cursor.len(),
        cursor.col as usize,
        "never trimmed"
    );
    assert_eq!(
        cursor.model_rev, screen.model_rev,
        "no change between the reads"
    );
    let OpResult::Ok(OpOutput::Modes(modes)) = op(&mut w, 3, Op::ReadModeFlags { session: sid() })
    else {
        panic!("modes");
    };
    assert_eq!(modes.flags, expected.modes());
    assert_eq!(modes.model_rev, screen.model_rev);
}

fn paste(req: u64, require_bracketed: bool) -> HostMsg {
    HostMsg::Op {
        req,
        op: Op::WriteInput {
            session: sid(),
            payload: InputPayload::Paste {
                bytes: HexBytes(b"hi".to_vec()),
                require_bracketed,
            },
            guard: None,
        },
    }
}

/// IN-8, IN-2: a paste is bare while bracketed paste is off, refused with exact zero when it is required, and wrapped by the
/// model's frame once the mode is on; the payload count excludes the markers, the PTY count includes them.
#[test]
fn a_paste_follows_the_bracketed_mode_at_its_start() {
    let mut w = World::running();
    assert_eq!(pty_writes(&w.send(&paste(1, false))), [b"hi".to_vec()]);
    w.feed(Input::PtyWritten(Ok(2)));
    let actions = w.send(&paste(2, true));
    let result = input_result(&mut w, &actions, 2).expect("done");
    assert_eq!(
        outcome(&result),
        (
            WriteOutcome::NotWritten(NotWrittenReason::ModePreconditionFailed),
            0,
            0
        )
    );
    w.feed(Input::PtyOutput(b"\x1b[?2004h".to_vec()));
    let mut expected = oracle();
    expected.vt_write(b"\x1b[?2004h");
    let (start, end) = expected.paste_frame().expect("the mode is on");
    let mut wrapped = start.clone();
    wrapped.extend_from_slice(b"hi");
    wrapped.extend_from_slice(&end);
    assert_eq!(pty_writes(&w.send(&paste(3, true))), [wrapped.clone()]);
    let actions = w.feed(Input::PtyWritten(Ok(wrapped.len())));
    let result = input_result(&mut w, &actions, 3).expect("done");
    assert_eq!(
        outcome(&result),
        (WriteOutcome::Written, 2, wrapped.len() as u64)
    );
}

/// IN-9: focus without focus reporting is a certain zero (`NotReported`).
#[test]
fn focus_needs_the_reporting_mode() {
    let mut w = World::running();
    let actions = w.send(&HostMsg::Op {
        req: 1,
        op: Op::WriteInput {
            session: sid(),
            payload: InputPayload::Focus { focused: true },
            guard: None,
        },
    });
    let result = input_result(&mut w, &actions, 1).expect("done");
    assert_eq!(
        outcome(&result),
        (
            WriteOutcome::NotWritten(NotWrittenReason::NotReported),
            0,
            0
        )
    );
}

/// EV-8 (no route yet): a query is answered by the model's shadow reply, written to the PTY as a transaction that advances
/// no input revision and completes no operation. A host write that came first is written first (arrival order, AM-2).
#[test]
fn a_query_with_no_route_is_answered_by_the_shadow_reply() {
    let mut w = World::running();
    let query = b"\x1b[c";
    let mut expected = oracle();
    let step = expected.vt_write_until_query(query).unwrap();
    let reply = step.query.expect("a query").shadow_reply;
    assert!(!reply.is_empty());
    let actions = w.feed(Input::PtyOutput(query.to_vec()));
    assert_eq!(pty_writes(&actions), std::slice::from_ref(&reply));
    assert!(!w.reports(&actions).iter().any(|m| matches!(
        m,
        WorkerMsg::Done { .. }
            | WorkerMsg::Observed {
                observation: Observation::HostInput { .. }
            }
    )));
    // The reply's write completes, and no `Done` follows: it is no operation.
    let actions = w.feed(Input::PtyWritten(Ok(reply.len())));
    assert!(!w
        .reports(&actions)
        .iter()
        .any(|m| matches!(m, WorkerMsg::Done { .. })));
    // A host write that is out when the query comes is written first; the reply follows it.
    assert_eq!(pty_writes(&w.send(&write(1, b"x", None))), [b"x".to_vec()]);
    assert!(pty_writes(&w.feed(Input::PtyOutput(query.to_vec()))).is_empty());
    let actions = w.feed(Input::PtyWritten(Ok(1)));
    assert_eq!(pty_writes(&actions), [reply]);
    assert!(input_result(&mut w, &actions, 1).is_some());
}

/// A2-4: a notification's body is bounded by clipboard_bytes, cut at a character boundary, and flagged.
#[test]
fn a_long_notification_is_truncated_and_flagged() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(LaunchSpec {
        limits: CoreLimits {
            clipboard_bytes: 4,
            ..CoreLimits::default()
        },
        ..spec()
    })));
    w.feed(Input::Spawned(Ok(PAYLOAD)));
    let actions = w.feed(Input::PtyOutput(
        "\x1b]9;héllo world\x07".as_bytes().to_vec(),
    ));
    let obs = observations(&mut w, &actions);
    let Some(Observation::Notification {
        body, truncated, ..
    }) = obs
        .iter()
        .find(|o| matches!(o, Observation::Notification { .. }))
    else {
        panic!("{obs:?}");
    };
    assert!(*truncated);
    assert!(
        body.len() <= 4 && "héllo world".starts_with(body.as_str()),
        "{body:?}"
    );
}

fn capture(w: &mut World, req: u64) -> Vec<WorkerMsg> {
    let actions = w.send(&HostMsg::Op {
        req,
        op: Op::CaptureSnapshot {
            session: sid(),
            owner: ClientId("c".into()),
        },
    });
    w.reports(&actions)
}

/// ST-6: a capture is the model's snapshot at this point, as one page (index 0, last), and then its `Capture` with the
/// page count, the byte count and the model's revision. The bytes are the oracle's snapshot of the same output.
#[test]
fn a_capture_is_the_models_snapshot_as_one_page() {
    let mut w = World::running();
    w.feed(Input::PtyOutput(b"hello\r\nworld".to_vec()));
    let OpResult::Ok(OpOutput::Modes(modes)) = op(&mut w, 1, Op::ReadModeFlags { session: sid() })
    else {
        panic!("modes");
    };
    let mut expected = oracle();
    expected.vt_write(b"hello\r\nworld");
    let snapshot = expected.snapshot().expect("a snapshot");
    let reports = capture(&mut w, 2);
    let [WorkerMsg::Pages { req: 2, pages }, WorkerMsg::Done {
        req: 2,
        result: OpResult::Ok(OpOutput::Capture(done)),
    }] = reports.as_slice()
    else {
        panic!("{reports:?}");
    };
    assert_eq!(
        pages,
        &[Page {
            index: 0,
            bytes: HexBytes(snapshot.clone()),
            last: true,
        }]
    );
    assert_eq!(
        (done.page_count, done.total_bytes, done.model_rev),
        (1, snapshot.len() as u64, modes.model_rev)
    );
}

/// ST-6: a snapshot over `max_snapshot_bytes` is `SnapshotTooLarge`, and no page is sent.
#[test]
fn a_capture_over_the_bound_is_snapshot_too_large_with_no_page() {
    let size = oracle().snapshot().expect("a snapshot").len() as u64;
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(LaunchSpec {
        limits: CoreLimits {
            max_snapshot_bytes: size - 1,
            ..CoreLimits::default()
        },
        ..spec()
    })));
    w.feed(Input::Spawned(Ok(PAYLOAD)));
    let reports = capture(&mut w, 1);
    let [WorkerMsg::Done {
        req: 1,
        result: OpResult::Err(error),
    }] = reports.as_slice()
    else {
        panic!("{reports:?}");
    };
    assert_eq!(error.code, ErrorCode::SnapshotTooLarge);
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(LaunchSpec {
        limits: CoreLimits {
            max_snapshot_bytes: size,
            ..CoreLimits::default()
        },
        ..spec()
    })));
    w.feed(Input::Spawned(Ok(PAYLOAD)));
    assert!(
        matches!(capture(&mut w, 1).as_slice(), [WorkerMsg::Pages { .. }, _]),
        "a snapshot at the bound is sent"
    );
}

/// The link's rule (no observation before `Launched`) and ST-4: output that comes while the spawn's answer is out waits in
/// the model. `Launched` carries the fresh model's state; the waiting output is fed after it, in order.
#[test]
fn output_before_the_spawns_answer_is_fed_after_launched() {
    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    let actions = w.feed(Input::PtyOutput(b"\x1b]2;early\x07".to_vec()));
    assert_eq!(w.reports(&actions), [], "nothing before the spawn's answer");
    let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
    let reports = w.reports(&actions);
    let Some(WorkerMsg::Launched { terminal, .. }) = reports.first() else {
        panic!("{reports:?}");
    };
    assert_eq!(terminal.title, None, "the fresh model's state");
    assert!(
        reports.iter().any(|m| matches!(
            m,
            WorkerMsg::Observed {
                observation: Observation::Title { title, .. }
            } if title == "early"
        )),
        "{reports:?}"
    );
    assert!(reports.iter().any(|m| matches!(
        m,
        WorkerMsg::Observed {
            observation: Observation::Output { .. }
        }
    )));
}

/// EV-8: a reply goes only to a live payload. A query in the output that is read after the payload ended gets no reply.
#[test]
fn no_reply_is_written_after_the_payload_ended() {
    let mut w = World::running();
    assert_eq!(
        w.feed(Input::PayloadExited(ExitStatus::Code(0))),
        [Action::DrainPty]
    );
    let actions = w.feed(Input::PtyOutput(b"\x1b[c".to_vec()));
    assert!(pty_writes(&actions).is_empty(), "{actions:?}");
}

/// The model's debug form names what waits, never the terminal's contents.
#[test]
fn the_models_debug_form_names_its_unfed_bytes() {
    let w = World::running();
    let shown = format!("{:?}", w.worker);
    assert!(shown.contains("Model { unfed: 0"), "{shown}");
}

fn semantic(req: u64, payload: &str) -> HostMsg {
    HostMsg::Op {
        req,
        op: Op::WriteInput {
            session: sid(),
            payload: serde_json::from_str(payload).expect("a payload"),
            guard: None,
        },
    }
}

/// IN-9: a key and a mouse event are encoded by the model with its modes at their start. The expected bytes, or the typed
/// zero, are the oracle's encoding with the same modes.
#[test]
fn key_and_mouse_events_are_encoded_by_the_model() {
    let key = r#"{"key": {"key": {"char": "a"}, "mods": [], "event": "press", "text": "a"}}"#;
    let mouse =
        r#"{"mouse": {"action": "press", "button": "left", "row": 0, "col": 3, "mods": []}}"#;
    let modes = b"\x1b[?1000h\x1b[?1006h";
    for (req, payload, output) in [
        (1, key, &b""[..]),
        (2, mouse, &b""[..]),
        (3, mouse, &modes[..]),
    ] {
        let mut w = World::running();
        let mut expected = oracle();
        if !output.is_empty() {
            w.feed(Input::PtyOutput(output.to_vec()));
            expected.vt_write(output);
        }
        let parsed: InputPayload = serde_json::from_str(payload).unwrap();
        let want = match &parsed {
            InputPayload::Key(k) => expected.encode_key(k),
            InputPayload::Mouse(m) => expected.encode_mouse(m),
            other => panic!("{other:?}"),
        };
        let actions = w.send(&semantic(req, payload));
        match want {
            Ok(bytes) => {
                assert!(!bytes.is_empty());
                assert_eq!(pty_writes(&actions), [bytes], "request {req}");
            }
            Err(_) => {
                let result = input_result(&mut w, &actions, req).expect("done");
                assert!(
                    matches!(result.outcome, WriteOutcome::NotWritten(_)),
                    "request {req}: {result:?}"
                );
                assert_eq!(
                    (result.payload_bytes_written, result.pty_bytes_written),
                    (0, 0)
                );
            }
        }
    }
}

/// The `input_rev` of each `HostInput` that `actions` report.
fn host_input_revs(w: &mut World, actions: &[Action]) -> Vec<InputRev> {
    observations(w, actions)
        .into_iter()
        .filter_map(|o| match o {
            Observation::HostInput { input_rev } => Some(input_rev),
            _ => None,
        })
        .collect()
}

/// A13-1b: each OSC 5522 acknowledgement is the model's own (an independent terminal's answer to the same output). Each
/// is one contiguous transaction at the admission point, in the order of its write. It waits behind the host write that
/// is out, a host write that comes later waits behind it, and it advances no `input_rev` and completes no operation.
#[test]
fn osc_5522_acknowledgements_are_written_in_order_through_the_admission_point() {
    let commit = |id: u32| {
        format!(
            "\x1b]5522;type=write:id={id}\x1b\\\x1b]5522;type=wdata:mime=dGV4dC9wbGFpbg==;aGk=\x1b\\\x1b]5522;type=wdata\x1b\\"
        )
    };
    let output = [commit(1), commit(2), commit(3)].concat().into_bytes();
    let mut expected = oracle();
    expected.vt_write(&output);
    let acks = expected.drain_events().clipboard_acks;
    assert_eq!(acks.len(), 3);
    assert!(acks.iter().all(|ack| ack.len() > 1));

    let mut w = World::running();
    let actions = w.send(&write(1, b"x", None));
    assert_eq!(pty_writes(&actions), [b"x".to_vec()]);
    let first_rev = host_input_revs(&mut w, &actions);
    assert_eq!(first_rev.len(), 1);
    let actions = w.feed(Input::PtyOutput(output));
    assert!(
        pty_writes(&actions).is_empty(),
        "the host write owns the PTY input"
    );
    let writes = observations(&mut w, &actions)
        .into_iter()
        .filter(|o| matches!(o, Observation::ClipboardWrite { .. }))
        .count();
    assert_eq!(writes, 3);

    let actions = w.feed(Input::PtyWritten(Ok(1)));
    assert_eq!(pty_writes(&actions), [acks[0].clone()]);
    assert!(input_result(&mut w, &actions, 1).is_some());
    assert!(host_input_revs(&mut w, &actions).is_empty());
    // A host write that comes now waits for every acknowledgement.
    assert!(pty_writes(&w.send(&write(2, b"y", None))).is_empty());
    // A partial write keeps the acknowledgement contiguous: its rest comes next.
    let actions = w.feed(Input::PtyWritten(Ok(1)));
    assert_eq!(pty_writes(&actions), [acks[0][1..].to_vec()]);
    let mut actions = w.feed(Input::PtyWritten(Ok(acks[0].len() - 1)));
    for ack in &acks[1..] {
        assert_eq!(pty_writes(&actions), std::slice::from_ref(ack));
        assert!(!w.reports(&actions).iter().any(|m| matches!(
            m,
            WorkerMsg::Done { .. }
                | WorkerMsg::Observed {
                    observation: Observation::HostInput { .. }
                }
        )));
        actions = w.feed(Input::PtyWritten(Ok(ack.len())));
    }
    assert_eq!(pty_writes(&actions), [b"y".to_vec()]);
    assert_eq!(
        host_input_revs(&mut w, &actions),
        [InputRev(first_rev[0].0 + 1)],
        "the acknowledgements advanced no input_rev"
    );
    let actions = w.feed(Input::PtyWritten(Ok(1)));
    assert!(input_result(&mut w, &actions, 2).is_some());
}

/// ST-3, R-7: the cursor's row text is the model's cells as they are. The second cell of a wide character has no text,
/// so it adds nothing to the row or to the text before the cursor.
#[test]
fn a_wide_character_adds_no_text_for_its_second_cell() {
    let output = "a日b".as_bytes();
    let mut w = World::running();
    w.feed(Input::PtyOutput(output.to_vec()));
    let mut expected = oracle();
    expected.vt_write(output);
    let at = expected.cursor();
    let cells = expected.row_cells(at.row).expect("the cursor's row");
    let OpResult::Ok(OpOutput::Cursor(cursor)) = op(&mut w, 1, Op::ReadCursor { session: sid() })
    else {
        panic!("a cursor");
    };
    assert_eq!(cursor.col, at.col);
    assert_eq!(
        cursor.row_text,
        cells.concat().trim_end_matches(' '),
        "trailing spaces are trimmed"
    );
    assert_eq!(cursor.text_before_cursor, cells[..at.col as usize].concat());
    assert!(
        cells.iter().any(String::is_empty),
        "the model has a wide character's second cell"
    );
}

// ---- adoption (P5; botster-core-host DESIGN.md "Adoption (P5)", parts 3, 4 and 7) ----

const ADOPTER: CandidateId = CandidateId(1);

/// The actions after the fence, and the reports among them, decoded on the new link.
fn after_fence(w: &mut World, actions: &[Action]) -> (Vec<Action>, Vec<Frame>) {
    let at = actions
        .iter()
        .position(|a| *a == Action::AdoptLink(ADOPTER))
        .expect("the candidate replaced the link");
    let rest = actions[at + 1..].to_vec();
    let frames = w.frames(&rest);
    (rest, frames)
}

/// A new host at `epoch` connects to the endpoint and sends its hello.
fn adopt(w: &mut World, epoch: u64) -> Vec<Action> {
    let mut actions = w.feed(Input::Candidate(ADOPTER));
    actions.extend(w.feed(Input::CandidateBytes(
        ADOPTER,
        World::host_hello(instance(), epoch, TOKEN),
    )));
    actions
}

/// The worker's hello and its report on the new link.
fn adopted(w: &mut World, actions: &[Action]) -> (Hello, AdoptReport) {
    let (_, frames) = after_fence(w, actions);
    assert_eq!(frames.len(), 2, "{frames:?}");
    assert_eq!(frames[0].kind, FrameType::HELLO);
    let hello = Hello::decode(&frames[0].payload).unwrap();
    let report = match WorkerMsg::decode(&frames[1].payload).unwrap() {
        WorkerMsg::Adopted { report } => *report,
        other => panic!("{other:?}"),
    };
    (hello, report)
}

/// DESIGN.md 3.3, 3.4, part 4; AD-6, DP-8: a new host proves the host role at a higher epoch. The worker fences the old
/// link, records the epoch, answers with its own proof at that epoch, and reports a payload that runs.
#[test]
fn a_candidate_that_proves_the_host_role_replaces_the_link() {
    let mut w = World::running();
    let actions = adopt(&mut w, EPOCH + 3);
    let (hello, report) = adopted(&mut w, &actions);
    assert_eq!(hello.host_epoch, EPOCH + 3);
    assert_eq!(hello.instance, instance());
    assert_eq!(hello.proof, token_proof(&TOKEN, &instance(), EPOCH + 3));
    assert_eq!(report.payload, AdoptedPayload::Running { payload: PAYLOAD });
    assert_eq!(report.features, worker_features());
    assert_eq!(report.terminal.map(|t| t.size), Some(size()));
    // The new link obeys the new host.
    let stop = w.send(&HostMsg::Stop);
    assert_eq!(signals(&stop), [SIGTERM]);
}

/// DESIGN.md 3.3 (integration D1, P5-F20): an equal epoch adopts again; a lower one is refused, and the refusal closes only
/// the candidate.
#[test]
fn an_equal_epoch_adopts_and_a_lower_one_is_refused() {
    let mut w = World::running();
    let first = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &first);
    let again = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &again);
    let lower = adopt(&mut w, EPOCH);
    assert_eq!(lower, [Action::CandidateClose(ADOPTER)]);
    assert_eq!(
        signals(&w.send(&HostMsg::Stop)),
        [SIGTERM],
        "the link stays"
    );
}

/// AD-6, A11: a candidate whose hello fails closes only the candidate. The worker's own proof sent back (a replay), another
/// token, another instance, a message, and a hello in a frame of another kind are all refused.
#[test]
fn a_candidate_that_does_not_prove_the_host_role_is_closed_alone() {
    let replay = {
        let hello = Hello {
            protocol: WORKER_PROTOCOL,
            proof: token_proof(&TOKEN, &instance(), EPOCH + 1),
            instance: instance(),
            host_epoch: EPOCH + 1,
        };
        let mut payload = Vec::new();
        hello.encode(&mut payload).unwrap();
        World::frame(FrameType::HELLO, &payload)
    };
    let wrong = [
        replay,
        World::host_hello(instance(), EPOCH + 1, [8; TOKEN_LEN]),
        World::host_hello(InstanceId("4-2".into()), EPOCH + 1, TOKEN),
        World::msg(&HostMsg::Stop),
        // A valid host hello in a frame that is not a hello.
        {
            let mut hello = World::host_hello(instance(), EPOCH + 1, TOKEN);
            let mut decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
            decoder.push(&hello);
            let payload = decoder.next_frame().unwrap().unwrap().payload;
            hello = World::frame(FrameType::HOST_MSG, &payload);
            hello
        },
    ];
    for bytes in wrong {
        let mut w = World::running();
        w.feed(Input::Candidate(ADOPTER));
        let actions = w.feed(Input::CandidateBytes(ADOPTER, bytes));
        assert_eq!(actions, [Action::CandidateClose(ADOPTER)]);
        assert_eq!(
            signals(&w.send(&HostMsg::Stop)),
            [SIGTERM],
            "the link stays"
        );
    }
}

/// DESIGN.md part 7: at most one candidate at a time; a frame above one hello closes it; its hello has `startup`.
#[test]
fn a_candidate_is_bounded() {
    let mut w = World::running();
    assert!(w.feed(Input::Candidate(ADOPTER)).is_empty());
    assert_eq!(
        w.feed(Input::Candidate(CandidateId(2))),
        [Action::CandidateClose(CandidateId(2))]
    );
    let big = World::frame(FrameType::HELLO, &vec![b' '; 4096]);
    assert_eq!(
        w.feed(Input::CandidateBytes(ADOPTER, big)),
        [Action::CandidateClose(ADOPTER)]
    );
    assert!(w.feed(Input::Candidate(CandidateId(3))).is_empty());
    let startup = cfg().startup;
    assert_eq!(w.worker.next_deadline(), Some(w.now + startup));
    w.now += startup;
    assert_eq!(
        w.feed(Input::Timer),
        [Action::CandidateClose(CandidateId(3))]
    );
    // A closed candidate is gone: a new one is taken.
    assert!(w.feed(Input::Candidate(CandidateId(4))).is_empty());
    w.feed(Input::CandidateClosed(CandidateId(4)));
    assert!(w.feed(Input::Candidate(CandidateId(5))).is_empty());
}

/// DESIGN.md part 4: the report gives the payload's live state: `NotLaunched`, `Spawning`, `LaunchFailed` and `Exited`,
/// with a terminal state only for a payload that ran.
#[test]
fn the_adoption_report_gives_the_payload_state() {
    let mut w = World::linked();
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report) = adopted(&mut w, &actions);
    assert_eq!(report.payload, AdoptedPayload::NotLaunched);
    assert_eq!(report.terminal, None);

    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report) = adopted(&mut w, &actions);
    assert_eq!(report.payload, AdoptedPayload::Spawning);
    // The spawn's answer goes to the new host.
    let reports = {
        let actions = w.feed(Input::Spawned(Ok(PAYLOAD)));
        w.reports(&actions)
    };
    assert!(
        matches!(reports[0], WorkerMsg::Launched { .. }),
        "{reports:?}"
    );

    let mut w = World::linked();
    w.send(&HostMsg::Launch(Box::new(spec())));
    w.feed(Input::Spawned(Err(SpawnFailure::CwdMissing)));
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report) = adopted(&mut w, &actions);
    assert_eq!(
        report.payload,
        AdoptedPayload::LaunchFailed {
            reason: StartFailReason::CwdMissing
        }
    );
    assert_eq!(report.terminal, None);

    let (mut w, _) = World::exited(ExitStatus::Signal(9));
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report) = adopted(&mut w, &actions);
    assert_eq!(
        report.payload,
        AdoptedPayload::Exited {
            code: None,
            signal: Some(9)
        }
    );
    assert!(report.terminal.is_some(), "ST-5: the final model");
}

/// DESIGN.md part 4: an exit that is not reported yet is `Running` in the report; the exit follows on the new link after
/// the drain.
#[test]
fn an_exit_before_its_drain_follows_the_report() {
    let mut w = World::running();
    w.feed(Input::PayloadExited(ExitStatus::Code(3)));
    let actions = adopt(&mut w, EPOCH + 1);
    let (_, report) = adopted(&mut w, &actions);
    assert_eq!(report.payload, AdoptedPayload::Running { payload: PAYLOAD });
    let actions = w.feed(Input::PtyDrained);
    assert_eq!(
        w.reports(&actions),
        [WorkerMsg::Exited {
            code: Some(3),
            signal: None
        }]
    );
}

/// DESIGN.md 3.4 (P3's review): the fence retires every request of the old host. Its queued write never runs, the write in
/// progress runs to its end and reports nothing, and the new host's request of the same number completes only for itself.
#[test]
fn the_fence_retires_the_old_hosts_requests() {
    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    w.send(&write(2, b"zz", None));
    let actions = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &actions);
    // The new host's first request has the number of the old host's write in flight.
    let new = w.send(&write(1, b"n", None));
    assert!(
        pty_writes(&new).is_empty(),
        "the old write owns the PTY input"
    );
    let actions = w.feed(Input::PtyWritten(Ok(3)));
    assert_eq!(
        pty_writes(&actions),
        [b"n".to_vec()],
        "the old write ran to its end; the old queued write never runs"
    );
    assert!(
        input_result(&mut w, &actions, 1).is_none(),
        "the old write reports nothing"
    );
    let actions = w.feed(Input::PtyWritten(Ok(1)));
    let result = input_result(&mut w, &actions, 1).expect("the new write's own result");
    assert_eq!(outcome(&result), (WriteOutcome::Written, 1, 1));
}

/// AD-7 (`conf::ad_7_crash_between_steps_leaves_no_unregistered_payload`): a worker with no payload and no host ends by
/// itself after `startup`, and a `Launch` can never reach it later. A host that is linked, or a payload, keeps it alive.
#[test]
fn a_worker_with_no_payload_and_no_host_ends_after_startup() {
    let startup = cfg().startup;
    let mut w = World::linked();
    assert_eq!(w.worker.next_deadline(), None, "a linked host keeps it");
    w.feed(Input::LinkClosed);
    assert_eq!(w.worker.next_deadline(), Some(w.now + startup));
    w.now += startup;
    let actions = w.feed(Input::Timer);
    assert_eq!(actions, [Action::Exit]);
    assert_eq!(
        w.feed(Input::Candidate(ADOPTER)),
        [Action::CandidateClose(ADOPTER)],
        "an ended worker takes no candidate"
    );

    let mut w = World::running();
    w.feed(Input::LinkClosed);
    assert_eq!(w.worker.next_deadline(), None, "a payload keeps it (DP-8)");

    // An adoption before the deadline keeps it.
    let mut w = World::linked();
    w.feed(Input::LinkClosed);
    let actions = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &actions);
    assert_eq!(w.worker.next_deadline(), None);
}

/// LC-7, DESIGN.md part 7: a worker that is removing its session, or that waits to end after the removal, takes no
/// candidate: the removal ends the session.
#[test]
fn a_removing_worker_takes_no_candidate() {
    let mut w = World::running();
    w.send(&HostMsg::Remove);
    assert_eq!(
        w.feed(Input::Candidate(ADOPTER)),
        [Action::CandidateClose(ADOPTER)],
        "the removal is under way"
    );
    w.instant_link = false;
    w.feed(Input::PayloadExited(ExitStatus::Signal(9)));
    let actions = w.feed(Input::PtyDrained);
    assert!(
        w.reports(&actions)
            .iter()
            .any(|m| matches!(m, WorkerMsg::RemoveResult { .. })),
        "{actions:?}"
    );
    assert!(
        !actions.contains(&Action::Exit),
        "the result is not written yet"
    );
    assert_eq!(
        w.feed(Input::Candidate(ADOPTER)),
        [Action::CandidateClose(ADOPTER)],
        "the worker waits to end"
    );
}

/// DESIGN.md 3.4, AM-2: a cancel from the new host never reaches the old host's write in progress, even with the same
/// request number: the old write runs to its end.
#[test]
fn a_new_hosts_cancel_never_stops_the_retired_write() {
    let mut w = World::running();
    w.send(&write(1, b"abc", None));
    let actions = adopt(&mut w, EPOCH + 1);
    adopted(&mut w, &actions);
    w.send(&HostMsg::Cancel { req: 1 });
    let actions = w.feed(Input::PtyWritten(Ok(1)));
    assert_eq!(pty_writes(&actions), [b"bc".to_vec()]);
}
