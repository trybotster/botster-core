//! The `Worker` machine (plan 2.1, 2.2): the session worker as a sans-IO state machine.
//!
//! This milestone (plan 6.2, M1) has the control link (hello, token proof, host epoch), the payload launch, the ends of the
//! payload (`Stop`, `Kill`, `Signal`, the worker-control signal) and its exit with code and signal. The terminal model, input
//! and routes come with later milestones.
//!
//! The machine reads no clock, starts no thread and does no I/O. Its driver gives it the bytes that the control link
//! delivered, the result of a payload spawn, the PTY output and the payload's exit, and performs its [`Action`]s. The real
//! driver is the `botster-worker` binary; the testkit runs the same machine in-process (plan 2.1: one machine, two drivers).
//!
//! **The payload leader stays unreaped** until its group kill is complete (lead ruling on P1 finding F7): the driver reaps it
//! only on [`Action::ReapPayload`], so while the machine can still signal the group, its id cannot be reused.
//!
//! Clause: Core AD-6, Core AD-7, Core DP-8, Core EV-4, Core LC-5, Core LC-6, Core LC-7, Core A6-2.

use crate::{WORKER_FEATURES_BY_PROTOCOL, WORKER_PROTOCOL};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::ExitStatus;
use botster_core_edges::Machine;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{HostMsg, LaunchSpec, PayloadId, WorkerMsg};
use botster_core_link::proof::{token_proof, TOKEN_LEN};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

/// `SIGTERM`: the graceful request of LC-5.
pub const SIGTERM: i32 = 15;
/// `SIGKILL`: the kill of the payload's group after `stop_grace` (LC-5).
pub const SIGKILL: i32 = 9;

/// What a worker is told at its start (the command line and the token of `botster_core_link::launch`).
#[derive(Clone, PartialEq, Eq)]
pub struct WorkerConfig {
    pub instance: InstanceId,
    pub token: [u8; TOKEN_LEN],
    /// The host epoch that the worker obeys (DP-8).
    pub host_epoch: u64,
    /// The worker protocol number that the hello announces: [`WORKER_PROTOCOL`] in production. It is a construction input,
    /// so a harness can make a real worker announce another number (plan section 3, A6-2), never a test branch.
    pub protocol: u8,
}

impl std::fmt::Debug for WorkerConfig {
    /// The token is a credential: it is never printed.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkerConfig")
            .field("instance", &self.instance)
            .field("host_epoch", &self.host_epoch)
            .field("protocol", &self.protocol)
            .finish_non_exhaustive()
    }
}

impl WorkerConfig {
    /// A worker of the running protocol.
    pub fn new(instance: InstanceId, token: [u8; TOKEN_LEN], host_epoch: u64) -> WorkerConfig {
        WorkerConfig {
            instance,
            token,
            host_epoch,
            protocol: WORKER_PROTOCOL,
        }
    }
}

/// Why the payload did not start.
///
/// Clause: Core LC-4, Core A2-1 (`StartFailed{reason}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpawnFailure {
    /// The working directory does not exist.
    CwdMissing,
    /// The program could not be run.
    Exec { errno: i32 },
}

/// What to start on a new PTY: the payload of the session, with exactly this environment (A2-1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadSpec {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: String,
    pub size: Size,
}

/// An input of the worker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Input {
    /// Bytes that the control link delivered, in order.
    LinkBytes(Vec<u8>),
    /// The control link ended: the peer closed it, or it failed. Bytes that the driver still held for it are lost.
    LinkClosed,
    /// The driver has written `total` bytes of `LinkSend`s to the link, counted from the first one. A staged close and the
    /// worker's end wait until `total` covers every byte that the worker sent before them (LC-7: the result reaches the
    /// host before the worker ends), so a report about earlier bytes never releases a close over later ones.
    LinkWritten { total: u64 },
    /// The result of [`Action::SpawnPayload`].
    Spawned(Result<PayloadId, SpawnFailure>),
    /// Bytes that the payload wrote on the PTY.
    PtyOutput(Vec<u8>),
    /// The answer to [`Action::PtyWrite`]: the bytes that the PTY took (`Ok(0)`: none now, and [`Input::PtyWritable`]
    /// follows when it takes bytes again), or the OS error of the write.
    PtyWritten(Result<usize, i32>),
    /// The PTY takes input again after a write that it did not take.
    PtyWritable,
    /// A read of the PTY found no byte (it would block) or found the end of the output.
    PtyDrained,
    /// The payload's leader ended. It is not reaped: the driver reaps it only on [`Action::ReapPayload`].
    PayloadExited(ExitStatus),
    /// The worker-control signal (`GroupSignal::EndPayload`, `SIGUSR1`): end the payload without the control link (LC-5).
    EndPayload,
    /// `SIGTERM` to the worker: end the payload's group if the worker still holds its leader, reap it, and end. A worker that
    /// is told to end leaves no payload behind (plan R12: teardown is TERM, grace, KILL, reap). Nothing waits for the link.
    Terminate,
    /// A deadline of [`Machine::next_deadline`] is due.
    Timer,
}

/// An action of the worker, for its driver.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Write these bytes on the control link, after every byte of the earlier `LinkSend`s.
    LinkSend(Vec<u8>),
    /// Close the control link. Every earlier `LinkSend` is already on the link ([`Input::LinkWritten`]), except at
    /// [`Input::Terminate`]. The worker keeps running (DP-8).
    LinkClose,
    /// Start the payload on a new PTY as the leader of its own session and process group. The answer is [`Input::Spawned`].
    SpawnPayload(PayloadSpec),
    /// Read the output that the PTY holds now, then give [`Input::PtyDrained`]: the output that the payload wrote before
    /// its exit is read before the exit is reported (EV-4, ST-5: the final model holds it). The drain is bounded by what the
    /// PTY held when it was asked, so output that a remaining process of the group writes later cannot hold the exit back.
    DrainPty,
    /// Write these bytes to the payload's PTY, and answer with [`Input::PtyWritten`]. One write is out at a time (AM-2).
    PtyWrite(Vec<u8>),
    /// Send this signal to the payload's process group. It is emitted only while the leader is unreaped.
    SignalPayload(i32),
    /// Reap the payload's leader: its group kill is complete, so its id may be reused from now on.
    ReapPayload,
    /// End the worker process (LC-7 step 3 is done).
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkState {
    /// The worker's hello is sent; the host's hello has not come.
    AwaitHello,
    /// The host proved the token and the epoch: messages flow.
    Ready,
    /// The worker closes the link once its queued bytes are written. Nothing more is sent or obeyed.
    Closing,
    /// Closed by either end. A later host reaches the worker through adoption (P5).
    Closed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PayloadState {
    /// No launch was asked.
    None,
    /// [`Action::SpawnPayload`] is out.
    Spawning,
    /// The payload runs, or its leader ended and is not reaped yet.
    Live(PayloadId),
    /// The payload did not start.
    Failed,
    /// The leader is reaped. The group can no longer be signalled.
    Reaped,
}

/// The session worker.
#[derive(Debug)]
pub struct Worker {
    cfg: WorkerConfig,
    link: LinkState,
    decoder: FrameDecoder,
    /// The bound of the frames that this worker sends, and of the decoder after the launch (`LaunchSpec.link_frame_bound`).
    frame_bound: u32,
    payload: PayloadState,
    /// The exit of the leader, once seen.
    exit: Option<ExitStatus>,
    /// The exit waits for a drain of the PTY before it is reported.
    exit_drained: bool,
    exit_reported: bool,
    /// A `SIGTERM` went to the group.
    termed: bool,
    /// A `SIGKILL` went to the group: once the leader has ended, its group kill is complete.
    killed: bool,
    stop_grace: Duration,
    /// The size of the launch, for the state that `Launched` carries.
    launch_size: Option<Size>,
    /// The kill of the worker-control signal's grace (LC-5).
    grace: Option<Instant>,
    /// LC-7 step 3: the worker ends once the payload is reaped and the result is sent.
    removing: bool,
    /// The worker ends once its link is closed (LC-7 step 3 is done).
    exit_pending: bool,
    /// The bytes of every `LinkSend` so far, and the bytes that the driver reported written.
    queued_total: u64,
    written_total: u64,
    /// `Terminate` came: the worker ends as soon as no payload leader is held.
    terminating: bool,
    /// Inputs about the payload that came while its spawn was out (the drivers may deliver them before the spawn's answer):
    /// they apply once the spawn succeeds, and are dropped when it fails (no group to signal).
    early: Early,
    /// The admission point and the host's writes (AM-2, IN-1 to IN-10).
    input: input::InputState,
    actions: VecDeque<Action>,
}

/// What came for the payload while its spawn was out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct Early {
    exit: Option<ExitStatus>,
    stop: bool,
    kill: bool,
    end_payload: bool,
}

impl Worker {
    /// A worker whose control link is connected. Its first action sends the hello (AD-6, DP-8).
    pub fn new(cfg: WorkerConfig) -> Worker {
        let mut worker = Worker {
            decoder: FrameDecoder::new(DEFAULT_MAX_PAYLOAD),
            frame_bound: DEFAULT_MAX_PAYLOAD,
            cfg,
            link: LinkState::AwaitHello,
            payload: PayloadState::None,
            exit: None,
            exit_drained: false,
            exit_reported: false,
            termed: false,
            killed: false,
            stop_grace: CoreLimits::default().stop_grace,
            launch_size: None,
            grace: None,
            removing: false,
            exit_pending: false,
            queued_total: 0,
            written_total: 0,
            terminating: false,
            early: Early::default(),
            input: input::InputState::default(),
            actions: VecDeque::new(),
        };
        let hello = Hello {
            protocol: worker.cfg.protocol,
            instance: worker.cfg.instance.clone(),
            proof: worker.proof(),
            host_epoch: worker.cfg.host_epoch,
        };
        let mut payload = Vec::new();
        if hello.encode(&mut payload).is_ok() {
            worker.send_frame(FrameType::HELLO, &payload);
        } else {
            // An `InstanceId` that the hello cannot carry cannot be proven: no host will accept this worker.
            worker.close_link();
        }
        worker
    }

    fn proof(&self) -> botster_core_link::hello::TokenProof {
        token_proof(&self.cfg.token, &self.cfg.instance, self.cfg.host_epoch)
    }

    /// True while the payload's group can be signalled: it was launched and its leader is not reaped.
    fn group_live(&self) -> bool {
        matches!(self.payload, PayloadState::Live(_))
    }

    fn send_frame(&mut self, kind: FrameType, payload: &[u8]) {
        if matches!(self.link, LinkState::Closing | LinkState::Closed) {
            return;
        }
        let mut out = Vec::new();
        if encode_frame(kind, payload, self.frame_bound, &mut out).is_ok() {
            self.queued_total += u64::try_from(out.len()).unwrap_or(u64::MAX);
            self.actions.push_back(Action::LinkSend(out));
        }
    }

    /// Sends a report when the link is ready. Returns false when it is not, so the caller keeps what must be reported later.
    fn report(&mut self, msg: &WorkerMsg) -> bool {
        if self.link != LinkState::Ready {
            return false;
        }
        let mut payload = Vec::new();
        msg.encode(&mut payload);
        self.send_frame(FrameType::WORKER_MSG, &payload);
        true
    }

    /// Closes the link after the bytes already sent are written (LC-7: the result reaches the host first).
    fn close_link(&mut self) {
        if matches!(self.link, LinkState::AwaitHello | LinkState::Ready) {
            self.link = LinkState::Closing;
        }
        self.finish_close();
    }

    /// A staged close completes when nothing sent is left in the driver; a staged end follows the close.
    fn finish_close(&mut self) {
        if self.link == LinkState::Closing && self.written_total >= self.queued_total {
            self.link = LinkState::Closed;
            self.actions.push_back(Action::LinkClose);
        }
        if self.exit_pending && self.link == LinkState::Closed {
            self.exit_pending = false;
            self.actions.push_back(Action::Exit);
        }
    }

    fn signal(&mut self, signal: i32) {
        if self.group_live() {
            self.actions.push_back(Action::SignalPayload(signal));
            match signal {
                SIGTERM => self.termed = true,
                SIGKILL => {
                    self.killed = true;
                    self.grace = None;
                }
                _ => {}
            }
        }
    }

    // ---- the control link ----

    fn on_link_bytes(&mut self, now: Instant, mut bytes: &[u8]) {
        while !bytes.is_empty() && matches!(self.link, LinkState::AwaitHello | LinkState::Ready) {
            let took = self.decoder.push(bytes);
            bytes = &bytes[took..];
            loop {
                match self.decoder.next_frame() {
                    Ok(Some(frame)) => self.on_frame(now, frame.kind, &frame.payload),
                    Ok(None) => break,
                    Err(_) => {
                        // A frame above the bound: the stream cannot continue (plan section 3).
                        self.close_link();
                        return;
                    }
                }
                if !matches!(self.link, LinkState::AwaitHello | LinkState::Ready) {
                    return;
                }
            }
        }
    }

    fn on_frame(&mut self, now: Instant, kind: FrameType, payload: &[u8]) {
        match self.link {
            LinkState::AwaitHello => self.on_host_hello(kind, payload),
            LinkState::Ready if kind == FrameType::HOST_MSG => {
                // A message that this worker cannot decode is a later host's: it is ignored (plan section 3).
                if let Ok(msg) = HostMsg::decode(payload) {
                    self.on_host_msg(now, msg);
                }
            }
            LinkState::Ready | LinkState::Closing | LinkState::Closed => {}
        }
    }

    /// AD-6, DP-8: the host proves the same token for the same instance and epoch. Anything else closes the link.
    fn on_host_hello(&mut self, kind: FrameType, payload: &[u8]) {
        let proven = kind == FrameType::HELLO
            && Hello::decode(payload).is_ok_and(|hello| {
                hello.instance == self.cfg.instance
                    && hello.host_epoch == self.cfg.host_epoch
                    && hello.proof == self.proof()
            });
        if !proven {
            self.close_link();
            return;
        }
        self.link = LinkState::Ready;
        // An exit that happened while no host was linked is reported now (LC-5: "report the payload exit as for any exit").
        self.report_exit();
    }

    fn on_host_msg(&mut self, now: Instant, msg: HostMsg) {
        match msg {
            HostMsg::Launch(spec) => self.on_launch(*spec),
            HostMsg::Stop if self.payload == PayloadState::Spawning => self.early.stop = true,
            HostMsg::Stop => self.signal(SIGTERM),
            HostMsg::Kill if self.payload == PayloadState::Spawning => self.early.kill = true,
            HostMsg::Kill => {
                self.signal(SIGKILL);
                self.reap_when_complete();
            }
            HostMsg::Op { req, op } => self.on_op(now, req, op),
            HostMsg::Cancel { req } => self.on_cancel(req),
            HostMsg::Remove => self.on_remove(),
            // `AttachRoute` and `Detach` belong to the route milestone (P4a); a later variant of the enum is a later host's.
            _ => {}
        }
    }

    /// AD-7 step 4: the host sends the launch only after the worker's identity is durable, and the worker launches only on
    /// it. A second launch is ignored: a session has one payload.
    fn on_launch(&mut self, spec: LaunchSpec) {
        if self.payload != PayloadState::None {
            return;
        }
        self.frame_bound = spec.link_frame_bound;
        self.decoder = rebound(&self.decoder, spec.link_frame_bound);
        self.stop_grace = Duration::from_millis(spec.stop_grace_ms);
        self.payload = PayloadState::Spawning;
        self.launch_size = Some(spec.size);
        self.actions.push_back(Action::SpawnPayload(PayloadSpec {
            argv: spec.argv,
            env: spec.env,
            cwd: spec.cwd,
            size: spec.size,
        }));
    }

    fn on_spawned(&mut self, now: Instant, result: Result<PayloadId, SpawnFailure>) {
        if self.payload != PayloadState::Spawning {
            return;
        }
        let early = std::mem::take(&mut self.early);
        match result {
            Ok(id) => {
                self.payload = PayloadState::Live(id);
                let msg = WorkerMsg::Launched {
                    features: worker_features(),
                    terminal: self.initial_terminal(),
                    formats: Vec::new(),
                    payload: id,
                };
                self.report(&msg);
                // What came while the spawn was out applies now: a kill (a `Remove` or `Kill`) first, else the graceful
                // request; the grace of `EndPayload`; then the exit that the edge already saw.
                if self.removing || early.kill {
                    self.signal(SIGKILL);
                } else if early.stop {
                    self.signal(SIGTERM);
                }
                if early.end_payload {
                    self.on_end_payload(now);
                }
                if let Some(status) = early.exit {
                    self.on_exited(status);
                }
                self.try_start();
            }
            Err(failure) => {
                self.payload = PayloadState::Failed;
                let reason = match failure {
                    SpawnFailure::CwdMissing => StartFailReason::CwdMissing,
                    SpawnFailure::Exec { errno } => StartFailReason::ExecFailed { errno },
                };
                self.report(&WorkerMsg::LaunchFailed { reason });
                self.try_start();
                self.finish_remove();
                self.finish_terminate();
            }
        }
    }

    /// PLACEHOLDER until the terminal model (P3 M2): the state that `terminal_state` caches at the launch. It carries the
    /// launch size and no tracked mode, title or cwd. M2 replaces it with the state of the libghostty model (BUILD.md: the
    /// terminal semantics are libghostty's), and no terminal-state id leaves `core-pending.txt` before that.
    fn initial_terminal(&self) -> TerminalState {
        TerminalState {
            size: self.launch_size.unwrap_or(Size {
                rows: 0,
                cols: 0,
                cell_px: None,
            }),
            modes: ModeFlags::default(),
            title: None,
            cwd: None,
            last_output_at: None,
            focused: None,
            model_rev: ModelRev(self.input.model_rev),
            input_rev: self.input.input_revs(),
        }
    }

    /// The operations that need the worker: `Signal` (LC-6) and `WriteInput` (IN-1). The reads and the setters come with the
    /// terminal model, and the worker answers them `Internal` until then.
    fn on_op(&mut self, _now: Instant, req: u64, op: Op) {
        let result = match op {
            Op::WriteInput { payload, guard, .. } => {
                // Its `Done` comes when its transaction ends (IN-3).
                self.on_write_input(req, payload, guard);
                return;
            }
            Op::Signal { sig, .. } => {
                if let (true, Some(number)) = (self.group_live(), signal_number(sig)) {
                    self.signal(number);
                    if number == SIGKILL {
                        self.reap_when_complete();
                    }
                    OpResult::Ok(OpOutput::Unit)
                } else if signal_number(sig).is_none() {
                    // A later variant of the non-exhaustive enum that this worker does not know (LC-6).
                    OpResult::Err(CoreError::new(
                        ErrorCode::Unsupported { what: None },
                        "this worker does not know the signal",
                    ))
                } else {
                    OpResult::Err(CoreError::new(
                        ErrorCode::SessionEnded,
                        "the payload has no live process group",
                    ))
                }
            }
            other => OpResult::Err(CoreError::new(
                ErrorCode::Internal,
                format!("this worker does not serve {} yet", op_name(&other)),
            )),
        };
        self.report(&WorkerMsg::Done { req, result });
    }

    /// LC-7 step 3: end the payload, reap it, send the complete result, and end. This milestone has no uploads (DP-5b, P4b),
    /// so the result is `Deleted` (A6-3: every file is gone, or there were none).
    fn on_remove(&mut self) {
        self.removing = true;
        match self.payload {
            PayloadState::Live(_) => {
                if !self.killed {
                    self.signal(SIGKILL);
                }
                self.reap_when_complete();
            }
            // The spawn answer comes first; `on_spawned` continues the removal.
            PayloadState::Spawning => {}
            PayloadState::None | PayloadState::Failed | PayloadState::Reaped => {
                self.finish_remove()
            }
        }
    }

    fn finish_remove(&mut self) {
        if !self.removing || matches!(self.payload, PayloadState::Live(_) | PayloadState::Spawning)
        {
            return;
        }
        self.report(&WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted,
        });
        self.removing = false;
        self.exit_pending = true;
        self.close_link();
    }

    // ---- the payload ----

    fn on_exited(&mut self, status: ExitStatus) {
        if self.payload == PayloadState::Spawning {
            self.early.exit = Some(status);
            return;
        }
        if !self.group_live() || self.exit.is_some() {
            return;
        }
        self.exit = Some(status);
        self.actions.push_back(Action::DrainPty);
        self.input_payload_ended();
    }

    fn on_drained(&mut self) {
        if self.exit.is_some() && !self.exit_drained {
            self.exit_drained = true;
            self.report_exit();
            self.reap_when_complete();
        }
    }

    /// EV-4: the exit carries the code or the signal. It is reported once, after the output written before it was read.
    fn report_exit(&mut self) {
        let Some(status) = self.exit else {
            return;
        };
        if self.exit_reported || !self.exit_drained {
            return;
        }
        let (code, signal) = match status {
            ExitStatus::Code(code) => (Some(code), None),
            ExitStatus::Signal(signal) => (None, Some(signal)),
        };
        self.exit_reported = self.report(&WorkerMsg::Exited { code, signal });
    }

    /// The leader is reaped only when its group kill is complete: a `SIGKILL` went to the group and the leader has ended
    /// (lead ruling on P1 F7). Then the removal can finish.
    fn reap_when_complete(&mut self) {
        if self.group_live() && self.killed && self.exit_drained {
            self.payload = PayloadState::Reaped;
            self.actions.push_back(Action::ReapPayload);
            self.finish_remove();
            self.finish_terminate();
        }
    }

    /// `SIGTERM`: the payload's group is killed only while the worker holds its leader (a reaped id is never signalled),
    /// and the worker ends once no leader is held.
    fn on_terminate(&mut self) {
        self.terminating = true;
        match self.payload {
            PayloadState::Live(_) => {
                if !self.killed {
                    self.signal(SIGKILL);
                }
                self.reap_when_complete();
            }
            PayloadState::Spawning => self.early.kill = true,
            PayloadState::None | PayloadState::Failed | PayloadState::Reaped => {
                self.finish_terminate()
            }
        }
    }

    /// The end at `Terminate` does not wait for the link: a host that does not read never keeps a terminated worker alive.
    fn finish_terminate(&mut self) {
        if !self.terminating
            || matches!(self.payload, PayloadState::Live(_) | PayloadState::Spawning)
        {
            return;
        }
        self.terminating = false;
        if self.link != LinkState::Closed {
            self.link = LinkState::Closed;
            self.actions.push_back(Action::LinkClose);
        }
        self.exit_pending = false;
        self.actions.push_back(Action::Exit);
    }

    /// LC-5 without the link: the graceful request, then the group kill after `stop_grace`. A repeated signal changes nothing.
    fn on_end_payload(&mut self, now: Instant) {
        if self.payload == PayloadState::Spawning {
            self.early.end_payload = true;
            return;
        }
        if !self.group_live() || self.killed {
            return;
        }
        if self.exit.is_some() {
            // The leader already ended: only the rest of its group can remain, and nothing waits for a graceful end.
            self.signal(SIGKILL);
            self.reap_when_complete();
            return;
        }
        if !self.termed {
            self.signal(SIGTERM);
        }
        if self.grace.is_none() {
            self.grace = Some(now + self.stop_grace);
        }
    }

    fn on_timer(&mut self, now: Instant) {
        if self.grace.is_some_and(|at| at <= now) {
            self.grace = None;
            self.signal(SIGKILL);
            self.reap_when_complete();
        }
    }
}

impl Machine for Worker {
    type Input = Input;
    type Action = Action;

    fn handle(&mut self, now: Instant, input: Input) {
        match input {
            Input::LinkBytes(bytes) => self.on_link_bytes(now, &bytes),
            Input::LinkClosed => {
                // DP-8: the worker keeps its payload and its model when the host is gone. Unwritten bytes are lost with
                // the link, so a staged close is complete, and a staged end follows.
                self.link = LinkState::Closed;
                self.finish_close();
            }
            Input::LinkWritten { total } => {
                self.written_total = self.written_total.max(total);
                self.finish_close();
            }
            Input::Spawned(result) => self.on_spawned(now, result),
            // The terminal model takes the output in M2. Until then the worker reads it, so the payload never blocks on a
            // full PTY.
            // Output changes the read-visible state (ST-1); the terminal model takes the bytes when it comes.
            Input::PtyOutput(_) => self.input.model_rev += 1,
            Input::PtyWritten(result) => self.on_pty_written(result),
            Input::PtyWritable => self.on_pty_writable(),
            Input::PtyDrained => self.on_drained(),
            Input::PayloadExited(status) => self.on_exited(status),
            Input::EndPayload => self.on_end_payload(now),
            Input::Terminate => self.on_terminate(),
            Input::Timer => self.on_timer(now),
        }
    }

    fn poll_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.grace
    }
}

/// A decoder with a new bound. The link carries whole frames, so the old decoder holds no partial frame between two
/// `HostMsg`s; a partial one is kept as it is.
fn rebound(old: &FrameDecoder, bound: u32) -> FrameDecoder {
    if old.buffered() == 0 {
        FrameDecoder::new(bound)
    } else {
        old.clone()
    }
}

/// The per-worker features of the running protocol (A2-6, A6-2).
fn worker_features() -> BTreeSet<Feature> {
    WORKER_FEATURES_BY_PROTOCOL
        .iter()
        .find(|(protocol, _)| *protocol == WORKER_PROTOCOL)
        .map(|(_, features)| features.iter().cloned().collect())
        .unwrap_or_default()
}

/// The POSIX number of a signal of `Signal` (LC-6). The host refuses an unsupported value at `begin`.
fn signal_number(sig: Signal) -> Option<i32> {
    match sig {
        Signal::Term => Some(SIGTERM),
        Signal::Kill => Some(SIGKILL),
        Signal::Int => Some(2),
        Signal::Hup => Some(1),
        Signal::Other(n) => Some(n),
        _ => None,
    }
}

/// The name of an operation for a diagnostic: its JSON tag.
fn op_name(op: &Op) -> String {
    serde_json::to_value(op)
        .ok()
        .and_then(|v| v.as_object().and_then(|o| o.keys().next().cloned()))
        .unwrap_or_else(|| "this operation".to_string())
}

mod input;

#[cfg(test)]
mod tests;
