//! Ready work and its execution: the steps of the operations, and the deadlines (plan 2.4, TM-3, TM-5, TM-6).
//!
//! `ready` lists what a `pump` could do now, in the order that the production policy runs it. A step that posts a mandatory
//! event is not ready while the queue has no room: it is parked, it keeps `PumpReport.more` false, and a poll that frees
//! room makes it ready again (EV-5b, EV-5d, TM-6).

use crate::engine::{HostEngine, Next, Owner, Step, Wait};
use crate::flow::*;
use crate::io::{Action, Work};
use crate::session::{unknown_request, Admit, Row, ROW_PREFIX};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{GroupSignal, StorageError};
use botster_core_link::msg::{HostMsg, LaunchSpec};
use std::collections::BTreeSet;
use std::mem::{discriminant, Discriminant};
use std::time::Instant;

/// How a decoded row is recovered (AD-1).
enum Recovery {
    /// The row's state is known now.
    Post(SessionState),
    /// The row names a worker: its adoption, with the row's recorded state.
    Adopt(SessionState),
}

/// A deadline that the engine owns (TM-3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DeadlineKind {
    Startup(SessionId),
    StopGrace(SessionId),
    RemoveGrace(SessionId),
    /// The reachability deadline of an adoption (DESIGN.md "Adoption (P5)" 3.7).
    Adopt(SessionId),
    Silence(SessionId),
    Capture(CaptureId),
}

pub(crate) fn registry_failed(error: StorageError) -> CoreError {
    CoreError::new(
        ErrorCode::RegistryFailed {
            uncertain: matches!(error, StorageError::Uncertain { .. }),
        },
        format!("the registry failed: {error:?}"),
    )
}

impl HostEngine {
    /// Every deadline that is outstanding, with its instant (TM-3).
    pub(crate) fn deadlines(&self) -> Vec<(Instant, DeadlineKind)> {
        let mut out = Vec::new();
        for (id, s) in &self.sessions {
            match &s.flow {
                Flow::Start(f) => {
                    if let Some(at) = f.deadline {
                        out.push((at, DeadlineKind::Startup(id.clone())));
                    }
                }
                Flow::Stop(f) => {
                    if let Some(at) = f.deadline {
                        out.push((at, DeadlineKind::StopGrace(id.clone())));
                    }
                }
                Flow::Adopt(f) => {
                    if let Some(at) = f.deadline {
                        out.push((at, DeadlineKind::Adopt(id.clone())));
                    }
                }
                Flow::Remove(f) => {
                    if let Some(at) = f.deadline {
                        out.push((at, DeadlineKind::RemoveGrace(id.clone())));
                    }
                }
                _ => {}
            }
            if let Some(at) = s.silence.deadline() {
                out.push((at, DeadlineKind::Silence(id.clone())));
            }
        }
        for (id, c) in &self.captures {
            out.push((c.expires, DeadlineKind::Capture(*id)));
        }
        out.sort();
        out
    }

    fn step_needs_room(&self, next: &Next) -> bool {
        matches!(next, Next::AdoptRow | Next::DetachLocal)
    }

    fn flow_needs_room(&self, session: &crate::session::Session) -> bool {
        match &session.flow {
            Flow::Idle => false,
            Flow::Create(f) => f.phase == CreatePhase::PostCreated,
            Flow::Start(f) => matches!(
                f.phase,
                StartPhase::PostStarting | StartPhase::PostRunning | StartPhase::PostFailed
            ),
            Flow::Stop(f) => matches!(f.phase, StopPhase::PostStopping | StopPhase::PostEnd),
            Flow::Adopt(f) => f.phase == AdoptPhase::Post,
            Flow::Remove(f) => match f.phase {
                RemovePhase::CloseRoutes => !session.routes.is_empty(),
                RemovePhase::PostReleased => true,
                _ => false,
            },
        }
    }

    fn flow_waiting(&self, session: &crate::session::Session) -> bool {
        match &session.flow {
            Flow::Idle => true,
            Flow::Create(_) => false,
            Flow::Start(f) => matches!(f.phase, StartPhase::AwaitHello | StartPhase::AwaitLaunched),
            Flow::Stop(f) => f.phase == StopPhase::AwaitExit,
            Flow::Remove(f) => f.phase == RemovePhase::AwaitTeardown,
            Flow::Adopt(f) => matches!(
                f.phase,
                AdoptPhase::AwaitProbe
                    | AdoptPhase::AwaitConnect
                    | AdoptPhase::AwaitHello
                    | AdoptPhase::AwaitReport
            ),
        }
    }

    /// The work that a `pump` could do now (plan 2.4). Index 0 is the work that the production policy runs first.
    pub fn ready(&self) -> Vec<Work> {
        let room = self.has_room();
        let mut out = Vec::new();
        // A due deadline is processed in its pump, before other work (TM-3, TM-5). An effect without an event comes first
        // (E3-1 item 5); a due `Silent` follows as a carried step, before newer work (E3-1 item 3).
        if self.due_deadline(false).is_some() {
            out.push(Work::Deadline);
        }
        if self.due_deadline(true).is_some() {
            out.push(Work::Silent);
        }
        // The ops of one kind for one session reach its worker in `begin` order: the writes (AM-2), and the resizes, so that
        // the last one begun is the one applied (SZ-3, OR-1). Only the first op of each kind and session that is not forwarded
        // yet is offered. The order across kinds is left to the scheduler (OR-3).
        let mut unsent: Vec<(&SessionId, Discriminant<Op>)> = Vec::new();
        for (id, p) in &self.ops {
            match (&p.step, &p.session) {
                (Step::Ready(Next::Forward), Some(session)) => {
                    let key = (session, discriminant(&p.op));
                    if !unsent.contains(&key) {
                        unsent.push(key);
                        out.push(Work::Op(*id));
                    }
                }
                (Step::Ready(Next::MetaWrite | Next::PolicyWrite), Some(session))
                    if self
                        .sessions
                        .get(session)
                        .is_some_and(|s| matches!(s.flow, Flow::Create(_))) =>
                {
                    // A row write of an op admitted after `Create` waits for the create's own row (AM-1): if that write
                    // fails, the session never existed, and no row of it may stay.
                }
                (Step::Ready(Next::AdoptRow), _) if self.row_waits_for_its_session(p) => {
                    // The row's session is being created by this handle: its state is posted when its `Created` is
                    // (LC-11: AdoptAll's state of every row comes before its completion).
                }
                (Step::Ready(next), _) => {
                    if !self.step_needs_room(next) || room {
                        out.push(Work::Op(*id));
                    }
                }
                (Step::Await(Wait::StopAll(targets)), _) if self.stop_all_done(targets) => {
                    out.push(Work::Op(*id));
                }
                (Step::Await(Wait::Adopt), _) if !self.adopting_rows(*id) => {
                    out.push(Work::Op(*id));
                }
                _ => {}
            }
        }
        for (id, s) in &self.sessions {
            if s.metadata_pending {
                out.push(Work::Session(id.clone()));
                continue;
            }
            if s.ticket.is_some() || self.flow_waiting(s) {
                continue;
            }
            // `Remove` does not begin its teardown before an `UpdateMetadata` admitted ahead of it has written its row.
            if matches!(&s.flow, Flow::Remove(f) if f.phase == RemovePhase::SendRemove)
                && self.ops.values().any(|p| {
                    p.session.as_ref() == Some(id)
                        && matches!(p.op, Op::UpdateMetadata { .. })
                        && !matches!(p.step, Step::Done)
                })
            {
                continue;
            }
            // A start does not begin before the setters that were admitted ahead of it have run (AM-1 order).
            if matches!(&s.flow, Flow::Start(f) if f.phase == StartPhase::Token)
                && s.pending_setters > 0
            {
                continue;
            }
            if self.flow_needs_room(s) && !room {
                continue;
            }
            out.push(Work::Session(id.clone()));
        }
        if self.parked_work() && room {
            out.push(Work::Parked);
        }
        out
    }

    /// The earliest due deadline: a `Silent` when `silent`, and any other kind otherwise (E3-1).
    fn due_deadline(&self, silent: bool) -> Option<DeadlineKind> {
        self.due_deadlines(silent).next()
    }

    /// The deadlines that are due now: the silences, or every other kind.
    fn due_deadlines(&self, silent: bool) -> impl Iterator<Item = DeadlineKind> {
        let now = self.now;
        self.deadlines()
            .into_iter()
            .filter(move |(at, kind)| {
                now.is_some_and(|now| *at <= now)
                    && matches!(kind, DeadlineKind::Silence(_)) == silent
            })
            .map(|(_, kind)| kind)
    }

    /// How many silences are due now. Each `Work::Silent` step fires one, so this is the most `Silent` steps that a pump
    /// runs before other work (E3-1 item 3).
    pub(crate) fn due_silences(&self) -> usize {
        self.due_deadlines(true).count()
    }

    /// Runs one piece of ready work (plan 2.4).
    pub(crate) fn run(&mut self, work: Work) {
        match work {
            Work::Op(op) => self.run_op(op),
            Work::Session(id) => self.run_flow(&id),
            Work::Deadline => self.run_deadline(false),
            Work::Silent => self.run_deadline(true),
            Work::Parked => self.run_parked(),
        }
    }

    fn run_deadline(&mut self, silent: bool) {
        let Some(kind) = self.due_deadline(silent) else {
            return;
        };
        match kind {
            DeadlineKind::Capture(id) => {
                self.captures.remove(&id);
            }
            DeadlineKind::Silence(id) => {
                let unix = self.unix;
                if let Some(s) = self.sessions.get_mut(&id) {
                    s.silence.fired = true;
                    let since = s.silence.idle_start.map_or(unix, |(_, u)| u);
                    let event = Event::Silent {
                        id: s.id.clone(),
                        instance: s.instance.clone(),
                        since,
                    };
                    self.queue.post_keyed(event);
                }
            }
            DeadlineKind::StopGrace(id) => self.kill_payload(&id),
            DeadlineKind::RemoveGrace(id) => self.remove_grace_expired(&id),
            DeadlineKind::Startup(id) => self.startup_expired(&id),
            DeadlineKind::Adopt(id) => self.adopt_expired(&id),
        }
    }

    /// The kill of `stop_grace` (LC-5). With the link it goes to the worker, which kills the payload's group and keeps the
    /// final model. Without it, the host sends `GroupSignal::EndPayload` to the worker alone, which it identifies by pid and
    /// start time (AD-6). The host never signals a bare payload group and never kills the worker: the worker holds the
    /// payload leader unreaped and ends the group itself (P3), and it keeps the final model (LC-5).
    fn kill_payload(&mut self, id: &SessionId) {
        let sent = self.send_msg(id, HostMsg::Kill);
        if !sent {
            if let Some(identity) = self.identity_of(id) {
                self.act(Action::SignalGroup {
                    identity,
                    signal: GroupSignal::EndPayload,
                });
            }
        }
        let broken = !sent;
        if let Some(s) = self.sessions.get_mut(id) {
            s.killed = true;
            if let Flow::Stop(f) = &mut s.flow {
                f.deadline = None;
            }
        }
        if broken {
            // The worker cannot answer: the payload was killed and Core cannot learn the exit. The session ends
            // `Lost(WorkerUnreachable)` (AD-2: the worker is alive, and no connection is made).
            self.begin_end_flow(id, End::Lost(LostReason::WorkerUnreachable));
        }
    }

    pub(crate) fn row_of(&self, id: &SessionId, state: SessionState) -> Row {
        let mut row = self.sessions[id].to_row();
        row.state = state;
        row
    }

    /// One durable write of `row` for `owner`; the answer carries the returned ticket (AD-7).
    pub(crate) fn write_row(&mut self, owner: Owner, row: &Row) -> crate::io::Ticket {
        let ticket = self.ticket(owner);
        self.act(Action::WriteRow {
            ticket,
            key: crate::session::row_key(&row.id),
            bytes: serde_json::to_vec(row).expect("a row is JSON"),
        });
        ticket
    }

    /// The row of the state that a session ended in, or that an adoption posted. No operation waits for it; a failure is
    /// counted (`FinalRow`).
    pub(crate) fn write_final_row(&mut self, id: &SessionId, state: SessionState) {
        if self.sessions.contains_key(id) {
            let row = self.row_of(id, state);
            self.write_row(Owner::FinalRow, &row);
        }
    }

    // ---- the steps of the operations ----

    pub(crate) fn run_op(&mut self, op_id: OpId) {
        let Some(pending) = self.ops.get(&op_id) else {
            return;
        };
        if let Step::Await(Wait::StopAll(targets)) = &pending.step {
            if self.stop_all_done(targets) {
                self.complete(op_id, OpResult::Ok(OpOutput::Unit));
            }
            return;
        }
        if matches!(pending.step, Step::Await(Wait::Adopt)) {
            // LC-11: `Completed{AdoptAll}` follows the last row's state.
            if !self.adopting_rows(op_id) {
                self.complete(op_id, OpResult::Ok(OpOutput::Unit));
            }
            return;
        }
        let Step::Ready(next) = pending.step.clone() else {
            return;
        };
        let session = pending.session.clone();
        // An op of an instance that is gone never reaches a later instance of the same id (ID-1, AM-3).
        if let (Some(id), Some(instance), false) = (
            &pending.session,
            &pending.instance,
            matches!(next, Next::Complete(_)),
        ) {
            if self
                .sessions
                .get(id)
                .is_none_or(|s| &s.instance != instance)
            {
                let result = self.ended_result(op_id, None);
                self.complete(op_id, result);
                return;
            }
        }
        match next {
            Next::Complete(result) => self.complete(op_id, result),
            Next::MetaWrite => {
                let Some(session) = session else { return };
                let Op::UpdateMetadata { labels, .. } = self.ops[&op_id].op.clone() else {
                    return;
                };
                let state = self.sessions[&session].recorded_state();
                let mut row = self.row_of(&session, state);
                row.labels = labels;
                self.write_row(Owner::Op(op_id), &row);
                self.set_step(op_id, Step::Await(Wait::Ticket));
            }
            Next::PolicyWrite => {
                let Some(session) = session else { return };
                let Op::SetNotificationPolicy { policy, .. } = self.ops[&op_id].op.clone() else {
                    return;
                };
                let state = self.sessions[&session].recorded_state();
                let mut row = self.row_of(&session, state);
                row.request.notification_policy = Some(policy);
                self.write_row(Owner::Op(op_id), &row);
                self.set_step(op_id, Step::Await(Wait::Ticket));
            }
            Next::Forward => self.forward(op_id),
            Next::Setter => {
                let Some(session) = session else { return };
                let op = self.ops[&op_id].op.clone();
                let s = self.sessions.get_mut(&session).expect("checked above");
                s.pending_setters = s.pending_setters.saturating_sub(1);
                let output = match op {
                    Op::Resize { size, .. } => {
                        s.size = size;
                        s.request.size = size;
                        OpOutput::Resize(ResizeResult::Applied { actual: size })
                    }
                    Op::SetSizePolicy { policy, .. } => {
                        s.request.size_policy = Some(policy);
                        OpOutput::Unit
                    }
                    Op::SetColorProfile { profile, .. } => {
                        s.request.color_profile = Some(profile);
                        OpOutput::Unit
                    }
                    _ => OpOutput::Unit,
                };
                self.complete(op_id, OpResult::Ok(output));
            }
            Next::StopAllStart(targets) => self.stop_all_start(op_id, targets),
            Next::Detach => {
                let Op::Detach { route, reason } = self.ops[&op_id].op.clone() else {
                    return;
                };
                let session = self.routes.get(&route).map(|r| r.session.clone());
                match session {
                    Some(session) if self.send_msg(&session, HostMsg::Detach { route, reason }) => {
                        self.set_step(op_id, Step::Await(Wait::Route(route)));
                    }
                    _ => self.set_step(op_id, Step::Ready(Next::DetachLocal)),
                }
            }
            Next::DetachLocal => {
                let Op::Detach { route, reason } = self.ops[&op_id].op.clone() else {
                    return;
                };
                let reason = match reason {
                    DetachReason::Replaced => RouteCloseReason::Replaced,
                    DetachReason::Revoked => RouteCloseReason::Revoked,
                    // `Detached`; the enum is non-exhaustive, so a later reason closes the route as a plain detach.
                    _ => RouteCloseReason::Detached,
                };
                // `RouteClosed` first, then the completion in a step of its own (DP-7).
                if self.close_route(route, reason) {
                    self.complete_later(op_id, OpResult::Ok(OpOutput::Unit));
                }
            }
            Next::AdoptRead => {
                let ticket = self.ticket(Owner::Op(op_id));
                self.act(Action::ReadRows {
                    ticket,
                    prefix: crate::session::ROW_PREFIX.to_string(),
                });
                self.set_step(op_id, Step::Await(Wait::Ticket));
            }
            Next::AdoptRow => self.adopt_next_row(op_id),
        }
    }

    /// Sends an op to the worker, waits for the launch, or ends it (A2-1, IN-7).
    fn forward(&mut self, op_id: OpId) {
        let Some(session_id) = self.ops[&op_id].session.clone() else {
            return;
        };
        let is_write = matches!(self.ops[&op_id].op, Op::WriteInput { .. });
        let (shown, link, launched, failed, flow_busy) = {
            let s = &self.sessions[&session_id];
            (
                s.shown,
                s.worker.link,
                s.terminal.is_some(),
                s.worker.link_failed,
                matches!(s.flow, Flow::Start(_)) || s.admit == Admit::Starting,
            )
        };
        // A write to an `Exited` or `Lost` session is a certain zero (IN-7, A2-2).
        if is_write && matches!(shown, Some(SessionState::Exited(_) | SessionState::Lost(_))) {
            self.complete(
                op_id,
                OpResult::Ok(OpOutput::Input(InputResult {
                    outcome: WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
                    payload_bytes_written: 0,
                    pty_bytes_written: 0,
                    detail: "the session ended".into(),
                })),
            );
            return;
        }
        if let (Some(_link), true, false) = (link, launched, failed) {
            let req = self.next_req;
            self.next_req += 1;
            let op = self.ops[&op_id].op.clone();
            self.send_msg(&session_id, HostMsg::Op { req, op });
            self.sessions
                .get_mut(&session_id)
                .expect("checked")
                .inflight
                .insert(req, op_id);
            if let Some(p) = self.ops.get_mut(&op_id) {
                p.req = Some(req);
            }
            self.set_step(op_id, Step::Await(Wait::Worker));
        } else if flow_busy && !failed {
            self.set_step(op_id, Step::Await(Wait::Launch));
        } else if is_write {
            self.complete(
                op_id,
                OpResult::Ok(OpOutput::Input(InputResult {
                    outcome: WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
                    payload_bytes_written: 0,
                    pty_bytes_written: 0,
                    detail: "the session has no worker link".into(),
                })),
            );
        } else {
            self.complete(
                op_id,
                OpResult::Err(CoreError::new(
                    ErrorCode::WorkerLinkFailed,
                    "the session has no worker link",
                )),
            );
        }
    }

    /// Re-runs the ops that waited for the start of a session to end (they forward, or fail, now).
    pub(crate) fn wake_launch_waiters(&mut self, session: &SessionId) {
        let waiting: Vec<OpId> = self
            .ops
            .iter()
            .filter(|(_, p)| {
                p.session.as_ref() == Some(session) && matches!(p.step, Step::Await(Wait::Launch))
            })
            .map(|(id, _)| *id)
            .collect();
        for op in waiting {
            self.set_step(op, Step::Ready(Next::Forward));
        }
    }

    // ---- StopAll (LC-12) ----

    fn stop_all_start(&mut self, op_id: OpId, targets: BTreeSet<SessionId>) {
        let mut waiting = BTreeSet::new();
        for id in targets {
            match self.sessions.get(&id).map(|s| s.admit) {
                Some(Admit::Running | Admit::Starting) => {
                    self.request_stop(&id);
                    waiting.insert(id);
                }
                Some(Admit::Stopping) => {
                    waiting.insert(id);
                }
                // `Created`, `Exited` and `Lost` targets are left as they are (LC-12).
                _ => {}
            }
        }
        self.set_step(op_id, Step::Await(Wait::StopAll(waiting)));
    }

    /// True when every target of a `StopAll` reached `Exited` or `Lost`, or is gone (LC-12). The op is then ready work, so
    /// its completion is a step of its own (`pump_events` bounds events, not steps that complete several ops).
    pub(crate) fn stop_all_done(&self, targets: &BTreeSet<SessionId>) -> bool {
        targets.iter().all(|t| {
            self.sessions.get(t).is_none_or(|s| {
                matches!(
                    s.shown,
                    Some(SessionState::Exited(_) | SessionState::Lost(_))
                )
            })
        })
    }

    /// Whether a `StopAll` waits for this session to end (R-16).
    pub(crate) fn stop_all_targets(&self, session: &SessionId) -> bool {
        self.ops
            .values()
            .any(|p| matches!(&p.step, Step::Await(Wait::StopAll(t)) if t.contains(session)))
    }

    /// The result of an op whose instance ended before the op could finish (AM-3, IN-7): each op ends through a code of its own
    /// A2-1 row. `failed_create` is the failure of the `Create` that the op was admitted behind, for the rows that list
    /// `RegistryFailed`.
    pub(crate) fn ended_result(&self, op: OpId, failed_create: Option<&CoreError>) -> OpResult {
        let Some(pending) = self.ops.get(&op) else {
            return OpResult::Err(CoreError::new(ErrorCode::Internal, "the op is gone"));
        };
        let err = |code| {
            OpResult::Err(CoreError::new(
                code,
                "the session ended before the operation finished",
            ))
        };
        match &pending.op {
            // A write that was sent and not acknowledged is `Unknown`; one that was never sent is a certain zero (IN-7).
            Op::WriteInput { .. } => OpResult::Ok(OpOutput::Input(InputResult {
                outcome: if pending.req.is_some() {
                    WriteOutcome::Unknown {
                        max_payload_bytes: pending.held_bytes,
                    }
                } else {
                    WriteOutcome::NotWritten(NotWrittenReason::SessionEnded)
                },
                payload_bytes_written: 0,
                pty_bytes_written: 0,
                detail: "the session ended".into(),
            })),
            Op::Start { .. } | Op::Remove { .. } | Op::UpdateMetadata { .. } => match failed_create
            {
                Some(e) => OpResult::Err(e.clone()),
                None => err(ErrorCode::RegistryFailed { uncertain: false }),
            },
            // The registry row path is only the one of a `Created` session (A2-1); a worker path ends `WorkerLinkFailed`.
            Op::SetNotificationPolicy { .. } if pending.created_path => match failed_create {
                Some(e) => OpResult::Err(e.clone()),
                None => err(ErrorCode::RegistryFailed { uncertain: false }),
            },
            Op::Detach { .. } => OpResult::Ok(OpOutput::Unit),
            Op::Resize { .. } | Op::SetSizePolicy { .. } | Op::Signal { .. } => {
                err(ErrorCode::SessionEnded)
            }
            _ => err(ErrorCode::WorkerLinkFailed),
        }
    }

    // ---- routes ----

    /// Closes a route: posts `RouteClosed`, retires its keyed events, and completes a `Detach` that waits for it (DP-7).
    /// Returns false when the queue has no room: the close then waits (EV-5b).
    pub(crate) fn close_route(&mut self, route: RouteId, reason: RouteCloseReason) -> bool {
        let Some(entry) = self.routes.get(&route) else {
            return true;
        };
        let event = Event::RouteClosed {
            route,
            reason,
            route_tag: entry.route_tag.clone(),
        };
        if self.queue.post_mandatory(event).is_err() {
            return false;
        }
        let entry = self.routes.remove(&route).expect("read above");
        if let Some(s) = self.sessions.get_mut(&entry.session) {
            s.routes.remove(&route);
        }
        self.queue.retire_route(route);
        let waiting: Vec<OpId> = self
            .ops
            .iter()
            .filter(|(_, p)| matches!(p.step, Step::Await(Wait::Route(r)) if r == route))
            .map(|(id, _)| *id)
            .collect();
        for op in waiting {
            self.complete_later(op, OpResult::Ok(OpOutput::Unit));
        }
        true
    }

    // ---- AdoptAll (AD-1, LC-11) ----

    /// Whether the next row of an `AdoptAll` names a session of this handle whose `Created` is not shown yet.
    fn row_waits_for_its_session(&self, pending: &crate::engine::PendingOp) -> bool {
        pending
            .rows
            .front()
            .and_then(|(key, _)| key.strip_prefix(ROW_PREFIX))
            .and_then(|id| self.sessions.get(&SessionId(id.into())))
            .is_some_and(|s| s.shown.is_none())
    }

    /// Recovers the next row, in a step of its own, so that each row posts at most one event (EV-5b, 9B).
    fn adopt_next_row(&mut self, op_id: OpId) {
        let Some((key, bytes)) = self.ops.get_mut(&op_id).and_then(|p| p.rows.pop_front()) else {
            self.complete(op_id, OpResult::Ok(OpOutput::Unit));
            return;
        };
        if let Some(id) = key.strip_prefix(ROW_PREFIX) {
            self.adopt_row(op_id, SessionId(id.into()), &bytes);
        }
        let more = self.ops.get(&op_id).is_some_and(|p| !p.rows.is_empty());
        if more {
            self.set_step(op_id, Step::Ready(Next::AdoptRow));
        } else if self.adopting_rows(op_id) {
            // LC-11: the rows whose adoption runs post their states first. They do not block each other.
            self.set_step(op_id, Step::Await(Wait::Adopt));
        } else {
            self.complete(op_id, OpResult::Ok(OpOutput::Unit));
        }
    }

    /// AD-1: recovers the row of `id` as a session of this handle (DESIGN.md "Adoption (P5)" 5). A row that Core's decoder
    /// rejects is `Lost(RegistryCorrupt)` (AD-2, A10-2): it keeps its id in use until `Remove` (AD-2). A row that names a
    /// live worker begins its adoption, which posts the row's state when it ends; every other row posts its state now. Each
    /// row posts one `SessionState` (LC-11).
    ///
    /// A session that this handle holds already made the row itself: it keeps its instance and its state, and the row posts
    /// that state (LC-11: one `SessionState` for every row). A row whose session is still being created waits until the
    /// session's `Created` is shown (`row_waits_for_its_session`).
    fn adopt_row(&mut self, op: OpId, id: SessionId, bytes: &[u8]) {
        if let Some(session) = self.sessions.get(&id) {
            // `row_waits_for_its_session` holds this step until the session's state is shown.
            if let Some(state) = session.shown {
                self.post_state(&id, state);
            }
            return;
        }
        self.unadopted.remove(&id);
        let (session, recovery) = match Row::decode(&id, bytes) {
            Some(row) => self.session_of_row(row),
            None => {
                let instance = self.mint_instance();
                let session =
                    crate::engine::new_session(id.clone(), instance, unknown_request(), Flow::Idle);
                (
                    session,
                    Recovery::Post(SessionState::Lost(LostReason::RegistryCorrupt)),
                )
            }
        };
        self.sessions.insert(id.clone(), session);
        let state = match recovery {
            Recovery::Adopt(recorded) => {
                self.begin_adoption(op, &id, Some(recorded));
                return;
            }
            Recovery::Post(state) => state,
        };
        if let Some(s) = self.sessions.get_mut(&id) {
            s.admit = match state {
                SessionState::Created => Admit::Created,
                _ => Admit::Lost,
            };
        }
        // `AdoptRow` runs only with mandatory room (`step_needs_room`), so the state is posted.
        if self.post_state(&id, state) {
            if let Some(s) = self.sessions.get_mut(&id) {
                s.shown = Some(state);
            }
        }
    }

    /// The session of a row that decodes, and how it is recovered (AD-1; DESIGN.md "Adoption (P5)" 5). The checks of the
    /// row come before any probe or connect.
    fn session_of_row(&mut self, row: Row) -> (crate::session::Session, Recovery) {
        let mut session = crate::engine::new_session(
            row.id.clone(),
            row.instance.clone(),
            row.request.clone(),
            Flow::Idle,
        );
        session.labels = row.labels.clone();
        session.token = row
            .token
            .as_deref()
            .and_then(botster_core_link::proof::token_from_hex);
        session.worker_protocol = row.worker_protocol;
        session.worker_features = row.worker_features.clone();
        session.worker.identity = row.worker.map(|w| w.identity());
        session.payload = row.payload.map(|p| p.identity());
        // A10-1, AD-6 (review A2-F1): a `WorkerGone` row records a worker that is gone, or a process that the AD-6 check
        // refused. No later host probes or signals that identity; only an adoption proves a worker again.
        session.worker.gone = row.state == SessionState::Lost(LostReason::WorkerGone);
        let names_worker = session.worker.identity.is_some() && session.token.is_some();
        let recovery = match row.state {
            // AD-1: no process exists.
            SessionState::Created => Recovery::Post(SessionState::Created),
            // Core never writes `Lost(Other)` (R-35 correction `c3ed727`), so such a row is a corrupt record.
            SessionState::Lost(LostReason::Other) => {
                Recovery::Post(SessionState::Lost(LostReason::RegistryCorrupt))
            }
            // A worker identity with no valid token cannot be authenticated (AD-6): a corrupt record, as for the other
            // states below (review P5-F26).
            SessionState::Lost(LostReason::WorkerUnreachable | LostReason::WorkerVersion)
                if session.worker.identity.is_some() && session.token.is_none() =>
            {
                Recovery::Post(SessionState::Lost(LostReason::RegistryCorrupt))
            }
            // The row recorded the end. `Adopt(id)` may retry a `Lost(WorkerUnreachable)` or `Lost(WorkerVersion)` row
            // with the worker's identity that it keeps (steward ruling R-36, contracts `main` `c62085f`).
            SessionState::Lost(reason) => Recovery::Post(SessionState::Lost(reason)),
            // AD-1: a start that never recorded its worker.
            SessionState::Starting if session.worker.identity.is_none() => {
                Recovery::Post(SessionState::Lost(LostReason::StartInterrupted))
            }
            state @ (SessionState::Starting
            | SessionState::Running
            | SessionState::Stopping
            | SessionState::Exited(_))
                if names_worker =>
            {
                Recovery::Adopt(state)
            }
            // A row that Core's encoder writes never has these contents (AD-7): the bytes decode, and the record is corrupt
            // (AD-2; R-35 correction `c3ed727`). Core never posts `Lost(Other)`.
            _ => Recovery::Post(SessionState::Lost(LostReason::RegistryCorrupt)),
        };
        // LC-9 (review A2-F2): this host reads no hello of a posted `Lost(WorkerUnreachable)` row's worker, so `get` shows no
        // protocol. The row keeps the one that it recorded (R-36).
        if matches!(
            recovery,
            Recovery::Post(SessionState::Lost(LostReason::WorkerUnreachable))
        ) {
            session.row_protocol = session.worker_protocol.take();
        }
        (session, recovery)
    }

    /// The launch request that a worker gets (A3-1, DP-6: the policies come from the row).
    pub(crate) fn launch_spec(&self, id: &SessionId) -> LaunchSpec {
        let s = &self.sessions[id];
        LaunchSpec {
            argv: s.request.argv.clone(),
            env: s.request.env.clone(),
            cwd: s.request.cwd.clone(),
            size: s.size,
            color_profile: s.request.color_profile.clone(),
            notification_policy: s.request.notification_policy.clone().unwrap_or_default(),
            size_policy: s.request.size_policy.unwrap_or_default(),
            link_frame_bound: self.link_frame_bound(),
            stop_grace_ms: u64::try_from(self.cfg.limits.stop_grace.as_millis())
                .unwrap_or(u64::MAX),
            limits: self.cfg.limits.clone(),
        }
    }
}
