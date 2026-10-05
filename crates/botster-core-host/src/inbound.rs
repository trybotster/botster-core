//! The inputs of the engine: edge results, worker links and processes (plan 2.1).

use crate::engine::{CaptureEntry, HostEngine, Next, Owner, Step};
use crate::flow::*;
use crate::io::{Action, Input, LinkId, Ticket};
use crate::run::registry_failed;
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{ExitStatus, GroupSignal, ProcessIdentity};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{Observation, WorkerMsg};
use botster_core_link::proof::token_proof;
use std::collections::BTreeSet;

impl HostEngine {
    pub(crate) fn on_input(&mut self, input: Input) {
        self.step_mark = self.queue.total_posted();
        match input {
            Input::Clock(unix) => self.unix = unix,
            Input::Run(work) => self.run(work),
            Input::Random { ticket, bytes } => {
                if let Some(Owner::Session(id)) = self.take_ticket(ticket) {
                    self.flow_random(&id, bytes);
                }
            }
            Input::RowWritten { ticket, result } | Input::RowDeleted { ticket, result } => {
                match self.take_ticket(ticket) {
                    Some(Owner::Session(id)) => self.flow_row(&id, result),
                    Some(Owner::Op(op)) => self.op_row(op, result),
                    Some(Owner::Ignored) | None => {}
                }
            }
            Input::Spawned { ticket, result } => {
                if let Some(Owner::Session(id)) = self.take_ticket(ticket) {
                    self.flow_spawned(&id, result);
                }
            }
            Input::Rows { ticket, result } => {
                if let Some(Owner::Op(op)) = self.take_ticket(ticket) {
                    self.adopt_rows(op, result);
                }
            }
            Input::LinkHello { link, hello } => self.on_hello(link, hello),
            Input::LinkMsg { link, msg } => {
                if let Some(id) = self.links.get(&link).cloned() {
                    self.on_worker_msg(&id, msg);
                }
            }
            Input::LinkClosed { link } => self.on_link_closed(link),
            Input::HandoffFailed { route } => {
                if self.routes.contains_key(&route)
                    && !self.close_route(route, RouteCloseReason::HandoffFailed)
                {
                    self.parked_closes
                        .push_back((route, RouteCloseReason::HandoffFailed));
                }
            }
            Input::ProcessExited { identity, status } => self.on_process_exited(identity, status),
            Input::IdentityState { identity, state } => self.flow_remove_probed(identity, state),
            Input::Features(features) => self.features = features,
        }
    }

    /// Resolves a ticket: the owner that waited for it, with its flag cleared.
    fn take_ticket(&mut self, ticket: Ticket) -> Option<Owner> {
        let owner = self.tickets.remove(&ticket)?;
        if let Owner::Session(id) = &owner {
            if let Some(s) = self.sessions.get_mut(id) {
                if s.ticket == Some(ticket) {
                    s.ticket = None;
                }
            }
        }
        Some(owner)
    }

    fn op_row(&mut self, op: OpId, result: Result<(), botster_core_edges::edges::StorageError>) {
        let Some(pending) = self.ops.get(&op) else {
            return;
        };
        let session = pending.session.clone();
        let body = pending.op.clone();
        match (body, result) {
            (Op::UpdateMetadata { labels, .. }, Ok(())) => {
                if let Some(session) = session.and_then(|id| self.sessions.get_mut(&id)) {
                    session.labels = labels;
                    session.metadata_pending = true;
                    // LC-9: `MetadataChanged` follows the completion, in the next step of the session.
                    self.complete(op, OpResult::Ok(OpOutput::Unit));
                } else {
                    self.complete(op, OpResult::Ok(OpOutput::Unit));
                }
            }
            (Op::SetNotificationPolicy { policy, .. }, Ok(())) => {
                if let Some(session) = session.and_then(|id| self.sessions.get_mut(&id)) {
                    session.request.notification_policy = Some(policy);
                    session.pending_setters = session.pending_setters.saturating_sub(1);
                }
                self.complete(op, OpResult::Ok(OpOutput::Unit));
            }
            (body, Err(error)) => {
                if let (Op::SetNotificationPolicy { .. }, Some(id)) = (&body, &session) {
                    if let Some(s) = self.sessions.get_mut(id) {
                        s.pending_setters = s.pending_setters.saturating_sub(1);
                    }
                }
                self.complete(op, OpResult::Err(registry_failed(error)))
            }
            _ => {}
        }
    }

    fn adopt_rows(
        &mut self,
        op: OpId,
        result: Result<Vec<(String, Vec<u8>)>, botster_core_edges::edges::StorageError>,
    ) {
        match result {
            Ok(rows) => {
                // The registry is authoritative (AD-7): an id that no row holds is free, and every row is recovered.
                let held: std::collections::BTreeSet<SessionId> = rows
                    .iter()
                    .filter_map(|(key, _)| key.strip_prefix(crate::session::ROW_PREFIX))
                    .map(|id| SessionId(id.into()))
                    .collect();
                self.unadopted.retain(|id| held.contains(id));
                if let Some(p) = self.ops.get_mut(&op) {
                    p.rows = rows.into();
                }
                self.set_step(op, Step::Ready(Next::AdoptRow));
            }
            Err(error) => self.complete(op, OpResult::Err(registry_failed(error))),
        }
    }

    // ---- the hello (AD-6, AD-4, A6-2) ----

    fn on_hello(&mut self, link: LinkId, hello: Hello) {
        let found = self
            .sessions
            .iter()
            .find(|(_, s)| s.instance == hello.instance)
            .map(|(id, _)| id.clone());
        let Some(id) = found else {
            self.act(Action::CloseLink { link });
            return;
        };
        let session = &self.sessions[&id];
        let accepting = matches!(
            &session.flow,
            Flow::Start(f) if matches!(f.phase, StartPhase::Spawn | StartPhase::RowIdentity | StartPhase::AwaitHello)
        );
        let Some(token) = session
            .token
            .filter(|_| accepting && session.worker.link.is_none())
        else {
            self.act(Action::CloseLink { link });
            return;
        };
        let proof = token_proof(&token, &hello.instance, self.cfg.host_epoch);
        if hello.proof != proof || hello.host_epoch != self.cfg.host_epoch {
            // AD-6: a link that does not prove the token and the epoch is closed, and the start keeps waiting.
            self.act(Action::CloseLink { link });
            return;
        }
        if !self.adoptable_worker_protocols().contains(&hello.protocol) {
            // AD-4, A6-2: a worker outside {T, T - 1} is `Lost(WorkerVersion)`, and Core never misbehaves.
            self.act(Action::CloseLink { link });
            if let Some(identity) = self.identity_of(&id) {
                self.act(Action::SignalGroup {
                    identity,
                    signal: GroupSignal::Kill,
                });
            }
            self.fail_start(
                &id,
                StartFailReason::WorkerFailed,
                SessionState::Lost(LostReason::WorkerVersion),
            );
            return;
        }
        self.act(Action::SendHello {
            link,
            hello: Hello {
                protocol: self.cfg.worker_protocol,
                instance: hello.instance.clone(),
                proof,
                host_epoch: self.cfg.host_epoch,
            },
        });
        self.links.insert(link, id.clone());
        let s = self.sessions.get_mut(&id).expect("found above");
        s.worker.link = Some(link);
        s.worker.link_failed = false;
        s.worker_protocol = Some(hello.protocol);
        if let Flow::Start(f) = &mut s.flow {
            if f.phase == StartPhase::AwaitHello {
                f.phase = StartPhase::SendLaunch;
            } else {
                f.hello_seen = true;
            }
        }
    }

    // ---- reports of a worker ----

    fn on_worker_msg(&mut self, id: &SessionId, msg: WorkerMsg) {
        match msg {
            WorkerMsg::Launched {
                features,
                terminal,
                formats,
                payload,
            } => {
                let awaiting = matches!(
                    self.sessions.get(id).map(|s| &s.flow),
                    Some(Flow::Start(f)) if f.phase == StartPhase::AwaitLaunched
                );
                if !awaiting {
                    return;
                }
                let s = self.sessions.get_mut(id).expect("checked");
                s.worker_features = Some(features);
                s.payload = Some(ProcessIdentity {
                    pid: payload.pid,
                    start_time: payload.start_time,
                });
                s.terminal = Some(terminal);
                s.formats = formats;
                if let Flow::Start(f) = &mut s.flow {
                    f.deadline = None;
                    f.phase = StartPhase::PostRunning;
                }
                self.flush_handoffs(id);
            }
            WorkerMsg::LaunchFailed { reason } => {
                let awaiting = matches!(
                    self.sessions.get(id).map(|s| &s.flow),
                    Some(Flow::Start(f)) if f.phase == StartPhase::AwaitLaunched
                );
                if awaiting {
                    self.fail_start(
                        id,
                        reason,
                        SessionState::Exited(Exit {
                            code: None,
                            signal: None,
                            cause: ExitCause::Other,
                        }),
                    );
                }
            }
            WorkerMsg::Exited { code, signal } => {
                let exit = self.exit_of(id, code, signal);
                self.begin_end_flow(id, SessionEnd::Exited(exit));
            }
            WorkerMsg::Done { req, result } => self.on_done(id, req, result),
            WorkerMsg::Pages { req, pages } => {
                let op = self
                    .sessions
                    .get(id)
                    .and_then(|s| s.inflight.get(&req).copied());
                if let Some(p) = op.and_then(|op| self.ops.get_mut(&op)) {
                    p.pages = Some(pages);
                }
            }
            WorkerMsg::Observed { observation } => self.observe(id, observation),
            WorkerMsg::RouteClosed {
                route,
                reason,
                route_tag: _,
            } => {
                if self.routes.contains_key(&route) && !self.close_route(route, reason) {
                    self.parked_closes.push_back((route, reason));
                }
            }
            WorkerMsg::RouteStalled { route } => self.route_event(route, true),
            WorkerMsg::RouteResumed { route } => self.route_event(route, false),
            WorkerMsg::RemoveResult { uploads } => self.flow_remove_result(id, uploads),
            // The enum is non-exhaustive: a report that a later worker adds is ignored by this host.
            _ => {}
        }
    }

    fn route_event(&mut self, route: RouteId, stalled: bool) {
        if !self.routes.contains_key(&route) {
            return;
        }
        let event = if stalled {
            Event::RouteStalled { route }
        } else {
            Event::RouteResumed { route }
        };
        if let Err(event) = self.queue.post_mandatory(event) {
            self.parked_events.push_back(*event);
        }
    }

    fn on_done(&mut self, id: &SessionId, req: u64, result: OpResult) {
        let Some(op) = self
            .sessions
            .get_mut(id)
            .and_then(|s| s.inflight.remove(&req))
        else {
            return;
        };
        let result = match (&result, self.ops.get(&op)) {
            (OpResult::Ok(OpOutput::Capture(c)), Some(p))
                if matches!(p.op, Op::CaptureSnapshot { .. }) =>
            {
                self.take_capture(op, c.clone())
            }
            _ => result,
        };
        self.complete(op, result);
    }

    /// A capture is a frozen copy that Core keeps (ST-6): it mints the `CaptureId` and keeps the pages.
    fn take_capture(&mut self, op: OpId, reported: Capture) -> OpResult {
        let alive = self.ops.get(&op).is_some_and(|p| {
            p.session
                .as_ref()
                .and_then(|s| self.sessions.get(s))
                .is_some_and(|s| Some(&s.instance) == p.instance.as_ref())
        });
        if !alive {
            // A capture of a removed instance is never kept (LC-7 step 2, ID-1).
            return OpResult::Err(CoreError::new(
                ErrorCode::SessionEnded,
                "the session was removed before the capture finished",
            ));
        }
        let Some(p) = self.ops.get_mut(&op) else {
            return OpResult::Err(CoreError::new(
                ErrorCode::Internal,
                "the capture op is gone",
            ));
        };
        let pages = p.pages.take().unwrap_or_default();
        let Op::CaptureSnapshot { owner, .. } = p.op.clone() else {
            return OpResult::Err(CoreError::new(ErrorCode::Internal, "not a capture op"));
        };
        let instance = p.instance.clone().expect("a capture op has an instance");
        let bytes: u64 = pages.iter().map(|page| page.bytes.0.len() as u64).sum();
        if bytes > self.cfg.limits.max_snapshot_bytes {
            return OpResult::Err(CoreError::new(
                ErrorCode::SnapshotTooLarge,
                format!("{bytes} bytes are over max_snapshot_bytes"),
            ));
        }
        let Some(now) = self.mono() else {
            return OpResult::Err(CoreError::new(ErrorCode::Internal, "no clock yet"));
        };
        let capture = CaptureId(self.next_capture);
        self.next_capture += 1;
        let page_count = pages.len() as u32;
        self.captures.insert(
            capture,
            CaptureEntry {
                op,
                owner,
                instance,
                pages,
                bytes,
                expires: now + self.cfg.limits.capture_ttl,
            },
        );
        OpResult::Ok(OpOutput::Capture(Capture {
            capture,
            page_count,
            total_bytes: bytes,
            model_rev: reported.model_rev,
        }))
    }

    /// What a worker saw becomes an event, with the instance and the injected time (EV-7, EV-9, TM-1).
    pub(crate) fn observe(&mut self, id: &SessionId, observation: Observation) {
        let unix = self.unix;
        let mono = self.mono();
        let Some(s) = self.sessions.get_mut(id) else {
            return;
        };
        let (sid, instance) = (s.id.clone(), s.instance.clone());
        match observation {
            Observation::Output { model_rev } => {
                if let Some(t) = &mut s.terminal {
                    t.model_rev = model_rev;
                    t.last_output_at = Some(unix);
                }
                if let Some(now) = mono {
                    s.silence.last_output = Some((now, unix));
                    s.silence.fired = false;
                }
                self.queue.post_keyed(Event::Activity {
                    id: sid,
                    instance,
                    source: ActivitySource::Output,
                    at: unix,
                });
            }
            Observation::Modes { flags, model_rev } => {
                if let Some(t) = &mut s.terminal {
                    t.modes = flags.clone();
                    t.model_rev = model_rev;
                }
                self.queue.post_keyed(Event::ModesChanged {
                    id: sid,
                    instance,
                    flags,
                });
            }
            Observation::Title { title, model_rev } => {
                if let Some(t) = &mut s.terminal {
                    t.title = Some(title.clone());
                    t.model_rev = model_rev;
                }
                self.queue.post_keyed(Event::TitleChanged {
                    id: sid,
                    instance,
                    title,
                });
            }
            Observation::Cwd { cwd, model_rev } => {
                if let Some(t) = &mut s.terminal {
                    t.cwd = Some(cwd.clone());
                    t.model_rev = model_rev;
                }
                self.queue.post_keyed(Event::CwdChanged {
                    id: sid,
                    instance,
                    cwd,
                });
            }
            Observation::Size { size, model_rev } => {
                s.size = size;
                if let Some(t) = &mut s.terminal {
                    t.size = size;
                    t.model_rev = model_rev;
                }
                self.queue.post_keyed(Event::SizeChanged {
                    id: sid,
                    instance,
                    size,
                });
            }
            Observation::ClientInput { route, input_rev } => {
                if let Some(t) = &mut s.terminal {
                    t.input_rev.client = input_rev;
                }
                self.queue.post_keyed(Event::Activity {
                    id: sid,
                    instance,
                    source: ActivitySource::Client(route),
                    at: unix,
                });
            }
            Observation::HostInput { input_rev } => {
                if let Some(t) = &mut s.terminal {
                    t.input_rev.host = input_rev;
                }
            }
            Observation::Focus { focused } => {
                if let Some(t) = &mut s.terminal {
                    t.focused = match focused {
                        FocusState::Focused => Some(true),
                        FocusState::Unfocused => Some(false),
                        _ => None,
                    };
                }
                self.queue.post_keyed(Event::FocusChanged {
                    id: sid,
                    instance,
                    focused,
                });
            }
            Observation::Bell => self.queue.post_droppable(Event::Bell {
                id: sid,
                instance,
                at: unix,
            }),
            Observation::PromptMark { mark, exit_code } => {
                self.queue.post_droppable(Event::PromptMark {
                    id: sid,
                    instance,
                    mark,
                    exit_code,
                    at: unix,
                });
            }
            Observation::Notification {
                source,
                title,
                body,
                truncated,
            } => self.queue.post_droppable(Event::Notification {
                id: sid,
                instance,
                source,
                title,
                body,
                truncated,
                at: unix,
            }),
            Observation::ClipboardWrite {
                selection,
                bytes,
                total_bytes,
                reason,
            } => self.queue.post_droppable(Event::ClipboardWrite {
                id: sid,
                instance,
                selection,
                bytes: bytes.map(botster_route_codec::prelude::HexBytes),
                total_bytes,
                reason,
            }),
            Observation::Lost {
                kind,
                tap_dropped_bytes,
            } => self
                .queue
                .post_lost(&sid, &instance, kind, tap_dropped_bytes),
            Observation::Writable => self
                .queue
                .post_keyed(Event::SessionWritable { id: sid, instance }),
            // The enum is non-exhaustive: an observation that a later worker adds is ignored by this host.
            _ => {}
        }
    }

    /// Hands the registered routes of `id` to the worker (DP-2): they waited for the link and the launch.
    pub(crate) fn flush_handoffs(&mut self, id: &SessionId) {
        let Some(link) = self.sessions.get(id).and_then(|s| s.worker.link) else {
            return;
        };
        let routes: BTreeSet<RouteId> = self.sessions[id].routes.clone();
        let (mine, rest): (Vec<_>, Vec<_>) = std::mem::take(&mut self.pending_handoffs)
            .into_iter()
            .partition(|(route, _, _)| routes.contains(route));
        self.pending_handoffs = rest;
        for (route, transport, options) in mine {
            self.act(Action::HandoffRoute {
                link,
                route,
                transport,
                options,
            });
        }
    }

    // ---- losses ----

    fn on_link_closed(&mut self, link: LinkId) {
        let Some(id) = self.links.remove(&link) else {
            return;
        };
        let Some(s) = self.sessions.get_mut(&id) else {
            return;
        };
        s.worker.link = None;
        s.worker.link_failed = true;
        match s.flow.clone() {
            Flow::Start(f)
                if matches!(
                    f.phase,
                    StartPhase::AwaitHello | StartPhase::SendLaunch | StartPhase::AwaitLaunched
                ) =>
            {
                self.fail_start(
                    &id,
                    StartFailReason::WorkerFailed,
                    SessionState::Exited(Exit {
                        code: None,
                        signal: None,
                        cause: ExitCause::Other,
                    }),
                );
            }
            Flow::Stop(f) if f.end.is_none() && f.phase != StopPhase::RowWrite => {
                // LC-5: a session whose control link is broken still ends: the host signals the verified worker (pid and
                // start time, AD-6), which ends its payload group. The host never signals a bare payload group.
                if let Some(identity) = self.identity_of(&id) {
                    self.act(Action::SignalGroup {
                        identity,
                        signal: GroupSignal::EndPayload,
                    });
                }
            }
            // A `Remove` needs nothing here. Its result can no longer come (no other link reaches this session), and the
            // teardown advances only when the worker is gone or its grace ended, which both record `OutcomeUnknown` when no
            // result came (A6-3: `flow_remove_worker_gone`, `remove_grace_expired`).
            _ => {}
        }
        self.fail_inflight(&id);
    }

    fn on_process_exited(&mut self, identity: ProcessIdentity, _status: ExitStatus) {
        let found = self
            .sessions
            .iter()
            .find(|(_, s)| s.worker.identity == Some(identity))
            .map(|(id, _)| id.clone());
        let Some(id) = found else {
            return;
        };
        self.sessions.get_mut(&id).expect("found above").worker.gone = true;
        let shown = self.sessions[&id].shown;
        let flow = self.sessions[&id].flow.clone();
        // The worker is gone: its link is gone with it.
        if let Some(link) = self
            .sessions
            .get_mut(&id)
            .and_then(|s| s.worker.link.take())
        {
            self.links.remove(&link);
            self.act(Action::CloseLink { link });
            self.sessions
                .get_mut(&id)
                .expect("found above")
                .worker
                .link_failed = true;
        }
        match flow {
            Flow::Start(f)
                if f.phase != StartPhase::Finish && f.phase != StartPhase::PostFailed =>
            {
                self.fail_start(
                    &id,
                    StartFailReason::WorkerFailed,
                    SessionState::Lost(LostReason::WorkerGone),
                );
            }
            Flow::Remove(_) => self.flow_remove_worker_gone(&id),
            Flow::Create(_) => {}
            _ => {
                if matches!(
                    shown,
                    Some(SessionState::Running | SessionState::Starting | SessionState::Stopping)
                ) && self.sessions[&id].pending_end.is_none()
                    && !matches!(&flow, Flow::Stop(f) if f.phase == StopPhase::PostEnd || f.phase == StopPhase::Finish)
                {
                    self.begin_end_flow(&id, SessionEnd::Lost(LostReason::WorkerGone));
                }
            }
        }
        self.fail_inflight(&id);
    }

    /// The wake of a parked route event: it posts when the queue has room again (EV-5d).
    pub(crate) fn run_parked(&mut self) {
        if let Some((route, reason)) = self.parked_closes.pop_front() {
            if self.routes.contains_key(&route) && !self.close_route(route, reason) {
                self.parked_closes.push_front((route, reason));
            }
            return;
        }
        if let Some(event) = self.parked_events.pop_front() {
            if let Err(event) = self.queue.post_mandatory(event) {
                self.parked_events.push_front(*event);
            }
        }
    }

    pub(crate) fn parked_work(&self) -> bool {
        !self.parked_closes.is_empty() || !self.parked_events.is_empty()
    }
}
