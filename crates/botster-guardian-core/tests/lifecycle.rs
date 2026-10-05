//! Guardian clauses through the machine's inputs, actions, and private wire.

use botster_core_contract::prelude::*;
use botster_core_edges::edges::{ExitStatus, ProcessIdentity};
use botster_core_edges::Machine;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::hello::Hello;
use botster_core_link::msg::PayloadId;
use botster_core_link::proof::token_proof;
use botster_guardian_core::guardian::SpawnResult;
use botster_guardian_core::wire::{Command, Report, ServiceSpec};
use botster_guardian_core::{Action, Guardian, GuardianConfig, Input};
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

const TOKEN: [u8; 32] = [7; 32];
const LEADER: PayloadId = PayloadId {
    pid: 42,
    start_time: 100,
};
const CHILD: ProcessIdentity = ProcessIdentity {
    pid: 43,
    start_time: 101,
};

#[allow(clippy::disallowed_methods)]
fn now() -> Instant {
    Instant::now()
}

fn config(log_bytes: usize) -> GuardianConfig {
    GuardianConfig {
        service: ServiceId([1; 32]),
        instance: InstanceId("service-instance".into()),
        token: TOKEN,
        host_epoch: 1,
        protocol: 1,
        orphan_grace: Duration::from_millis(5000),
        log_bytes,
    }
}

fn wire(kind: FrameType, payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut bytes).unwrap();
    bytes
}

fn hello(epoch: u64) -> Hello {
    let instance = config(0).instance;
    Hello {
        protocol: 1,
        proof: token_proof(&TOKEN, &instance, epoch),
        instance,
        host_epoch: epoch,
    }
}

fn authenticate(g: &mut Guardian, now: Instant, hello: Hello) {
    let mut payload = Vec::new();
    hello.encode(&mut payload).unwrap();
    for byte in wire(FrameType::HELLO, &payload) {
        g.handle(now, Input::LinkBytes(vec![byte]));
    }
}

fn command(g: &mut Guardian, now: Instant, command: Command) {
    g.handle(
        now,
        Input::LinkBytes(wire(
            FrameType::HOST_MSG,
            &serde_json::to_vec(&command).unwrap(),
        )),
    );
}

fn actions(g: &mut Guardian) -> Vec<Action> {
    std::iter::from_fn(|| g.poll_action()).collect()
}

fn reports(actions: &[Action]) -> Vec<Report> {
    actions
        .iter()
        .filter_map(|action| {
            let Action::LinkSend(bytes) = action else {
                return None;
            };
            let mut decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
            assert_eq!(decoder.push(bytes), bytes.len());
            let frame = decoder.next_frame().unwrap().unwrap();
            (frame.kind == FrameType::WORKER_MSG)
                .then(|| serde_json::from_slice(&frame.payload).unwrap())
        })
        .collect()
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

fn ready(now: Instant) -> Guardian {
    let mut g = Guardian::new(config(1024), now);
    actions(&mut g);
    authenticate(&mut g, now, hello(1));
    actions(&mut g);
    g
}

fn running(now: Instant) -> Guardian {
    let mut g = ready(now);
    command(&mut g, now, Command::Launch(Box::new(spec())));
    assert_eq!(
        actions(&mut g),
        vec![Action::SpawnService(Box::new(spec()))]
    );
    g.handle(
        now,
        Input::Spawned(SpawnResult::Started {
            payload: LEADER,
            report: SpawnReport::default(),
        }),
    );
    assert_eq!(
        reports(&actions(&mut g)),
        vec![Report::Started {
            payload: LEADER,
            report: SpawnReport::default()
        }]
    );
    command(&mut g, now, Command::EpochCommitted);
    g
}

fn killed(g: &mut Guardian, now: Instant, live: bool, remain: bool) {
    g.handle(
        now,
        Input::TreeKilled {
            leader_signalled: live,
            leader_exiting: !live,
            descendants_may_remain: remain,
        },
    );
}

fn finish_exit(
    g: &mut Guardian,
    now: Instant,
    status: ExitStatus,
    live_kill: bool,
    remain: bool,
) -> Vec<Action> {
    g.handle(now, Input::PayloadExited(status));
    let cleanup = actions(g);
    assert!(cleanup.contains(&Action::DrainLogs));
    assert!(cleanup
        .iter()
        .any(|a| matches!(a, Action::KillTree { leader: LEADER, .. })));
    assert!(!cleanup
        .iter()
        .any(|a| matches!(a, Action::ReapService { .. })));
    g.handle(now, Input::LogsDrained);
    killed(g, now, live_kill, remain);
    actions(g)
}

/// Core AD-6, A6-2: only the exact identity, proof, and protocol permit a launch.
#[test]
fn authentication_controls_launch_and_hides_the_token() {
    let at = now();
    for field in ["instance", "token", "epoch", "protocol", "kind"] {
        let mut g = Guardian::new(config(1024), at);
        let greeting = actions(&mut g);
        let Action::LinkSend(bytes) = &greeting[0] else {
            panic!("hello");
        };
        let mut decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
        decoder.push(bytes);
        let h = Hello::decode(&decoder.next_frame().unwrap().unwrap().payload).unwrap();
        assert_eq!(h, hello(1));
        let mut bad = hello(1);
        match field {
            "instance" => bad.instance = InstanceId("other".into()),
            "token" => bad.proof = token_proof(&[8; 32], &bad.instance, 1),
            "epoch" => bad = hello(0),
            "protocol" => bad.protocol = 2,
            "kind" => {}
            _ => unreachable!(),
        }
        if field == "kind" {
            command(&mut g, at, Command::Launch(Box::new(spec())));
        } else {
            authenticate(&mut g, at, bad);
        }
        command(&mut g, at, Command::Launch(Box::new(spec())));
        assert_eq!(actions(&mut g), vec![Action::LinkClose], "{field}");
        assert_eq!(g.next_deadline(), Some(at + Duration::from_millis(5000)));
    }
    assert!(!format!("{:?}", config(0)).contains("token"));
}

/// Core SV-1, SV-4, SV-7: the spawn result describes exec and no second launch exists.
#[test]
fn exec_report_is_exact_and_a_second_launch_is_ignored() {
    let at = now();
    let mut g = ready(at);
    let s = spec();
    command(&mut g, at, Command::Launch(Box::new(s.clone())));
    assert_eq!(actions(&mut g), vec![Action::SpawnService(Box::new(s))]);
    command(&mut g, at, Command::Launch(Box::new(spec())));
    assert!(actions(&mut g).is_empty());
    let report = SpawnReport {
        applied: vec![Bound::OpenFiles],
        unsupported: vec![Bound::Processes],
    };
    g.handle(
        at,
        Input::Spawned(SpawnResult::Started {
            payload: LEADER,
            report: report.clone(),
        }),
    );
    assert_eq!(
        reports(&actions(&mut g)),
        vec![Report::Started {
            payload: LEADER,
            report: report.clone()
        }]
    );
    assert_eq!(g.status().report, Some(report));
    let done = finish_exit(&mut g, at, ExitStatus::Code(1), false, false);
    assert!(done.contains(&Action::ReapService { leader: LEADER }));
    g.handle(at, Input::Reaped);
    command(&mut g, at, Command::Launch(Box::new(spec())));
    g.pump(at + Duration::from_secs(60000));
    assert!(actions(&mut g).is_empty());
    assert_eq!(g.status().payload, Some(LEADER));
}

/// Core SV-9, A2-5: a failed exec has no group, and Remove releases the guardian.
#[test]
fn failed_starts_report_the_edge_failure_and_roll_back_without_signalling() {
    let at = now();
    for result in [
        SpawnResult::Failed(StartFailReason::CwdMissing),
        SpawnResult::Failed(StartFailReason::ExecFailed { errno: 2 }),
        SpawnResult::BoundUnavailable(Bound::CpuSeconds),
    ] {
        let mut g = ready(at);
        command(&mut g, at, Command::Launch(Box::new(spec())));
        actions(&mut g);
        let expected = match result.clone() {
            SpawnResult::Failed(reason) => Report::StartFailed { reason },
            SpawnResult::BoundUnavailable(bound) => Report::BoundUnavailable { bound },
            _ => unreachable!(),
        };
        g.handle(at, Input::Spawned(result));
        assert_eq!(reports(&actions(&mut g)), vec![expected]);
        command(&mut g, at, Command::Remove);
        let pending = actions(&mut g);
        assert_eq!(reports(&pending), vec![Report::Removed]);
        assert!(!pending.contains(&Action::Exit));
        g.handle(at, Input::LinkWritten { total: u64::MAX });
        assert_eq!(actions(&mut g), vec![Action::LinkClose, Action::Exit]);
    }
}

/// Core A2-5: startup lasts through the first complete epoch, starting after exec.
#[test]
fn startup_deadline_is_armed_after_exec_and_cancelled_by_epoch_commit() {
    let at = now();
    let mut g = ready(at);
    command(&mut g, at, Command::Launch(Box::new(spec())));
    actions(&mut g);
    assert_eq!(g.next_deadline(), None);
    g.handle(
        at,
        Input::Spawned(SpawnResult::Started {
            payload: LEADER,
            report: SpawnReport::default(),
        }),
    );
    actions(&mut g);
    let due = at + spec().limits.startup;
    assert_eq!(g.next_deadline(), Some(due));
    g.pump(due - Duration::from_nanos(1));
    assert!(actions(&mut g).is_empty());
    g.pump(due);
    assert_eq!(
        actions(&mut g),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![]
        }]
    );
    killed(&mut g, due, true, false);
    g.handle(due, Input::PayloadExited(ExitStatus::Signal(9)));
    actions(&mut g);
    g.handle(due, Input::LogsDrained);
    assert_eq!(
        g.status().exit.unwrap().cause,
        ServiceExitCause::StartupTimeout
    );
    assert_eq!(g.next_deadline(), None);

    let mut g = running(at);
    assert_eq!(g.next_deadline(), None);
    g.pump(due);
    assert!(actions(&mut g).is_empty());
}

/// Core SV-8, AD-6: transport acceptance does not cancel grace; authentication does.
#[test]
fn orphan_grace_survives_invalid_adoption_and_valid_adoption_cancels_it() {
    let at = now();
    let mut g = running(at);
    g.handle(at, Input::LinkClosed);
    actions(&mut g);
    let due = at + Duration::from_millis(5000);
    assert_eq!(g.next_deadline(), Some(due));
    g.handle(at + Duration::from_secs(1), Input::LinkConnected);
    actions(&mut g);
    authenticate(&mut g, at + Duration::from_secs(1), hello(0));
    assert_eq!(actions(&mut g), vec![Action::LinkClose]);
    assert_eq!(g.next_deadline(), Some(due));
    g.handle(at + Duration::from_secs(2), Input::LinkConnected);
    actions(&mut g);
    authenticate(&mut g, at + Duration::from_secs(2), hello(2));
    assert_eq!(reports(&actions(&mut g))[0], Report::Status(g.status()));
    assert_eq!(g.next_deadline(), None);
    g.pump(due);
    assert!(actions(&mut g).is_empty());
    g.handle(due, Input::LinkClosed);
    actions(&mut g);
    g.pump(due + Duration::from_millis(5000));
    assert_eq!(
        actions(&mut g),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![]
        }]
    );
    let ended = due + Duration::from_millis(5000);
    killed(&mut g, ended, true, false);
    g.handle(ended, Input::PayloadExited(ExitStatus::Signal(9)));
    assert_eq!(actions(&mut g), vec![Action::DrainLogs]);
    g.handle(ended, Input::LogsDrained);
    assert_eq!(
        g.status().exit.unwrap().cause,
        ServiceExitCause::OrphanGrace
    );
    assert_eq!(
        actions(&mut g),
        vec![Action::ReapService { leader: LEADER }]
    );
    g.handle(ended, Input::Reaped);
    assert_eq!(actions(&mut g), vec![Action::Exit]);
    g.handle(ended, Input::Terminate);
    g.pump(ended + Duration::from_secs(60000));
    assert!(actions(&mut g).is_empty());
}

/// Core SV-9: enumerate before TERM; KILL after the exact grace; never signal after reap.
#[test]
fn stop_enumerates_then_terms_then_kills_the_group_and_descendants() {
    let at = now();
    let mut g = running(at);
    command(&mut g, at, Command::Stop);
    assert_eq!(actions(&mut g), vec![Action::Enumerate { leader: LEADER }]);
    g.handle(at, Input::Descendants(vec![CHILD]));
    assert_eq!(
        actions(&mut g),
        vec![Action::TermService { leader: LEADER }]
    );
    g.handle(
        at,
        Input::TermSent {
            delivered: true,
            leader_exiting: false,
        },
    );
    command(&mut g, at + Duration::from_millis(100), Command::Stop);
    assert!(actions(&mut g).is_empty());
    let due = at + Duration::from_millis(300);
    assert_eq!(g.next_deadline(), Some(due));
    g.pump(due - Duration::from_nanos(1));
    assert!(actions(&mut g).is_empty());
    g.pump(due);
    assert_eq!(
        actions(&mut g),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![CHILD]
        }]
    );
    killed(&mut g, due, true, true);
    g.handle(due, Input::PayloadExited(ExitStatus::Signal(9)));
    assert_eq!(actions(&mut g), vec![Action::DrainLogs]);
    g.handle(due, Input::LogsDrained);
    let report = actions(&mut g);
    assert_eq!(
        g.status().exit,
        Some(ServiceExit {
            code: None,
            signal: Some(9),
            cause: ServiceExitCause::Killed,
            descendants_may_remain: true
        })
    );
    assert_eq!(
        reports(&report),
        vec![Report::Exited {
            exit: g.status().exit.unwrap()
        }]
    );
    assert_eq!(report.last(), Some(&Action::ReapService { leader: LEADER }));
    g.handle(due, Input::Reaped);
    command(&mut g, due, Command::Stop);
    assert!(actions(&mut g).is_empty());
}

/// Core SV-5, SV-9: TERM exit stays HostStop; cleanup KILL does not claim the leader's exit.
#[test]
fn a_term_exit_is_host_stop_and_the_cleanup_kill_is_not_an_exit_cause() {
    let at = now();
    let mut g = running(at);
    command(&mut g, at, Command::Stop);
    actions(&mut g);
    g.handle(at, Input::Descendants(vec![]));
    actions(&mut g);
    g.handle(
        at,
        Input::TermSent {
            delivered: true,
            leader_exiting: false,
        },
    );
    finish_exit(&mut g, at, ExitStatus::Signal(15), false, false);
    assert_eq!(g.status().exit.unwrap().cause, ServiceExitCause::HostStop);
}

/// Core SV-5: limits are not causes; a failed signal cannot claim an unrelated exit.
#[test]
fn external_exits_and_cooperative_cause_bytes_remain_observable() {
    let at = now();
    for (status, byte, expected) in [
        (ExitStatus::Code(1), None, ServiceExitCause::Normal),
        (ExitStatus::Signal(24), None, ServiceExitCause::Signal),
        (
            ExitStatus::Code(1),
            Some(7),
            ServiceExitCause::ChildReported(7),
        ),
    ] {
        let mut g = running(at);
        command(&mut g, at, Command::Stop);
        actions(&mut g);
        g.handle(at, Input::Descendants(vec![]));
        actions(&mut g);
        g.handle(
            at,
            Input::TermSent {
                delivered: false,
                leader_exiting: true,
            },
        );
        if let Some(byte) = byte {
            g.handle(at, Input::Cause(byte));
            g.handle(at, Input::Cause(8));
        }
        finish_exit(&mut g, at, status, false, false);
        assert_eq!(g.status().exit.unwrap().cause, expected);
    }
}

/// Core SV-9: the ring retains only the newest bytes and survives a control reconnect.
#[test]
fn logs_are_bounded_and_remain_available_after_exit_and_adoption() {
    let at = now();
    let mut g = running(at);
    g.handle(at, Input::Log(vec![0x11; 2000]));
    g.handle(at, Input::Log(vec![0xbb; 16]));
    assert_eq!(g.log_tail(100000).len(), 1024);
    assert_eq!(g.log_tail(16), vec![0xbb; 16]);
    assert!(g.log_tail(0).is_empty());
    let before = g.log_tail(1024);
    finish_exit(&mut g, at, ExitStatus::Code(0), false, false);
    g.handle(at, Input::Reaped);
    g.handle(at, Input::LinkClosed);
    actions(&mut g);
    g.handle(at, Input::LinkConnected);
    actions(&mut g);
    authenticate(&mut g, at, hello(2));
    actions(&mut g);
    assert_eq!(g.log_tail(1024), before);
    command(&mut g, at, Command::LogTail { req: 1, max: 16 });
    assert_eq!(
        reports(&actions(&mut g)),
        vec![Report::Log {
            req: 1,
            bytes: vec![0xbb; 16],
            last: true
        }]
    );
}

/// Core SV-9: large tails use bounded link frames without changing the log limit.
#[test]
fn log_reports_split_at_the_link_bound_and_empty_tails_finish() {
    let at = now();
    let mut cfg = config(DEFAULT_MAX_PAYLOAD as usize);
    cfg.log_bytes = DEFAULT_MAX_PAYLOAD as usize;
    let mut g = Guardian::new(cfg, at);
    actions(&mut g);
    authenticate(&mut g, at, hello(1));
    actions(&mut g);
    command(
        &mut g,
        at,
        Command::LogTail {
            req: u64::MAX,
            max: u64::MAX,
        },
    );
    assert_eq!(
        reports(&actions(&mut g)),
        vec![Report::Log {
            req: u64::MAX,
            bytes: vec![],
            last: true
        }]
    );
    let bytes = vec![255; DEFAULT_MAX_PAYLOAD as usize];
    g.handle(at, Input::Log(bytes.clone()));
    command(
        &mut g,
        at,
        Command::LogTail {
            req: u64::MAX,
            max: u64::MAX,
        },
    );
    let chunks = reports(&actions(&mut g));
    let mut received = Vec::new();
    for (index, report) in chunks.iter().enumerate() {
        let Report::Log { req, bytes, last } = report else {
            panic!("log chunk");
        };
        assert_eq!(*req, u64::MAX);
        assert_eq!(*last, index + 1 == chunks.len());
        received.extend_from_slice(bytes);
    }
    assert_eq!(received, bytes);
}

/// Core SV-9: teardown waits for an outstanding exec, census, kill, drain, and reap.
#[test]
fn cleanup_requests_during_exec_are_not_lost() {
    let at = now();
    for terminate in [false, true] {
        let mut g = ready(at);
        command(&mut g, at, Command::Launch(Box::new(spec())));
        actions(&mut g);
        if terminate {
            g.handle(at, Input::Terminate);
        } else {
            command(&mut g, at, Command::Remove);
        }
        assert!(actions(&mut g).is_empty());
        g.handle(
            at,
            Input::Spawned(SpawnResult::Started {
                payload: LEADER,
                report: SpawnReport::default(),
            }),
        );
        let spawned = actions(&mut g);
        assert!(spawned.contains(&Action::KillTree {
            leader: LEADER,
            known: vec![]
        }));
        assert!(!spawned.contains(&Action::Exit));
        killed(&mut g, at, true, false);
        g.handle(at, Input::PayloadExited(ExitStatus::Signal(9)));
        actions(&mut g);
        g.handle(at, Input::LogsDrained);
        assert!(actions(&mut g).contains(&Action::ReapService { leader: LEADER }));
        assert!(g.status().exit.is_some());
        g.handle(at, Input::Reaped);
        let end = actions(&mut g);
        if terminate {
            assert_eq!(end, vec![Action::LinkClose, Action::Exit]);
        } else {
            assert_eq!(reports(&end), vec![Report::Removed]);
            assert!(!end.contains(&Action::Exit));
            g.handle(at, Input::LinkClosed);
            assert_eq!(actions(&mut g), vec![Action::LinkClose, Action::Exit]);
        }
    }
}

/// Core SV-9: a census that completes after the stop deadline still supplies the kill's identities.
#[test]
fn a_late_census_cannot_omit_known_descendants_or_reap_the_leader_early() {
    let at = now();
    let mut g = running(at);
    command(&mut g, at, Command::Stop);
    actions(&mut g);
    let due = at + Duration::from_millis(300);
    g.pump(due);
    assert!(actions(&mut g).is_empty());
    g.handle(due, Input::Descendants(vec![CHILD]));
    assert_eq!(
        actions(&mut g),
        vec![Action::KillTree {
            leader: LEADER,
            known: vec![CHILD]
        }]
    );
    g.handle(due, Input::PayloadExited(ExitStatus::Signal(9)));
    assert_eq!(actions(&mut g), vec![Action::DrainLogs]);
    g.handle(due, Input::LogsDrained);
    assert!(actions(&mut g).is_empty());
    killed(&mut g, due, true, false);
    assert!(actions(&mut g).contains(&Action::ReapService { leader: LEADER }));
}

/// Core SV-9: a leader exit before the spawn answer remains owned through cleanup.
#[test]
fn exit_before_spawn_answer_is_retained() {
    let at = now();
    let mut g = ready(at);
    command(&mut g, at, Command::Launch(Box::new(spec())));
    actions(&mut g);
    g.handle(at, Input::PayloadExited(ExitStatus::Code(1)));
    assert!(actions(&mut g).is_empty());
    g.handle(
        at,
        Input::Spawned(SpawnResult::Started {
            payload: LEADER,
            report: SpawnReport::default(),
        }),
    );
    let cleanup = actions(&mut g);
    assert!(cleanup.contains(&Action::DrainLogs));
    assert!(cleanup.contains(&Action::KillTree {
        leader: LEADER,
        known: vec![]
    }));
    killed(&mut g, at, false, false);
    g.handle(at, Input::LogsDrained);
    assert_eq!(g.status().exit.unwrap().cause, ServiceExitCause::Normal);
    assert!(actions(&mut g).contains(&Action::ReapService { leader: LEADER }));
}

/// Core SV-5, SV-9: reordered edge results retain Killed and wait for every pending result.
#[test]
fn a_late_term_result_cannot_overwrite_the_kill_cause() {
    let at = now();
    let mut g = running(at);
    command(&mut g, at, Command::Stop);
    actions(&mut g);
    g.handle(at, Input::Descendants(vec![]));
    actions(&mut g);
    let due = at + Duration::from_millis(300);
    g.pump(due);
    actions(&mut g);
    killed(&mut g, due, true, false);
    g.handle(due, Input::PayloadExited(ExitStatus::Signal(9)));
    actions(&mut g);
    g.handle(due, Input::LogsDrained);
    assert!(actions(&mut g).is_empty());
    g.handle(
        due,
        Input::TermSent {
            delivered: true,
            leader_exiting: false,
        },
    );
    assert_eq!(g.status().exit.unwrap().cause, ServiceExitCause::Killed);
    assert!(actions(&mut g).contains(&Action::ReapService { leader: LEADER }));
}

/// Core SV-9: Remove waits for the actual report bytes, then closes and ends once.
#[test]
fn removal_waits_for_every_report_byte_and_cannot_signal_after_it_ends() {
    let at = now();
    let mut g = Guardian::new(config(0), at);
    let mut written = actions(&mut g)
        .into_iter()
        .map(|a| match a {
            Action::LinkSend(bytes) => bytes.len() as u64,
            _ => panic!("hello"),
        })
        .sum::<u64>();
    authenticate(&mut g, at, hello(1));
    written += actions(&mut g)
        .into_iter()
        .map(|a| match a {
            Action::LinkSend(bytes) => bytes.len() as u64,
            _ => panic!("status"),
        })
        .sum::<u64>();
    command(&mut g, at, Command::Remove);
    let removed = actions(&mut g);
    assert_eq!(reports(&removed), vec![Report::Removed]);
    let total = written
        + removed
            .into_iter()
            .map(|a| match a {
                Action::LinkSend(bytes) => bytes.len() as u64,
                _ => panic!("removed"),
            })
            .sum::<u64>();
    g.handle(at, Input::LinkWritten { total: total - 1 });
    assert!(actions(&mut g).is_empty());
    g.handle(at, Input::LinkWritten { total: written });
    assert!(actions(&mut g).is_empty());
    g.handle(at, Input::LinkWritten { total });
    assert_eq!(actions(&mut g), vec![Action::LinkClose, Action::Exit]);
    for input in [
        Input::Terminate,
        Input::LinkClosed,
        Input::LinkConnected,
        Input::Reaped,
        Input::PayloadExited(ExitStatus::Code(0)),
    ] {
        g.handle(at, input);
    }
    g.pump(at + Duration::from_secs(60000));
    assert_eq!(g.next_deadline(), None);
    assert!(actions(&mut g).is_empty());
}

/// Core SV-8, SV-9: orphan grace and termination also release a guardian with no launched payload.
#[test]
fn unlaunched_guardians_end_on_orphan_grace_and_termination() {
    let at = now();
    let mut g = Guardian::new(config(0), at);
    actions(&mut g);
    let due = at + Duration::from_millis(5000);
    g.pump(due - Duration::from_nanos(1));
    assert!(actions(&mut g).is_empty());
    g.pump(due);
    assert_eq!(actions(&mut g), vec![Action::LinkClose, Action::Exit]);
    assert_eq!(g.next_deadline(), None);
    let mut g = ready(at);
    g.handle(at, Input::Terminate);
    assert_eq!(actions(&mut g), vec![Action::LinkClose, Action::Exit]);
}

/// Core AD-6: a malformed or oversize control frame closes only the control connection.
#[test]
fn malformed_and_oversize_control_frames_do_not_launch_or_extend_grace() {
    let at = now();
    for bytes in [
        wire(FrameType::HELLO, b"not JSON"),
        wire(FrameType::HELLO, b"{}"),
        vec![255, 255, 255, 255, 1],
    ] {
        let mut g = Guardian::new(config(0), at);
        actions(&mut g);
        g.handle(at, Input::LinkBytes(bytes));
        assert_eq!(actions(&mut g), vec![Action::LinkClose]);
        assert_eq!(g.next_deadline(), Some(at + Duration::from_millis(5000)));
    }
}
