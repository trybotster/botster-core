//! The driver of the `HostEngine` (plan 2.1, 2.5): it connects the machine to the edges and implements `CoreApi`.
//!
//! There is one driver. The real `Core` and the testkit's core both use it, each with its own [`HostEdges`], so the logic
//! that moves bytes between the links and the engine, orders the work of a `pump` with the scheduler, and keeps the wake
//! flag exists once (plan 2.1, Core A5-1: "one code path").
//!
//! The driver holds no clock and draws no random number: the host passes `now` to `pump` (TM-1), and the edges draw (2.3a).
//! It calls no operating system function: every effect is a call of an edge.

use crate::engine::{EngineConfig, HostEngine};
use crate::io::{Action, Input, LinkId, Work};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, ProcessIdentity, SpawnError, StorageError, Wake as WakeEdge,
};
use botster_core_edges::scheduler::{ChoicePoint, Scheduler};
use botster_core_edges::Machine;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{HostMsg, WorkerMsg};
use botster_core_link::proof::TOKEN_LEN;
use botster_route_codec::prelude::QueryKind;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What a worker needs to find the host and prove itself (AD-6, DP-8). The edge turns it into a command line and an
/// environment; the token never goes on a command line that other users can read.
#[derive(Debug, Clone)]
pub struct WorkerSpawn {
    pub program: PathBuf,
    pub instance: InstanceId,
    pub token: [u8; TOKEN_LEN],
    pub host_epoch: u64,
}

/// The handoff of a route's stream failed (DP-2): the route closes `HandoffFailed`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HandoffError;

/// The wake object: the `WakeHandle` that the host waits on, and the "runnable work exists" flag that the driver sets and
/// clears (TM-6, TH-2).
pub trait HostWake: WakeHandle + WakeEdge {}

impl<T: WakeHandle + WakeEdge> HostWake for T {}

/// Every edge that the host driver uses (plan 2.3), as one trait with a real implementation in `botster-core` and a testkit
/// implementation in `botster-core-testkit`. A call that cannot proceed returns `io::ErrorKind::WouldBlock`.
pub trait HostEdges: Send {
    /// The registry (`Storage`) and the random values (`Entropy`).
    fn fill_random(&mut self, buf: &mut [u8]);
    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError>;
    fn delete_row(&mut self, key: &str) -> Result<(), StorageError>;
    /// Every row whose key starts with `prefix`, in key order.
    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError>;

    /// Workers as processes (`Process`).
    fn spawn_worker(&mut self, spec: &WorkerSpawn) -> Result<ProcessIdentity, SpawnError>;
    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal);
    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)>;

    /// The control links (`Link`). A worker connects to the host; `accept_link` returns the next new link, or `None`.
    fn accept_link(&mut self) -> Option<LinkId>;
    /// `Ok(0)` means that the peer closed the link.
    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize>;
    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize>;
    fn link_close(&mut self, link: LinkId);
    /// Write interest follows the outbound buffer (plan 2.5): on while bytes wait, off when none do.
    fn set_write_interest(&mut self, link: LinkId, on: bool);
    /// Read interest follows what the engine can take (plan 2.5 rule 7): off while a frame is held, on again when the engine
    /// can take it.
    fn set_read_interest(&mut self, link: LinkId, on: bool);
    /// Hands the stream of a route to the worker over its link (DP-2).
    fn handoff_route(
        &mut self,
        link: LinkId,
        route: RouteId,
        transport: StreamEndpoint,
        options: &AttachOptions,
    ) -> Result<(), HandoffError>;

    /// The wake object (`Wake`). The driver signals and drains it, and hands it out as the `WakeHandle`.
    fn wake(&self) -> Arc<dyn HostWake>;
    /// Consumes the readiness that the wake object holds. The driver calls it when a `pump` found nothing to do, and reads
    /// every link once more after it, so that a readiness that arrives after the read is never consumed unread (plan 2.5).
    fn settle_wake(&mut self);
    /// The scheduling policy (`Scheduler`): the production policy, or the seeded policy of the testkit.
    fn scheduler(&mut self) -> &mut dyn Scheduler;
}

/// The errors of `open` that do not need the data directory: the limits (9B, LC-1) and the worker path (LC-1).
///
/// Clause: Core LC-1, Core 9B.
pub fn check_open(config: &OpenConfig) -> Result<PathBuf, CoreError> {
    config.limits.validate().map_err(CoreError::from)?;
    config.worker_path.clone().ok_or_else(|| {
        CoreError::new(
            ErrorCode::MissingWorkerPath,
            "OpenConfig.worker_path is not set",
        )
    })
}

/// The first byte count that a link read asks for.
const READ_CHUNK: usize = 16 * 1024;

#[derive(Debug)]
struct LinkState {
    decoder: FrameDecoder,
    /// Bytes that the link did not take yet (plan 2.5: write interest while this is not empty).
    out: Vec<u8>,
    /// The first frame of a link is the hello; after it, messages.
    hello_seen: bool,
    /// Bytes that were read and not decoded yet: at most one read chunk.
    pending: Vec<u8>,
    /// A decoded input that the engine cannot take now (EV-5b): the link is not read until it can.
    held: Option<Input>,
}

/// The host driver. It is `Send`, and `Core` is `Send` and not `Sync` (TH-1) because the owner thread alone calls it.
pub struct HostDriver<E: HostEdges> {
    engine: HostEngine,
    edges: E,
    links: BTreeMap<LinkId, LinkState>,
    wake: Arc<dyn HostWake>,
    frame_bound: u32,
    /// Routes whose handoff failed: each one is fed to the engine in a step of its own, within the budget of a pump (9B).
    failed_handoffs: std::collections::VecDeque<RouteId>,
}

impl<E: HostEdges> HostDriver<E> {
    /// `epoch` is an instant before the first `pump`, for example the time of `open`: the driver never reads a clock.
    pub fn new(cfg: EngineConfig, edges: E, epoch: Instant) -> HostDriver<E> {
        let engine = HostEngine::new(cfg, epoch);
        let frame_bound = engine.link_frame_bound();
        let wake = edges.wake();
        HostDriver {
            engine,
            edges,
            links: BTreeMap::new(),
            wake,
            frame_bound,
            failed_handoffs: std::collections::VecDeque::new(),
        }
    }

    pub fn engine(&self) -> &HostEngine {
        &self.engine
    }

    pub fn edges(&mut self) -> &mut E {
        &mut self.edges
    }

    /// TM-6: a call that leaves work for `pump` signals the wake before it returns.
    fn sync_wake(&self) {
        if self.engine.runnable() {
            self.wake.signal();
        }
    }

    // ---- actions ----

    fn perform(&mut self) {
        while let Some(action) = self.engine.poll_action() {
            self.act(action);
        }
    }

    /// Performs the engine's actions, and counts the events that their results post against the pump (9B).
    fn perform_counted(&mut self, budget: &mut Budget) {
        self.perform();
        budget.account(&mut self.engine);
        // A failed handoff posts `RouteClosed`: one input per route, and only while the pump has budget (9B `pump_events`).
        while !budget.exhausted() {
            let Some(route) = self.failed_handoffs.pop_front() else {
                break;
            };
            let now = self.now();
            self.feed(now, Input::HandoffFailed { route });
            budget.account(&mut self.engine);
            self.perform();
        }
        if !self.failed_handoffs.is_empty() {
            budget.more_input = true;
        }
    }

    /// The production policy visits sessions round-robin (plan 2.4): when the chosen work is a session step, the scheduler
    /// picks among the session steps at the `Session` choice point, so one busy session cannot starve another.
    fn pick_session(&mut self, ready: &[Work], at: usize) -> Work {
        if !matches!(ready[at], Work::Session(_)) {
            return ready[at].clone();
        }
        let sessions: Vec<&Work> = ready
            .iter()
            .filter(|w| matches!(w, Work::Session(_)))
            .collect();
        let i = self
            .edges
            .scheduler()
            .pick(ChoicePoint::Session, sessions.len())
            .min(sessions.len() - 1);
        sessions[i].clone()
    }

    /// A poll freed mandatory room: a link whose held frame can be taken now gets its read interest back, and the wake is
    /// signalled so that the host pumps (EV-5d, TM-6). The frame itself is delivered in the next `pump` (OR-1).
    fn unpark_links(&mut self) {
        let ids: Vec<LinkId> = self.links.keys().copied().collect();
        for link in ids {
            let ready = self
                .links
                .get(&link)
                .and_then(|s| s.held.as_ref())
                .is_some_and(|input| self.engine.can_accept(input));
            if ready {
                self.edges.set_read_interest(link, true);
                self.wake.signal();
            }
        }
    }

    fn feed(&mut self, now: Instant, input: Input) {
        self.engine.handle(now, input);
    }

    fn act(&mut self, action: Action) {
        let now = self.now();
        match action {
            Action::Random { ticket, len } => {
                let mut bytes = vec![0u8; len];
                self.edges.fill_random(&mut bytes);
                self.feed(now, Input::Random { ticket, bytes });
            }
            Action::WriteRow { ticket, key, bytes } => {
                let result = self.edges.write_row(&key, &bytes);
                self.feed(now, Input::RowWritten { ticket, result });
            }
            Action::DeleteRow { ticket, key } => {
                let result = self.edges.delete_row(&key);
                self.feed(now, Input::RowDeleted { ticket, result });
            }
            Action::ReadRows { ticket, prefix } => {
                let result = self.edges.read_rows(&prefix);
                self.feed(now, Input::Rows { ticket, result });
            }
            Action::SpawnWorker {
                ticket,
                program,
                instance,
                token,
                host_epoch,
            } => {
                let result = self.edges.spawn_worker(&WorkerSpawn {
                    program,
                    instance,
                    token,
                    host_epoch,
                });
                self.feed(now, Input::Spawned { ticket, result });
            }
            Action::SendHello { link, hello } => {
                let mut payload = Vec::new();
                if hello.encode(&mut payload).is_ok() {
                    self.send_frame(link, FrameType::HELLO, &payload);
                }
            }
            Action::SendMsg { link, msg } => {
                let mut payload = Vec::new();
                msg.encode(&mut payload);
                self.send_frame(link, FrameType::HOST_MSG, &payload);
            }
            Action::CloseLink { link } => self.close_link(link),
            Action::SignalGroup { identity, signal } => self.edges.signal_group(identity, signal),
            Action::HandoffRoute {
                link,
                route,
                transport,
                options,
            } => {
                if self
                    .edges
                    .handoff_route(link, route, transport, &options)
                    .is_err()
                {
                    self.failed_handoffs.push_back(route);
                }
            }
        }
    }

    fn now(&self) -> Instant {
        // The engine keeps the last time that the host passed; a call before the first `pump` uses a fixed past instant that
        // no deadline can be due at.
        self.engine.last_now()
    }

    // ---- links ----

    fn send_frame(&mut self, link: LinkId, kind: FrameType, payload: &[u8]) {
        let Some(state) = self.links.get_mut(&link) else {
            return;
        };
        if encode_frame(kind, payload, self.frame_bound, &mut state.out).is_err() {
            // A frame over the bound can never be sent: the link is broken.
            self.close_link(link);
            return;
        }
        self.flush(link);
    }

    fn flush(&mut self, link: LinkId) {
        let Some(state) = self.links.get_mut(&link) else {
            return;
        };
        while !state.out.is_empty() {
            match self.edges.link_send(link, &state.out) {
                Ok(0) => break,
                Ok(n) => {
                    state.out.drain(..n);
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.close_link(link);
                    return;
                }
            }
        }
        let wanted = !state.out.is_empty();
        self.edges.set_write_interest(link, wanted);
    }

    fn close_link(&mut self, link: LinkId) {
        if self.links.remove(&link).is_some() {
            self.edges.link_close(link);
            let now = self.now();
            self.feed(now, Input::LinkClosed { link });
        }
    }

    /// Reads every link that has input, and gives the engine what it decoded (plan 2.5). Control comes before data: this runs
    /// before the work of a `pump`. A link is read only as far as the engine can take what it decoded and as far as the
    /// budget of the pump allows: the rest stays unread (kernel buffer, or the one frame in `held`).
    fn service_links(&mut self, budget: &mut Budget) {
        let now = self.now();
        while let Some(link) = self.edges.accept_link() {
            self.links.insert(
                link,
                LinkState {
                    decoder: FrameDecoder::new(self.frame_bound),
                    out: Vec::new(),
                    hello_seen: false,
                    pending: Vec::new(),
                    held: None,
                },
            );
        }
        let ids: Vec<LinkId> = self.links.keys().copied().collect();
        for link in ids {
            self.flush(link);
            self.retry_held(link, now, budget);
            self.read_link(link, now, budget);
        }
    }

    /// Delivers the frame that the engine could not take before, as soon as it can (EV-5d): it runs whenever the link is
    /// serviced, so a frame held behind a step (the end of a start) goes in after that step, before any later frame.
    fn retry_held(&mut self, link: LinkId, now: Instant, budget: &mut Budget) {
        let Some(input) = self.links.get_mut(&link).and_then(|s| s.held.take()) else {
            return;
        };
        // A held frame posts events like any other input: it waits for a pump with budget left (9B `pump_events`).
        if budget.exhausted() {
            self.links.get_mut(&link).expect("kept").held = Some(input);
            budget.more_input = true;
            return;
        }
        if !self.engine.can_accept(&input) {
            self.links.get_mut(&link).expect("kept").held = Some(input);
            return;
        }
        self.feed(now, input);
        budget.account(&mut self.engine);
        self.edges.set_read_interest(link, true);
    }

    fn read_link(&mut self, link: LinkId, now: Instant, budget: &mut Budget) {
        let mut buf = vec![0u8; READ_CHUNK];
        loop {
            let Some(state) = self.links.get_mut(&link) else {
                return;
            };
            if state.held.is_some() {
                return;
            }
            // 1. The bytes that were read earlier and not yet decoded: one frame at a time.
            if !state.pending.is_empty() {
                if budget.exhausted() {
                    budget.more_input = true;
                    return;
                }
                let took = state.decoder.push(&state.pending);
                state.pending.drain(..took);
                match state.decoder.next_frame() {
                    Ok(Some(frame)) => {
                        let first = !state.hello_seen;
                        state.hello_seen = true;
                        match self.deliver(link, first, frame.kind, &frame.payload, now, budget) {
                            Delivery::Done => {}
                            Delivery::Held => return,
                            Delivery::Bad => {
                                self.close_link(link);
                                return;
                            }
                        }
                    }
                    Ok(None) => {}
                    Err(_) => {
                        // A frame over the bound ends the link (plan section 3).
                        self.close_link(link);
                        return;
                    }
                }
                if !self.links.contains_key(&link) {
                    return;
                }
                continue;
            }
            // 2. More bytes, within the bytes that one pump may read from this link (9B `pump_bytes`). A read never takes
            // more than what is left of that allowance.
            let slice = budget.bytes_left(link).min(READ_CHUNK);
            if budget.exhausted() || slice == 0 {
                if budget.exhausted() || budget.bytes_left(link) == 0 {
                    budget.more_input = true;
                }
                return;
            }
            let n = match self.edges.link_recv(link, &mut buf[..slice]) {
                Ok(0) => {
                    self.close_link(link);
                    return;
                }
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => return,
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(_) => {
                    self.close_link(link);
                    return;
                }
            };
            budget.read(link, n);
            if let Some(state) = self.links.get_mut(&link) {
                state.pending.extend_from_slice(&buf[..n]);
            }
        }
    }

    /// Turns one frame into an input. `Held` when the engine cannot take it now: it stays unread on its link (EV-5b).
    fn deliver(
        &mut self,
        link: LinkId,
        first: bool,
        kind: FrameType,
        payload: &[u8],
        now: Instant,
        budget: &mut Budget,
    ) -> Delivery {
        let input = if first {
            if kind != FrameType::HELLO {
                return Delivery::Bad;
            }
            match Hello::decode(payload) {
                Ok(hello) => Input::LinkHello { link, hello },
                Err(_) => return Delivery::Bad,
            }
        } else {
            if kind != FrameType::WORKER_MSG {
                return Delivery::Bad;
            }
            match WorkerMsg::decode(payload) {
                Ok(msg) => Input::LinkMsg { link, msg },
                Err(_) => return Delivery::Bad,
            }
        };
        if !self.engine.can_accept(&input) {
            self.edges.set_read_interest(link, false);
            if let Some(state) = self.links.get_mut(&link) {
                state.held = Some(input);
            }
            return Delivery::Held;
        }
        self.feed(now, input);
        budget.account(&mut self.engine);
        Delivery::Done
    }
}

enum Delivery {
    Done,
    Held,
    Bad,
}

/// The bounds of one `pump` (9B `pump_events`, `pump_bytes`, plan 2.4).
struct Budget {
    /// The events that this pump may post (the scheduler's bound).
    bound: usize,
    posted: usize,
    /// The bytes that one link may deliver in this pump.
    per_link_bytes: usize,
    read: BTreeMap<LinkId, usize>,
    /// Input remains unread because of a bound: the host must pump again.
    more_input: bool,
}

impl Budget {
    fn exhausted(&self) -> bool {
        self.posted >= self.bound
    }

    fn account(&mut self, engine: &mut HostEngine) {
        self.posted += engine.take_posted() as usize;
    }

    fn bytes_left(&self, link: LinkId) -> usize {
        self.per_link_bytes
            .saturating_sub(self.read.get(&link).copied().unwrap_or(0))
    }

    fn read(&mut self, link: LinkId, n: usize) {
        *self.read.entry(link).or_insert(0) += n;
    }
}

impl<E: HostEdges> CoreApi for HostDriver<E> {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        let id = self.engine.begin(op)?;
        self.sync_wake();
        Ok(id)
    }

    /// The only call that makes progress (OR-1, TM-2): the order of the work is the scheduler's (plan 2.4), and every kind of
    /// work (link input, process exits, steps) counts against the bounds of this pump (9B).
    fn pump(&mut self, now: Now) -> PumpReport {
        // Events that an earlier call posted (a `cancel`) are not this pump's.
        let _ = self.engine.take_posted();
        let limits = self.engine.limits();
        let bound = self
            .edges
            .scheduler()
            .bound(ChoicePoint::PumpBound, limits.pump_events as usize);
        let mut budget = Budget {
            bound,
            posted: 0,
            per_link_bytes: usize::try_from(limits.pump_bytes).unwrap_or(usize::MAX),
            read: BTreeMap::new(),
            more_input: false,
        };
        self.feed(now.monotonic, Input::Clock(now.unix));
        // A due `Silent` is an older step than any input that arrives now: it runs first, with the budget it needs (E3-1 item 3).
        while !budget.exhausted() && self.engine.ready().contains(&Work::Silent) {
            self.feed(now.monotonic, Input::Run(Work::Silent));
            budget.account(&mut self.engine);
        }
        while !budget.exhausted() {
            let Some((identity, status)) = self.edges.poll_process_exit() else {
                break;
            };
            self.feed(now.monotonic, Input::ProcessExited { identity, status });
            budget.account(&mut self.engine);
        }
        // An exit that stays in the edge is for the next pump; the host must call again.
        if budget.exhausted() {
            budget.more_input = true;
        }
        // Each link's held frame goes first, before the link's later frames (`service_links`).
        self.service_links(&mut budget);
        self.perform_counted(&mut budget);
        let mut deferred: BTreeSet<Work> = BTreeSet::new();
        loop {
            // At the event bound only an effect without an event may run; every step with an event is carried (E3-1).
            let ready: Vec<Work> = self
                .engine
                .ready()
                .into_iter()
                .filter(|w| !deferred.contains(w))
                .filter(|w| !budget.exhausted() || *w == Work::Deadline)
                .collect();
            if ready.is_empty() {
                break;
            }
            let at = self
                .edges
                .scheduler()
                .pick(ChoicePoint::ReadyWork, ready.len())
                .min(ready.len() - 1);
            let work = self.pick_session(&ready, at);
            // A5-2: the scheduler may defer the progress of an operation to a later pump. A deadline is never deferred.
            if !matches!(work, Work::Deadline | Work::Silent)
                && !self.engine.never_deferred(&work)
                && self
                    .edges
                    .scheduler()
                    .pick(ChoicePoint::OperationDeferral, 2)
                    == 1
            {
                deferred.insert(work);
                continue;
            }
            self.feed(now.monotonic, Input::Run(work));
            budget.account(&mut self.engine);
            self.perform_counted(&mut budget);
            self.service_links(&mut budget);
            self.perform_counted(&mut budget);
        }
        let mut more = self.engine.runnable() || budget.more_input;
        if !more {
            self.edges.settle_wake();
            self.service_links(&mut budget);
            self.perform_counted(&mut budget);
            more = self.engine.runnable() || budget.more_input;
        }
        if more {
            // Work or input remains: the wake stays set, so a host that waits on it pumps again (TM-6).
            self.wake.signal();
        } else {
            self.wake.drain();
        }
        PumpReport {
            more,
            events_posted: u32::try_from(budget.posted).unwrap_or(u32::MAX),
        }
    }

    /// A5-2: the scheduler chooses the size of the non-empty part of the queue that a poll returns; the production policy
    /// returns `max`.
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let batch = self.edges.scheduler().bound(ChoicePoint::PollBatch, max);
        let events = self.engine.poll_events(batch);
        self.unpark_links();
        self.sync_wake();
        events
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        let wake: Arc<dyn HostWake> = Arc::clone(&self.wake);
        wake
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.engine.next_deadline()
    }

    fn cancel(&mut self, op: OpId) -> CancelResult {
        let result = self.engine.cancel(op);
        self.perform();
        self.sync_wake();
        result
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.engine.get(id)
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.engine.list()
    }

    fn status(&self) -> Status {
        self.engine.status()
    }

    fn diagnostics(&self) -> serde_json::Value {
        self.engine.diagnostics()
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.engine.terminal_state(id)
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.engine.read_page(capture, page)
    }

    fn release(&mut self, capture: CaptureId) {
        self.engine.release(capture);
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.engine.release_owner(client);
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.engine.snapshot_formats(session)
    }

    fn shadow_answerable_kinds(&self) -> Vec<QueryKind> {
        self.engine.shadow_answerable_kinds()
    }

    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        let result = self.engine.attach(client, session, transport, options)?;
        self.sync_wake();
        Ok(result)
    }

    fn tap_read(&mut self, session: &SessionId, _max: usize) -> Result<TapChunk, CoreError> {
        self.engine.tap_read(session)
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        let result = self.engine.set_silence_threshold(session, threshold);
        self.sync_wake();
        result
    }

    fn service_send(
        &mut self,
        _id: &ServiceId,
        _lane: u8,
        _frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        Err(SendError::UnknownService)
    }

    fn service_recv(&mut self, _id: &ServiceId, _lane: u8) -> Result<Option<Frame>, RecvError> {
        Err(RecvError::UnknownService)
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        Err(CoreError::new(
            ErrorCode::UnknownService,
            format!("no service {id}"),
        ))
    }

    fn service_log_tail(&self, id: &ServiceId, _max: usize) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::new(
            ErrorCode::UnknownService,
            format!("no service {id}"),
        ))
    }

    fn features(&self) -> Features {
        self.engine.features()
    }

    fn limits(&self) -> CoreLimits {
        self.engine.limits()
    }

    fn worker_protocol(&self) -> u8 {
        self.engine.worker_protocol()
    }

    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        self.engine.adoptable_worker_protocols()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.engine.worker_protocol_compatibility(protocol)
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        self.engine.terminal_identity()
    }
}

// The driver sends host messages through the engine's `SendMsg` action: this keeps the type in the public docs.
#[allow(dead_code)]
fn _host_msg_is_the_wire(_: &HostMsg) {}
