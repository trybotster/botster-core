//! Tests of the `Worker` machine through its inputs and actions only.

use super::*;
use botster_core_link::frame::Frame;
use botster_core_link::msg::Observation;
use botster_route_codec::prelude::HexBytes;
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

impl World {
    fn new() -> World {
        World::with(cfg())
    }

    fn with(cfg: WorkerConfig) -> World {
        #[allow(clippy::disallowed_methods)] // a test starts the injected clock at a real instant
        let now = Instant::now();
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
            if let Action::LinkSend(bytes) = action {
                self.sent += bytes.len() as u64;
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
            proof: token_proof(&token, &instance, epoch),
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
    assert_eq!(w.feed(Input::PtyOutput(b"tail".to_vec())), []);
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
        op: Op::ReadCursor {
            session: SessionId("s".into()),
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
    assert!(error.detail.contains("ReadCursor"), "{}", error.detail);
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
