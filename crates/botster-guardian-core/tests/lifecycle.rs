//! Guardian clauses through the machine's inputs, actions, and private wire.
//!
//! The rig plays the driver: it feeds inputs at a chosen time and decodes every `LinkSend` the way a host does.

use botster_core_contract::prelude::*;
use botster_core_edges::edges::{ExitStatus, ProcessIdentity};
use botster_core_edges::Machine;
use botster_core_link::frame::{
    encode_frame, FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD, HEADER_LEN,
};
use botster_core_link::hello::{Hello, HelloError, MAX_INSTANCE_ID_LEN};
use botster_core_link::msg::PayloadId;
use botster_core_link::proof::{host_proof, token_proof};
use botster_guardian_core::guardian::SpawnResult;
use botster_guardian_core::wire::{Command, LogChunk, Report, ServiceSpec, Status, LOG_FRAME};
use botster_guardian_core::{Action, Guardian, GuardianConfig, Input};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const LEADER: PayloadId = PayloadId {
    pid: 42,
    start_time: 100,
};
const CHILD: ProcessIdentity = ProcessIdentity {
    pid: 43,
    start_time: 101,
};

/// The base of the injected clock. The machine never reads a clock.
#[allow(clippy::disallowed_methods)]
fn now() -> Instant {
    Instant::now()
}

fn config() -> GuardianConfig {
    GuardianConfig {
        service: ServiceId([1; 32]),
        instance: InstanceId("service-instance".into()),
        token: [7; 32],
        host_epoch: 1,
        protocol: 1,
        orphan_grace: Duration::from_millis(5000),
        log_bytes: 1024,
    }
}

fn spec() -> ServiceSpec {
    ServiceSpec {
        argv: vec!["/bin/service".into(), "--flag".into(), "x y".into()],
        env: BTreeMap::from([("A".into(), "one".into())]),
        cwd: "/srv".into(),
        limits: ServiceLimits {
            lanes: vec![LaneSpec {
                max_frame_bytes: 1024,
                outbound_queue_bytes: 4096,
                inbound_queue_bytes: 4096,
            }],
            rlimits: Rlimits::default(),
            startup: Duration::from_millis(10000),
            stop_grace: Duration::from_millis(300),
            orphan_grace: Duration::from_millis(5000),
            peer_identity: None,
        },
    }
}

/// The valid hello of a host at `epoch`: the proof of the host's role.
fn host_hello(cfg: &GuardianConfig, epoch: u64) -> Hello {
    Hello {
        proof: host_proof(&cfg.token, &cfg.instance, epoch),
        ..guardian_hello(cfg, epoch)
    }
}

/// The guardian's own hello at `epoch`: the proof of the service's role.
fn guardian_hello(cfg: &GuardianConfig, epoch: u64) -> Hello {
    Hello {
        protocol: cfg.protocol,
        instance: cfg.instance.clone(),
        proof: token_proof(&cfg.token, &cfg.instance, epoch),
        host_epoch: epoch,
    }
}

fn frame(kind: FrameType, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut bytes).unwrap();
    bytes
}

fn hello_frame(hello: &Hello) -> Vec<u8> {
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    frame(FrameType::HELLO, &payload)
}

/// What a host reads from the guardian's `LinkSend` bytes.
#[derive(Debug, PartialEq)]
enum Sent {
    Hello(Hello),
    Report(Report),
    Log(LogChunk),
}

fn sent(actions: &[Action]) -> Vec<Sent> {
    let mut decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
    let mut out = Vec::new();
    for action in actions {
        let Action::LinkSend(mut bytes) = action.clone() else {
            continue;
        };
        while !bytes.is_empty() {
            let took = decoder.push(&bytes);
            bytes.drain(..took);
            let frame = decoder
                .next_frame()
                .unwrap()
                .expect("each LinkSend holds whole frames");
            out.push(match frame.kind {
                FrameType::HELLO => Sent::Hello(Hello::decode(&frame.payload).unwrap()),
                FrameType::WORKER_MSG => {
                    Sent::Report(serde_json::from_slice(&frame.payload).unwrap())
                }
                LOG_FRAME => Sent::Log(LogChunk::decode(&frame.payload).unwrap()),
                other => panic!("unexpected frame {other:?}"),
            });
        }
    }
    out
}

fn reports(actions: &[Action]) -> Vec<Report> {
    sent(actions)
        .into_iter()
        .filter_map(|s| match s {
            Sent::Report(report) => Some(report),
            _ => None,
        })
        .collect()
}

fn logs(actions: &[Action]) -> Vec<LogChunk> {
    sent(actions)
        .into_iter()
        .filter_map(|s| match s {
            Sent::Log(chunk) => Some(chunk),
            _ => None,
        })
        .collect()
}

fn link_bytes(actions: &[Action]) -> u64 {
    actions
        .iter()
        .map(|a| match a {
            Action::LinkSend(bytes) => bytes.len() as u64,
            _ => 0,
        })
        .sum()
}

/// The exit that the guardian reports.
fn exited(actions: &[Action]) -> Option<ServiceExit> {
    reports(actions).into_iter().find_map(|r| match r {
        Report::Exited { exit } => Some(exit),
        _ => None,
    })
}

/// A driver over one guardian, with its clock and the bytes written on the current connection.
struct Rig {
    g: Guardian,
    cfg: GuardianConfig,
    start: Instant,
    at: Instant,
    written: u64,
}

impl Rig {
    /// A guardian that sent its hello and waits for a host.
    fn connected() -> Rig {
        Rig::with(config())
    }

    fn with(cfg: GuardianConfig) -> Rig {
        let start = now();
        let mut g = Guardian::new(cfg.clone(), start).unwrap();
        let hello = std::iter::from_fn(|| g.poll_action()).collect::<Vec<_>>();
        let written = link_bytes(&hello);
        Rig {
            g,
            cfg,
            start,
            at: start,
            written,
        }
    }

    /// An authenticated guardian.
    fn ready() -> Rig {
        let mut rig = Rig::connected();
        rig.authenticate(rig.cfg.host_epoch);
        rig
    }

    /// A launched service whose host committed epoch 1.
    fn running() -> Rig {
        let mut rig = Rig::launched();
        rig.command(Command::EpochCommitted);
        rig
    }

    /// A launched service before the host committed epoch 1.
    fn launched() -> Rig {
        let mut rig = Rig::ready();
        rig.command(Command::Launch(Box::new(spec())));
        rig.started();
        rig
    }

    fn input(&mut self, input: Input) -> Vec<Action> {
        self.g.handle(self.at, input);
        let actions: Vec<Action> = std::iter::from_fn(|| self.g.poll_action()).collect();
        self.written += link_bytes(&actions);
        actions
    }

    /// Advances the injected clock and delivers the timer.
    fn at(&mut self, at: Instant) -> Vec<Action> {
        self.at = at;
        self.input(Input::Timer)
    }

    fn command(&mut self, command: Command) -> Vec<Action> {
        let json = serde_json::to_vec(&command).unwrap();
        self.input(Input::LinkBytes(frame(FrameType::HOST_MSG, &json)))
    }

    fn authenticate(&mut self, epoch: u64) -> Vec<Action> {
        let hello = hello_frame(&host_hello(&self.cfg, epoch));
        // One byte at a time: the decoder keeps partial frames.
        hello
            .iter()
            .flat_map(|byte| self.input(Input::LinkBytes(vec![*byte])))
            .collect()
    }

    /// The driver reports every byte so far as written.
    fn flush(&mut self) -> Vec<Action> {
        let total = self.written;
        self.input(Input::LinkWritten { total })
    }

    fn reconnect(&mut self, epoch: u64) -> Vec<Action> {
        self.input(Input::LinkClosed);
        self.written = 0;
        let mut actions = self.input(Input::LinkConnected);
        actions.extend(self.authenticate(epoch));
        actions
    }

    fn started(&mut self) -> Vec<Action> {
        self.input(Input::Spawned(SpawnResult::Started {
            payload: LEADER,
            report: SpawnReport::default(),
        }))
    }

    fn stop_to_term(&mut self, known: Vec<ProcessIdentity>) {
        assert_eq!(
            self.command(Command::Stop),
            vec![Action::Enumerate { leader: LEADER }]
        );
        assert_eq!(
            self.input(Input::Descendants(known)),
            vec![Action::TermService { leader: LEADER }]
        );
    }

    fn term_result(&mut self, delivered: bool, leader_exiting: bool) -> Vec<Action> {
        self.input(Input::TermSent {
            delivered,
            leader_exiting,
        })
    }

    fn tree_killed(
        &mut self,
        reached_live_leader: bool,
        descendants_may_remain: bool,
    ) -> Vec<Action> {
        self.input(Input::TreeKilled {
            leader_signalled: reached_live_leader,
            leader_exiting: !reached_live_leader,
            descendants_may_remain,
        })
    }

    /// The leader exits after the guardian's kill reached it, and the driver drains its output.
    fn killed_leader_exits(&mut self, descendants_may_remain: bool) -> Vec<Action> {
        let mut actions = self.tree_killed(true, descendants_may_remain);
        actions.extend(self.input(Input::PayloadExited(ExitStatus::Signal(9))));
        actions.extend(self.input(Input::LogsDrained));
        actions
    }

    /// The leader exits on its own; the cleanup kill finds it exiting.
    fn leader_exits(&mut self, status: ExitStatus) -> Vec<Action> {
        let cleanup = self.input(Input::PayloadExited(status));
        assert_eq!(
            cleanup,
            vec![
                Action::DrainLogs,
                Action::KillTree {
                    leader: LEADER,
                    known: vec![]
                }
            ]
        );
        let mut actions = self.input(Input::LogsDrained);
        actions.extend(self.tree_killed(false, false));
        actions
    }
}

/// Core AD-6, DP-8: the guardian proves the token in the service's role, and only a host's valid hello (the host's role) at no
/// lower epoch authenticates; the guardian's own hello sent back does not.
#[test]
fn only_a_valid_host_hello_authenticates() {
    let cfg = config();
    let rig = Rig::connected();
    assert_eq!(rig.g.next_deadline(), Some(rig.start + cfg.orphan_grace));

    let mut own = Guardian::new(cfg.clone(), rig.start).unwrap();
    let hello = std::iter::from_fn(|| own.poll_action()).collect::<Vec<_>>();
    assert_eq!(
        sent(&hello),
        vec![Sent::Hello(guardian_hello(&cfg, cfg.host_epoch))]
    );
    assert!(!format!("{cfg:?}").contains(&format!("{:?}", cfg.token)));

    let wrong: [(&str, Vec<u8>); 6] = [
        (
            "instance",
            hello_frame(&Hello {
                instance: InstanceId("other".into()),
                ..host_hello(&cfg, 1)
            }),
        ),
        (
            "token",
            hello_frame(&Hello {
                proof: host_proof(&[8; 32], &cfg.instance, 1),
                ..host_hello(&cfg, 1)
            }),
        ),
        // The guardian's own hello sent back: the token and epoch are right, the role is not.
        ("reflected", hello_frame(&guardian_hello(&cfg, 1))),
        (
            "lower epoch",
            hello_frame(&host_hello(&cfg, cfg.host_epoch - 1)),
        ),
        (
            "protocol",
            hello_frame(&Hello {
                protocol: cfg.protocol + 1,
                ..host_hello(&cfg, 1)
            }),
        ),
        ("frame type", {
            let mut payload = Vec::new();
            host_hello(&cfg, 1).encode(&mut payload).unwrap();
            frame(FrameType::HOST_MSG, &payload)
        }),
    ];
    for (case, bytes) in wrong {
        let mut rig = Rig::connected();
        assert_eq!(
            rig.input(Input::LinkBytes(bytes)),
            vec![Action::LinkClose],
            "{case}"
        );
        assert!(
            rig.command(Command::Launch(Box::new(spec()))).is_empty(),
            "{case}"
        );
        assert_eq!(
            rig.g.next_deadline(),
            Some(rig.start + cfg.orphan_grace),
            "{case}"
        );
    }

    let mut rig = Rig::connected();
    let higher = cfg.host_epoch + 1;
    assert_eq!(
        reports(&rig.authenticate(higher)),
        vec![Report::Status(rig.g.status())]
    );
    assert_eq!(rig.g.next_deadline(), None);
    // The proved epoch is now the floor: a reconnect at the configured epoch is refused.
    rig.input(Input::LinkClosed);
    rig.input(Input::LinkConnected);
    assert_eq!(rig.authenticate(cfg.host_epoch), vec![Action::LinkClose]);
}

/// Core AD-6: a guardian that could not send its hello does not start.
#[test]
fn a_config_whose_hello_cannot_be_encoded_is_refused() {
    let cfg = GuardianConfig {
        instance: InstanceId("i".repeat(MAX_INSTANCE_ID_LEN + 1)),
        ..config()
    };
    let refused = Guardian::new(cfg, now()).unwrap_err();
    assert_eq!(refused, HelloError::InstanceIdTooLong);
}

/// Core AD-6: a malformed or oversize control frame closes only the control connection.
#[test]
fn malformed_and_oversize_control_frames_close_the_connection() {
    let oversize = (DEFAULT_MAX_PAYLOAD + 1).to_le_bytes();
    for bytes in [
        frame(FrameType::HELLO, b"not JSON"),
        frame(FrameType::HELLO, b"{}"),
        [&oversize[..], &[FrameType::HELLO.0]].concat(),
    ] {
        let mut rig = Rig::connected();
        assert_eq!(rig.input(Input::LinkBytes(bytes)), vec![Action::LinkClose]);
        assert_eq!(
            rig.g.next_deadline(),
            Some(rig.start + rig.cfg.orphan_grace)
        );
    }
}

/// Core AD-6: after authentication, only a host message frame carries a command.
#[test]
fn only_host_message_frames_carry_commands() {
    let mut rig = Rig::ready();
    let launch = serde_json::to_vec(&Command::Launch(Box::new(spec()))).unwrap();
    assert!(rig
        .input(Input::LinkBytes(frame(FrameType::WORKER_MSG, &launch)))
        .is_empty());
    assert!(rig
        .input(Input::LinkBytes(frame(
            FrameType::HOST_MSG,
            b"{\"t\":\"unknown\"}"
        )))
        .is_empty());
    assert_eq!(
        rig.command(Command::Launch(Box::new(spec()))),
        vec![Action::SpawnService(Box::new(spec()))]
    );
}

/// Core SV-1, SV-4, A4-1, SV-7: exec runs once with the host's spec, and its report survives the exit and a reconnect.
#[test]
fn the_service_launches_once_and_its_report_is_retained() {
    let mut rig = Rig::ready();
    assert_eq!(
        rig.command(Command::Launch(Box::new(spec()))),
        vec![Action::SpawnService(Box::new(spec()))]
    );
    assert!(rig.command(Command::Launch(Box::new(spec()))).is_empty());
    let report = SpawnReport {
        applied: vec![Bound::OpenFiles],
        unsupported: vec![Bound::Processes],
    };
    let started = rig.input(Input::Spawned(SpawnResult::Started {
        payload: LEADER,
        report: report.clone(),
    }));
    assert_eq!(
        reports(&started),
        vec![Report::Started {
            payload: LEADER,
            report: report.clone()
        }]
    );
    rig.command(Command::EpochCommitted);

    let status = ExitStatus::Code(1);
    let exit = exited(&rig.leader_exits(status)).unwrap();
    rig.input(Input::Reaped);
    assert!(rig.command(Command::Launch(Box::new(spec()))).is_empty());

    // An empty ring sends no log frame: the status follows the hello directly.
    let greeting = sent(&rig.reconnect(rig.cfg.host_epoch));
    assert_eq!(
        greeting,
        vec![
            Sent::Hello(guardian_hello(&rig.cfg, rig.cfg.host_epoch)),
            Sent::Report(Report::Status(Status {
                service: rig.cfg.service,
                payload: Some(LEADER),
                report: Some(report),
                exit: Some(exit),
            }))
        ]
    );
}

/// Core SV-9, A2-5: a failed exec reports the edge's failure, signals nothing, and Remove releases the guardian.
#[test]
fn a_failed_start_reports_the_edge_failure_and_rolls_back() {
    for (result, expected) in [
        (
            SpawnResult::Failed(StartFailReason::ExecFailed { errno: 2 }),
            Report::StartFailed {
                reason: StartFailReason::ExecFailed { errno: 2 },
            },
        ),
        (
            SpawnResult::BoundUnavailable(Bound::CpuSeconds),
            Report::BoundUnavailable {
                bound: Bound::CpuSeconds,
            },
        ),
    ] {
        let mut rig = Rig::ready();
        rig.command(Command::Launch(Box::new(spec())));
        assert_eq!(reports(&rig.input(Input::Spawned(result))), vec![expected]);
        assert_eq!(rig.g.next_deadline(), None);
        let removed = rig.command(Command::Remove);
        assert_eq!(sent(&removed), vec![Sent::Report(Report::Removed)]);
        assert_eq!(rig.flush(), vec![Action::LinkClose, Action::Exit]);
    }
}

/// Core A2-5: startup runs from exec until the host commits epoch 1; expiry ends the service as `StartupTimeout`.
#[test]
fn startup_runs_from_exec_until_epoch_one_commits() {
    let startup = spec().limits.startup;
    let mut rig = Rig::ready();
    rig.command(Command::Launch(Box::new(spec())));
    assert_eq!(rig.g.next_deadline(), None);
    let exec_at = rig.start + Duration::from_millis(1);
    rig.at = exec_at;
    rig.started();
    let due = exec_at + startup;
    assert_eq!(rig.g.next_deadline(), Some(due));
    assert!(rig.at(due - Duration::from_nanos(1)).is_empty());
    assert_eq!(
        rig.at(due),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![]
        }]
    );
    let exit = exited(&rig.killed_leader_exits(false)).unwrap();
    assert_eq!(exit.cause, ServiceExitCause::StartupTimeout);

    let mut rig = Rig::launched();
    assert_eq!(rig.g.next_deadline(), Some(rig.start + startup));
    rig.command(Command::EpochCommitted);
    assert_eq!(rig.g.next_deadline(), None);
    assert!(rig.at(rig.start + startup).is_empty());
}

/// Core SV-8, AD-6: grace runs while no host is authenticated; a failed hello does not extend it; adoption cancels it.
#[test]
fn orphan_grace_ends_the_service_unless_a_host_authenticates() {
    let grace = config().orphan_grace;
    let mut rig = Rig::running();
    rig.input(Input::LinkClosed);
    let due = rig.start + grace;
    assert_eq!(rig.g.next_deadline(), Some(due));

    rig.at = rig.start + grace / 4;
    rig.input(Input::LinkConnected);
    assert_eq!(
        rig.authenticate(rig.cfg.host_epoch - 1),
        vec![Action::LinkClose]
    );
    assert_eq!(rig.g.next_deadline(), Some(due));

    rig.at = rig.start + grace / 2;
    rig.input(Input::LinkConnected);
    rig.authenticate(rig.cfg.host_epoch + 1);
    assert_eq!(rig.g.next_deadline(), None);
    assert!(rig.at(due).is_empty());

    rig.input(Input::LinkClosed);
    let due = rig.at + grace;
    assert_eq!(rig.g.next_deadline(), Some(due));
    assert_eq!(
        rig.at(due),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![]
        }]
    );
    // A dying guardian accepts no host.
    assert!(rig.input(Input::LinkConnected).is_empty());
    let cleanup = rig.killed_leader_exits(false);
    assert_eq!(
        cleanup,
        vec![Action::DrainLogs, Action::ReapService { leader: LEADER }]
    );
    assert_eq!(
        rig.g.status().exit.unwrap().cause,
        ServiceExitCause::OrphanGrace
    );
    assert_eq!(rig.input(Input::Reaped), vec![Action::Exit]);
}

/// Core SV-8: a guardian that never received a launch also ends at the grace.
#[test]
fn an_unlaunched_guardian_ends_at_orphan_grace() {
    let mut rig = Rig::connected();
    let due = rig.start + rig.cfg.orphan_grace;
    assert!(rig.at(due - Duration::from_nanos(1)).is_empty());
    assert_eq!(rig.at(due), vec![Action::LinkClose, Action::Exit]);
    assert_eq!(rig.g.next_deadline(), None);
}

/// Core SV-9, SV-5 (conf::sv_9_sigterm_then_group_kill): census, SIGTERM, the grace from SIGTERM, then the group kill of
/// the leader and the known descendants. A kill that reaches the live leader is `Killed`.
#[test]
fn stop_terms_then_kills_the_group_and_the_known_descendants() {
    let grace = spec().limits.stop_grace;
    let mut rig = Rig::running();
    rig.stop_to_term(vec![CHILD]);
    assert!(rig.term_result(true, false).is_empty());
    let due = rig.at + grace;
    assert_eq!(rig.g.next_deadline(), Some(due));
    rig.at = due - Duration::from_nanos(1);
    assert!(rig.command(Command::Stop).is_empty());
    assert!(rig.at(due - Duration::from_nanos(1)).is_empty());
    assert_eq!(
        rig.at(due),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![CHILD]
        }]
    );
    let end = rig.killed_leader_exits(true);
    let exit = exited(&end).unwrap();
    assert_eq!(
        exit,
        ServiceExit {
            code: None,
            signal: Some(9),
            cause: ServiceExitCause::Killed,
            descendants_may_remain: true,
        }
    );
    assert_eq!(end.last(), Some(&Action::ReapService { leader: LEADER }));
    rig.input(Input::Reaped);
    assert!(rig.command(Command::Stop).is_empty());
}

/// Core SV-5, SV-9 (conf::sv_9_sigterm_then_group_kill): a leader that SIGTERM ends is `HostStop`; the cleanup kill
/// that finds it exiting is no cause.
#[test]
fn a_leader_that_sigterm_ends_is_host_stop() {
    let mut rig = Rig::running();
    rig.stop_to_term(vec![]);
    rig.term_result(true, false);
    let status = ExitStatus::Signal(15);
    let exit = exited(&rig.leader_exits(status)).unwrap();
    assert_eq!(
        (exit.signal, exit.cause),
        (Some(15), ServiceExitCause::HostStop)
    );
}

/// Core SV-9: census and delivery delays cannot shorten the grace; it runs from the SIGTERM result.
#[test]
fn the_stop_grace_runs_from_the_sigterm_result() {
    let grace = spec().limits.stop_grace;
    let mut rig = Rig::running();
    assert_eq!(
        rig.command(Command::Stop),
        vec![Action::Enumerate { leader: LEADER }]
    );
    let census_at = rig.start + grace;
    assert!(rig.at(census_at).is_empty());
    assert_eq!(rig.g.next_deadline(), None);
    assert_eq!(
        rig.input(Input::Descendants(vec![CHILD])),
        vec![Action::TermService { leader: LEADER }]
    );
    let term_at = census_at + grace;
    assert!(rig.at(term_at).is_empty());
    assert_eq!(rig.g.next_deadline(), None);
    rig.term_result(true, false);
    assert_eq!(rig.g.next_deadline(), Some(term_at + grace));
}

/// Core SV-9: a failed SIGTERM to a live leader still runs the grace, then kills; it claims no `HostStop`.
#[test]
fn a_failed_sigterm_runs_the_grace_and_claims_no_cause() {
    let grace = spec().limits.stop_grace;
    let mut rig = Rig::running();
    rig.stop_to_term(vec![]);
    rig.term_result(false, false);
    let due = rig.at + grace;
    assert_eq!(
        rig.at(due),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![]
        }]
    );
    rig.tree_killed(false, false);
    rig.input(Input::PayloadExited(ExitStatus::Code(0)));
    let exit = exited(&rig.input(Input::LogsDrained)).unwrap();
    assert_eq!(exit.cause, ServiceExitCause::Normal);
}

/// Core SV-9, SV-8: a kill requested during the census or SIGTERM runs after that result, with the known descendants.
#[test]
fn a_kill_waits_for_the_outstanding_census_or_sigterm() {
    let mut rig = Rig::running();
    rig.command(Command::Stop);
    assert_eq!(sent(&rig.command(Command::Remove)), vec![]);
    assert_eq!(
        rig.input(Input::Descendants(vec![CHILD])),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![CHILD]
        }]
    );

    let mut rig = Rig::running();
    rig.stop_to_term(vec![CHILD]);
    rig.input(Input::LinkClosed);
    let due = rig.start + rig.cfg.orphan_grace;
    assert_eq!(rig.at(due), vec![]);
    assert_eq!(
        rig.term_result(true, false),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![CHILD]
        }]
    );
    rig.killed_leader_exits(false);
    assert_eq!(
        rig.g.status().exit.unwrap().cause,
        ServiceExitCause::OrphanGrace
    );
}

/// Core SV-5 (conf::sv_5_no_unobservable_limit_causes): the cause is how the leader ended, or its cooperative byte.
#[test]
fn an_exit_reports_its_observed_cause() {
    for (status, bytes, cause) in [
        (ExitStatus::Code(0), vec![], ServiceExitCause::Normal),
        (ExitStatus::Signal(24), vec![], ServiceExitCause::Signal),
        (
            ExitStatus::Code(1),
            vec![7, 8],
            ServiceExitCause::ChildReported(7),
        ),
    ] {
        let mut rig = Rig::running();
        for byte in bytes {
            rig.input(Input::Cause(byte));
        }
        let exit = exited(&rig.leader_exits(status)).unwrap();
        let (code, signal) = match status {
            ExitStatus::Code(code) => (Some(code), None),
            ExitStatus::Signal(signal) => (None, Some(signal)),
        };
        assert_eq!(
            exit,
            ServiceExit {
                code,
                signal,
                cause,
                descendants_may_remain: false
            }
        );
    }
}

/// Core SV-5: a kill that did not reach a live leader claims no cause: it failed, or the leader had already begun to exit.
#[test]
fn a_kill_that_misses_the_live_leader_claims_no_cause() {
    for (leader_signalled, leader_exiting, status, cause) in [
        (false, false, ExitStatus::Code(0), ServiceExitCause::Normal),
        (true, true, ExitStatus::Signal(6), ServiceExitCause::Signal),
    ] {
        let mut rig = Rig::launched();
        let due = rig.start + spec().limits.startup;
        assert_eq!(
            rig.at(due),
            vec![Action::KillTree {
                leader: LEADER,
                known: vec![]
            }]
        );
        rig.input(Input::TreeKilled {
            leader_signalled,
            leader_exiting,
            descendants_may_remain: false,
        });
        rig.input(Input::PayloadExited(status));
        let exit = exited(&rig.input(Input::LogsDrained)).unwrap();
        assert_eq!(exit.cause, cause, "{leader_signalled} {leader_exiting}");
    }
}

/// Core SV-9: an exit during the census keeps the census for the kill and waits for the drain.
#[test]
fn an_exit_during_the_census_keeps_the_descendants_and_waits_for_the_drain() {
    let mut rig = Rig::running();
    rig.command(Command::Stop);
    assert_eq!(
        rig.input(Input::PayloadExited(ExitStatus::Code(1))),
        vec![Action::DrainLogs]
    );
    assert_eq!(
        rig.input(Input::Descendants(vec![CHILD])),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![CHILD]
        }]
    );
    assert!(rig.tree_killed(false, false).is_empty());
    let end = rig.input(Input::LogsDrained);
    assert_eq!(exited(&end).unwrap().cause, ServiceExitCause::Normal);
    assert_eq!(end.last(), Some(&Action::ReapService { leader: LEADER }));
}

/// Core SV-9: a leader that exits before the exec result is cleaned up when the result arrives.
#[test]
fn an_exit_before_the_exec_result_is_cleaned_up() {
    let mut rig = Rig::ready();
    rig.command(Command::Launch(Box::new(spec())));
    assert!(rig
        .input(Input::PayloadExited(ExitStatus::Code(1)))
        .is_empty());
    let started = rig.started();
    assert_eq!(
        &started[1..],
        [
            Action::DrainLogs,
            Action::KillTree {
                leader: LEADER,
                known: vec![]
            }
        ]
    );
    assert_eq!(rig.g.next_deadline(), None);
    rig.tree_killed(false, false);
    assert_eq!(
        exited(&rig.input(Input::LogsDrained)).unwrap().code,
        Some(1)
    );
}

/// Core SV-5: a cause byte that the service wrote before the exec result arrived is its reported cause.
#[test]
fn a_cause_byte_before_the_exec_result_is_kept() {
    let (byte, code) = (7, 3);
    let status = ExitStatus::Code(code);
    let mut rig = Rig::ready();
    rig.command(Command::Launch(Box::new(spec())));
    rig.input(Input::Cause(byte));
    rig.input(Input::PayloadExited(status));
    rig.started();
    rig.tree_killed(false, false);
    let exit = exited(&rig.input(Input::LogsDrained)).unwrap();
    assert_eq!(
        (exit.code, exit.cause),
        (Some(code), ServiceExitCause::ChildReported(byte))
    );
}

/// Core SV-9: Remove and shutdown during exec kill the service when exec returns, then end the guardian.
#[test]
fn teardown_during_exec_kills_when_exec_returns() {
    for remove in [true, false] {
        let mut rig = Rig::ready();
        rig.command(Command::Launch(Box::new(spec())));
        let request = if remove {
            rig.command(Command::Remove)
        } else {
            rig.input(Input::Terminate)
        };
        assert!(request.is_empty());
        assert!(rig.started().ends_with(&[Action::KillTree {
            leader: LEADER,
            known: vec![]
        }]));
        rig.killed_leader_exits(false);
        assert_eq!(rig.g.status().exit.unwrap().cause, ServiceExitCause::Killed);
        let end = rig.input(Input::Reaped);
        if remove {
            assert_eq!(reports(&end), vec![Report::Removed]);
            assert_eq!(
                rig.input(Input::LinkClosed),
                vec![Action::LinkClose, Action::Exit]
            );
        } else {
            assert_eq!(end, vec![Action::LinkClose, Action::Exit]);
        }
    }
}

/// Core SV-9: Remove ends the guardian only after the driver wrote `Removed`; after that, every input is ignored.
#[test]
fn removal_waits_for_the_written_report_then_ignores_every_input() {
    let mut rig = Rig::ready();
    let before = rig.written;
    rig.command(Command::Remove);
    assert!(rig.input(Input::LinkWritten { total: before }).is_empty());
    assert!(rig
        .input(Input::LinkWritten {
            total: rig.written - 1
        })
        .is_empty());
    assert_eq!(rig.flush(), vec![Action::LinkClose, Action::Exit]);
    for input in [
        Input::Terminate,
        Input::LinkClosed,
        Input::LinkConnected,
        Input::Log(vec![1]),
        Input::PayloadExited(ExitStatus::Code(0)),
        Input::Timer,
    ] {
        assert!(rig.input(input).is_empty());
    }
    assert_eq!(rig.g.next_deadline(), None);
}

/// Core SV-5, SV-9: results that answer no outstanding action change nothing.
#[test]
fn stale_process_results_change_nothing() {
    let stale = [
        Input::LogsDrained,
        Input::Descendants(vec![CHILD]),
        Input::TermSent {
            delivered: true,
            leader_exiting: false,
        },
        Input::TreeKilled {
            leader_signalled: true,
            leader_exiting: false,
            descendants_may_remain: true,
        },
        Input::Reaped,
        Input::Spawned(SpawnResult::Failed(StartFailReason::CwdMissing)),
    ];
    let mut rig = Rig::running();
    for input in stale.clone() {
        assert!(rig.input(input).is_empty());
    }
    assert_eq!(rig.g.status().exit, None);
    let mut rig = Rig::ready();
    for input in stale {
        assert!(rig.input(input).is_empty());
    }
    assert_eq!(
        rig.command(Command::Launch(Box::new(spec()))),
        vec![Action::SpawnService(Box::new(spec()))]
    );
}

/// Core SV-9 (conf::sv_9_log_tail_bounded): the guardian streams its output to the host, retains the newest
/// `log_bytes`, and resends that tail to a new host.
#[test]
fn the_log_streams_to_the_host_and_a_new_host_gets_the_bounded_tail() {
    let mut rig = Rig::running();
    let first = vec![0x11; rig.cfg.log_bytes * 2];
    let last = vec![0xbb; 16];
    // The ring keeps the newest bytes, so a write larger than the ring sends only its tail.
    let kept = first.len() - rig.cfg.log_bytes;
    assert_eq!(
        logs(&rig.input(Input::Log(first.clone()))),
        vec![LogChunk {
            offset: kept as u64,
            bytes: first[kept..].to_vec()
        }]
    );
    // One batch at a time: the next waits until the first is written.
    assert!(rig.input(Input::Log(last.clone())).is_empty());
    assert_eq!(
        logs(&rig.flush()),
        vec![LogChunk {
            offset: first.len() as u64,
            bytes: last.clone()
        }]
    );

    // A new host gets the retained tail before the status, so the status marks a complete tail.
    let written = [first, last].concat();
    let tail = written[written.len() - rig.cfg.log_bytes..].to_vec();
    let offset = (written.len() - tail.len()) as u64;
    assert_eq!(
        sent(&rig.reconnect(rig.cfg.host_epoch)),
        vec![
            Sent::Hello(guardian_hello(&rig.cfg, rig.cfg.host_epoch)),
            Sent::Log(LogChunk {
                offset,
                bytes: tail
            }),
            Sent::Report(Report::Status(rig.g.status())),
        ]
    );
}

/// The log bytes of consecutive chunks that start at `offset`. Each chunk must start where the previous one ended.
fn joined(sent: &[Sent], offset: u64) -> Vec<u8> {
    let mut bytes = Vec::new();
    for item in sent {
        let Sent::Log(chunk) = item else {
            panic!("only log chunks: {item:?}")
        };
        assert_eq!(chunk.offset, offset + bytes.len() as u64);
        bytes.extend_from_slice(&chunk.bytes);
    }
    bytes
}

/// Core SV-9: a tail larger than one link frame streams in bounded frames, and a new host receives every frame of it
/// before the status.
#[test]
fn a_large_tail_streams_and_replays_before_the_status() {
    let full_frame = HEADER_LEN + DEFAULT_MAX_PAYLOAD as usize;
    let mut rig = Rig::with(GuardianConfig {
        log_bytes: full_frame * 2,
        ..config()
    });
    rig.authenticate(rig.cfg.host_epoch);
    let bytes: Vec<u8> = (0..rig.cfg.log_bytes).map(|i| i as u8).collect();

    let streamed = rig.input(Input::Log(bytes.clone()));
    assert!(streamed
        .iter()
        .all(|action| matches!(action, Action::LinkSend(frame) if frame.len() <= full_frame)));
    assert_eq!(joined(&sent(&streamed), 0), bytes);

    let replay = sent(&rig.reconnect(rig.cfg.host_epoch));
    let [hello, logs @ .., status] = &replay[..] else {
        panic!("a hello, the ring and the status: {replay:?}")
    };
    assert_eq!(
        hello,
        &Sent::Hello(guardian_hello(&rig.cfg, rig.cfg.host_epoch))
    );
    assert!(logs.len() > 1);
    assert_eq!(joined(logs, 0), bytes);
    assert_eq!(status, &Sent::Report(Report::Status(rig.g.status())));
}
