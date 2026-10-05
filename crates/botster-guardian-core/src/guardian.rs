//! Service lifetime and cleanup (Core SV-5, SV-7, SV-8, SV-9; plan 2.2).

use crate::link::{frame, Link, LinkState};
use crate::log::LogRing;
use crate::wire::{Command, LogChunk, Report, ServiceSpec, Status, LOG_CHUNK_BYTES, LOG_FRAME};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{ExitStatus, ProcessIdentity};
use botster_core_edges::Machine;
use botster_core_link::frame::{Frame, FrameType};
use botster_core_link::hello::HelloError;
use botster_core_link::msg::PayloadId;
use botster_core_link::proof::TOKEN_LEN;
use std::collections::VecDeque;
use std::time::{Duration, Instant};

/// Configuration from the host. The driver supplies `now` separately.
#[derive(Clone)]
pub struct GuardianConfig {
    pub service: ServiceId,
    pub instance: InstanceId,
    pub token: [u8; TOKEN_LEN],
    pub host_epoch: u64,
    pub protocol: u8,
    /// The service's `ServiceLimits.orphan_grace`. It runs from the start, before any host authenticates (SV-8).
    pub orphan_grace: Duration,
    /// `CoreLimits.service_log_bytes` (SV-9).
    pub log_bytes: usize,
}

impl std::fmt::Debug for GuardianConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GuardianConfig")
            .field("service", &self.service)
            .field("instance", &self.instance)
            .field("host_epoch", &self.host_epoch)
            .field("protocol", &self.protocol)
            .field("orphan_grace", &self.orphan_grace)
            .field("log_bytes", &self.log_bytes)
            .finish_non_exhaustive()
    }
}

/// A process edge result. Success means that `exec` succeeded (SV-1, SV-4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpawnResult {
    Started {
        payload: PayloadId,
        report: SpawnReport,
    },
    Failed(StartFailReason),
    BoundUnavailable(Bound),
}

/// Inputs from either driver. No input reads a clock or performs I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    LinkBytes(Vec<u8>),
    LinkClosed,
    /// A new transport connected. Authentication must still succeed.
    LinkConnected,
    /// Bytes written on this connection, counted from its first `LinkSend`.
    LinkWritten {
        total: u64,
    },
    Spawned(SpawnResult),
    /// Captured stdout or stderr bytes.
    Log(Vec<u8>),
    /// The first byte on the cooperative cause descriptor (SV-5).
    Cause(u8),
    /// The leader exited. The edge must leave it unreaped.
    PayloadExited(ExitStatus),
    /// The driver read the output and cause bytes present at the exit.
    LogsDrained,
    /// The result of `Enumerate`. The driver verifies each identity before a kill.
    Descendants(Vec<ProcessIdentity>),
    /// The result of `TermService`: whether SIGTERM was delivered, and whether the leader had already started to exit.
    TermSent {
        delivered: bool,
        leader_exiting: bool,
    },
    /// The result of `KillTree`, including the driver's census after the kill.
    TreeKilled {
        leader_signalled: bool,
        leader_exiting: bool,
        descendants_may_remain: bool,
    },
    /// The driver reaped the leader after `ReapService`.
    Reaped,
    /// Process shutdown. Cleanup continues without waiting for the host.
    Terminate,
    /// A deadline from [`Machine::next_deadline`] is due.
    Timer,
}

/// Actions for either driver. The driver must preserve their order and answer each process action with its input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    LinkSend(Vec<u8>),
    LinkClose,
    /// Answered by `Spawned`.
    SpawnService(Box<ServiceSpec>),
    /// Enumerate descendants before SIGTERM can end the leader (SV-9). Answered by `Descendants`.
    Enumerate {
        leader: PayloadId,
    },
    /// Send SIGTERM to the service, not to the guardian or its host. Answered by `TermSent`.
    TermService {
        leader: PayloadId,
    },
    /// Kill the unreaped leader's group, the verified `known` descendants and a current enumeration, then check for
    /// survivors (SV-9). Answered by `TreeKilled`.
    KillTree {
        leader: PayloadId,
        known: Vec<ProcessIdentity>,
    },
    /// Read the bytes present now; a descendant that holds a pipe cannot prolong it. Answered by `LogsDrained`.
    DrainLogs,
    /// Answered by `Reaped`.
    ReapService {
        leader: PayloadId,
    },
    Exit,
}

/// The guardian. Only the driver owns descriptors, process watches, and the clock.
#[derive(Debug)]
pub struct Guardian {
    cfg: GuardianConfig,
    link: Link,
    service: Service,
    /// Facts retained across host loss (SV-8, A4-1).
    payload: Option<PayloadId>,
    spawn_report: Option<SpawnReport>,
    exit: Option<ServiceExit>,
    log: LogRing,
    /// The log offset that this connection has queued, and the link byte count at the end of that batch.
    log_sent: u64,
    log_batch_end: u64,
    /// Runs while no host is authenticated (SV-8).
    orphan: Option<Instant>,
    ending: Option<Ending>,
    actions: VecDeque<Action>,
}

/// Why the guardian ends itself once the service is gone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ending {
    /// `Remove`: report `Removed` once the service is gone.
    Remove,
    /// `Removed` is queued; the guardian ends when the driver wrote it or the link closed.
    Flush { until: u64 },
    /// Shutdown or orphan grace: end without waiting for a host.
    Now,
    /// `Exit` is emitted. Every later input is ignored.
    Done,
}

#[derive(Debug)]
enum Service {
    Unlaunched,
    /// `SpawnService` is outstanding.
    Spawning(Spawn),
    Live(Leader),
    /// `ReapService` is outstanding.
    Reaping,
    /// Reaped, or never started. The guardian never launches again (SV-7).
    Gone,
}

/// A launch whose exec result is outstanding. Inputs that arrive before the result wait here.
#[derive(Debug, Clone, Copy)]
struct Spawn {
    startup: Duration,
    stop_grace: Duration,
    exited: Option<ExitStatus>,
    pending: Option<Teardown>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Teardown {
    Stop,
    Kill(ServiceExitCause),
}

/// The service's leader process, which the guardian keeps unreaped until cleanup ends.
#[derive(Debug)]
struct Leader {
    id: PayloadId,
    stop_grace: Duration,
    /// Runs from exec until the host commits epoch 1 (A2-5).
    startup: Option<Instant>,
    exit: Exit,
    signals: Signals,
    known: Vec<ProcessIdentity>,
    cause_byte: Option<u8>,
    /// The cause of the guardian's last signal that reached a live leader (SV-5: a cause is only what is observed).
    signalled: Option<ServiceExitCause>,
}

/// The leader's exit, observed without reaping it.
#[derive(Debug, Clone, Copy)]
enum Exit {
    Running,
    /// `DrainLogs` is outstanding.
    Exited(ExitStatus),
    Drained(ExitStatus),
}

/// The guardian's signals to the service (SV-9). Each request waits for its result, so a kill requested meanwhile runs
/// after it: SIGKILL never overtakes the census or SIGTERM.
#[derive(Debug, Clone, Copy)]
enum Signals {
    Idle,
    /// `Enumerate` is outstanding. SIGTERM follows, or the kill in `then_kill`.
    Census {
        then_kill: Option<ServiceExitCause>,
    },
    /// `TermService` is outstanding. The grace follows, or the kill in `then_kill`.
    Term {
        then_kill: Option<ServiceExitCause>,
    },
    /// SIGTERM's result arrived. The grace runs from that result's time, so census and delivery delays cannot shorten it.
    Grace {
        kill_at: Instant,
    },
    /// `KillTree` is outstanding. `cause` is the exit cause if the kill reaches a live leader.
    Killing {
        cause: ServiceExitCause,
    },
    Killed {
        descendants_may_remain: bool,
    },
}

impl Guardian {
    /// Start with a connected, unauthenticated link. Only authentication cancels orphan grace (AD-6, SV-8).
    /// Refuses a configuration whose hello cannot be encoded.
    pub fn new(cfg: GuardianConfig, now: Instant) -> Result<Self, HelloError> {
        let link = Link::new(cfg.host_epoch);
        let hello = link.hello(&cfg)?;
        let mut guardian = Guardian {
            orphan: Some(now + cfg.orphan_grace),
            log: LogRing::new(cfg.log_bytes),
            cfg,
            link,
            service: Service::Unlaunched,
            payload: None,
            spawn_report: None,
            exit: None,
            log_sent: 0,
            log_batch_end: 0,
            ending: None,
            actions: VecDeque::new(),
        };
        guardian.emit(hello);
        Ok(guardian)
    }

    /// The retained state. It survives host loss, and the guardian never restarts a service (SV-7).
    pub fn status(&self) -> Status {
        Status {
            service: self.cfg.service,
            payload: self.payload,
            report: self.spawn_report.clone(),
            exit: self.exit,
        }
    }

    fn emit(&mut self, bytes: Vec<u8>) {
        self.link.queue(&bytes);
        self.actions.push_back(Action::LinkSend(bytes));
    }

    fn report(&mut self, report: Report) {
        if self.link.state == LinkState::Ready {
            let json = serde_json::to_vec(&report).expect("guardian reports serialize");
            self.emit(frame(FrameType::WORKER_MSG, &json));
        }
    }

    fn close_link(&mut self) {
        if self.link.state != LinkState::Closed {
            self.link.state = LinkState::Closed;
            self.actions.push_back(Action::LinkClose);
        }
    }

    fn on_bytes(&mut self, now: Instant, mut bytes: &[u8]) {
        while !bytes.is_empty() && self.link.state != LinkState::Closed {
            let (took, frame) = self.link.read(bytes);
            bytes = &bytes[took..];
            match frame {
                Ok(Some(frame)) => self.on_frame(now, frame),
                Ok(None) => {}
                Err(_) => self.on_closed(now),
            }
        }
    }

    fn on_frame(&mut self, now: Instant, frame: Frame) {
        match self.link.state {
            LinkState::AwaitHello => {
                if self.link.authenticate(&self.cfg, &frame) {
                    self.orphan = None;
                    self.report(Report::Status(self.status()));
                    self.log_sent = 0;
                    self.log_batch_end = 0;
                    self.flush_log();
                } else {
                    self.on_closed(now);
                }
            }
            LinkState::Ready if frame.kind == FrameType::HOST_MSG => {
                if let Ok(command) = Command::decode(&frame.payload) {
                    self.command(command);
                }
            }
            LinkState::Ready | LinkState::Closed => {}
        }
    }

    fn on_closed(&mut self, now: Instant) {
        self.close_link();
        // A rejected connection does not extend a running orphan deadline (SV-8).
        if self.ending.is_none() && self.orphan.is_none() {
            self.orphan = Some(now + self.cfg.orphan_grace);
        }
        self.try_finish();
    }

    /// Sends the log bytes that this connection lacks, one batch at a time: a batch waits until the previous one is
    /// written, so the queued log never exceeds the ring (SV-9).
    fn flush_log(&mut self) {
        if self.link.state != LinkState::Ready || self.link.written() < self.log_batch_end {
            return;
        }
        let (from, bytes) = self.log.since(self.log_sent);
        if bytes.is_empty() {
            return;
        }
        for (offset, chunk) in (from..)
            .step_by(LOG_CHUNK_BYTES)
            .zip(bytes.chunks(LOG_CHUNK_BYTES))
        {
            let chunk = LogChunk {
                offset,
                bytes: chunk.to_vec(),
            };
            self.emit(frame(LOG_FRAME, &chunk.encode()));
        }
        self.log_sent = self.log.end();
        self.log_batch_end = self.link.queued();
    }

    /// An authenticated command. The guardian takes no command once it is ending.
    fn command(&mut self, command: Command) {
        if self.ending.is_some() {
            return;
        }
        match command {
            Command::Launch(spec) => {
                if let Service::Unlaunched = self.service {
                    self.service = Service::Spawning(Spawn {
                        startup: spec.limits.startup,
                        stop_grace: spec.limits.stop_grace,
                        exited: None,
                        pending: None,
                    });
                    self.actions.push_back(Action::SpawnService(spec));
                }
            }
            Command::EpochCommitted => {
                if let Service::Live(leader) = &mut self.service {
                    leader.startup = None;
                }
            }
            Command::Stop => self.teardown(Teardown::Stop),
            Command::Remove => {
                self.ending = Some(Ending::Remove);
                self.orphan = None;
                self.teardown(Teardown::Kill(ServiceExitCause::Killed));
                self.try_finish();
            }
        }
    }

    fn teardown(&mut self, teardown: Teardown) {
        match &mut self.service {
            Service::Spawning(Spawn { pending, .. }) => {
                // A kill replaces a stop; the first kill keeps its cause.
                if !matches!(pending, Some(Teardown::Kill(_))) {
                    *pending = Some(teardown);
                }
            }
            Service::Live(leader) => match teardown {
                Teardown::Stop => leader.stop(&mut self.actions),
                Teardown::Kill(cause) => leader.kill(cause, &mut self.actions),
            },
            Service::Unlaunched | Service::Reaping | Service::Gone => {}
        }
    }

    /// Shutdown or orphan grace: kill the tree and end without a host (SV-8, SV-9).
    fn end_now(&mut self, cause: ServiceExitCause) {
        self.ending = Some(Ending::Now);
        self.orphan = None;
        self.teardown(Teardown::Kill(cause));
        self.try_finish();
    }

    /// Applies what arrived while exec was outstanding: an exit, then a teardown, else the startup deadline (A2-5).
    fn on_spawned(&mut self, now: Instant, spawn: Spawn, result: SpawnResult) {
        self.service = Service::Gone;
        match result {
            SpawnResult::Started { payload, report } => {
                self.payload = Some(payload);
                self.spawn_report = Some(report.clone());
                self.report(Report::Started { payload, report });
                let mut leader = Leader::new(payload, spawn.stop_grace);
                let out = &mut self.actions;
                match (spawn.exited, spawn.pending) {
                    (Some(status), _) => leader.on_exit(status, out),
                    (None, Some(Teardown::Kill(cause))) => leader.kill(cause, out),
                    (None, Some(Teardown::Stop)) => leader.stop(out),
                    (None, None) => leader.startup = Some(now + spawn.startup),
                }
                self.service = Service::Live(leader);
            }
            SpawnResult::Failed(reason) => self.report(Report::StartFailed { reason }),
            SpawnResult::BoundUnavailable(bound) => self.report(Report::BoundUnavailable { bound }),
        }
        self.try_finish();
    }

    /// Reports the exit and reaps once the leader is drained and the tree is killed (SV-5, SV-9).
    fn settle(&mut self) {
        let Service::Live(leader) = &self.service else {
            return;
        };
        let Some(exit) = leader.settled() else {
            return;
        };
        let id = leader.id;
        self.exit = Some(exit);
        self.service = Service::Reaping;
        self.report(Report::Exited { exit });
        self.actions.push_back(Action::ReapService { leader: id });
    }

    fn try_finish(&mut self) {
        if !matches!(self.service, Service::Unlaunched | Service::Gone) {
            return;
        }
        if self.ending == Some(Ending::Remove) {
            self.report(Report::Removed);
            self.ending = Some(Ending::Flush {
                until: self.link.queued(),
            });
        }
        let end = match self.ending {
            Some(Ending::Now) => true,
            Some(Ending::Flush { until }) => {
                self.link.state == LinkState::Closed || self.link.written() >= until
            }
            Some(Ending::Remove | Ending::Done) | None => false,
        };
        if end {
            self.close_link();
            self.ending = Some(Ending::Done);
            self.actions.push_back(Action::Exit);
        }
    }

    fn on_timer(&mut self, now: Instant) {
        if self.orphan.is_some_and(|at| at <= now) {
            self.end_now(ServiceExitCause::OrphanGrace);
        }
        if let Service::Live(leader) = &mut self.service {
            leader.on_timer(now, &mut self.actions);
        }
    }

    /// A process result for the live leader. Results for no live leader are stale and change nothing.
    fn with_leader(&mut self, apply: impl FnOnce(&mut Leader, &mut VecDeque<Action>)) {
        if let Service::Live(leader) = &mut self.service {
            apply(leader, &mut self.actions);
            self.settle();
        }
    }
}

impl Leader {
    fn new(id: PayloadId, stop_grace: Duration) -> Self {
        Leader {
            id,
            stop_grace,
            startup: None,
            exit: Exit::Running,
            signals: Signals::Idle,
            known: Vec::new(),
            cause_byte: None,
            signalled: None,
        }
    }

    /// Stop starts with a census, so a descendant that SIGTERM orphans is still known to the kill (SV-9).
    fn stop(&mut self, out: &mut VecDeque<Action>) {
        if let Signals::Idle = self.signals {
            self.startup = None;
            self.signals = Signals::Census { then_kill: None };
            out.push_back(Action::Enumerate { leader: self.id });
        }
    }

    fn kill(&mut self, cause: ServiceExitCause, out: &mut VecDeque<Action>) {
        self.startup = None;
        match self.signals {
            Signals::Idle | Signals::Grace { .. } => self.send_kill(cause, out),
            Signals::Census { then_kill: None } => {
                self.signals = Signals::Census {
                    then_kill: Some(cause),
                }
            }
            Signals::Term { then_kill: None } => {
                self.signals = Signals::Term {
                    then_kill: Some(cause),
                }
            }
            // The first kill keeps its cause.
            Signals::Census { .. }
            | Signals::Term { .. }
            | Signals::Killing { .. }
            | Signals::Killed { .. } => {}
        }
    }

    fn send_kill(&mut self, cause: ServiceExitCause, out: &mut VecDeque<Action>) {
        self.signals = Signals::Killing { cause };
        out.push_back(Action::KillTree {
            leader: self.id,
            known: self.known.clone(),
        });
    }

    fn on_census(&mut self, known: Vec<ProcessIdentity>, out: &mut VecDeque<Action>) {
        let Signals::Census { then_kill } = self.signals else {
            return;
        };
        self.known = known;
        match then_kill {
            Some(cause) => self.send_kill(cause, out),
            None => {
                self.signals = Signals::Term { then_kill: None };
                out.push_back(Action::TermService { leader: self.id });
            }
        }
    }

    /// Only `Stop` sends SIGTERM, so a SIGTERM that ends a live leader is `HostStop` (SV-5).
    fn on_term(&mut self, now: Instant, reached_live_leader: bool, out: &mut VecDeque<Action>) {
        let Signals::Term { then_kill } = self.signals else {
            return;
        };
        if reached_live_leader {
            self.signalled = Some(ServiceExitCause::HostStop);
        }
        match then_kill {
            Some(cause) => self.send_kill(cause, out),
            None => {
                self.signals = Signals::Grace {
                    kill_at: now + self.stop_grace,
                }
            }
        }
    }

    fn on_tree_killed(&mut self, reached_live_leader: bool, descendants_may_remain: bool) {
        let Signals::Killing { cause } = self.signals else {
            return;
        };
        if reached_live_leader {
            self.signalled = Some(cause);
        }
        self.signals = Signals::Killed {
            descendants_may_remain,
        };
    }

    /// The exit also starts the tree kill: the group's survivors die with the leader (SV-9).
    fn on_exit(&mut self, status: ExitStatus, out: &mut VecDeque<Action>) {
        if let Exit::Running = self.exit {
            self.exit = Exit::Exited(status);
            out.push_back(Action::DrainLogs);
            self.kill(ServiceExitCause::Killed, out);
        }
    }

    fn on_drained(&mut self) {
        if let Exit::Exited(status) = self.exit {
            self.exit = Exit::Drained(status);
        }
    }

    /// The first byte counts. The drain reads every byte present at the exit, so a later byte is not the service's.
    fn on_cause(&mut self, byte: u8) {
        if !matches!(self.exit, Exit::Drained(_)) {
            self.cause_byte.get_or_insert(byte);
        }
    }

    fn on_timer(&mut self, now: Instant, out: &mut VecDeque<Action>) {
        if self.startup.is_some_and(|at| at <= now) {
            self.kill(ServiceExitCause::StartupTimeout, out);
        }
        if let Signals::Grace { kill_at } = self.signals {
            if kill_at <= now {
                self.send_kill(ServiceExitCause::Killed, out);
            }
        }
    }

    fn deadline(&self) -> Option<Instant> {
        let grace = match self.signals {
            Signals::Grace { kill_at } => Some(kill_at),
            _ => None,
        };
        self.startup.into_iter().chain(grace).min()
    }

    /// The exit, once the leader is drained and the tree is killed. A guardian signal that reached the live leader is
    /// the cause; otherwise the cooperative byte; otherwise how the leader ended (SV-5).
    fn settled(&self) -> Option<ServiceExit> {
        let (
            Exit::Drained(status),
            Signals::Killed {
                descendants_may_remain,
            },
        ) = (self.exit, self.signals)
        else {
            return None;
        };
        let (code, signal) = match status {
            ExitStatus::Code(code) => (Some(code), None),
            ExitStatus::Signal(signal) => (None, Some(signal)),
        };
        let observed = if signal.is_some() {
            ServiceExitCause::Signal
        } else {
            ServiceExitCause::Normal
        };
        let cause = self
            .signalled
            .or(self.cause_byte.map(ServiceExitCause::ChildReported))
            .unwrap_or(observed);
        Some(ServiceExit {
            code,
            signal,
            cause,
            descendants_may_remain,
        })
    }
}

impl Machine for Guardian {
    type Input = Input;
    type Action = Action;

    fn handle(&mut self, now: Instant, input: Input) {
        if self.ending == Some(Ending::Done) {
            return;
        }
        match input {
            Input::LinkBytes(bytes) => self.on_bytes(now, &bytes),
            Input::LinkClosed => self.on_closed(now),
            Input::LinkConnected => {
                // A dying guardian accepts no host; a live connection is never replaced.
                if self.link.state == LinkState::Closed && self.ending.is_none() {
                    self.link.reconnect();
                    let hello = self
                        .link
                        .hello(&self.cfg)
                        .expect("Guardian::new encoded this hello");
                    self.emit(hello);
                }
            }
            Input::LinkWritten { total } => {
                self.link.wrote(total);
                self.flush_log();
                self.try_finish();
            }
            Input::Spawned(result) => {
                if let Service::Spawning(spawn) = self.service {
                    self.on_spawned(now, spawn, result);
                }
            }
            Input::Log(bytes) => {
                self.log.push(&bytes);
                self.flush_log();
            }
            Input::PayloadExited(status) => {
                if let Service::Spawning(Spawn { exited, .. }) = &mut self.service {
                    exited.get_or_insert(status);
                }
                self.with_leader(|leader, out| leader.on_exit(status, out));
            }
            Input::Cause(byte) => self.with_leader(|leader, _| leader.on_cause(byte)),
            Input::LogsDrained => self.with_leader(|leader, _| leader.on_drained()),
            Input::Descendants(known) => {
                self.with_leader(|leader, out| leader.on_census(known, out))
            }
            Input::TermSent {
                delivered,
                leader_exiting,
            } => self
                .with_leader(|leader, out| leader.on_term(now, delivered && !leader_exiting, out)),
            Input::TreeKilled {
                leader_signalled,
                leader_exiting,
                descendants_may_remain,
            } => self.with_leader(|leader, _| {
                leader.on_tree_killed(leader_signalled && !leader_exiting, descendants_may_remain)
            }),
            Input::Reaped => {
                if let Service::Reaping = self.service {
                    self.service = Service::Gone;
                    self.try_finish();
                }
            }
            Input::Terminate => self.end_now(ServiceExitCause::Killed),
            Input::Timer => self.on_timer(now),
        }
    }

    fn poll_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }

    fn next_deadline(&self) -> Option<Instant> {
        let leader = match &self.service {
            Service::Live(leader) => leader.deadline(),
            _ => None,
        };
        self.orphan.into_iter().chain(leader).min()
    }
}
