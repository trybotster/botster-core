//! The flows of a session: create, start (AD-7), stop (LC-5) and remove (LC-7). See [`crate::flow`].

use crate::engine::{HostEngine, Next, Owner, Step};
use crate::flow::*;
use crate::io::Action;
use crate::run::registry_failed;
use crate::session::Admit;
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{GroupSignal, IdentityState, ProcessIdentity, StorageError};
use botster_core_link::msg::HostMsg;

/// `RemoveReport` is `#[non_exhaustive]` and has no constructor in `contracts-v0.1.2`, so an external crate builds it through
/// its own JSON form. The result is the value that the contract defines (A6-3); a constructor in the next tag removes this.
fn remove_report(uploads: UploadsOutcome) -> RemoveReport {
    serde_json::from_value(serde_json::json!({ "uploads": uploads }))
        .expect("a RemoveReport is its uploads outcome")
}

fn failed_start_exit() -> Exit {
    // The payload never ran: no code and no signal. `Other` is the cause that a host cannot confuse with a run.
    Exit {
        code: None,
        signal: None,
        cause: ExitCause::Other,
    }
}

impl HostEngine {
    fn start_flow(&mut self, id: &SessionId) -> Option<&mut StartFlow> {
        match &mut self.sessions.get_mut(id)?.flow {
            Flow::Start(f) => Some(f),
            _ => None,
        }
    }

    fn stop_flow(&mut self, id: &SessionId) -> Option<&mut StopFlow> {
        match &mut self.sessions.get_mut(id)?.flow {
            Flow::Stop(f) => Some(f),
            _ => None,
        }
    }

    /// Runs one step of the session's flow (plan 2.2). A step posts at most one event.
    pub(crate) fn run_flow(&mut self, id: &SessionId) {
        let Some(flow) = self.sessions.get(id).map(|s| s.flow.clone()) else {
            return;
        };
        if let Some(s) = self.sessions.get_mut(id) {
            if std::mem::take(&mut s.metadata_pending) {
                let event = Event::MetadataChanged {
                    id: s.id.clone(),
                    instance: s.instance.clone(),
                };
                self.queue.post_keyed(event);
                return;
            }
        }
        match flow {
            Flow::Idle => {}
            Flow::Create(f) => self.run_create(id, f),
            Flow::Start(f) => self.run_start(id, f),
            Flow::Stop(f) => self.run_stop(id, f),
            Flow::Remove(f) => self.run_remove(id, f),
        }
    }

    fn await_ticket(&mut self, id: &SessionId, ticket: crate::io::Ticket) {
        self.sessions
            .get_mut(id)
            .expect("a flow has a session")
            .ticket = Some(ticket);
    }

    fn write_session_row(&mut self, id: &SessionId, state: SessionState) {
        let row = self.row_of(id, state);
        let ticket = self.write_row(Owner::Session(id.clone()), &row);
        self.await_ticket(id, ticket);
    }

    // ---- create ----

    fn run_create(&mut self, id: &SessionId, f: CreateFlow) {
        match f.phase {
            CreatePhase::WriteRow => self.write_session_row(id, SessionState::Created),
            CreatePhase::PostCreated => {
                if self.post_state(id, SessionState::Created) {
                    let s = self.sessions.get_mut(id).expect("a flow has a session");
                    s.shown = Some(SessionState::Created);
                    s.flow = Flow::Create(CreateFlow {
                        phase: CreatePhase::Finish,
                        ..f
                    });
                }
            }
            CreatePhase::Finish => {
                let record = self.sessions[id].record().expect("Created was shown");
                self.flow_done(id);
                self.complete(f.op, OpResult::Ok(OpOutput::Record(record)));
            }
        }
    }

    // ---- start (AD-7) ----

    fn set_start_phase(&mut self, id: &SessionId, phase: StartPhase) {
        if let Some(f) = self.start_flow(id) {
            f.phase = phase;
        }
    }

    /// Ends the start with `reason`, in the state `state` (LC-4: `Exited` or `Lost`, never `Starting`).
    pub(crate) fn fail_start(
        &mut self,
        id: &SessionId,
        reason: StartFailReason,
        state: SessionState,
    ) {
        if let Some(f) = self.start_flow(id) {
            f.failure = Some(StartFailure { reason, state });
            f.deadline = None;
            f.phase = StartPhase::PostFailed;
        }
    }

    /// The `startup` deadline passed before the payload ran (LC-4).
    pub(crate) fn startup_expired(&mut self, id: &SessionId) {
        if let Some(identity) = self.identity_of(id) {
            self.act(Action::SignalGroup {
                identity,
                signal: GroupSignal::Kill,
            });
        }
        self.close_worker_link(id);
        self.fail_start(
            id,
            StartFailReason::StartupTimeout,
            SessionState::Exited(failed_start_exit()),
        );
    }

    pub(crate) fn close_worker_link(&mut self, id: &SessionId) {
        if let Some(s) = self.sessions.get_mut(id) {
            if let Some(link) = s.worker.link.take() {
                self.links.remove(&link);
                self.act(Action::CloseLink { link });
            }
        }
    }

    fn run_start(&mut self, id: &SessionId, f: StartFlow) {
        match f.phase {
            StartPhase::Token => {
                let ticket = self.ticket(Owner::Session(id.clone()));
                self.act(Action::Random {
                    ticket,
                    len: botster_core_link::proof::TOKEN_LEN,
                });
                self.await_ticket(id, ticket);
            }
            StartPhase::RowStarting => self.write_session_row(id, SessionState::Starting),
            StartPhase::PostStarting => {
                if self.post_state(id, SessionState::Starting) {
                    self.sessions
                        .get_mut(id)
                        .expect("a flow has a session")
                        .shown = Some(SessionState::Starting);
                    self.set_start_phase(id, StartPhase::Spawn);
                }
            }
            StartPhase::Spawn => {
                let ticket = self.ticket(Owner::Session(id.clone()));
                let s = &self.sessions[id];
                self.act(Action::SpawnWorker {
                    ticket,
                    program: self.cfg.worker_path.clone(),
                    instance: s.instance.clone(),
                    token: s.token.expect("the token was drawn"),
                    host_epoch: self.cfg.host_epoch,
                });
                self.await_ticket(id, ticket);
            }
            StartPhase::RowIdentity => self.write_session_row(id, SessionState::Starting),
            StartPhase::SendLaunch => {
                let spec = self.launch_spec(id);
                if self.send_msg(id, HostMsg::Launch(Box::new(spec))) {
                    self.set_start_phase(id, StartPhase::AwaitLaunched);
                } else {
                    self.fail_start(
                        id,
                        StartFailReason::WorkerFailed,
                        SessionState::Exited(failed_start_exit()),
                    );
                }
            }
            StartPhase::PostRunning => {
                if self.post_state(id, SessionState::Running) {
                    let s = self.sessions.get_mut(id).expect("a flow has a session");
                    s.shown = Some(SessionState::Running);
                    s.admit = if s.admit == Admit::Stopping {
                        Admit::Stopping
                    } else {
                        Admit::Running
                    };
                    self.set_start_phase(id, StartPhase::Finish);
                }
            }
            StartPhase::PostFailed => {
                let failure = f.failure.expect("a failed start has its failure");
                if self.post_state(id, failure.state) {
                    let s = self.sessions.get_mut(id).expect("a flow has a session");
                    s.shown = Some(failure.state);
                    match failure.state {
                        SessionState::Exited(exit) => {
                            s.exit = Some(exit);
                            s.admit = Admit::Exited;
                        }
                        _ => s.admit = Admit::Lost,
                    }
                    self.write_final_row(id, failure.state);
                    self.set_start_phase(id, StartPhase::Finish);
                }
            }
            StartPhase::Finish => self.finish_start(id, f),
            StartPhase::AwaitHello | StartPhase::AwaitLaunched => {}
        }
    }

    fn finish_start(&mut self, id: &SessionId, f: StartFlow) {
        let running = f.failure.is_none() && f.error.is_none();
        let result = if let Some(error) = f.error.clone() {
            OpResult::Err(error)
        } else if let Some(failure) = f.failure {
            OpResult::Err(CoreError::new(
                ErrorCode::StartFailed(failure.reason),
                format!("the session did not start: {:?}", failure.reason),
            ))
        } else {
            OpResult::Ok(OpOutput::Record(
                self.sessions[id].record().expect("Running was shown"),
            ))
        };
        self.flow_done(id);
        let (stop_after, pending_end) = {
            let s = self.sessions.get_mut(id).expect("a flow has a session");
            (
                std::mem::take(&mut s.stop_after_start),
                s.pending_end.take(),
            )
        };
        self.complete(f.op, result);
        if running {
            if let Some(end) = pending_end {
                // The payload ended while the start was finishing: the exit is applied now, after `Running` (OR-2).
                self.begin_end_flow(id, end);
            } else if stop_after {
                self.request_stop(id);
            }
        } else {
            // The start failed: a `Stop` that waited ends with the state that the failure reached.
            let end = self.session_end(id);
            let waiters = std::mem::take(&mut self.sessions.get_mut(id).expect("kept").waiters);
            for op in waiters {
                self.complete_later(op, OpResult::Ok(OpOutput::End(end)));
            }
        }
        self.wake_launch_waiters(id);
    }

    // ---- stop (LC-5) ----

    /// The one entry of a stop (LC-5, LC-12), for `Stop`, `StopAll` and a stop that waited for its start. The session is
    /// `Stopping` from here (AM-1). A stop joins an end that is being posted; it waits for a start or a create that is still
    /// running (`stop_after_start`); otherwise it begins now. The waiters (`Stop` ops) are the caller's.
    pub(crate) fn request_stop(&mut self, id: &SessionId) {
        let s = self.sessions.get_mut(id).expect("a stop has a session");
        s.admit = Admit::Stopping;
        match s.flow {
            Flow::Stop(_) => {}
            Flow::Start(_) | Flow::Create(_) => s.stop_after_start = true,
            _ => {
                s.host_ended = true;
                s.flow = Flow::Stop(StopFlow {
                    phase: StopPhase::RowWrite,
                    deadline: None,
                    end: None,
                });
            }
        }
    }

    /// The payload ended, or the worker was lost: the session reaches `end` (EV-4, AD-2).
    pub(crate) fn begin_end_flow(&mut self, id: &SessionId, end: SessionEnd) {
        let Some(s) = self.sessions.get_mut(id) else {
            return;
        };
        match &mut s.flow {
            Flow::Stop(f) => {
                f.end = Some(end);
                f.deadline = None;
                // `Stopping` may still wait for room: the end follows it (OR-2).
                if f.phase == StopPhase::AwaitExit {
                    f.phase = StopPhase::PostEnd;
                }
            }
            Flow::Start(_) => s.pending_end = Some(end),
            Flow::Idle => {
                s.flow = Flow::Stop(StopFlow {
                    phase: StopPhase::PostEnd,
                    deadline: None,
                    end: Some(end),
                });
            }
            // A session that is being created or removed has no payload that Core still waits for.
            _ => {}
        }
    }

    fn run_stop(&mut self, id: &SessionId, f: StopFlow) {
        match f.phase {
            StopPhase::RowWrite => self.write_session_row(id, SessionState::Stopping),
            // The graceful request goes out before the state event, so that a full queue never delays the effect of a stop
            // (EV-5c). Without a link the host asks the verified worker (pid and start time, AD-6) to end its payload (`GroupSignal::EndPayload`).
            StopPhase::SendStop => {
                if !self.send_msg(id, HostMsg::Stop) {
                    if let Some(identity) = self.identity_of(id) {
                        self.act(Action::SignalGroup {
                            identity,
                            signal: GroupSignal::EndPayload,
                        });
                    }
                }
                let deadline = self.mono().map(|now| now + self.cfg.limits.stop_grace);
                if let Some(f) = self.stop_flow(id) {
                    f.deadline = deadline;
                    f.phase = StopPhase::PostStopping;
                }
            }
            StopPhase::PostStopping => {
                if self.post_state(id, SessionState::Stopping) {
                    self.sessions
                        .get_mut(id)
                        .expect("a flow has a session")
                        .shown = Some(SessionState::Stopping);
                    if let Some(f) = self.stop_flow(id) {
                        f.phase = if f.end.is_some() {
                            StopPhase::PostEnd
                        } else {
                            StopPhase::AwaitExit
                        };
                    }
                }
            }
            StopPhase::PostEnd => {
                let end = f.end.expect("PostEnd has its end");
                let state = match end {
                    SessionEnd::Exited(exit) => SessionState::Exited(exit),
                    SessionEnd::Lost(reason) => SessionState::Lost(reason),
                    _ => SessionState::Lost(LostReason::Other),
                };
                if self.post_state(id, state) {
                    let s = self.sessions.get_mut(id).expect("a flow has a session");
                    s.shown = Some(state);
                    match end {
                        SessionEnd::Exited(exit) => {
                            s.exit = Some(exit);
                            s.admit = Admit::Exited;
                        }
                        _ => s.admit = Admit::Lost,
                    }
                    self.write_final_row(id, state);
                    if let Some(f) = self.stop_flow(id) {
                        f.phase = StopPhase::Finish;
                    }
                }
            }
            // One waiter per step, so that a step posts at most one event (9B `pump_events`).
            StopPhase::Finish => {
                let end = f.end.expect("Finish has its end");
                let waiters = &mut self
                    .sessions
                    .get_mut(id)
                    .expect("a flow has a session")
                    .waiters;
                let next = (!waiters.is_empty()).then(|| waiters.remove(0));
                match next {
                    Some(op) => self.complete(op, OpResult::Ok(OpOutput::End(end))),
                    None => {
                        self.flow_done(id);
                        self.fail_inflight(id);
                    }
                }
            }
            StopPhase::AwaitExit => {}
        }
    }

    /// The decision of the `ExitCause` (LC-5, LC-6, EV-4): the host knows what it asked for.
    pub(crate) fn exit_of(&self, id: &SessionId, code: Option<i32>, signal: Option<i32>) -> Exit {
        let s = &self.sessions[id];
        let cause = if s.killed {
            ExitCause::Killed
        } else if s.host_ended {
            ExitCause::HostStop
        } else if signal.is_some() {
            ExitCause::Signal
        } else {
            ExitCause::Normal
        };
        Exit {
            code,
            signal,
            cause,
        }
    }

    /// The link of a session that ended failed its pending ops (AM-3, IN-7): a write that was sent and not acknowledged is
    /// `Unknown`, and every other op is `WorkerLinkFailed`. Reads of an `Exited` session keep their worker.
    pub(crate) fn fail_inflight(&mut self, id: &SessionId) {
        let Some(s) = self.sessions.get(id) else {
            return;
        };
        let dead = s.worker.link.is_none();
        if !dead {
            return;
        }
        let inflight: Vec<(u64, OpId)> = s.inflight.iter().map(|(r, o)| (*r, *o)).collect();
        self.sessions
            .get_mut(id)
            .expect("read above")
            .inflight
            .clear();
        for (_, op) in inflight {
            let result = match self.ops.get(&op).map(|p| &p.op) {
                Some(Op::WriteInput { payload, .. }) => {
                    OpResult::Ok(OpOutput::Input(InputResult {
                        outcome: WriteOutcome::Unknown {
                            max_payload_bytes: Self::held_bytes(payload),
                        },
                        payload_bytes_written: 0,
                        pty_bytes_written: 0,
                        detail: "the worker link failed after the write was sent".into(),
                    }))
                }
                _ => OpResult::Err(CoreError::new(
                    ErrorCode::WorkerLinkFailed,
                    "the worker link ended with the operation pending",
                )),
            };
            self.complete_later(op, result);
        }
        self.wake_launch_waiters(id);
    }

    // ---- remove (LC-7) ----

    fn set_remove_phase(&mut self, id: &SessionId, phase: RemovePhase) {
        if let Some(s) = self.sessions.get_mut(id) {
            if let Flow::Remove(f) = &mut s.flow {
                f.phase = phase;
            }
        }
    }

    /// Completes every op of the instance that is still pending, except `keep` (AM-3, ID-1): the session is going, so no op
    /// may stay attached to it. `error` is the result of an op that has no better one (a failed `Create` passes its own).
    pub(crate) fn retire_session_ops(
        &mut self,
        id: &SessionId,
        keep: Option<OpId>,
        error: Option<CoreError>,
    ) {
        let Some(instance) = self.sessions.get(id).map(|s| s.instance.clone()) else {
            return;
        };
        let doomed: Vec<OpId> = self
            .ops
            .iter()
            .filter(|(op, p)| {
                Some(**op) != keep
                    && p.session.as_ref() == Some(id)
                    && p.instance.as_ref() == Some(&instance)
                    && !matches!(p.step, Step::Done)
            })
            .map(|(op, _)| *op)
            .collect();
        for op in doomed {
            let result = self.ended_result(op, error.as_ref());
            self.complete_later(op, result);
        }
        if let Some(s) = self.sessions.get_mut(id) {
            s.inflight.clear();
            s.waiters.clear();
            s.queue.clear();
        }
    }

    fn run_remove(&mut self, id: &SessionId, f: RemoveFlow) {
        match f.phase {
            // Steps 2 and 3 (R-15): they run once every bound route is closed, under pressure if need be.
            RemovePhase::SendRemove => {
                // Step 2: the open captures of the session are released, and no op stays attached to it.
                let instance = self.sessions[id].instance.clone();
                self.captures.retain(|_, c| c.instance != instance);
                self.retire_session_ops(id, Some(f.op), None);
                let (link, worker, gone, corrupt) = {
                    let s = &self.sessions[id];
                    (
                        s.worker.link.is_some(),
                        s.worker.identity,
                        s.worker.gone,
                        s.shown == Some(SessionState::Lost(LostReason::RegistryCorrupt)),
                    )
                };
                let deadline = self.mono().map(|now| now + self.cfg.limits.stop_grace);
                let (uploads, worker_gone, deadline) = if link && self.send_msg(id, HostMsg::Remove)
                {
                    (None, false, deadline)
                } else if let (Some(_), true) = (worker, gone) {
                    // The worker ended already: its cleanup result cannot come (A6-3).
                    (
                        Some(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)),
                        true,
                        None,
                    )
                } else if let Some(identity) = worker {
                    // A worker that cannot be asked may still run: a stray worker. The host checks its identity, ends it if it
                    // matches, and waits until it is gone (LC-7 step 3, A6-3, AD-6).
                    self.act(Action::ProbeIdentity { identity });
                    (
                        Some(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)),
                        false,
                        deadline,
                    )
                } else if corrupt {
                    // A corrupt row tells nothing of what ran, so nothing is claimed (A6-3).
                    (
                        Some(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown)),
                        true,
                        None,
                    )
                } else {
                    // A session that never had a worker has no uploads.
                    (Some(UploadsOutcome::Deleted), true, None)
                };
                if let Some(s) = self.sessions.get_mut(id) {
                    if let Flow::Remove(f) = &mut s.flow {
                        f.uploads = uploads;
                        f.worker_gone = worker_gone;
                        f.deadline = deadline;
                        f.phase = RemovePhase::AwaitTeardown;
                    }
                }
                self.remove_progress(id);
            }
            RemovePhase::CloseRoutes => {
                let next = self.sessions[id].routes.iter().next().copied();
                match next {
                    Some(route) => {
                        let _ = self.close_route(route, RouteCloseReason::SessionRemoved);
                    }
                    None => self.set_remove_phase(id, RemovePhase::SendRemove),
                }
            }
            RemovePhase::DeleteRow => {
                let ticket = self.ticket(Owner::Session(id.clone()));
                self.act(Action::DeleteRow {
                    ticket,
                    key: crate::session::row_key(id),
                });
                self.await_ticket(id, ticket);
            }
            RemovePhase::PostReleased => self.release_session(id, f),
            RemovePhase::AwaitTeardown | RemovePhase::Finish => {}
        }
    }

    /// Advances to step 4 when the cleanup result is known and the worker process ended (LC-7 step 3 before steps 4 and 5).
    pub(crate) fn remove_progress(&mut self, id: &SessionId) {
        if let Some(s) = self.sessions.get_mut(id) {
            if let Flow::Remove(f) = &mut s.flow {
                if f.phase == RemovePhase::AwaitTeardown && f.uploads.is_some() && f.worker_gone {
                    f.deadline = None;
                    f.phase = RemovePhase::DeleteRow;
                }
            }
        }
    }

    /// The worker did not end within `stop_grace` after its teardown. A signal is not an observed exit, so the host checks the
    /// worker's identity again: a worker that still matches is killed, and the check repeats each `stop_grace`; a worker
    /// that is gone lets steps 4 and 5 run (LC-7, A6-3, AD-6).
    pub(crate) fn remove_grace_expired(&mut self, id: &SessionId) {
        if let Some(identity) = self.identity_of(id) {
            self.act(Action::ProbeIdentity { identity });
        }
        let next = self.mono().map(|now| now + self.cfg.limits.stop_grace);
        if let Some(s) = self.sessions.get_mut(id) {
            if let Flow::Remove(f) = &mut s.flow {
                f.deadline = next;
                if f.uploads.is_none() {
                    // The result can no longer be trusted to come (A6-3).
                    f.uploads = Some(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown));
                }
            }
        }
        self.remove_progress(id);
    }

    /// The answer to an identity check of a removed session's worker (AD-6). A worker that matches is killed; one that is
    /// absent, or whose pid another process now has, is gone (AD-2 `WorkerGone`), and the teardown goes on.
    pub(crate) fn flow_remove_probed(&mut self, identity: ProcessIdentity, state: IdentityState) {
        let found = self
            .sessions
            .iter()
            .find(|(_, s)| {
                s.worker.identity == Some(identity)
                    && !s.worker.gone
                    && matches!(s.flow, Flow::Remove(_))
            })
            .map(|(id, _)| id.clone());
        let Some(id) = found else {
            return;
        };
        match state {
            IdentityState::Matches => self.act(Action::SignalGroup {
                identity,
                signal: GroupSignal::Kill,
            }),
            IdentityState::Absent | IdentityState::Reused => {
                self.sessions.get_mut(&id).expect("found above").worker.gone = true;
                self.flow_remove_worker_gone(&id);
            }
        }
    }

    /// Step 5 of LC-7 and `SessionState{Released}`, as one atomic step: the id is freed only when the event fits (EV-5b).
    fn release_session(&mut self, id: &SessionId, f: RemoveFlow) {
        let s = &self.sessions[id];
        let event = Event::SessionState {
            id: s.id.clone(),
            instance: s.instance.clone(),
            state: SessionState::Released,
        };
        let instance = s.instance.clone();
        // The events of the instance are retired first, so that only its class M events remain (6.2).
        if !self.has_room() {
            return;
        }
        self.queue.retire_instance(&instance);
        self.queue
            .post_mandatory(event)
            .expect("the room was checked above");
        let mut session = self.sessions.remove(id).expect("a flow has a session");
        if let Some(link) = session.worker.link.take() {
            self.links.remove(&link);
            self.act(Action::CloseLink { link });
        }
        self.retired_ops.extend(&session.ops);
        let uploads = f
            .uploads
            .unwrap_or(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown));
        // The completion is the next step: one event per step (9B `pump_events`).
        self.set_step(
            f.op,
            Step::Ready(Next::Complete(OpResult::Ok(OpOutput::RemoveReport(
                remove_report(uploads),
            )))),
        );
    }

    // ---- results of the edges for a flow ----

    pub(crate) fn flow_random(&mut self, id: &SessionId, bytes: Vec<u8>) {
        let Some(s) = self.sessions.get_mut(id) else {
            return;
        };
        let mut token = [0u8; botster_core_link::proof::TOKEN_LEN];
        let n = token.len().min(bytes.len());
        token[..n].copy_from_slice(&bytes[..n]);
        s.token = Some(token);
        self.set_start_phase(id, StartPhase::RowStarting);
    }

    pub(crate) fn flow_row(&mut self, id: &SessionId, result: Result<(), StorageError>) {
        let Some(flow) = self.sessions.get(id).map(|s| s.flow.clone()) else {
            return;
        };
        match (flow, result) {
            (Flow::Create(f), Ok(())) => {
                self.sessions.get_mut(id).expect("kept").flow = Flow::Create(CreateFlow {
                    phase: CreatePhase::PostCreated,
                    ..f
                });
            }
            (Flow::Create(f), Err(e)) => {
                // The row was not written: the session never existed (LC-3). The ops that were admitted after the `Create`
                // (AM-1) end with the same failure, so none stays attached to a session that is gone (AM-3).
                let error = registry_failed(e);
                self.retire_session_ops(id, Some(f.op), Some(error.clone()));
                if let Some(session) = self.sessions.remove(id) {
                    self.retired_ops.extend(&session.ops);
                }
                self.complete(f.op, OpResult::Err(error));
            }
            (Flow::Start(f), result) => match (f.phase, result) {
                (StartPhase::RowStarting, Ok(())) => {
                    self.set_start_phase(id, StartPhase::PostStarting)
                }
                (StartPhase::RowStarting, Err(e)) => {
                    // Nothing ran: the session is `Created` again.
                    let s = self.sessions.get_mut(id).expect("kept");
                    s.admit = Admit::Created;
                    s.token = None;
                    let waiters = std::mem::take(&mut s.waiters);
                    s.stop_after_start = false;
                    self.flow_done(id);
                    self.complete(f.op, OpResult::Err(registry_failed(e)));
                    // A2-1: `RegistryFailed` is the only asynchronous error of `Stop`. The stop waited for this row write.
                    for op in waiters {
                        self.complete_later(op, OpResult::Err(registry_failed(e)));
                    }
                }
                (StartPhase::RowIdentity, Ok(())) => {
                    let phase = if f.hello_seen {
                        StartPhase::SendLaunch
                    } else {
                        StartPhase::AwaitHello
                    };
                    self.set_start_phase(id, phase);
                }
                (StartPhase::RowIdentity, Err(e)) => {
                    // AD-7: the identity is not durable, so the payload is never launched. The worker is ended.
                    if let Some(identity) = self.identity_of(id) {
                        self.act(Action::SignalGroup {
                            identity,
                            signal: GroupSignal::Kill,
                        });
                    }
                    self.close_worker_link(id);
                    if let Some(flow) = self.start_flow(id) {
                        flow.error = Some(registry_failed(e));
                    }
                    self.fail_start(
                        id,
                        StartFailReason::WorkerFailed,
                        SessionState::Lost(LostReason::StartInterrupted),
                    );
                }
                _ => {}
            },
            (Flow::Stop(f), Ok(())) if f.phase == StopPhase::RowWrite => {
                if let Some(flow) = self.stop_flow(id) {
                    flow.phase = StopPhase::SendStop;
                }
            }
            (Flow::Stop(f), Err(e)) if f.phase == StopPhase::RowWrite => {
                // R-16: the Stopping row of a `StopAll` target is best effort. A failed or uncertain write is ignored, the stop
                // proceeds, and `StopAll` completes when the target ends (LC-12). A plain `Stop` keeps `RegistryFailed`.
                let by_stop_all = self.stop_all_targets(id);
                let waiters = {
                    let s = self.sessions.get_mut(id).expect("kept");
                    std::mem::take(&mut s.waiters)
                };
                if waiters.is_empty() && by_stop_all {
                    if let Some(flow) = self.stop_flow(id) {
                        flow.phase = StopPhase::SendStop;
                    }
                    return;
                }
                {
                    let s = self.sessions.get_mut(id).expect("kept");
                    s.admit = Admit::Running;
                    s.host_ended = false;
                }
                self.flow_done(id);
                for op in waiters {
                    self.complete_later(op, OpResult::Err(registry_failed(e)));
                }
                if by_stop_all {
                    // The `StopAll` still stops the target, without waiting for the row (R-16).
                    self.request_stop(id);
                    if let Some(flow) = self.stop_flow(id) {
                        flow.phase = StopPhase::SendStop;
                    }
                }
            }
            (Flow::Remove(f), Ok(())) if f.phase == RemovePhase::DeleteRow => {
                self.set_remove_phase(id, RemovePhase::PostReleased);
            }
            (Flow::Remove(f), Err(e)) if f.phase == RemovePhase::DeleteRow => {
                let s = self.sessions.get_mut(id).expect("kept");
                s.admit = match s.shown {
                    Some(SessionState::Exited(_)) => Admit::Exited,
                    Some(SessionState::Lost(_)) => Admit::Lost,
                    _ => Admit::Created,
                };
                self.flow_done(id);
                self.complete(f.op, OpResult::Err(registry_failed(e)));
            }
            _ => {}
        }
    }

    pub(crate) fn flow_spawned(
        &mut self,
        id: &SessionId,
        result: Result<
            botster_core_edges::edges::ProcessIdentity,
            botster_core_edges::edges::SpawnError,
        >,
    ) {
        match result {
            Ok(identity) => {
                let deadline = self.mono().map(|now| now + self.cfg.limits.startup);
                self.sessions.get_mut(id).expect("kept").worker.identity = Some(identity);
                if let Some(f) = self.start_flow(id) {
                    f.deadline = deadline;
                    f.phase = StartPhase::RowIdentity;
                }
            }
            Err(e) => self.fail_start(
                id,
                StartFailReason::ExecFailed { errno: e.errno },
                SessionState::Exited(failed_start_exit()),
            ),
        }
    }

    /// The cleanup result of the worker (A6-3), or `OutcomeUnknown` when none can come. The first result stays.
    pub(crate) fn flow_remove_result(&mut self, id: &SessionId, uploads: UploadsOutcome) {
        if let Some(s) = self.sessions.get_mut(id) {
            if let Flow::Remove(f) = &mut s.flow {
                if f.uploads.is_none() {
                    f.uploads = Some(uploads);
                }
            }
        }
        self.remove_progress(id);
    }

    /// The worker process ended while the session is removed.
    pub(crate) fn flow_remove_worker_gone(&mut self, id: &SessionId) {
        if let Some(s) = self.sessions.get_mut(id) {
            if let Flow::Remove(f) = &mut s.flow {
                f.worker_gone = true;
                if f.uploads.is_none() {
                    f.uploads = Some(UploadsOutcome::NotDeleted(NotDeleted::OutcomeUnknown));
                }
            }
        }
        self.remove_progress(id);
    }
}
