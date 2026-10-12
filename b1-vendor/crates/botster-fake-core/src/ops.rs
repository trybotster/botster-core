//! `begin`: the admission table and the refusals of every row of A2-1 (Core AM-1, ER-0, A2-1).

use crate::choice::Choice;
use crate::state::*;
use crate::Inner;
use botster_core_contract::prelude::*;
use std::time::Instant;

/// The size range of A2-1: 1 to 65,535 on each side.
const SIZE_MAX: u32 = 65_535;

fn wrong_state() -> CoreError {
    CoreError::new(ErrorCode::WrongState, "not admitted in this state")
}

impl Inner {
    /// Core AM-1: `begin` checks and updates the admission table in `begin` order, then reserves the pending slot.
    pub(crate) fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        // Core ER-0, EV-5: every row has the sync error `PendingLimit`; a refusal allocates no op and posts no event.
        if self.slots >= self.limits.pending_ops as usize {
            return Self::err(ErrorCode::PendingLimit, "the pending table is full");
        }
        let next_pump = self.completes_next_pump(&op);
        let (lane, instance) = self.admit(&op)?;
        let size = match &op {
            Op::WriteInput {
                session, payload, ..
            } => self.worst_case(session, payload),
            _ => 0,
        };
        let id = OpId(self.next_op);
        self.next_op += 1;
        self.minted
            .insert(id, (matches!(op, Op::WriteInput { .. }), instance.clone()));
        self.slots += 1;
        // Core OR-1: an operation never completes before `begin` returns, so it takes at least one pump (Core OR-3).
        // Core LC-5, SZ-2, A2-1: an op that the contract completes in the next pump takes exactly one.
        let delay = if next_pump {
            1
        } else {
            1 + self.ch.pick(Choice::PumpsPerOp, 3) as u32
        };
        self.ops.push(Pending {
            id,
            op,
            lane,
            instance,
            delay,
            stage: 0,
            waiting: false,
            silent: false,
            size,
            targets: std::mem::take(&mut self.stop_targets.0),
            service_targets: std::mem::take(&mut self.stop_targets.1),
        });
        // Core TM-6: the work that `begin` leaves for `pump` signals the wake handle.
        self.signal();
        Ok(id)
    }

    /// Core LC-5, SZ-2, A2-1: `Stop` of a payload that already ended, and a `Resize` that changes nothing or acts on a `Created` row, complete in the next pump.
    fn completes_next_pump(&self, op: &Op) -> bool {
        match op {
            Op::Stop { id } => self
                .adm
                .get(id)
                .is_some_and(|r| matches!(r.state, Adm::Exited | Adm::Lost)),
            Op::Resize { session, size } => {
                self.adm
                    .get(session)
                    .is_some_and(|r| r.state == Adm::Created)
                    || self
                        .sessions
                        .get(session)
                        .is_some_and(|s| s.size.rows == size.rows && s.size.cols == size.cols)
            }
            _ => false,
        }
    }

    fn row(&mut self, id: &SessionId) -> Result<&mut AdmRow, CoreError> {
        self.adm
            .get_mut(id)
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownSession, "no such session"))
    }

    /// The refusals of A2-1 for one operation. `Ok` is the lane and the instance that the op acts on.
    fn admit(&mut self, op: &Op) -> Result<(Lane, Option<InstanceId>), CoreError> {
        use Adm::*;
        match op {
            Op::Create { session, request } => {
                // Core ID-1: an id over `max_session_id_bytes`.
                if session.0.len() > self.limits.max_session_id_bytes as usize {
                    return Self::err(ErrorCode::IdTooLong, "the session id is too long");
                }
                // Core LC-3, LC-7: an existing id, or one that `Remove` has not freed yet.
                if self.adm.contains_key(session) {
                    return Self::err(ErrorCode::IdInUse, "the id is taken");
                }
                if self.adm.len() >= self.limits.max_sessions as usize {
                    return Self::err(ErrorCode::SessionLimit, "max_sessions reached");
                }
                // Core A2-1: an empty `argv` and a size outside 1 to 65,535 are `InvalidInput`.
                let size = request.size;
                // A probe script that does not parse is a transcript error, so it fails at once and is never ignored.
                if let Err(why) = crate::script::of(request) {
                    return Err(CoreError::new(
                        ErrorCode::InvalidInput { field: None },
                        format!("probe script: {why}"),
                    ));
                }
                if request.argv.is_empty()
                    || !(1..=SIZE_MAX).contains(&size.rows)
                    || !(1..=SIZE_MAX).contains(&size.cols)
                {
                    return Self::err(
                        ErrorCode::InvalidInput { field: None },
                        "argv or size is invalid",
                    );
                }
                let instance = {
                    let mut store = self.store.lock().expect("store lock");
                    store.next_instance += 1;
                    InstanceId(format!("i-{}", store.next_instance))
                };
                self.adm.insert(
                    session.clone(),
                    AdmRow {
                        instance: instance.clone(),
                        state: Created,
                        io_ops: 0,
                        io_bytes: 0,
                    },
                );
                Ok((Lane::Life(session.clone()), Some(instance)))
            }
            Op::Start { id } => {
                let row = self.row(id)?;
                // Core AM-1: a second `Start` sees `Starting`; `Create` then `Start` in one turn sees `Created`.
                if row.state != Created {
                    return Err(wrong_state());
                }
                row.state = Starting;
                let instance = row.instance.clone();
                Ok((Lane::Life(id.clone()), Some(instance)))
            }
            Op::Stop { id } => {
                let row = self.row(id)?;
                if matches!(row.state, Created | Removing) {
                    return Err(wrong_state());
                }
                if matches!(row.state, Starting | Running) {
                    row.state = Stopping;
                }
                Ok((Lane::Life(id.clone()), Some(row.instance.clone())))
            }
            Op::Signal { id, sig } => {
                let row = self.row(id)?;
                if !matches!(row.state, Running | Stopping) {
                    return Err(wrong_state());
                }
                // Core LC-6: a signal number that no process can receive is `Unsupported`.
                if matches!(sig, Signal::Other(n) if !(1..=31).contains(n)) {
                    return Self::err(ErrorCode::Unsupported { what: None }, "no such signal");
                }
                Ok((Lane::Life(id.clone()), Some(row.instance.clone())))
            }
            Op::Remove { id } => {
                let row = self.row(id)?;
                // Core LC-7, AM-1: only `Created`, `Exited` and `Lost` sessions can be removed.
                if !matches!(row.state, Created | Exited | Lost) {
                    return Err(wrong_state());
                }
                row.state = Removing;
                Ok((Lane::Life(id.clone()), Some(row.instance.clone())))
            }
            Op::StopAll => {
                // Core LC-12: the targets are the sessions that run when `StopAll` is begun; later ones are not targets.
                self.stop_targets.1 = self.services.keys().copied().collect();
                self.stop_targets.0 = self
                    .adm
                    .iter()
                    .filter(|(_, r)| matches!(r.state, Starting | Running | Stopping))
                    .map(|(id, _)| id.clone())
                    .collect();
                for row in self.adm.values_mut() {
                    if matches!(row.state, Starting | Running) {
                        row.state = Stopping;
                    }
                }
                Ok((Lane::Free, None))
            }
            Op::AdoptAll => {
                // Core A2-1: a second `AdoptAll` on one handle is `WrongState` (sync).
                if std::mem::replace(&mut self.adopted, true) {
                    return Err(wrong_state());
                }
                Ok((Lane::Free, None))
            }
            Op::UpdateMetadata { id, .. } => {
                let row = self.row(id)?;
                if row.state == Removing {
                    return Err(wrong_state());
                }
                Ok((Lane::Free, Some(row.instance.clone())))
            }
            Op::Resize { session, size } => {
                let row = self.row(session)?;
                if !matches!(row.state, Created | Starting | Running) {
                    return Err(wrong_state());
                }
                if !(1..=SIZE_MAX).contains(&size.rows) || !(1..=SIZE_MAX).contains(&size.cols) {
                    return Self::err(
                        ErrorCode::InvalidInput { field: None },
                        "size is outside 1 to 65,535",
                    );
                }
                // Core SZ-3: resizes of one session are applied in `begin` order, and a later one replaces an earlier one.
                Ok((Lane::Life(session.clone()), Some(row.instance.clone())))
            }
            Op::SetSizePolicy { session, policy } => {
                let row = self.row(session)?;
                if !matches!(row.state, Created | Starting | Running) {
                    return Err(wrong_state());
                }
                // Core DP-6, A2-6: `Smallest` and `Largest` need the feature `size_policy_other`.
                if !matches!(policy, SizePolicy::Latest)
                    && self.withheld.contains(&Feature::SizePolicyOther)
                {
                    return Self::err(
                        ErrorCode::Unsupported { what: None },
                        "no size_policy_other",
                    );
                }
                Ok((Lane::Free, Some(self.adm[session].instance.clone())))
            }
            Op::ReadScreen { session, .. }
            | Op::ReadCursor { session }
            | Op::ReadModeFlags { session }
            | Op::ReadFacts { session, .. } => self.admit_read(session),
            Op::CaptureSnapshot { session, owner } => {
                let admitted = self.admit_read(session)?;
                // Core ST-6, 9B: the open captures of one owner are bounded, counting those in flight.
                let open = self.captures.values().filter(|c| &c.owner == owner).count()
                    + self
                        .ops
                        .iter()
                        .filter(
                            |p| matches!(&p.op, Op::CaptureSnapshot { owner: o, .. } if o == owner),
                        )
                        .count();
                if open >= self.limits.open_captures_per_client as usize {
                    return Self::err(ErrorCode::CaptureLimit, "open_captures_per_client reached");
                }
                Ok(admitted)
            }
            Op::WriteInput {
                session,
                payload,
                guard,
            } => self.admit_write(session, payload, guard),
            Op::Detach { route, .. } => {
                // Core ID-1: a route belongs to the instance it was attached to; a closed route is gone.
                let live = self.routes.get(route).is_some_and(|r| {
                    !r.closed
                        && self
                            .adm
                            .get(&r.session)
                            .is_some_and(|a| a.instance == r.instance)
                });
                if !live {
                    return Self::err(ErrorCode::UnknownRoute, "no such route");
                }
                Ok((Lane::Free, None))
            }
            Op::SpawnService {
                service_id, limits, ..
            } => self.admit_spawn(service_id, limits),
            Op::StopService { id } | Op::RemoveService { id } => self.admit_service(id, op),
            // Core A2-1: these rows belong to the later groups of the suite and are not modeled yet.
            _ => Self::err(
                ErrorCode::Unsupported { what: None },
                "this operation is not modeled by FakeCore yet",
            ),
        }
    }

    fn admit_read(&self, id: &SessionId) -> Result<(Lane, Option<InstanceId>), CoreError> {
        let row = self
            .adm
            .get(id)
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownSession, "no such session"))?;
        // Core A2-1: reads are admitted in `Starting`, `Running`, `Stopping` and `Exited`.
        if !matches!(
            row.state,
            Adm::Starting | Adm::Running | Adm::Stopping | Adm::Exited
        ) {
            return Err(wrong_state());
        }
        Ok((Lane::Free, Some(row.instance.clone())))
    }

    /// `cancel` of a host write (Core IN-6, A2-1).
    pub(crate) fn cancel(&mut self, op: OpId) -> CancelResult {
        match self.minted.get(&op).map(|(w, i)| (*w, i.clone())) {
            None => CancelResult::UnknownOp,
            // Core ID-1: an op is bound to the instance it acted on; a later instance of the same id never sees it.
            Some((_, Some(i))) if !self.adm.values().any(|r| r.instance == i) => {
                CancelResult::UnknownOp
            }
            // Core IN-6: the op already completed, so its `Completed` event carries the outcome.
            _ if self.completed.contains(&op) => CancelResult::TooLate,
            // Core A2-1: `cancel` applies to `WriteInput` only.
            Some((false, _)) => CancelResult::Refused(CancelRefusal::NotCancellable),
            Some((true, _)) => {
                let session = self
                    .ops
                    .iter()
                    .find(|p| p.id == op)
                    .and_then(|p| match &p.lane {
                        Lane::Io(s) => Some(s.clone()),
                        _ => None,
                    });
                // Core IN-6: a repeat of an admitted cancel is idempotent and takes no slot.
                if self.cancels.contains(&op) {
                    return CancelResult::AlreadyAdmitted;
                }
                if let Some(s) = session.filter(|s| self.full.contains(s)) {
                    // Core IN-6: the control queue has no room; `SessionWritable` follows when it has.
                    self.refused_full.insert(s);
                    return CancelResult::Refused(CancelRefusal::Full);
                }
                self.cancels.insert(op);
                self.signal();
                CancelResult::Admitted
            }
        }
    }

    /// `poll_events`: the host reads events and makes no progress (Core OR-1). A `Completed` that is polled frees its slot (Core EV-5, AM-4).
    pub(crate) fn poll(&mut self, max: usize) -> Vec<Event> {
        // Core OR-3: the batching of events per `poll_events` is a choice point.
        let n = match self.queue.len() {
            0 => 0,
            len => 1 + self.ch.pick(Choice::PollBatch, max.min(len)),
        };
        let events = self.queue.take(n.min(max));
        for ev in &events {
            if let Event::Completed { op, .. } = ev {
                self.slots -= 1;
                // Core AM-4: the input capacity of a write returns when its completion is polled.
                // Core ID-1: only the instance that took the charge gets it back.
                if let Some((s, instance, size)) = self.charged.remove(op) {
                    if let Some(row) = self.adm.get_mut(&s).filter(|r| r.instance == instance) {
                        row.io_ops = row.io_ops.saturating_sub(1);
                        row.io_bytes = row.io_bytes.saturating_sub(size);
                    }
                }
            }
        }
        events
    }

    /// The earliest deadline that Core owns: a stop grace and a capture expiry (Core TM-3).
    pub(crate) fn next_deadline(&self) -> Option<Instant> {
        let kills = self.sessions.values().filter_map(|s| s.kill_at);
        let ttl = self.limits.capture_ttl;
        let expiries = self.captures.values().map(|c| c.taken + ttl);
        kills.chain(expiries).min()
    }
}
