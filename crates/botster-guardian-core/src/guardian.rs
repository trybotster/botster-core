//! Service lifetime and cleanup (Core SV-5, SV-7, SV-8, SV-9; plan 2.2).

use crate::wire::{Command, Report, ServiceSpec, Status};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{ExitStatus, ProcessIdentity};
use botster_core_edges::Machine;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::hello::Hello;
use botster_core_link::msg::PayloadId;
use botster_core_link::proof::{token_proof, TOKEN_LEN};
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
    pub orphan_grace: Duration,
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
    Log(Vec<u8>),
    /// The first byte on the cooperative cause descriptor (SV-5).
    Cause(u8),
    /// The leader exited. The edge must leave it unreaped.
    PayloadExited(ExitStatus),
    /// The driver read the output and cause bytes present at the exit.
    LogsDrained,
    /// The direct SIGTERM result, observed before the driver checks the exit.
    TermSent {
        delivered: bool,
        leader_exiting: bool,
    },
    /// The census before SIGTERM. The driver verifies each identity before a kill.
    Descendants(Vec<ProcessIdentity>),
    /// The driver completed `KillTree`, including a census after the kill.
    TreeKilled {
        leader_signalled: bool,
        leader_exiting: bool,
        descendants_may_remain: bool,
    },
    /// The driver reaped only the leader that the machine owns.
    Reaped,
    /// Process shutdown. Cleanup continues without waiting for the host.
    Terminate,
    Timer,
}

/// Actions for either driver. The driver must preserve their order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    LinkSend(Vec<u8>),
    LinkClose,
    SpawnService(Box<ServiceSpec>),
    /// Enumerate descendants before the leader can exit on SIGTERM (SV-9).
    Enumerate {
        leader: PayloadId,
    },
    /// Send SIGTERM to the service, not to the guardian or its host.
    TermService {
        leader: PayloadId,
    },
    /// Kill the unreaped leader's group and verified descendants (SV-9).
    /// The driver also enumerates current descendants and checks for survivors.
    KillTree {
        leader: PayloadId,
        known: Vec<ProcessIdentity>,
    },
    /// Read the bytes present now. A descendant cannot prolong this drain.
    DrainLogs,
    ReapService {
        leader: PayloadId,
    },
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkState {
    AwaitHello,
    Ready,
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadState {
    Empty,
    Spawning,
    Live(PayloadId),
    Reaping,
    Ended,
    Failed,
}

/// The guardian. Only the driver owns descriptors, process watches, and the clock.
#[derive(Debug)]
pub struct Guardian {
    cfg: GuardianConfig,
    link: LinkState,
    decoder: FrameDecoder,
    payload: PayloadState,
    report: Option<SpawnReport>,
    payload_id: Option<PayloadId>,
    status: Option<ExitStatus>,
    exit: Option<ServiceExit>,
    cause_byte: Option<u8>,
    stop_reason: Option<ServiceExitCause>,
    delivered_reason: Option<ServiceExitCause>,
    term_pending: bool,
    census_pending: bool,
    tree_pending: bool,
    kill_requested: bool,
    term_started: bool,
    tree_done: bool,
    drained: bool,
    descendants_may_remain: bool,
    known: Vec<ProcessIdentity>,
    startup: Option<Instant>,
    startup_duration: Duration,
    orphan: Option<Instant>,
    stop: Option<Instant>,
    stop_grace: Duration,
    removing: bool,
    terminating: bool,
    orphan_ending: bool,
    removed_sent: bool,
    finished: bool,
    queued_total: u64,
    written_total: u64,
    log: VecDeque<u8>,
    actions: VecDeque<Action>,
}

impl Guardian {
    /// Start with a connected, unauthenticated link. Only authentication cancels orphan grace (AD-6, SV-8).
    pub fn new(cfg: GuardianConfig, now: Instant) -> Self {
        let orphan = Some(now + cfg.orphan_grace);
        let mut guardian = Self {
            cfg,
            link: LinkState::AwaitHello,
            decoder: FrameDecoder::new(DEFAULT_MAX_PAYLOAD),
            payload: PayloadState::Empty,
            payload_id: None,
            report: None,
            status: None,
            exit: None,
            cause_byte: None,
            stop_reason: None,
            delivered_reason: None,
            term_pending: false,
            census_pending: false,
            tree_pending: false,
            kill_requested: false,
            term_started: false,
            tree_done: false,
            drained: false,
            descendants_may_remain: false,
            known: Vec::new(),
            startup: None,
            startup_duration: Duration::ZERO,
            orphan,
            stop: None,
            stop_grace: Duration::ZERO,
            removing: false,
            terminating: false,
            orphan_ending: false,
            removed_sent: false,
            finished: false,
            queued_total: 0,
            written_total: 0,
            log: VecDeque::new(),
            actions: VecDeque::new(),
        };
        guardian.send_hello();
        guardian
    }

    /// The retained state survives host loss. The guardian never restarts a service (SV-7).
    pub fn status(&self) -> Status {
        Status {
            service: self.cfg.service,
            payload: self.payload_id,
            report: self.report.clone(),
            exit: self.exit,
        }
    }

    /// An owned tail of captured stdout and stderr (SV-9).
    pub fn log_tail(&self, max: usize) -> Vec<u8> {
        self.log
            .iter()
            .skip(self.log.len().saturating_sub(max))
            .copied()
            .collect()
    }

    /// The driver calls this with its injected clock when a deadline is due (TM-1, TM-3).
    pub fn pump(&mut self, now: Instant) {
        if self.finished {
            return;
        }
        self.on_timer(now);
    }

    fn send(&mut self, kind: FrameType, payload: &[u8]) {
        if self.link == LinkState::Closed {
            return;
        }
        let mut bytes = Vec::new();
        if encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut bytes).is_err() {
            self.close_link();
            return;
        }
        self.queued_total += bytes.len() as u64;
        self.actions.push_back(Action::LinkSend(bytes));
    }

    fn send_hello(&mut self) {
        let hello = Hello {
            protocol: self.cfg.protocol,
            instance: self.cfg.instance.clone(),
            proof: token_proof(&self.cfg.token, &self.cfg.instance, self.cfg.host_epoch),
            host_epoch: self.cfg.host_epoch,
        };
        let mut bytes = Vec::new();
        if hello.encode(&mut bytes).is_ok() {
            self.send(FrameType::HELLO, &bytes);
        } else {
            self.close_link();
        }
    }

    fn report(&mut self, report: Report) {
        if self.link != LinkState::Ready {
            return;
        }
        let bytes = serde_json::to_vec(&report).expect("guardian reports serialize");
        self.send(FrameType::WORKER_MSG, &bytes);
    }

    fn close_link(&mut self) {
        if self.link != LinkState::Closed {
            self.link = LinkState::Closed;
            self.actions.push_back(Action::LinkClose);
        }
    }

    fn on_bytes(&mut self, now: Instant, mut bytes: &[u8]) {
        while !bytes.is_empty() && self.link != LinkState::Closed {
            let took = self.decoder.push(bytes);
            bytes = &bytes[took..];
            match self.decoder.next_frame() {
                Ok(Some(frame)) if self.link == LinkState::AwaitHello => {
                    let hello = Hello::decode(&frame.payload).ok();
                    let valid = frame.kind == FrameType::HELLO
                        && hello.as_ref().is_some_and(|hello| {
                            hello.protocol == self.cfg.protocol
                                && hello.instance == self.cfg.instance
                                && hello.host_epoch >= self.cfg.host_epoch
                                && hello.proof
                                    == token_proof(
                                        &self.cfg.token,
                                        &self.cfg.instance,
                                        hello.host_epoch,
                                    )
                        });
                    if valid {
                        self.cfg.host_epoch = hello.expect("validated hello").host_epoch;
                        self.link = LinkState::Ready;
                        self.orphan = None;
                        self.report(Report::Status(self.status()));
                    } else {
                        self.on_closed(now);
                    }
                }
                Ok(Some(frame)) if frame.kind == FrameType::HOST_MSG => {
                    if let Ok(command) = Command::decode(&frame.payload) {
                        self.begin(now, command);
                    }
                }
                Ok(_) => {}
                Err(_) => self.on_closed(now),
            }
        }
    }

    fn on_closed(&mut self, now: Instant) {
        self.close_link();
        self.decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
        // Rejected connections do not extend an existing orphan deadline (SV-8).
        if self.orphan.is_none() {
            self.orphan = Some(now + self.cfg.orphan_grace);
        }
        self.finish();
    }

    /// Run an authenticated command. The wire driver uses this same entry point.
    pub fn begin(&mut self, now: Instant, command: Command) {
        if self.finished
            || self.link != LinkState::Ready
            || self.removing
            || self.terminating
            || self.orphan_ending
        {
            return;
        }
        match command {
            Command::Launch(spec) if self.payload == PayloadState::Empty => {
                self.stop_grace = spec.limits.stop_grace;
                self.startup_duration = spec.limits.startup;
                self.cfg.orphan_grace = spec.limits.orphan_grace;
                self.payload = PayloadState::Spawning;
                self.actions.push_back(Action::SpawnService(spec));
            }
            Command::EpochCommitted => self.startup = None,
            Command::Stop => self.start_stop(now, ServiceExitCause::HostStop),
            Command::Remove => {
                self.removing = true;
                self.startup = None;
                self.orphan = None;
                self.stop = None;
                self.kill_tree();
                self.finish();
            }
            Command::Status => self.report(Report::Status(self.status())),
            Command::LogTail { req, max } => {
                let bytes = self.log_tail(usize::try_from(max).unwrap_or(usize::MAX));
                // JSON represents one byte with at most four bytes, including its separator.
                // Reserve 128 bytes for the tag, request number, and final-chunk flag.
                let chunk = (DEFAULT_MAX_PAYLOAD as usize - 128) / 4;
                if bytes.is_empty() {
                    self.report(Report::Log {
                        req,
                        bytes,
                        last: true,
                    });
                } else {
                    let count = bytes.len().div_ceil(chunk);
                    for (index, bytes) in bytes.chunks(chunk).enumerate() {
                        self.report(Report::Log {
                            req,
                            bytes: bytes.to_vec(),
                            last: index + 1 == count,
                        });
                    }
                }
            }
            Command::Launch(_) => {}
        }
    }

    fn on_spawned(&mut self, now: Instant, result: SpawnResult) {
        if self.payload != PayloadState::Spawning {
            return;
        }
        match result {
            SpawnResult::Started { payload, report } => {
                self.payload = PayloadState::Live(payload);
                self.payload_id = Some(payload);
                self.report = Some(report.clone());
                self.report(Report::Started { payload, report });
                if self.removing || self.terminating || self.orphan_ending || self.kill_requested {
                    self.kill_tree();
                } else if let Some(reason) = self.stop_reason {
                    self.start_stop(now, reason);
                } else if self.status.is_none() {
                    self.startup = Some(now + self.startup_duration);
                }
                if self.status.is_some() {
                    self.on_exit();
                }
            }
            SpawnResult::Failed(reason) => {
                self.payload = PayloadState::Failed;
                self.startup = None;
                self.stop = None;
                self.report(Report::StartFailed { reason });
                self.finish();
            }
            SpawnResult::BoundUnavailable(bound) => {
                self.payload = PayloadState::Failed;
                self.startup = None;
                self.stop = None;
                self.report(Report::BoundUnavailable { bound });
                self.finish();
            }
        }
    }

    fn start_stop(&mut self, now: Instant, reason: ServiceExitCause) {
        if !matches!(self.payload, PayloadState::Spawning | PayloadState::Live(_))
            || self.exit.is_some()
            || self.tree_done
            || self.kill_requested
        {
            return;
        }
        if self.stop_reason.is_none() {
            self.stop_reason = Some(reason);
        }
        self.startup = None;
        if self.stop.is_none() {
            self.stop = Some(now + self.stop_grace);
        }
        if let PayloadState::Live(leader) = self.payload {
            if !self.term_started {
                self.term_started = true;
                self.census_pending = true;
                self.actions.push_back(Action::Enumerate { leader });
            }
        }
    }

    fn kill_tree(&mut self) {
        self.kill_requested = true;
        self.stop = None;
        self.startup = None;
        if self.tree_pending || self.tree_done || self.census_pending {
            return;
        }
        if let PayloadState::Live(leader) = self.payload {
            self.tree_pending = true;
            self.actions.push_back(Action::KillTree {
                leader,
                known: self.known.clone(),
            });
        }
    }

    fn on_exit(&mut self) {
        if !matches!(self.payload, PayloadState::Live(_)) {
            return;
        }
        self.startup = None;
        self.stop = None;
        self.actions.push_back(Action::DrainLogs);
        self.kill_tree();
    }

    fn settle_exit(&mut self) {
        if self.exit.is_some()
            || !self.drained
            || !self.tree_done
            || self.term_pending
            || self.census_pending
        {
            return;
        }
        let (Some(status), PayloadState::Live(leader)) = (self.status, self.payload) else {
            return;
        };
        let (code, signal) = match status {
            ExitStatus::Code(code) => (Some(code), None),
            ExitStatus::Signal(signal) => (None, Some(signal)),
        };
        let cause = self
            .delivered_reason
            .or_else(|| self.cause_byte.map(ServiceExitCause::ChildReported))
            .unwrap_or(if signal.is_some() {
                ServiceExitCause::Signal
            } else {
                ServiceExitCause::Normal
            });
        let exit = ServiceExit {
            code,
            signal,
            cause,
            descendants_may_remain: self.descendants_may_remain,
        };
        self.exit = Some(exit);
        self.report(Report::Exited { exit });
        self.payload = PayloadState::Reaping;
        self.actions.push_back(Action::ReapService { leader });
    }

    fn finish(&mut self) {
        if self.finished
            || matches!(
                self.payload,
                PayloadState::Spawning | PayloadState::Live(_) | PayloadState::Reaping
            )
        {
            return;
        }
        if self.terminating || self.orphan_ending {
            self.terminating = false;
            self.orphan_ending = false;
            self.orphan = None;
            self.close_link();
            self.finished = true;
            self.actions.push_back(Action::Exit);
        } else if self.removing {
            if !self.removed_sent {
                self.report(Report::Removed);
                self.removed_sent = true;
            }
            if self.link == LinkState::Closed || self.written_total >= self.queued_total {
                self.removing = false;
                self.close_link();
                self.finished = true;
                self.actions.push_back(Action::Exit);
            }
        }
    }

    fn on_timer(&mut self, now: Instant) {
        if self.orphan.is_some_and(|at| at <= now) {
            self.orphan = None;
            self.orphan_ending = true;
            self.startup = None;
            self.stop = None;
            self.stop_reason = Some(ServiceExitCause::OrphanGrace);
            self.kill_tree();
            self.finish();
        }
        if self.startup.is_some_and(|at| at <= now) {
            self.startup = None;
            self.stop_reason = Some(ServiceExitCause::StartupTimeout);
            self.kill_tree();
        }
        if self.stop.is_some_and(|at| at <= now) {
            self.kill_tree();
        }
    }
}

impl Machine for Guardian {
    type Input = Input;
    type Action = Action;

    fn handle(&mut self, now: Instant, input: Input) {
        if self.finished {
            return;
        }
        match input {
            Input::LinkBytes(bytes) => self.on_bytes(now, &bytes),
            Input::LinkClosed => self.on_closed(now),
            Input::LinkConnected if self.link == LinkState::Closed && !self.orphan_ending => {
                self.link = LinkState::AwaitHello;
                self.decoder = FrameDecoder::new(DEFAULT_MAX_PAYLOAD);
                self.queued_total = 0;
                self.written_total = 0;
                self.send_hello();
            }
            Input::LinkConnected => {}
            Input::LinkWritten { total } => {
                self.written_total = self.written_total.max(total);
                self.finish();
            }
            Input::Spawned(result) => self.on_spawned(now, result),
            Input::Log(bytes) => {
                let tail = &bytes[bytes.len().saturating_sub(self.cfg.log_bytes)..];
                let keep = self.cfg.log_bytes.saturating_sub(tail.len());
                self.log.drain(..self.log.len().saturating_sub(keep));
                self.log.extend(tail);
            }
            Input::Cause(byte) if self.cause_byte.is_none() && self.exit.is_none() => {
                self.cause_byte = Some(byte)
            }
            Input::Cause(_) => {}
            Input::PayloadExited(status)
                if self.status.is_none()
                    && matches!(self.payload, PayloadState::Spawning | PayloadState::Live(_)) =>
            {
                self.status = Some(status);
                self.on_exit();
            }
            Input::PayloadExited(_) => {}
            Input::LogsDrained
                if self.status.is_some() && matches!(self.payload, PayloadState::Live(_)) =>
            {
                self.drained = true;
                self.settle_exit();
            }
            Input::LogsDrained => {}
            Input::Descendants(known) if self.census_pending => {
                self.census_pending = false;
                self.known = known;
                if let PayloadState::Live(leader) = self.payload {
                    if self.kill_requested {
                        self.kill_tree();
                    } else if self.status.is_none() {
                        self.term_pending = true;
                        self.actions.push_back(Action::TermService { leader });
                    }
                }
                self.settle_exit();
            }
            Input::Descendants(_) => {}
            Input::TermSent {
                delivered,
                leader_exiting,
            } if self.term_pending => {
                self.term_pending = false;
                if delivered && !leader_exiting && self.delivered_reason.is_none() {
                    self.delivered_reason = self.stop_reason;
                }
                self.settle_exit();
            }
            Input::TermSent { .. } => {}
            Input::TreeKilled {
                leader_signalled,
                leader_exiting,
                descendants_may_remain,
            } if self.tree_pending => {
                self.tree_pending = false;
                self.tree_done = true;
                self.descendants_may_remain = descendants_may_remain;
                if leader_signalled && !leader_exiting {
                    self.delivered_reason = Some(match self.stop_reason {
                        Some(ServiceExitCause::OrphanGrace) => ServiceExitCause::OrphanGrace,
                        Some(ServiceExitCause::StartupTimeout) => ServiceExitCause::StartupTimeout,
                        _ => ServiceExitCause::Killed,
                    });
                }
                self.settle_exit();
            }
            Input::TreeKilled { .. } => {}
            Input::Reaped if self.payload == PayloadState::Reaping => {
                self.payload = PayloadState::Ended;
                self.finish();
            }
            Input::Reaped => {}
            Input::Terminate => {
                self.terminating = true;
                self.startup = None;
                self.orphan = None;
                self.stop = None;
                self.kill_tree();
                self.finish();
            }
            Input::Timer => self.pump(now),
        }
    }

    fn poll_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }

    fn next_deadline(&self) -> Option<Instant> {
        [self.startup, self.orphan, self.stop]
            .into_iter()
            .flatten()
            .min()
    }
}
