//! `HostEngine`: the registry model, the admission table, the operation table, the one event queue, the captures and the
//! deadlines of Core (plan 2.2). It is a sans-IO machine: see [`crate::io`] for what a driver feeds in and performs.

use crate::flow::Flow;
use crate::io::{Action, Input, LinkId, Ticket, Work};
use crate::queue::{EventQueue, QueueBounds};
use crate::session::{Admit, Session, WorkerHandle};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::ProcessIdentity;
use botster_core_edges::Machine;
use botster_core_link::msg::HostMsg;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// The largest link frame payload of any limits: half of `u32`, so a length and its arithmetic never overflow (plan
/// section 3).
const LINK_FRAME_CAP: u32 = u32::MAX / 2;

/// How many link closes `diagnostics()` keeps, newest last (LC-10).
const LINK_CLOSE_RECORDS: usize = 16;

/// What the engine needs to know before its first input.
///
/// Clause: Core LC-1, Core 9B, Core A2-6, Core DP-8, Core AD-4.
#[derive(Debug, Clone)]
pub struct EngineConfig {
    /// Validated by the driver with `CoreLimits::validate` (9B).
    pub limits: CoreLimits,
    pub features: Features,
    /// The host epoch of this handle, strictly above every earlier one (DP-8).
    pub host_epoch: u64,
    pub worker_path: PathBuf,
    /// The worker protocol number T of this Core (AD-4, A6-2).
    pub worker_protocol: u8,
    /// The query kinds that the pinned emulator's shadow answers (EV-8, non-normative list of the implementation).
    pub shadow_answerable: Vec<botster_route_codec::prelude::QueryKind>,
    /// The terminal identity of the pinned emulator (A2-8).
    pub terminal_identity: TerminalIdentity,
}

/// What a pending operation does next (the table of A2-1, one machine per row).
#[derive(Debug, Clone)]
pub(crate) enum Step {
    /// The op has a step to take. It is ready work unless the step needs queue room that is not there.
    Ready(Next),
    /// The op waits for an edge result, a worker message or a flow.
    Await(Wait),
    /// `Completed` was posted; the slot is held until the host polls it (EV-5a).
    Done,
}

#[derive(Debug, Clone)]
pub(crate) enum Next {
    /// Posts `Completed` with this result.
    Complete(OpResult),
    /// Writes the row for `UpdateMetadata`, then completes.
    MetaWrite,
    /// Writes the row for `SetNotificationPolicy` in `Created`, then completes.
    PolicyWrite,
    /// Sends the op to the worker (the reads, `WriteInput`, the setters).
    Forward,
    /// Starts the stops of the targets of `StopAll` (LC-12).
    StopAllStart(BTreeSet<SessionId>),
    /// `Detach`: asks the worker to close the route.
    Detach,
    /// `Detach` of a route whose worker is gone: the host closes it.
    DetachLocal,
    /// A setter of a `Created` session (`Resize`, `SetSizePolicy`, `SetColorProfile`): its effect is visible only when this
    /// step runs, in a `pump` (OR-1).
    Setter,
    /// `AdoptAll`: reads the rows.
    AdoptRead,
    /// `AdoptAll`: recovers the next row and posts its state.
    AdoptRow,
}

#[derive(Debug, Clone)]
pub(crate) enum Wait {
    Ticket,
    /// For the start of the session to end, then forward (`Starting` sessions).
    Launch,
    Worker,
    /// For every target of a `StopAll` to reach `Exited`, `Lost` or `Created`.
    StopAll(BTreeSet<SessionId>),
    /// For the stop flow of the session to end (a joined `Stop`).
    Flow,
    /// For the route to close (`Detach`).
    Route(RouteId),
    /// `AdoptAll`: for every row that this op adopts to post its state (LC-11: the completion follows the last one).
    Adopt,
}

#[derive(Debug)]
pub(crate) struct PendingOp {
    pub op: Op,
    pub session: Option<SessionId>,
    pub instance: Option<InstanceId>,
    pub step: Step,
    /// The pages of a capture that arrived before its `Done` (ST-6).
    pub pages: Option<Vec<Page>>,
    /// The request number that was sent to the worker.
    pub req: Option<u64>,
    /// The bytes that a `WriteInput` holds against the lane bound (IN-5).
    pub held_bytes: u64,
    /// A `SetNotificationPolicy` that was admitted in `Created`: it follows the registry path (A2-1).
    pub created_path: bool,
    /// The clause fixes the timing of this op: it completes in the next `pump`, and the scheduler never defers it (steward
    /// ruling R-20, A5-2 "not varied"): LC-5 (a `Stop` of a session whose payload exited), SZ-2 (a `Resize` to the current
    /// size) and the A2-1 `Resize` row in `Created`.
    pub fixed_timing: bool,
    /// A `cancel` was admitted (IN-6).
    pub cancelled: bool,
    /// `AdoptAll`: the rows that remain.
    pub rows: VecDeque<(String, Vec<u8>)>,
}

/// A capture: a frozen copy of a snapshot (ST-6).
#[derive(Debug)]
pub(crate) struct CaptureEntry {
    /// The `CaptureSnapshot` op that made it: its reservation turns into `bytes` when the host polls its `Completed` (A8-1).
    pub op: OpId,
    pub owner: ClientId,
    pub instance: InstanceId,
    pub pages: Vec<Page>,
    pub bytes: u64,
    pub expires: Instant,
}

#[derive(Debug)]
pub(crate) struct RouteEntry {
    pub session: SessionId,
    pub instance: InstanceId,
    pub route_tag: Option<String>,
}

/// A route event that waits for mandatory room (EV-5b).
#[derive(Debug)]
pub(crate) enum ParkedRoute {
    Close(RouteId, RouteCloseReason),
    /// `RouteStalled` (true) or `RouteResumed` (false).
    Progress(RouteId, bool),
}

/// Who a ticket belongs to.
#[derive(Debug, Clone)]
pub(crate) enum Owner {
    Session(SessionId),
    Op(OpId),
    /// The final row of a session that ended: no operation waits for it, so a failure is only recorded (LC-10).
    FinalRow,
}

/// The host engine.
pub struct HostEngine {
    pub(crate) cfg: EngineConfig,
    pub(crate) queue: EventQueue,
    /// `queue.total_posted()` when the current input began: a step posts at most one event (9B `pump_events`).
    pub(crate) step_mark: u64,
    pub(crate) sessions: BTreeMap<SessionId, Session>,
    pub(crate) ops: BTreeMap<OpId, PendingOp>,
    /// The ops of removed instances, and of no session at all: `cancel` tells them apart exactly (ID-1, IN-6).
    pub(crate) retired_ops: crate::session::IdRanges,
    pub(crate) captures: BTreeMap<CaptureId, CaptureEntry>,
    pub(crate) routes: BTreeMap<RouteId, RouteEntry>,
    pub(crate) links: BTreeMap<LinkId, SessionId>,
    pub(crate) tickets: BTreeMap<Ticket, Owner>,
    pub(crate) actions: VecDeque<Action>,
    pub(crate) next_op: u64,
    pub(crate) next_ticket: u64,
    pub(crate) next_capture: u64,
    pub(crate) next_route: u64,
    pub(crate) next_instance: u64,
    pub(crate) next_req: u64,
    /// The monotonic time of the last `pump` (TM-1: the host passes it; Core reads no clock). `None` before the first one.
    pub(crate) now: Option<Instant>,
    pub(crate) unix: UnixSeconds,
    pub(crate) adopt_all_begun: bool,
    /// Final rows whose write failed or is uncertain: the registry may still show an earlier state, which the next
    /// `AdoptAll` reads (AD-7: the registry as read after the next `open` is authoritative).
    pub(crate) final_row_failures: u64,
    /// Why the last links were closed (a bad frame, a hello that failed AD-6 or AD-4, a peer that left), newest last: an
    /// interoperability failure is visible where its cause was known (LC-10).
    link_closes: VecDeque<String>,
    /// The ids of durable rows that no session of this handle holds yet: `Create` refuses them (ID-1: an id is unique among
    /// registry rows), and `AdoptAll` turns each into a session (AD-1).
    pub(crate) unadopted: BTreeSet<SessionId>,
    /// Routes that are registered and wait for the handoff of their stream to the worker (DP-2).
    pub(crate) pending_handoffs: Vec<(RouteId, StreamEndpoint, AttachOptions, AppliedRouteLimits)>,
    /// Route closes and route events that wait for mandatory-queue room (EV-5b).
    /// One queue in arrival order, so that the events of a route keep the worker's order (EV-6): a close is never posted
    /// before an event that the worker sent ahead of it.
    pub(crate) parked: VecDeque<ParkedRoute>,
}

impl HostEngine {
    /// `registry_ids` are the ids of the durable rows that the registry holds when the handle opens: they are in use until
    /// `AdoptAll` recovers them or `Remove` frees them (ID-1, LC-3, AD-2).
    pub fn new(cfg: EngineConfig, registry_ids: BTreeSet<SessionId>) -> HostEngine {
        let bounds = QueueBounds {
            droppable: cfg.limits.event_queue as usize,
            mandatory: cfg.limits.mandatory_events as usize,
        };
        HostEngine {
            queue: EventQueue::new(bounds),
            step_mark: 0,
            sessions: BTreeMap::new(),
            ops: BTreeMap::new(),
            retired_ops: Default::default(),
            captures: BTreeMap::new(),
            routes: BTreeMap::new(),
            links: BTreeMap::new(),
            tickets: BTreeMap::new(),
            actions: VecDeque::new(),
            next_op: 1,
            next_ticket: 1,
            next_capture: 1,
            next_route: 1,
            next_instance: 1,
            next_req: 1,
            now: None,
            unadopted: registry_ids,
            unix: 0,
            adopt_all_begun: false,
            final_row_failures: 0,
            link_closes: VecDeque::new(),
            pending_handoffs: Vec::new(),
            parked: VecDeque::new(),
            cfg,
        }
    }

    // ---- helpers shared by the modules of the engine ----

    pub(crate) fn error(code: ErrorCode, detail: impl Into<String>) -> CoreError {
        CoreError::new(code, detail)
    }

    pub(crate) fn ticket(&mut self, owner: Owner) -> Ticket {
        let ticket = Ticket(self.next_ticket);
        self.next_ticket += 1;
        self.tickets.insert(ticket, owner);
        ticket
    }

    pub(crate) fn mint_instance(&mut self) -> InstanceId {
        let n = self.next_instance;
        self.next_instance += 1;
        InstanceId(format!("{}-{}", self.cfg.host_epoch, n))
    }

    /// Records why `link` was closed, for `diagnostics()` (LC-10). The driver records the closes that it decides.
    pub fn record_link_close(&mut self, link: LinkId, why: &dyn std::fmt::Display) {
        if self.link_closes.len() == LINK_CLOSE_RECORDS {
            self.link_closes.pop_front();
        }
        self.link_closes
            .push_back(format!("link {}: {why}", link.0));
    }

    /// Closes `link` and records why.
    pub(crate) fn close_link(&mut self, link: LinkId, why: &str) {
        self.record_link_close(link, &why);
        self.act(Action::CloseLink { link });
    }

    pub(crate) fn act(&mut self, action: Action) {
        self.actions.push_back(action);
    }

    /// Whether the engine can take `input` now. A route event needs mandatory room (EV-5b); the driver keeps a frame that
    /// cannot be taken unread on its link and removes the read interest (plan 2.5 rule 7).
    pub fn can_accept(&self, input: &Input) -> bool {
        use botster_core_link::msg::WorkerMsg;
        match input {
            Input::LinkMsg {
                msg:
                    WorkerMsg::RouteClosed { .. }
                    | WorkerMsg::RouteStalled { .. }
                    | WorkerMsg::RouteResumed { .. }
                    // The frames that cause a state transition stay unread until their event fits (EV-5b).
                    | WorkerMsg::Exited { .. }
                    | WorkerMsg::Launched { .. }
                    | WorkerMsg::LaunchFailed { .. }
                    | WorkerMsg::Adopted { .. },
                ..
            } => self.has_room(),
            // The events of the model belong to the running session: an observation stays unread on its link while the
            // session's start or adoption is not through, so it follows `Running` and the completion of `Start` (OR-2, EV-5).
            // The link keeps the worker's order (ST-4, EV-6), and its one held frame bounds what waits (plan 2.5 rule 7).
            Input::LinkMsg {
                link,
                msg: WorkerMsg::Observed { .. },
            } => !self.links.get(link).is_some_and(|id| {
                self.sessions
                    .get(id)
                    .is_some_and(|s| matches!(s.flow, Flow::Start(_) | Flow::Adopt(_)))
            }),
            _ => true,
        }
    }

    pub(crate) fn mono(&self) -> Option<Instant> {
        self.now
    }

    pub(crate) fn send_msg(&mut self, session: &SessionId, msg: HostMsg) -> bool {
        match self.sessions.get(session).and_then(|s| s.worker.link) {
            Some(link) => {
                self.act(Action::SendMsg { link, msg });
                true
            }
            None => false,
        }
    }

    pub(crate) fn complete(&mut self, op: OpId, result: OpResult) {
        if let Some(pending) = self.ops.get_mut(&op) {
            if matches!(pending.step, Step::Done) {
                return;
            }
            // A step that posted an event already completes the op in a step of its own (9B `pump_events`).
            if self.queue.total_posted() > self.step_mark {
                pending.step = Step::Ready(Next::Complete(result));
                return;
            }
            pending.step = Step::Done;
            self.queue.post_completed(op, result);
        }
    }

    /// Completes `op` in a step of its own: a step that completes several ops would post several events (9B `pump_events`).
    pub(crate) fn complete_later(&mut self, op: OpId, result: OpResult) {
        self.set_step(op, Step::Ready(Next::Complete(result)));
    }

    pub(crate) fn set_step(&mut self, op: OpId, step: Step) {
        if let Some(pending) = self.ops.get_mut(&op) {
            if !matches!(pending.step, Step::Done) {
                pending.step = step;
            }
        }
    }

    // ---- the sync reads of CoreApi ----

    pub fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.sessions
            .get(id)
            .and_then(Session::record)
            .ok_or_else(|| Self::error(ErrorCode::UnknownSession, format!("no session {}", id.0)))
    }

    pub fn list(&self) -> Vec<SessionRecord> {
        self.sessions.values().filter_map(Session::record).collect()
    }

    pub fn status(&self) -> Status {
        Status {
            sessions: self
                .sessions
                .values()
                .filter_map(|s| {
                    s.shown.map(|state| SessionStatus {
                        id: s.id.clone(),
                        state,
                    })
                })
                .collect(),
        }
    }

    pub fn diagnostics(&self) -> serde_json::Value {
        serde_json::json!({
            "sessions": self.sessions.len(),
            "pending_ops": self.ops.len(),
            "queued_events": self.queue.len(),
            "captures": self.captures.len(),
            "routes": self.routes.len(),
            "host_epoch": self.cfg.host_epoch,
            "final_row_failures": self.final_row_failures,
            "link_closes": self.link_closes,
        })
    }

    pub fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        let session = self
            .sessions
            .get(id)
            .filter(|s| s.shown.is_some())
            .ok_or_else(|| {
                Self::error(ErrorCode::UnknownSession, format!("no session {}", id.0))
            })?;
        session.terminal.clone().ok_or_else(|| {
            Self::error(
                ErrorCode::WrongState,
                "the session has no terminal state: no worker model",
            )
        })
    }

    pub fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        let entry = self.captures.get(&capture).ok_or_else(|| {
            Self::error(
                ErrorCode::UnknownCapture,
                format!("no capture {}", capture.0),
            )
        })?;
        entry.pages.get(page as usize).cloned().ok_or_else(|| {
            Self::error(
                ErrorCode::PageOutOfRange,
                format!("page {page} of {}", entry.pages.len()),
            )
        })
    }

    /// Idempotent (ST-6).
    pub fn release(&mut self, capture: CaptureId) {
        self.captures.remove(&capture);
    }

    pub fn release_owner(&mut self, client: &ClientId) {
        self.captures.retain(|_, c| &c.owner != client);
    }

    pub fn snapshot_formats(&self, id: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        let session = self
            .sessions
            .get(id)
            .filter(|s| s.shown.is_some())
            .ok_or_else(|| {
                Self::error(ErrorCode::UnknownSession, format!("no session {}", id.0))
            })?;
        Ok(session.formats.clone())
    }

    pub fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        self.cfg.shadow_answerable.clone()
    }

    pub fn features(&self) -> Features {
        self.cfg.features.clone()
    }

    /// The largest frame payload of a worker link, in bytes (plan section 3). A host write travels in its JSON form and a
    /// snapshot travels as pages, so the bound is a multiple of the largest payload that `CoreLimits` allows.
    pub fn link_frame_bound(&self) -> u32 {
        let limits = &self.cfg.limits;
        let largest = limits.max_paste_bytes.max(limits.max_snapshot_bytes);
        let bound = largest.saturating_mul(4).saturating_add(1 << 20);
        u32::try_from(bound).map_or(LINK_FRAME_CAP, |b| b.min(LINK_FRAME_CAP))
    }

    pub fn limits(&self) -> CoreLimits {
        self.cfg.limits.clone()
    }

    pub fn worker_protocol(&self) -> u8 {
        self.cfg.worker_protocol
    }

    /// `{T, T - 1}`, and no T - 1 below 1 (A6-2).
    pub fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        let t = self.cfg.worker_protocol;
        let mut set = BTreeSet::from([t]);
        if t > 1 {
            set.insert(t - 1);
        }
        set
    }

    pub fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        match protocol {
            None => WorkerCompatibility::Unknown,
            Some(p) if self.adoptable_worker_protocols().contains(&p) => {
                WorkerCompatibility::Compatible
            }
            Some(_) => WorkerCompatibility::Incompatible,
        }
    }

    /// The raw-output tap is off by default and no worker feeds it yet: the chunk is empty (TP-1).
    pub fn tap_read(&self, id: &SessionId) -> Result<TapChunk, CoreError> {
        let session = self
            .sessions
            .get(id)
            .filter(|s| s.shown.is_some())
            .ok_or_else(|| {
                Self::error(ErrorCode::UnknownSession, format!("no session {}", id.0))
            })?;
        Ok(TapChunk {
            instance: session.instance.clone(),
            bytes: botster_route_codec::prelude::HexBytes(Vec::new()),
            dropped_before: 0,
        })
    }

    pub fn terminal_identity(&self) -> TerminalIdentity {
        self.cfg.terminal_identity.clone()
    }

    /// Sets the silence threshold (A2-7, TM-4). `None` turns it off.
    pub fn set_silence_threshold(
        &mut self,
        id: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        if !self.cfg.features.names.contains(&Feature::Silence) {
            return Err(Self::error(
                ErrorCode::Unsupported { what: None },
                "the feature silence is not offered",
            ));
        }
        let session = self
            .sessions
            .get_mut(id)
            .filter(|s| s.shown.is_some())
            .ok_or_else(|| {
                Self::error(ErrorCode::UnknownSession, format!("no session {}", id.0))
            })?;
        session.silence.threshold = threshold;
        // A new threshold starts a new idle period (TM-4: once per period).
        session.silence.fired = false;
        Ok(())
    }

    // ---- the event queue ----

    /// Reads at most `max` events (OR-1: no progress). A polled `Completed` frees its slot (EV-5a), and room that a poll frees
    /// lets parked work run again (EV-5d).
    pub fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let polled = self.queue.poll(max);
        for event in &polled {
            if let Event::Completed { op, .. } = event {
                if let Some(done) = self.ops.remove(op) {
                    // The lane of the session is free again when the host polls the completion (AM-4, EV-5a).
                    if let (Some(session), Some(instance), Op::WriteInput { .. }) =
                        (done.session, done.instance, &done.op)
                    {
                        if let Some(s) = self
                            .sessions
                            .get_mut(&session)
                            .filter(|s| s.instance == instance)
                        {
                            s.input_ops = s.input_ops.saturating_sub(1);
                            s.input_bytes = s.input_bytes.saturating_sub(done.held_bytes);
                        }
                    }
                }
            }
        }
        polled
    }

    /// Whether a clause fixes the timing of this work: the scheduler never defers it (R-20).
    pub fn never_deferred(&self, work: &Work) -> bool {
        match work {
            Work::Op(op) => self.ops.get(op).is_some_and(|p| p.fixed_timing),
            // The end of a payload that an edge reported (the exit of the worker, its loss) is posted by the step that follows the
            // report: no operation makes progress there, so the scheduler has nothing to defer (A5-2: it defers the progress
            // of an operation).
            Work::Session(id) => self
                .sessions
                .get(id)
                .is_some_and(|s| matches!(&s.flow, Flow::Stop(f) if f.end.is_some())),
            _ => false,
        }
    }

    /// The count of events posted since the last call: the `events_posted` of `PumpReport` (A2-7).
    pub fn take_posted(&mut self) -> u32 {
        self.queue.take_posted()
    }

    pub(crate) fn post_state(&mut self, session: &SessionId, state: SessionState) -> bool {
        let Some(s) = self.sessions.get(session) else {
            return false;
        };
        let event = Event::SessionState {
            id: s.id.clone(),
            instance: s.instance.clone(),
            state,
        };
        self.queue.post_mandatory(event).is_ok()
    }

    pub(crate) fn has_room(&self) -> bool {
        self.queue.has_mandatory_room()
    }

    // ---- time and deadlines (TM-1, TM-3, TM-5) ----

    /// The earliest deadline that the engine owns, on the monotonic clock (TM-3).
    pub fn deadline(&self) -> Option<Instant> {
        self.deadlines().into_iter().map(|(at, _)| at).min()
    }

    pub fn runnable(&self) -> bool {
        !self.ready().is_empty()
    }

    // ---- the Machine interface ----

    pub(crate) fn pop_action(&mut self) -> Option<Action> {
        self.actions.pop_front()
    }

    pub(crate) fn identity_of(&self, session: &SessionId) -> Option<ProcessIdentity> {
        self.sessions.get(session).and_then(|s| s.worker.identity)
    }
}

impl HostEngine {
    /// Takes an input with no new time: an edge result, a link message or a step. Only `pump` moves the clock (TM-1), through
    /// [`Machine::handle`].
    pub fn input(&mut self, input: Input) {
        self.on_input(input);
    }
}

impl Machine for HostEngine {
    type Input = Input;
    type Action = Action;

    fn handle(&mut self, now: Instant, input: Input) {
        self.now = Some(now);
        self.on_input(input);
    }

    fn poll_action(&mut self) -> Option<Action> {
        self.pop_action()
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.deadline()
    }
}

/// The handle of a worker that no flow owns yet.
pub(crate) fn no_worker() -> WorkerHandle {
    WorkerHandle::default()
}

/// A new session of the table, in the `Created` admission state (AM-1).
pub(crate) fn new_session(
    id: SessionId,
    instance: InstanceId,
    request: SpawnRequest,
    flow: Flow,
) -> Session {
    Session {
        id,
        instance,
        admit: Admit::Created,
        shown: None,
        size: request.size,
        labels: request.labels.clone(),
        request,
        exit: None,
        worker_protocol: None,
        row_protocol: None,
        worker_features: None,
        token: None,
        worker: no_worker(),
        terminal: None,
        silence: Default::default(),
        host_ended: false,
        killed: false,
        stop_after_start: false,
        waiters: Vec::new(),
        inflight: BTreeMap::new(),
        input_ops: 0,
        input_bytes: 0,
        routes: BTreeSet::new(),
        route_ends: BTreeMap::new(),
        bound_at_end: BTreeSet::new(),
        flow,
        queue: VecDeque::new(),
        ticket: None,
        formats: Vec::new(),
        ops: Default::default(),
        pending_setters: 0,
        metadata_pending: false,
        payload: None,
        pending_end: None,
        adopting: None,
    }
}
