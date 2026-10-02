//! Ready work and its execution: the steps of the operations, and the deadlines (plan 2.4, TM-3, TM-5, TM-6).
//!
//! `ready` lists what a `pump` could do now, in the order that the production policy runs it. A step that posts a mandatory
//! event is not ready while the queue has no room: it is parked, it keeps `PumpReport.more` false, and a poll that frees
//! room makes it ready again (EV-5b, EV-5d, TM-6).

use crate::engine::{HostEngine, Next, Owner, Step, Wait};
use crate::flow::*;
use crate::io::{Action, Work};
use crate::session::{Admit, Row};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{GroupSignal, StorageError};
use botster_core_link::msg::{HostMsg, LaunchSpec};
use std::collections::BTreeSet;
use std::time::Instant;

/// A deadline that the engine owns (TM-3).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum DeadlineKind {
    Startup(SessionId),
    StopGrace(SessionId),
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
            Flow::Remove(f) => f.phase == RemovePhase::AwaitResult,
        }
    }

    /// The work that a `pump` could do now (plan 2.4). Index 0 is the work that the production policy runs first.
    pub fn ready(&self) -> Vec<Work> {
        let room = self.has_room();
        let mut out = Vec::new();
        for (id, p) in &self.ops {
            if let Step::Ready(next) = &p.step {
                if !self.step_needs_room(next) || room {
                    out.push(Work::Op(*id));
                }
            }
        }
        for (id, s) in &self.sessions {
            if s.ticket.is_some() || self.flow_waiting(s) {
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
        if let (Some(now), Some((due, _))) = (self.now, self.deadlines().first()) {
            if *due <= now {
                out.push(Work::Deadline);
            }
        }
        out
    }

    /// Runs one piece of ready work (plan 2.4).
    pub(crate) fn run(&mut self, work: Work) {
        match work {
            Work::Op(op) => self.run_op(op),
            Work::Session(id) => self.run_flow(&id),
            Work::Deadline => self.run_deadline(),
            Work::Parked => self.run_parked(),
        }
    }

    fn run_deadline(&mut self) {
        let Some(now) = self.now else { return };
        let Some((at, kind)) = self.deadlines().into_iter().next() else {
            return;
        };
        if at > now {
            return;
        }
        match kind {
            DeadlineKind::Capture(id) => {
                self.captures.remove(&id);
            }
            DeadlineKind::Silence(id) => {
                let unix = self.unix;
                if let Some(s) = self.sessions.get_mut(&id) {
                    s.silence.fired = true;
                    let since = s.silence.last_output.map_or(unix, |(_, u)| u);
                    let event = Event::Silent {
                        id: s.id.clone(),
                        instance: s.instance.clone(),
                        since,
                    };
                    self.queue.post_keyed(event);
                }
            }
            DeadlineKind::StopGrace(id) => self.kill_payload(&id),
            DeadlineKind::Startup(id) => self.startup_expired(&id),
        }
    }

    /// The kill of `stop_grace` (LC-5): through the link, or through the process edge when the link is gone.
    fn kill_payload(&mut self, id: &SessionId) {
        let identity = self.identity_of(id);
        let sent = self.send_msg(id, HostMsg::Kill);
        if !sent {
            if let Some(identity) = identity {
                self.act(Action::SignalGroup {
                    identity,
                    signal: GroupSignal::Kill,
                });
            }
        }
        if let Some(s) = self.sessions.get_mut(id) {
            s.killed = true;
            if let Flow::Stop(f) = &mut s.flow {
                f.deadline = None;
            }
        }
    }

    pub(crate) fn row_of(&self, id: &SessionId, state: SessionState) -> Row {
        let mut row = self.sessions[id].to_row();
        row.state = state;
        row
    }

    fn write_row(&mut self, owner: Owner, row: &Row) {
        let ticket = self.ticket(owner);
        self.act(Action::WriteRow {
            ticket,
            key: crate::session::row_key(&row.id),
            bytes: serde_json::to_vec(row).expect("a row is JSON"),
        });
    }

    /// A best-effort row write whose result nobody needs (the final state of a session).
    pub(crate) fn write_row_ignored(&mut self, id: &SessionId, state: SessionState) {
        let Some(s) = self.sessions.get(id) else {
            return;
        };
        let mut row = s.to_row();
        row.state = state;
        let ticket = self.ticket(Owner::Ignored);
        self.act(Action::WriteRow {
            ticket,
            key: crate::session::row_key(id),
            bytes: serde_json::to_vec(&row).expect("a row is JSON"),
        });
    }

    // ---- the steps of the operations ----

    pub(crate) fn run_op(&mut self, op_id: OpId) {
        let Some(pending) = self.ops.get(&op_id) else {
            return;
        };
        let Step::Ready(next) = pending.step.clone() else {
            return;
        };
        let session = pending.session.clone();
        match next {
            Next::Complete(result) => self.complete(op_id, result),
            Next::MetaWrite => {
                let Some(session) = session else { return };
                let Op::UpdateMetadata { labels, .. } = self.ops[&op_id].op.clone() else {
                    return;
                };
                let state = self.sessions[&session]
                    .shown
                    .unwrap_or(SessionState::Created);
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
                let state = self.sessions[&session]
                    .shown
                    .unwrap_or(SessionState::Created);
                let mut row = self.row_of(&session, state);
                row.request.notification_policy = Some(policy);
                self.write_row(Owner::Op(op_id), &row);
                self.set_step(op_id, Step::Await(Wait::Ticket));
            }
            Next::Forward => self.forward(op_id),
            Next::Signal(sig) => {
                let Some(session) = session else { return };
                let result = if self.send_msg(&session, HostMsg::Signal { sig }) {
                    OpResult::Ok(OpOutput::Unit)
                } else {
                    OpResult::Err(CoreError::new(
                        ErrorCode::WorkerLinkFailed,
                        "the session has no worker link",
                    ))
                };
                self.complete(op_id, result);
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
                    DetachReason::Detached => RouteCloseReason::Detached,
                    DetachReason::Replaced => RouteCloseReason::Replaced,
                    DetachReason::Revoked => RouteCloseReason::Revoked,
                    _ => RouteCloseReason::Detached,
                };
                if self.close_route(route, reason) {
                    self.complete(op_id, OpResult::Ok(OpOutput::Unit));
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
            let Some(s) = self.sessions.get_mut(&id) else {
                continue;
            };
            match s.admit {
                Admit::Running | Admit::Starting => {
                    s.admit = Admit::Stopping;
                    match s.flow {
                        // An exit is being posted, or the start has not ended: the stop follows (LC-12).
                        Flow::Stop(_) => {}
                        Flow::Start(_) | Flow::Create(_) => s.stop_after_start = true,
                        _ if s.queue.iter().any(|f| matches!(f, Flow::Start(_))) => {
                            s.stop_after_start = true;
                        }
                        _ => {
                            s.host_ended = true;
                            s.flow = Flow::Stop(StopFlow {
                                phase: StopPhase::RowWrite,
                                deadline: None,
                                end: None,
                            });
                        }
                    }
                    waiting.insert(id);
                }
                Admit::Stopping => {
                    waiting.insert(id);
                }
                // `Created`, `Exited` and `Lost` targets are left as they are (LC-12).
                _ => {}
            }
        }
        self.set_step(op_id, Step::Await(Wait::StopAll(waiting)));
        self.check_stop_alls();
    }

    /// Completes the `StopAll` ops whose targets have all reached `Exited` or `Lost`, or are gone (LC-12).
    pub(crate) fn check_stop_alls(&mut self) {
        let done: Vec<OpId> = self
            .ops
            .iter()
            .filter_map(|(id, p)| match &p.step {
                Step::Await(Wait::StopAll(targets)) => targets
                    .iter()
                    .all(|t| {
                        self.sessions.get(t).is_none_or(|s| {
                            matches!(
                                s.shown,
                                Some(SessionState::Exited(_) | SessionState::Lost(_))
                            )
                        })
                    })
                    .then_some(*id),
                _ => None,
            })
            .collect();
        for op in done {
            self.complete(op, OpResult::Ok(OpOutput::Unit));
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
            self.complete(op, OpResult::Ok(OpOutput::Unit));
        }
        true
    }

    // ---- AdoptAll (AD-1, LC-11): the shell; the recovery of live workers is the adoption package's (P5) ----

    fn adopt_next_row(&mut self, op_id: OpId) {
        let Some((_key, bytes)) = self.ops.get_mut(&op_id).and_then(|p| p.rows.pop_front()) else {
            self.complete(op_id, OpResult::Ok(OpOutput::Unit));
            return;
        };
        if let Ok(row) = serde_json::from_slice::<Row>(&bytes) {
            if row.version == crate::session::ROW_VERSION && !self.sessions.contains_key(&row.id) {
                let state = match row.state {
                    SessionState::Created => SessionState::Created,
                    // A row that names a worker needs the adoption handshake (AD-6), which P5 builds. Until then Core
                    // cannot say what happened to the payload.
                    _ => SessionState::Lost(LostReason::Other),
                };
                let id = row.id.clone();
                let instance = row.instance.clone();
                let mut session = crate::engine::new_session(
                    id.clone(),
                    instance.clone(),
                    row.request.clone(),
                    Flow::Idle,
                );
                session.labels = row.labels.clone();
                session.token = row
                    .token
                    .as_deref()
                    .and_then(crate::session::token_from_hex);
                session.worker_protocol = row.worker_protocol;
                session.worker_features = row.worker_features.clone();
                session.worker.identity = row.worker.map(|w| w.identity());
                session.admit = match state {
                    SessionState::Created => Admit::Created,
                    _ => Admit::Lost,
                };
                self.sessions.insert(id.clone(), session);
                let _ = self.post_state(&id, state);
                if let Some(s) = self.sessions.get_mut(&id) {
                    s.shown = Some(state);
                }
            }
        }
        // The next row is a step of its own, so that each row posts at most one event (EV-5b).
        let more = self.ops.get(&op_id).is_some_and(|p| !p.rows.is_empty());
        if more {
            self.set_step(op_id, Step::Ready(Next::AdoptRow));
        } else {
            self.complete(op_id, OpResult::Ok(OpOutput::Unit));
        }
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
        }
    }
}
