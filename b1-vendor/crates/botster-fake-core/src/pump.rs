//! `pump`: the only call that makes progress (Core OR-1, TM-2). Every open order is a choice point (Core 11).

use crate::choice::Choice;
use crate::script::Step as ProgramStep;
use crate::state::*;
use crate::{Inner, WORKER_PROTOCOL};
use botster_core_contract::prelude::*;
use botster_fake_route::{FakeWorker, WorkerConfig};
use std::path::Path;

/// One unit of work in a pump. The seed orders them all (Core OR-3).
#[derive(Debug, Clone, PartialEq, Eq)]
enum Work {
    /// The payload, the program and the deadlines of one session.
    Tick(SessionId),
    /// The route side of one session: its worker's progress and what it reports to the host.
    Worker(SessionId),
    Life(SessionId),
    Io(SessionId),
    Free(OpId),
    Captures,
}

/// What an operation step did.
pub enum Step {
    Done(OpResult),
    /// Run the next stage after this many pumps.
    Later(u32),
    /// Wait for a condition that the session tick resolves.
    Wait,
    /// The op depends on something that does not exist yet: try again next pump.
    Retry,
}

pub fn ok(output: OpOutput) -> Step {
    Step::Done(OpResult::Ok(output))
}

pub fn fail(code: ErrorCode, detail: &str) -> Step {
    Step::Done(OpResult::Err(CoreError::new(code, detail)))
}

impl Inner {
    /// Posts an event and counts it for `PumpReport`.
    pub(crate) fn post(&mut self, ev: Event) {
        self.posted += 1;
        self.queue.post(ev);
    }

    /// A class K event is posted in the pump of its cause or one pump later (Core EV-6, 11: coalescing).
    pub(crate) fn post_keyed(&mut self, ev: Event) {
        if self.ch.pick(Choice::KeyedDefer, 2) == 0 {
            self.post(ev);
        } else {
            self.deferred.push(ev);
        }
    }

    pub(crate) fn complete(&mut self, id: OpId, result: OpResult) {
        let Some(at) = self.ops.iter().position(|p| p.id == id) else {
            return;
        };
        let p = self.ops.remove(at);
        self.completed.insert(id);
        // Core AM-4: a finished write keeps its capacity until the host polls its completion.
        if let Lane::Io(s) = &p.lane {
            let instance = self.adm.get(s).map(|r| r.instance.clone());
            if let Some(instance) = instance {
                self.charged.insert(id, (s.clone(), instance, p.size));
            }
        }
        // Core AM-3: exactly one completion per admitted operation. An internal op has none.
        if !p.silent {
            self.post(Event::Completed { op: id, result });
        }
    }

    pub(crate) fn pump(&mut self, now: Now) -> PumpReport {
        self.now = Some(now);
        self.posted = 0;
        for ev in std::mem::take(&mut self.deferred) {
            self.post(ev);
        }
        // Core OR-1: each op waits at least one pump. The delay counts down once per pump.
        for p in self.ops.iter_mut().filter(|p| !p.waiting) {
            p.delay = p.delay.saturating_sub(1);
        }
        let mut work = vec![Work::Captures];
        for id in self.sessions.keys() {
            work.push(Work::Tick(id.clone()));
            work.push(Work::Worker(id.clone()));
        }
        let due = |p: &&Pending| p.delay == 0 && !p.waiting;
        for p in self.ops.iter().filter(due) {
            let item = match &p.lane {
                Lane::Life(s) => Work::Life(s.clone()),
                Lane::Io(s) => Work::Io(s.clone()),
                Lane::Free => Work::Free(p.id),
            };
            if !work.contains(&item) {
                work.push(item);
            }
        }
        // Core OR-3, TM-5, 11: the events of different sessions, the completions and the due deadlines have no promised order.
        self.ch.shuffle(Choice::SessionOrder, &mut work);
        for item in work {
            match item {
                Work::Tick(s) => self.tick(&s),
                Work::Worker(s) => self.worker_progress(&s),
                Work::Life(s) => self.run_lane(|l| l == &Lane::Life(s.clone())),
                Work::Io(s) => self.run_lane(|l| l == &Lane::Io(s.clone())),
                Work::Free(id) => self.run_op(id),
                Work::Captures => self.expire_captures(),
            }
        }
        // Core TH-2: a wait may wake with no work.
        let spurious = self.ch.pick(Choice::SpuriousWake, 4) == 0;
        // Core TM-6: `more` means runnable work remains. A deferred event is posted by the next pump.
        let more = !self.deferred.is_empty()
            || self.ops.iter().any(|p| !p.waiting)
            || self.sessions.values().any(|s| {
                matches!(s.payload, Payload::Dying { .. })
                    || s.worker
                        .as_ref()
                        .is_some_and(|w| w.pty_busy() || w.modes() != &s.seen_modes)
            });
        let mut wake = self.wake.lock().expect("wake lock");
        // Core TM-6: the signal clears only when `pump` has no runnable work left.
        wake.signaled = more;
        wake.spurious = spurious;
        PumpReport {
            more,
            events_posted: self.posted,
        }
    }

    /// Runs the due ops of one lane in `begin` order (Core AM-2). A lifecycle op that waits holds back the ops behind it; a host write that
    /// was submitted does not, because the worker keeps the order.
    fn run_lane(&mut self, lane: impl Fn(&Lane) -> bool) {
        loop {
            let next = self
                .ops
                .iter()
                .find(|p| lane(&p.lane) && !(matches!(p.lane, Lane::Io(_)) && p.waiting));
            let Some(p) = next.filter(|p| p.delay == 0 && !p.waiting) else {
                return;
            };
            let (id, key) = (p.id, (p.stage, p.waiting, p.delay));
            self.run_op(id);
            let unchanged = self
                .ops
                .iter()
                .find(|q| q.id == id)
                .is_some_and(|q| (q.stage, q.waiting, q.delay) == key);
            if unchanged {
                return;
            }
        }
    }

    pub(crate) fn run_op(&mut self, id: OpId) {
        let Some(at) = self.ops.iter().position(|p| p.id == id) else {
            return;
        };
        let op = self.ops[at].op.clone();
        let stage = self.ops[at].stage;
        // Failure injection per error code (Core 11): the next op of that kind completes with the injected error.
        if stage == 0 {
            if let Some(i) = self.injected.iter().position(|(k, _)| *k == op_kind(&op)) {
                let (_, error) = self.injected.remove(i);
                self.unwind(&op);
                return self.complete(id, OpResult::Err(error));
            }
        }
        match self.step(id, &op, stage) {
            Step::Done(result) => self.complete(id, result),
            Step::Later(n) => {
                let p = &mut self.ops[at];
                p.stage = p.stage.saturating_add(1);
                p.delay = n;
            }
            Step::Wait => {
                let p = &mut self.ops[at];
                p.stage = p.stage.saturating_add(1);
                p.waiting = true;
            }
            Step::Retry => {}
        }
    }

    /// An op that failed before any effect gives back what its admission took (Core AM-1).
    fn unwind(&mut self, op: &Op) {
        match op {
            Op::Create { session, .. } => {
                self.adm.remove(session);
            }
            Op::Remove { id } | Op::Stop { id } | Op::Start { id } => {
                if let (Some(s), Some(row)) = (self.sessions.get(id), self.adm.get_mut(id)) {
                    row.state = adm_of(s.state);
                }
            }
            _ => {}
        }
    }

    fn step(&mut self, id: OpId, op: &Op, stage: u8) -> Step {
        let now = self.now.expect("a step runs inside pump");
        match op {
            Op::Create { session, request } => {
                // Core LC-3: `Create` makes a row in `Created` with no process.
                let instance = self.adm[session].instance.clone();
                let sess = Sess::new(session.clone(), instance.clone(), request.clone());
                let record = sess.record();
                self.sessions.insert(session.clone(), sess);
                self.post(Event::SessionState {
                    id: session.clone(),
                    instance,
                    state: SessionState::Created,
                });
                ok(OpOutput::Record(record))
            }
            Op::Start { id: sid } => self.start(sid, stage),
            Op::Stop { id: sid } => self.stop(sid, stage, now),
            Op::Signal { id: sid, sig } => self.signal_session(sid, *sig),
            Op::Remove { id: sid } => self.remove(sid),
            Op::StopAll => self.stop_all(id, stage),
            Op::AdoptAll => self.adopt_all(),
            Op::UpdateMetadata { id: sid, labels } => {
                let Some(s) = self.sessions.get_mut(sid) else {
                    return Step::Retry;
                };
                // Core LC-9: the registry row is written before `Completed`; `MetadataChanged` follows in a later pump.
                s.labels = labels.clone();
                let ev = Event::MetadataChanged {
                    id: sid.clone(),
                    instance: s.instance.clone(),
                };
                self.deferred.push(ev);
                ok(OpOutput::Unit)
            }
            Op::Resize { session, size } => self.resize(id, session, *size),
            Op::WriteInput {
                session,
                payload,
                guard,
            } => self.submit_write(id, session, payload.clone(), *guard, stage),
            Op::Detach { route, reason } => self.detach(*route, *reason),
            Op::SetSizePolicy { .. } => ok(OpOutput::Unit),
            Op::ReadScreen { .. }
            | Op::ReadCursor { .. }
            | Op::ReadModeFlags { .. }
            | Op::ReadFacts { .. }
            | Op::CaptureSnapshot { .. } => self.read_op(op, now),
            Op::SpawnService { .. } | Op::StopService { .. } | Op::RemoveService { .. } => {
                self.service_op(op, stage)
            }
            _ => fail(ErrorCode::Internal, "not modeled"),
        }
    }

    /// Core AD-1, LC-11: recovers every row of the registry by its recorded state and posts one `SessionState` for each.
    fn adopt_all(&mut self) -> Step {
        let rows = std::mem::take(&mut self.store.lock().expect("store lock").rows);
        let mut rows: Vec<Sess> = rows.into_values().collect();
        // Core OR-3: the events of different sessions have no promised order.
        self.ch.shuffle(Choice::SessionOrder, &mut rows);
        for mut s in rows {
            // Core ID-2: routes do not survive a host; the instance does.
            s.routes.clear();
            // Core AD-1: a `Starting` row whose worker identity is recorded is adopted as `Running`.
            if s.state == SessionState::Starting {
                s.state = SessionState::Running;
                s.payload = Payload::Alive;
            }
            let (id, instance, state) = (s.id.clone(), s.instance.clone(), s.state);
            self.adm.insert(
                id.clone(),
                AdmRow {
                    instance: instance.clone(),
                    state: adm_of(state),
                    io_ops: 0,
                    io_bytes: 0,
                },
            );
            self.store
                .lock()
                .expect("store lock")
                .holders
                .insert(id.clone(), self.me.clone());
            self.sessions.insert(id.clone(), s);
            // Core AD-1: a `Stopping` row is adopted as `Stopping`, and Core re-issues the stop.
            if state == SessionState::Stopping {
                self.spawn_internal_stop(&id);
            }
            self.post(Event::SessionState {
                id,
                instance,
                state,
            });
        }
        ok(OpOutput::Unit)
    }

    fn start(&mut self, sid: &SessionId, stage: u8) -> Step {
        let seed = self.seed;
        let (rows, cols) = {
            let Some(s) = self.sessions.get(sid) else {
                return Step::Retry;
            };
            (s.size.rows, s.size.cols)
        };
        let limits = self.limits.clone();
        let Some(s) = self.sessions.get_mut(sid) else {
            return Step::Retry;
        };
        if stage == 0 {
            // Core ER-0: a state that changed while the op was in flight is an async `WrongState`.
            if s.state != SessionState::Created {
                return fail(ErrorCode::WrongState, "the session is not Created");
            }
            let mut cfg = WorkerConfig {
                rows,
                cols,
                script: s.script.fake.clone(),
                admission_fairness_frames: limits.admission_fairness_frames,
                max_paste_bytes: limits.max_paste_bytes,
                max_file_bytes: limits.max_file_bytes,
                route_queue_bytes: limits.route_queue_bytes,
                ..WorkerConfig::default()
            };
            cfg.max_frame_bytes = limits.effective_max_route_frame_bytes();
            cfg.max_history_page_bytes = limits.max_history_page_bytes;
            s.worker = Some(FakeWorker::new(seed, cfg));
            s.state = SessionState::Starting;
            let (id, instance) = (s.id.clone(), s.instance.clone());
            self.post(Event::SessionState {
                id,
                instance,
                state: SessionState::Starting,
            });
            // Core OR-3: the number of pumps between `Start` and `Running` is a choice point.
            let n = 1 + self.ch.pick(Choice::StartPumps, 3) as u32;
            return Step::Later(n);
        }
        // Core LC-4, A2-1: a missing `cwd` completes `Start` with `StartFailed{CwdMissing}`; the row is `Exited`, never `Starting`.
        let reason = (!Path::new(&s.request.cwd).exists()).then_some(StartFailReason::CwdMissing);
        if let Some(reason) = reason {
            let exit = botster_core_contract::prelude::Exit {
                code: None,
                signal: None,
                cause: ExitCause::Other,
            };
            s.state = SessionState::Exited(exit);
            s.exit = Some(exit);
            let (id, instance) = (s.id.clone(), s.instance.clone());
            self.adm.get_mut(&id).expect("row").state = Adm::Exited;
            // Core OR-2: the completion of a failed start follows the `Exited` state.
            self.post(Event::SessionState {
                id,
                instance,
                state: SessionState::Exited(exit),
            });
            return fail(ErrorCode::StartFailed(reason), "the cwd does not exist");
        }
        s.state = SessionState::Running;
        s.protocol = Some(WORKER_PROTOCOL);
        s.payload = Payload::Alive;
        let record = s.record();
        let (id, instance) = (s.id.clone(), s.instance.clone());
        self.adm.get_mut(&id).expect("row").state = Adm::Running;
        self.post(Event::SessionState {
            id,
            instance,
            state: SessionState::Running,
        });
        // Core TH-3: the program starts with the payload, so its first steps have run when `Running` is seen.
        self.run_program(sid);
        ok(OpOutput::Record(record))
    }

    fn stop(&mut self, sid: &SessionId, stage: u8, now: Now) -> Step {
        let grace = self.limits.stop_grace;
        let pumps = 1 + self.ch.pick(Choice::StopPumps, 3) as u32;
        let Some(s) = self.sessions.get_mut(sid) else {
            return Step::Retry;
        };
        if stage > 0 {
            // Core LC-5: the completion arrives after the session reaches `Exited` or `Lost`.
            return match s.state {
                SessionState::Exited(e) => ok(OpOutput::End(SessionEnd::Exited(e))),
                SessionState::Lost(r) => ok(OpOutput::End(SessionEnd::Lost(r))),
                _ => Step::Wait,
            };
        }
        match s.state {
            // Core LC-5: a payload that already exited completes in the next pump.
            SessionState::Exited(e) => ok(OpOutput::End(SessionEnd::Exited(e))),
            SessionState::Lost(r) => ok(OpOutput::End(SessionEnd::Lost(r))),
            SessionState::Running => {
                s.state = SessionState::Stopping;
                let (id, instance) = (s.id.clone(), s.instance.clone());
                // Core LC-5: a graceful request, then a kill of the payload's group after `stop_grace`.
                // Core LC-5: a session whose control link is broken still ends: the graceful request cannot be delivered, so the kill follows.
                if s.script.ignores_sigterm() || s.control_broken {
                    s.kill_at = Some(now.monotonic + grace);
                } else if matches!(s.payload, Payload::Alive) {
                    s.payload = Payload::Dying {
                        pumps,
                        exit: terminated(15, ExitCause::HostStop),
                    };
                }
                self.post(Event::SessionState {
                    id,
                    instance,
                    state: SessionState::Stopping,
                });
                Step::Wait
            }
            _ => Step::Wait,
        }
    }

    fn signal_session(&mut self, sid: &SessionId, sig: Signal) -> Step {
        let pumps = 1 + self.ch.pick(Choice::StopPumps, 3) as u32;
        let Some(s) = self.sessions.get_mut(sid) else {
            return Step::Retry;
        };
        if !matches!(s.state, SessionState::Running | SessionState::Stopping) {
            return fail(ErrorCode::SessionEnded, "the session ended");
        }
        let n = match sig {
            Signal::Term => 15,
            Signal::Kill => 9,
            Signal::Int => 2,
            Signal::Hup => 1,
            Signal::Other(n) => n,
            _ => 15,
        };
        // Core LC-6: the signal reaches the payload's group. `SIGTERM` can be ignored by the program; `SIGKILL` cannot.
        let ignored = n == 15 && s.script.ignores_sigterm();
        // Core LC-6: the signal goes to the payload's process group, so the children receive it too.
        if !ignored {
            s.children = 0;
        }
        if !ignored && matches!(s.payload, Payload::Alive) {
            s.payload = Payload::Dying {
                pumps,
                exit: terminated(n, ExitCause::HostStop),
            };
        }
        ok(OpOutput::Unit)
    }

    /// Core LC-7: the five steps of `Remove`, in order. The completion follows `Released`.
    fn remove(&mut self, sid: &SessionId) -> Step {
        let Some(mut s) = self.sessions.remove(sid) else {
            return Step::Retry;
        };
        // Step 1: every route still bound is closed `SessionRemoved` (Core OU-2).
        let routes: Vec<RouteId> = s.routes.keys().copied().collect();
        for route in routes {
            self.routes.remove(&route);
            self.post(Event::RouteClosed {
                route,
                reason: RouteCloseReason::SessionRemoved,
                route_tag: None,
            });
        }
        // Step 2: the open captures of the session are released (Core ST-6).
        self.captures.retain(|_, c| c.instance != s.instance);
        // Step 3: the uploaded files are deleted and the worker ends (Core DP-5b).
        s.worker = None;
        // Steps 4 and 5: the row is deleted and the id is free, before `Released` and the completion (Core ID-1).
        self.adm.remove(sid);
        self.queue.retire_instance(&s.instance);
        self.post(Event::SessionState {
            id: sid.clone(),
            instance: s.instance.clone(),
            state: SessionState::Released,
        });
        ok(OpOutput::Unit)
    }

    fn stop_all(&mut self, id: OpId, stage: u8) -> Step {
        let (targets, services) = self
            .ops
            .iter()
            .find(|p| p.id == id)
            .map(|p| (p.targets.clone(), p.service_targets.clone()))
            .unwrap_or_default();
        if stage == 0 {
            // Core LC-12: each target is treated as by `Stop` or `StopService`: a running one is stopped, a stopping one joins its
            // stop, a starting one is stopped as soon as its start completes (the lifecycle lane keeps the order).
            for t in &targets {
                let stopping = self
                    .ops
                    .iter()
                    .any(|p| matches!(&p.op, Op::Stop { id } if id == t));
                if !stopping {
                    self.spawn_internal_stop(t);
                }
            }
        }
        // Core LC-12: the completion is posted when every target has ended; with no target to wait for it is posted now.
        let done = targets.iter().all(|t| {
            self.sessions.get(t).is_none_or(|s| {
                matches!(
                    s.state,
                    SessionState::Created | SessionState::Exited(_) | SessionState::Lost(_)
                )
            }) && !self
                .ops
                .iter()
                .any(|p| p.silent && matches!(&p.op, Op::Stop { id } if id == t))
        });
        // Core LC-12: a service admitted before `StopAll` is stopped as soon as its spawn has completed.
        let mut waiting = false;
        for svc in &services {
            match self.services.get(svc).map(|s| s.state) {
                None | Some(Some(ServiceState::Exited(_) | ServiceState::Lost { .. })) => {}
                Some(Some(_)) => {
                    let _ = self.service_op(&Op::StopService { id: *svc }, 0);
                }
                Some(None) => waiting = true,
            }
        }
        match (done, waiting) {
            (true, false) => ok(OpOutput::Unit),
            // A pending spawn is not tied to a session, so the op looks again in the next pump.
            (_, true) => Step::Later(1),
            _ => Step::Wait,
        }
    }

    /// A `StopAll` target is stopped as `Stop` does: an internal op with no slot and no completion (Core LC-12).
    fn spawn_internal_stop(&mut self, sid: &SessionId) {
        let id = OpId(self.next_op);
        self.next_op += 1;
        let instance = self.sessions.get(sid).map(|s| s.instance.clone());
        self.ops.push(Pending {
            id,
            op: Op::Stop { id: sid.clone() },
            lane: Lane::Life(sid.clone()),
            instance,
            delay: 0,
            stage: 0,
            waiting: false,
            silent: true,
            size: 0,
            targets: vec![],
            service_targets: vec![],
        });
    }

    fn resize(&mut self, id: OpId, sid: &SessionId, size: Size) -> Step {
        let Some(s) = self.sessions.get_mut(sid) else {
            return Step::Retry;
        };
        // Core SZ-3: a resize that a later one replaced before it was applied is `Superseded`.
        let later = self
            .ops
            .iter()
            .filter(|p| p.id > id && matches!(&p.op, Op::Resize { session, .. } if session == sid))
            .map(|p| p.id)
            .max();
        if let Some(by) = later {
            return ok(OpOutput::Resize(ResizeResult::Superseded { by }));
        }
        if !matches!(
            s.state,
            SessionState::Created | SessionState::Starting | SessionState::Running
        ) {
            return fail(ErrorCode::WrongState, "the session ended");
        }
        // Core SZ-2: a resize to the current size is applied and posts no `SizeChanged`.
        let changed = s.size.rows != size.rows || s.size.cols != size.cols;
        s.size = size;
        if let Some(w) = s.worker.as_mut() {
            w.host_resize(size.rows, size.cols);
        }
        let (id, instance) = (s.id.clone(), s.instance.clone());
        if changed && !matches!(s.state, SessionState::Created) {
            // Core SZ-3: `SizeChanged` is posted after the PTY resize.
            self.post_keyed(Event::SizeChanged { id, instance, size });
        }
        ok(OpOutput::Resize(ResizeResult::Applied { actual: size }))
    }

    /// The payload, the program and the deadlines of one session.
    fn tick(&mut self, sid: &SessionId) {
        let Some(now) = self.now else { return };
        self.run_program(sid);
        let Some(s) = self.sessions.get_mut(sid) else {
            return;
        };
        // Core LC-5: the kill of a payload that ignored the graceful request, after `stop_grace` (a deadline, Core TM-3).
        if s.kill_at.is_some_and(|at| now.monotonic >= at) && !matches!(s.payload, Payload::Dead(_))
        {
            s.kill_at = None;
            s.payload = Payload::Dead(terminated(9, ExitCause::Killed));
            s.children = 0;
            s.program_exit_started = false;
        }
        match s.payload {
            Payload::Alive => {}
            Payload::Dying { pumps, exit } => {
                s.payload = if pumps <= 1 {
                    Payload::Dead(exit)
                } else {
                    Payload::Dying {
                        pumps: pumps - 1,
                        exit,
                    }
                };
            }
            _ => {}
        }
        if let Payload::Dead(exit) = s.payload {
            if !s.program_exit_started {
                s.program_exit_started = true;
                if let Some(w) = s.worker.as_mut() {
                    w.program_exit(botster_route_codec::prelude::Exit {
                        code: exit.code,
                        signal: exit.signal,
                    });
                }
            }
            // Core OU-7: `Exited` is posted once the last output frame is queued for every open route.
            if s.worker.as_ref().is_none_or(|w| w.exit_delivered()) {
                s.payload = Payload::None;
                s.state = SessionState::Exited(exit);
                s.exit = Some(exit);
                s.kill_at = None;
                let (id, instance) = (s.id.clone(), s.instance.clone());
                self.adm.get_mut(&id).expect("row").state = Adm::Exited;
                self.post(Event::SessionState {
                    id,
                    instance,
                    state: SessionState::Exited(exit),
                });
            }
        }
        // Core OR-2: a waiting `Stop` or `StopAll` completes after the state event.
        self.resolve_waiters(sid);
    }

    fn resolve_waiters(&mut self, sid: &SessionId) {
        let waiting: Vec<OpId> = self
            .ops
            .iter()
            .filter(|p| p.waiting && p.stage > 0)
            .filter(|p| match &p.op {
                Op::Stop { id } => id == sid,
                Op::StopAll => true,
                _ => false,
            })
            .map(|p| p.id)
            .collect();
        for id in waiting {
            self.run_op(id);
        }
    }

    pub(crate) fn expire_captures(&mut self) {
        let Some(now) = self.now else { return };
        let ttl = self.limits.capture_ttl;
        // Core ST-6: expiry is processed in `pump`.
        self.captures.retain(|_, c| now.monotonic < c.taken + ttl);
    }
}

impl Inner {
    /// The probe program: its steps run in order, and a step that waits holds the later ones back (design 6.2, Core 11).
    pub(crate) fn run_program(&mut self, sid: &SessionId) {
        let Some(s) = self.sessions.get_mut(sid) else {
            return;
        };
        if !matches!(s.payload, Payload::Alive) {
            return;
        }
        loop {
            // A program that has run all its steps exits with code 0, as the probe binary does.
            let step = s
                .script
                .program
                .get(s.pc)
                .cloned()
                .unwrap_or(ProgramStep::Exit { code: 0 });
            if s.script.program.is_empty() {
                return;
            }
            match step {
                ProgramStep::Print { bytes_hex, fake } => {
                    print_bytes(s, &bytes_hex, fake.modes);
                    s.pc += 1;
                }
                ProgramStep::PrintAfterInput {
                    match_hex,
                    bytes_hex,
                    fake,
                } => {
                    let want =
                        botster_route_codec::prelude::hex_decode(&match_hex).unwrap_or_default();
                    let seen = s
                        .worker
                        .as_ref()
                        .map(|w| w.pty_stream())
                        .unwrap_or_default();
                    let from = s.input_read.min(seen.len());
                    let Some(at) = seen[from..]
                        .windows(want.len().max(1))
                        .position(|w| w == want.as_slice())
                    else {
                        return;
                    };
                    s.input_read = from + at + want.len();
                    print_bytes(s, &bytes_hex, fake.modes);
                    s.pc += 1;
                }
                ProgramStep::IgnoreSigterm {} => s.pc += 1,
                ProgramStep::ForkChild { .. } => {
                    s.children += 1;
                    s.pc += 1;
                }
                ProgramStep::Hold {} => return,
                ProgramStep::Exit { code } => {
                    return end_program(
                        s,
                        botster_core_contract::prelude::Exit {
                            code: Some(code),
                            signal: None,
                            cause: ExitCause::Normal,
                        },
                    )
                }
                ProgramStep::SignalSelf { n } => {
                    return end_program(s, terminated(n, ExitCause::Signal))
                }
            }
        }
    }
}

/// The program writes to the terminal. The script names the modes that the bytes set (R-5).
fn print_bytes(s: &mut Sess, hex: &str, modes: Option<serde_json::Value>) {
    let mut args = serde_json::json!({ "bytes_hex": hex });
    if let Some(m) = modes {
        args["scripted"] = serde_json::json!({ "modes": m });
    }
    if let Some(w) = s.worker.as_mut() {
        let _ = w.control("pty_output", &args, None);
    }
}

/// The program ends by itself (Core 11: scripted exits). The worker sees the exit at once, and the host learns of it in a pump (Core OU-7).
fn end_program(s: &mut Sess, exit: botster_core_contract::prelude::Exit) {
    s.payload = Payload::Dead(exit);
    s.program_exit_started = true;
    if let Some(w) = s.worker.as_mut() {
        w.program_exit(botster_route_codec::prelude::Exit {
            code: exit.code,
            signal: exit.signal,
        });
    }
}

/// An exit by a signal: death by a signal has no code (Core EV-4).
pub fn terminated(signal: i32, cause: ExitCause) -> botster_core_contract::prelude::Exit {
    botster_core_contract::prelude::Exit {
        code: None,
        signal: Some(signal),
        cause,
    }
}

pub fn adm_of(state: SessionState) -> Adm {
    match state {
        SessionState::Created => Adm::Created,
        SessionState::Starting => Adm::Starting,
        SessionState::Running => Adm::Running,
        SessionState::Stopping => Adm::Stopping,
        SessionState::Exited(_) => Adm::Exited,
        _ => Adm::Lost,
    }
}

/// The op kinds that failure injection names.
pub const KINDS: &[&str] = &[
    "Create",
    "Start",
    "Stop",
    "Signal",
    "Remove",
    "UpdateMetadata",
    "WriteInput",
    "ReadScreen",
    "CaptureSnapshot",
];

/// The name that failure injection uses for an operation.
pub fn op_kind(op: &Op) -> &'static str {
    match op {
        Op::Create { .. } => "Create",
        Op::Start { .. } => "Start",
        Op::Stop { .. } => "Stop",
        Op::Signal { .. } => "Signal",
        Op::Remove { .. } => "Remove",
        Op::UpdateMetadata { .. } => "UpdateMetadata",
        Op::WriteInput { .. } => "WriteInput",
        Op::ReadScreen { .. } => "ReadScreen",
        Op::CaptureSnapshot { .. } => "CaptureSnapshot",
        _ => "Other",
    }
}

impl Inner {
    /// What the worker of a session reports to the host in one pump: finished host writes, route closes and client activity.
    fn worker_progress(&mut self, sid: &SessionId) {
        let at = self.now.map_or(0, |n| n.unix);
        // Core OU-9: the transport reaches the worker in a pump, and the worker then writes the baseline.
        for r in self.routes.values_mut().filter(|r| &r.session == sid) {
            r.ready = true;
        }
        let cancelling: Vec<OpId> = self
            .ops
            .iter()
            .filter(|p| {
                p.lane == Lane::Io(sid.clone()) && p.stage > 0 && self.cancels.contains(&p.id)
            })
            .map(|p| p.id)
            .collect();
        let Some(s) = self.sessions.get_mut(sid) else {
            return;
        };
        let Some(w) = s.worker.as_mut() else { return };
        // Core IN-6: an admitted cancel reaches the worker, which reports exactly what was written.
        for op in cancelling {
            w.host_cancel(op.0);
        }
        // Core ST-1: a pump moves the route output a piece at a time; the host loops on wakes.
        w.pty_step();
        let done = w.host_done();
        let mut events = w.take_route_events();
        // Core OU-7: after `Exited`, each route's close follows. A close that the worker made first waits for the state event.
        if matches!(s.payload, Payload::Dead(_)) {
            s.held.append(&mut events);
        } else {
            events.splice(0..0, std::mem::take(&mut s.held));
        }
        let touched = w.take_activity();
        let output = w.take_output_activity();
        s.reported = (w.client_rev(), w.host_rev(), w.model_rev());
        // Core EV-7: every mode change posts `ModesChanged` with the full `ModeFlags`.
        let modes = (w.modes() != &s.seen_modes).then(|| w.modes().clone());
        if let Some(m) = &modes {
            s.seen_modes = m.clone();
        }
        let instance = s.instance.clone();
        let routes: Vec<(RouteId, u32)> = s.routes.iter().map(|(r, w)| (*r, *w)).collect();
        for d in done {
            let result = OpResult::Ok(OpOutput::Input(Self::input_result(&d.outcome)));
            self.complete(OpId(d.token), result);
        }
        for wr in touched {
            if let Some((route, _)) = routes.iter().find(|(_, w)| *w == wr) {
                // Core IN-4: the worker reports `Activity{source: Client(route)}` off the byte path, keyed by instance, source and route.
                self.post_keyed(Event::Activity {
                    id: sid.clone(),
                    instance: instance.clone(),
                    source: ActivitySource::Client(*route),
                    at,
                });
            }
        }
        if output {
            // Core EV-6: the session produced output (`Activity{source: Output}`, class K).
            self.post_keyed(Event::Activity {
                id: sid.clone(),
                instance: instance.clone(),
                source: ActivitySource::Output,
                at,
            });
        }
        if let Some(flags) = modes {
            self.post_keyed(Event::ModesChanged {
                id: sid.clone(),
                instance: instance.clone(),
                flags,
            });
        }
        for ev in events {
            self.route_event(sid, ev);
        }
    }
}
