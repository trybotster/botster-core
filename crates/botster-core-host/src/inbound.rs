//! The inputs of the engine: edge results, worker links and processes (plan 2.1).

use crate::engine::{CaptureEntry, HostEngine, Next, Owner, ParkedRoute, Step};
use crate::flow::*;
use crate::io::{Action, Input, LinkId, Ticket};
use crate::run::registry_failed;
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{ExitStatus, GroupSignal, ProcessIdentity};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{Observation, WorkerMsg};
use botster_core_link::proof::{host_proof, token_proof};
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
                    Some(Owner::FinalRow) if result.is_err() => self.final_row_failures += 1,
                    Some(Owner::FinalRow) | None => {}
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
                self.route_close(route, RouteCloseReason::HandoffFailed);
            }
            Input::HandoffLost { route } => self.on_handoff_lost(route),
            Input::ProcessExited { identity, status } => self.on_process_exited(identity, status),
            Input::WorkerConnected { ticket, link } => match self.take_ticket(ticket) {
                Some(Owner::Session(id)) => self.adopt_connected(&id, link),
                // No adoption waits for this link: it is closed, and nothing is read from it.
                _ => {
                    if let Some(link) = link {
                        self.close_link(link, "no adoption waits for the link (AD-6)");
                    }
                }
            },
            Input::IdentityState { identity, state } => {
                if !self.adopt_probed(identity, state) {
                    self.flow_remove_probed(identity, state);
                }
            }
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
        // A link that the host made for an adoption: this hello is the worker's answer (DESIGN.md 3.5).
        if let Some(id) = self.links.get(&link).cloned() {
            self.adopt_hello(link, &id, hello);
            return;
        }
        let found = self
            .sessions
            .iter()
            .find(|(_, s)| s.instance == hello.instance)
            .map(|(id, _)| id.clone());
        let Some(id) = found else {
            self.close_link(link, "the hello names an instance of no session (AD-6)");
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
            self.close_link(
                link,
                "the hello comes for a session that waits for none (AD-6)",
            );
            return;
        };
        let proof = token_proof(&token, &hello.instance, self.cfg.host_epoch);
        if hello.proof != proof || hello.host_epoch != self.cfg.host_epoch {
            // AD-6: a link that does not prove the token and the epoch is closed, and the start keeps waiting.
            self.close_link(
                link,
                "the hello does not prove the token or the host epoch (AD-6)",
            );
            return;
        }
        if !self.adoptable_worker_protocols().contains(&hello.protocol) {
            // AD-4, A6-2: a worker outside {T, T - 1} is `Lost(WorkerVersion)`, and Core never misbehaves.
            let why = format!(
                "the worker protocol {} is not adoptable (AD-4)",
                hello.protocol
            );
            self.close_link(link, &why);
            if let Some(identity) = self.identity_of(&id) {
                self.act(Action::SignalGroup {
                    identity,
                    signal: GroupSignal::Kill,
                });
            }
            self.fail_start(
                &id,
                StartFailReason::WorkerFailed,
                End::Lost(LostReason::WorkerVersion),
            );
            return;
        }
        self.act(Action::SendHello {
            link,
            hello: Hello {
                protocol: self.cfg.worker_protocol,
                instance: hello.instance.clone(),
                // AD-6: the host answers with its own role, so a worker never receives its own proof back.
                proof: host_proof(&token, &hello.instance, self.cfg.host_epoch),
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
                        End::Exited(Exit {
                            code: None,
                            signal: None,
                            cause: ExitCause::Other,
                        }),
                    );
                }
            }
            WorkerMsg::Exited { code, signal } => {
                let exit = self.exit_of(id, code, signal);
                self.begin_end_flow(id, End::Exited(exit));
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
            } => match reason {
                // OU-7: an ended session's route closes after its queue is delivered. The host posts the close in
                // `StopPhase::Finish`, after the session's state and with its own exit (A2-3, LC-5).
                RouteCloseReason::SessionEnded { .. } => self.route_delivered(id, route),
                _ => self.route_close(route, reason),
            },
            WorkerMsg::RouteStalled { route } => self.route_event(route, true),
            WorkerMsg::RouteResumed { route } => self.route_event(route, false),
            WorkerMsg::RemoveResult { uploads } => self.flow_remove_result(id, uploads),
            WorkerMsg::Adopted { report } => self.adopt_report(id, *report),
            // The enum is non-exhaustive: a report that a later worker adds is ignored by this host.
            _ => {}
        }
    }

    /// Records how a route of an ended session ended (OU-7). A route that is no longer the session's needs nothing.
    fn route_delivered(&mut self, id: &SessionId, route: RouteId) {
        if let Some(s) = self.sessions.get_mut(id) {
            if s.routes.contains(&route) {
                s.delivered.insert(route);
            }
        }
    }

    /// Closes a route now, or after the route events that wait ahead of it (EV-5b, EV-6).
    pub(crate) fn route_close(&mut self, route: RouteId, reason: RouteCloseReason) {
        if !self.routes.contains_key(&route) {
            return;
        }
        if !self.parked.is_empty() || !self.close_route(route, reason) {
            self.parked.push_back(ParkedRoute::Close(route, reason));
        }
    }

    fn route_event(&mut self, route: RouteId, stalled: bool) {
        // An event waits behind the ones parked before it, so the worker's order holds (EV-6).
        if !self.parked.is_empty() || !self.post_route_progress(route, stalled) {
            self.parked.push_back(ParkedRoute::Progress(route, stalled));
        }
    }

    /// Posts `RouteStalled` or `RouteResumed` of a route that still exists. False when the queue has no room.
    fn post_route_progress(&mut self, route: RouteId, stalled: bool) -> bool {
        if !self.routes.contains_key(&route) {
            return true;
        }
        let event = if stalled {
            Event::RouteStalled { route }
        } else {
            Event::RouteResumed { route }
        };
        self.queue.post_mandatory(event).is_ok()
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
                    s.silence.idle_start = Some((now, unix));
                    s.silence.output_seen = true;
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
                contents,
                total_bytes,
                reason,
            } => self.queue.post_droppable(Event::ClipboardWrite {
                id: sid,
                instance,
                selection,
                contents,
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

    /// The worker's link closed while the driver held the route's stream (`HandoffLost`). A stopping session ends:
    /// `StopPhase::Finish` closes the route once, after the state, with the end's reason (steward ruling R-50; no route of a
    /// gone link is delivered, so `SessionLost`). A start may still run, since the link alone does not end it: the route is
    /// kept as an obligation that `finish_start` settles (a failed start closes it with its end, as it closes every route
    /// of the session). Any other route closes `HandoffFailed`.
    fn on_handoff_lost(&mut self, route: RouteId) {
        let Some(id) = self.routes.get(&route).map(|entry| entry.session.clone()) else {
            return;
        };
        let Some(s) = self.sessions.get_mut(&id) else {
            return;
        };
        match &s.flow {
            Flow::Stop(_) => {}
            Flow::Start(_) => {
                s.lost_handoffs.insert(route);
            }
            _ => self.route_close(route, RouteCloseReason::HandoffFailed),
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
            .partition(|(route, _, _, _)| routes.contains(route));
        self.pending_handoffs = rest;
        for (route, transport, options, limits) in mine {
            self.act(Action::HandoffRoute {
                link,
                route,
                transport,
                options,
                limits,
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
            // The start of an adoption: the worker may have accepted the `Launch` before the link ended, so the session is
            // indeterminate, and `Adopt(id)` may retry it (AD-2; steward ruling R-36).
            Flow::Start(f)
                if f.adopted
                    && matches!(f.phase, StartPhase::SendLaunch | StartPhase::AwaitLaunched) =>
            {
                self.fail_start(
                    &id,
                    StartFailReason::WorkerFailed,
                    End::Lost(LostReason::WorkerUnreachable),
                );
            }
            Flow::Start(f)
                if matches!(
                    f.phase,
                    StartPhase::AwaitHello | StartPhase::SendLaunch | StartPhase::AwaitLaunched
                ) =>
            {
                self.fail_start(
                    &id,
                    StartFailReason::WorkerFailed,
                    End::Exited(Exit {
                        code: None,
                        signal: None,
                        cause: ExitCause::Other,
                    }),
                );
            }
            // The link of an adoption ended before the row's state was posted: the worker may live (AD-2).
            Flow::Adopt(_) => self.adopt_link_closed(&id),
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
            // A `Remove` whose worker ended already: the link's end of file is the last that can come, so a result that did
            // not come is `OutcomeUnknown` (A6-3), and the teardown goes on.
            Flow::Remove(_) if self.sessions[&id].worker.gone => self.flow_remove_worker_gone(&id),
            // Any other `Remove` needs nothing here. Its result can no longer come (no other link reaches this session), and
            // the teardown advances only when the worker is gone or its grace ended, which both record `OutcomeUnknown` when
            // no result came (A6-3: `flow_remove_worker_gone`, `remove_grace_expired`).
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
        // During a `Remove` the worker writes its cleanup result, closes the link and only then ends (LC-7 step 3), but the
        // host can see the exit before it reads the link (the edges report them independently). The link stays open, so
        // the result written before the exit is still read; the link's end of file completes the teardown
        // (`on_link_closed`), and the remove grace still bounds it (A6-3). Only a worker that was asked for its teardown can
        // have written a result: an exit before `SendRemove` closes the link below, and `SendRemove` then records
        // `OutcomeUnknown` without asking a gone worker.
        let tearing_down =
            matches!(&flow, Flow::Remove(f) if f.phase == RemovePhase::AwaitTeardown);
        if tearing_down && self.sessions[&id].worker.link.is_some() {
            if let Some(s) = self.sessions.get_mut(&id) {
                if let Flow::Remove(f) = &mut s.flow {
                    f.worker_gone = true;
                }
            }
            // A result that came before the exit lets the teardown go on now.
            self.remove_progress(&id);
            self.fail_inflight(&id);
            return;
        }
        // The worker is gone: its link is gone with it.
        if self.sessions[&id].worker.link.is_some() {
            self.close_worker_link(&id, "the worker process ended");
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
                    End::Lost(LostReason::WorkerGone),
                );
            }
            Flow::Remove(_) => self.flow_remove_worker_gone(&id),
            Flow::Create(_) => {}
            // The worker ended before the row's state was posted (AD-2).
            Flow::Adopt(_) => self.adopt_end(&id, End::Lost(LostReason::WorkerGone), ""),
            _ => {
                if matches!(
                    shown,
                    Some(SessionState::Running | SessionState::Starting | SessionState::Stopping)
                ) && self.sessions[&id].pending_end.is_none()
                    && !matches!(&flow, Flow::Stop(f) if f.phase == StopPhase::PostEnd || f.phase == StopPhase::Finish)
                {
                    self.begin_end_flow(&id, End::Lost(LostReason::WorkerGone));
                }
            }
        }
        self.fail_inflight(&id);
    }

    /// The wake of a parked route event: it posts when the queue has room again (EV-5d).
    pub(crate) fn run_parked(&mut self) {
        let Some(next) = self.parked.pop_front() else {
            return;
        };
        let posted = match next {
            ParkedRoute::Close(route, reason) => {
                !self.routes.contains_key(&route) || self.close_route(route, reason)
            }
            ParkedRoute::Progress(route, stalled) => self.post_route_progress(route, stalled),
        };
        if !posted {
            self.parked.push_front(next);
        }
    }

    pub(crate) fn parked_work(&self) -> bool {
        !self.parked.is_empty()
    }
}
