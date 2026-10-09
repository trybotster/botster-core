//! `begin`: the admission table and the sync errors of every row of A2-1 (AM-1, ER-0, AM-4).
//!
//! A refusal is synchronous: it allocates no `OpId`, posts no event, reserves no slot and changes no state. The order of the
//! checks is: the arguments and the state (`UnknownSession`, `IdTooLong`, `IdInUse`, `WrongState`, `InvalidInput`,
//! `Unsupported`, `PayloadTooLarge`), then the pending table (`PendingLimit`), then the capacity of the row (`SessionLimit`,
//! `CaptureLimit`, `LaneFull`, `RouteLimit`).

use crate::engine::{new_session, HostEngine, Next, PendingOp, Step, Wait};
use crate::flow::*;
use crate::session::Admit;
use botster_core_contract::prelude::*;
use std::collections::BTreeSet;
use std::time::Duration;

/// The largest `rows` or `cols` of a `Size` (A2-1).
const MAX_DIMENSION: u32 = 65_535;

fn err(code: ErrorCode, detail: impl Into<String>) -> CoreError {
    CoreError::new(code, detail)
}

fn unknown_session(id: &SessionId) -> CoreError {
    err(ErrorCode::UnknownSession, format!("no session {}", id.0))
}

fn wrong_state(what: &str, id: &SessionId, admit: Admit) -> CoreError {
    err(
        ErrorCode::WrongState,
        format!(
            "{what} does not apply to session {} in state {admit:?}",
            id.0
        ),
    )
}

/// `InvalidInput` with no `field`: no clause names the refused input.
fn invalid(detail: impl Into<String>) -> CoreError {
    err(ErrorCode::InvalidInput { field: None }, detail)
}

/// `InvalidInput` whose `field` names the refused input, where a clause names it (A7-1, A9-1, A15-1).
fn invalid_field(field: &str, detail: impl Into<String>) -> CoreError {
    err(
        ErrorCode::InvalidInput {
            field: Some(field.into()),
        },
        detail,
    )
}

fn size_valid(size: &Size) -> Result<(), CoreError> {
    if size.rows == 0 || size.rows > MAX_DIMENSION || size.cols == 0 || size.cols > MAX_DIMENSION {
        return Err(invalid(format!(
            "the size {}x{} is outside 1 to {MAX_DIMENSION}",
            size.rows, size.cols
        )));
    }
    if let Some(cell) = size.cell_px {
        if cell.width == 0 || cell.height == 0 {
            return Err(invalid("cell_px is from 1"));
        }
    }
    Ok(())
}

fn profile_valid(profile: &ColorProfile) -> Result<(), CoreError> {
    match &profile.palette {
        Some(palette) if palette.len() != 256 => Err(invalid(format!(
            "a palette has exactly 256 entries, not {}",
            palette.len()
        ))),
        _ => Ok(()),
    }
}

fn no_nul(text: &str) -> bool {
    !text.contains('\0')
}

fn request_valid(request: &SpawnRequest) -> Result<(), CoreError> {
    if request.argv.is_empty() {
        return Err(invalid("argv is empty"));
    }
    if !request.argv.iter().all(|a| no_nul(a)) {
        return Err(invalid("argv holds a NUL"));
    }
    for (key, value) in &request.env {
        if key.is_empty() || key.contains('=') || !no_nul(key) || !no_nul(value) {
            return Err(invalid(format!("the variable {key:?} is not valid")));
        }
    }
    if !request.cwd.starts_with('/') || !no_nul(&request.cwd) {
        return Err(invalid("cwd is an absolute path"));
    }
    size_valid(&request.size)?;
    if let Some(profile) = &request.color_profile {
        profile_valid(profile)?;
    }
    Ok(())
}

impl HostEngine {
    fn session_admit(&self, id: &SessionId) -> Result<Admit, CoreError> {
        match self.sessions.get(id) {
            // A row whose adoption has not posted its state is no session of this handle yet (AD-1, LC-11).
            Some(s) if s.admit == Admit::Adopting && s.shown.is_none() => Err(unknown_session(id)),
            Some(s) => Ok(s.admit),
            None => Err(unknown_session(id)),
        }
    }

    /// `UnknownSession`, then `WrongState` unless the admission state is one of `allowed` (A2-1, "Admitted in").
    fn require(&self, what: &str, id: &SessionId, allowed: &[Admit]) -> Result<Admit, CoreError> {
        let admit = self.session_admit(id)?;
        if allowed.contains(&admit) {
            Ok(admit)
        } else {
            Err(wrong_state(what, id, admit))
        }
    }

    /// The feature check of a row that needs an optional feature (A2-6, AD-4).
    fn require_feature(&self, session: &SessionId, feature: Feature) -> Result<(), CoreError> {
        let offered = self.cfg.features.names.contains(&feature);
        let worker_lacks = self
            .sessions
            .get(session)
            .and_then(|s| s.worker_features.as_ref())
            .is_some_and(|w| !w.contains(&feature));
        if offered && !worker_lacks {
            Ok(())
        } else {
            Err(err(
                ErrorCode::Unsupported { what: None },
                format!("the feature {feature:?} is not offered"),
            ))
        }
    }

    /// The captures of `owner`: operations whose `Completed` is not polled, and captures that are open after it was polled.
    /// Each capture counts once (A8-1).
    fn capture_load(&self, owner: &ClientId) -> usize {
        let unpolled = self
            .ops
            .values()
            .filter(|p| matches!(&p.op, Op::CaptureSnapshot { owner: o, .. } if o == owner))
            .count();
        let polled = self
            .captures
            .values()
            .filter(|c| &c.owner == owner && !self.ops.contains_key(&c.op))
            .count();
        unpolled + polled
    }

    /// The held bytes of A8-1: `max_snapshot_bytes` for every capture op whose `Completed` is not polled, and `total_bytes`
    /// for a successful capture after it was polled. A failed capture counts nothing after the poll.
    pub(crate) fn retained_bytes(&self) -> u64 {
        let reserved = self
            .ops
            .values()
            .filter(|p| matches!(p.op, Op::CaptureSnapshot { .. }))
            .count() as u64
            * self.cfg.limits.max_snapshot_bytes;
        let kept: u64 = self
            .captures
            .values()
            .filter(|c| !self.ops.contains_key(&c.op))
            .map(|c| c.bytes)
            .sum();
        reserved + kept
    }

    /// Checks the arguments and the state of `op` (the sync column of A2-1, without the capacity codes).
    fn check_arguments(&self, op: &Op) -> Result<(), CoreError> {
        use Admit::*;
        match op {
            Op::Create { session, request } => {
                if session.0.len() > self.cfg.limits.max_session_id_bytes as usize {
                    return Err(err(
                        ErrorCode::IdTooLong,
                        format!(
                            "the id is {} bytes; the limit is {}",
                            session.0.len(),
                            self.cfg.limits.max_session_id_bytes
                        ),
                    ));
                }
                // ID-1: an id is unique among registry rows, so a durable row that `AdoptAll` has not recovered holds it too.
                if self.sessions.contains_key(session) || self.unadopted.contains(session) {
                    return Err(err(
                        ErrorCode::IdInUse,
                        format!("the id {} is in use", session.0),
                    ));
                }
                request_valid(request)
            }
            Op::Start { id } => self.require("Start", id, &[Created]).map(drop),
            Op::Stop { id } => self
                .require("Stop", id, &[Starting, Running, Stopping, Exited, Lost])
                .map(drop),
            Op::Signal { id, sig } => {
                self.require("Signal", id, &[Running, Stopping])?;
                match sig {
                    Signal::Other(n) if !(1..=31).contains(n) => Err(err(
                        ErrorCode::Unsupported { what: None },
                        format!("the signal {n} is not supported"),
                    )),
                    _ => Ok(()),
                }
            }
            Op::Remove { id } => self
                .require("Remove", id, &[Created, Exited, Lost])
                .map(drop),
            Op::StopAll => Ok(()),
            Op::UpdateMetadata { id, .. } => self
                .require(
                    "UpdateMetadata",
                    id,
                    &[Created, Starting, Running, Stopping, Exited, Lost],
                )
                .map(drop),
            Op::SetSizePolicy { session, policy } => {
                self.require("SetSizePolicy", session, &[Created, Starting, Running])?;
                if *policy != SizePolicy::Latest {
                    self.require_feature(session, Feature::SizePolicyOther)?;
                }
                Ok(())
            }
            Op::SetColorProfile { session, profile } => {
                self.require(
                    "SetColorProfile",
                    session,
                    &[Created, Starting, Running, Stopping, Exited],
                )?;
                profile_valid(profile)
            }
            Op::SetOutputTap { session, .. } => self
                .require("SetOutputTap", session, &[Running, Stopping])
                .map(drop),
            Op::ReadScreen { session, .. }
            | Op::ReadCursor { session }
            | Op::ReadModeFlags { session }
            | Op::CaptureSnapshot { session, .. }
            | Op::ReadFacts { session, .. } => self
                .require("a read", session, &[Starting, Running, Stopping, Exited])
                .map(drop),
            Op::Resize { session, size } => {
                self.require("Resize", session, &[Created, Starting, Running])?;
                size_valid(size)
            }
            Op::WriteInput {
                session, payload, ..
            } => {
                self.require("WriteInput", session, &[Running, Stopping, Exited, Lost])?;
                self.check_payload(payload)
            }
            Op::Detach { route, .. } => {
                if self.routes.contains_key(route) {
                    Ok(())
                } else {
                    Err(err(
                        ErrorCode::UnknownRoute,
                        format!("no route {}", route.0),
                    ))
                }
            }
            Op::AttachWebRtc { session, .. } => {
                self.require("attach", session, &[Starting, Running, Exited])?;
                Err(err(
                    ErrorCode::Unsupported { what: None },
                    "route_transport:webrtc is not offered by this build (P4c)",
                ))
            }
            Op::AdoptAll => {
                if self.adopt_all_begun {
                    Err(err(
                        ErrorCode::WrongState,
                        "AdoptAll ran already on this handle",
                    ))
                } else {
                    Ok(())
                }
            }
            Op::Adopt { id } => {
                let admit = self.session_admit(id)?;
                // A2-1: always admitted for `Lost(WorkerUnreachable)` and `Lost(WorkerVersion)`, whatever path ended the
                // session; `WrongState` is only for another state (steward ruling R-36, contracts `main` `c62085f`).
                let adoptable = admit == Lost
                    && matches!(
                        self.sessions.get(id).and_then(|s| s.shown),
                        Some(SessionState::Lost(
                            LostReason::WorkerUnreachable | LostReason::WorkerVersion
                        ))
                    );
                if !adoptable {
                    return Err(wrong_state("Adopt", id, admit));
                }
                Ok(())
            }
            Op::SpawnService { .. } | Op::EndEpoch { .. } => Err(err(
                ErrorCode::Unsupported { what: None },
                "services are built by the service package (P7)",
            )),
            Op::StopService { .. } | Op::RemoveService { .. } => Err(err(
                ErrorCode::UnknownService,
                "no service exists in this build (P7)",
            )),
            Op::SetNotificationPolicy { session, .. } => {
                self.require(
                    "SetNotificationPolicy",
                    session,
                    &[Created, Starting, Running, Stopping, Exited],
                )?;
                self.require_feature(session, Feature::NotificationPolicy)
            }
            // The enum is non-exhaustive: an op that a later contract adds is not offered by this build.
            _ => Err(err(
                ErrorCode::Unsupported { what: None },
                "the operation is not offered by this build",
            )),
        }
    }

    /// The checks of a host write's payload that `begin` makes before its size (IN-9, A2-1): `InvalidInput`.
    fn check_payload(&self, payload: &InputPayload) -> Result<(), CoreError> {
        match payload {
            InputPayload::Key(key) => {
                if key.shifted_key.is_some()
                    && !key
                        .mods
                        .contains(&botster_route_codec::prelude::Modifier::Shift)
                {
                    return Err(invalid("shifted_key is valid only with shift"));
                }
                if let Some(repeat) = key.repeat {
                    if key.event != botster_route_codec::prelude::KeyEvent::Press {
                        return Err(invalid("repeat is valid only with a press"));
                    }
                    if repeat == 0 || u32::from(repeat) > self.cfg.limits.max_key_repeat {
                        return Err(invalid(format!(
                            "repeat {repeat} is outside 1 to {}",
                            self.cfg.limits.max_key_repeat
                        )));
                    }
                }
            }
            InputPayload::Mouse(mouse) => {
                use botster_route_codec::prelude::{MouseAction, MouseButton};
                let is_wheel_button = matches!(
                    mouse.button,
                    MouseButton::WheelUp
                        | MouseButton::WheelDown
                        | MouseButton::WheelLeft
                        | MouseButton::WheelRight
                );
                if is_wheel_button != (mouse.action == MouseAction::Wheel) {
                    return Err(invalid("action wheel pairs with a wheel button only"));
                }
                if mouse.notches.is_some() && mouse.action != MouseAction::Wheel {
                    return Err(invalid("notches is valid only with wheel"));
                }
                if mouse.notches == Some(0) {
                    return Err(invalid("notches is from 1"));
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Whether a `Resize` of the session can still change its size: it is admitted and has not completed, and it is not a
    /// `Resize` that completes with no effect (SZ-2, SZ-3).
    fn resize_in_flight(&self, session: &SessionId) -> bool {
        self.ops.values().any(|p| {
            p.session.as_ref() == Some(session)
                && matches!(p.op, Op::Resize { .. })
                && !matches!(p.step, Step::Done | Step::Ready(Next::Complete(_)))
        })
    }

    /// The size of a host write's payload (IN-5, IN-9): its bytes, or for a semantic kind the worst-case encoded size over
    /// every mode, which libghostty's encoders give, once for each repeat or notch (5.1A). Over `max_paste_bytes` it is
    /// `PayloadTooLarge`. The size counts against `input_retained_bytes` (IN-5) and bounds an `Unknown` write (IN-7).
    fn payload_size(&self, payload: &InputPayload) -> Result<u64, CoreError> {
        let limit = self.cfg.limits.max_paste_bytes;
        let size = match payload {
            InputPayload::Bytes { bytes } | InputPayload::Paste { bytes, .. } => {
                bytes.0.len() as u64
            }
            InputPayload::Text { text } => text.len() as u64,
            InputPayload::Key(key) => {
                let times = u64::from(key.repeat.unwrap_or(1));
                // A sequence over `limit / times` puts the whole write over `limit`, so the search can stop there.
                let longest = botster_terminal_ghostty::longest_key_sequence(key, limit / times)
                    .map_err(|_| err(ErrorCode::Internal, "the key encoder cannot be created"))?;
                times.saturating_mul(longest)
            }
            InputPayload::Mouse(mouse) => u64::from(mouse.notches.unwrap_or(1))
                .saturating_mul(botster_terminal_ghostty::longest_mouse_report(mouse)),
            InputPayload::Focus { focused } => {
                botster_terminal_ghostty::longest_focus_report(*focused)
            }
            // A later kind of the non-exhaustive enum is refused `Unsupported` by `check_arguments`.
            _ => 0,
        };
        if size > limit {
            return Err(err(
                ErrorCode::PayloadTooLarge,
                format!("{size} payload bytes are over max_paste_bytes {limit}"),
            ));
        }
        Ok(size)
    }

    /// The capacity codes of the rows (9.3).
    fn check_capacity(&self, op: &Op, size: u64) -> Result<(), CoreError> {
        match op {
            Op::Create { .. } if self.sessions.len() >= self.cfg.limits.max_sessions as usize => {
                Err(err(
                    ErrorCode::SessionLimit,
                    format!("max_sessions is {}", self.cfg.limits.max_sessions),
                ))
            }
            Op::CaptureSnapshot { owner, .. } => {
                if self.capture_load(owner) >= self.cfg.limits.open_captures_per_client as usize
                    || self
                        .retained_bytes()
                        .saturating_add(self.cfg.limits.max_snapshot_bytes)
                        > self.cfg.limits.snapshot_retained_bytes
                {
                    Err(err(
                        ErrorCode::CaptureLimit,
                        "an open-capture bound is reached",
                    ))
                } else {
                    Ok(())
                }
            }
            Op::WriteInput { session, .. } => {
                let s = &self.sessions[session];
                let limits = &self.cfg.limits;
                if s.input_ops >= limits.input_ops_per_session
                    || s.input_bytes + size > limits.input_retained_bytes
                {
                    Err(err(
                        ErrorCode::LaneFull,
                        "an input bound of the session is reached",
                    ))
                } else {
                    Ok(())
                }
            }
            _ => Ok(()),
        }
    }

    /// Starts an operation (Core 2, ER-0, OR-1, AM-1). It makes no progress: `begin` never completes an op.
    pub fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        self.check_arguments(&op)?;
        let size = match &op {
            Op::WriteInput { payload, .. } => self.payload_size(payload)?,
            _ => 0,
        };
        if self.ops.len() >= self.cfg.limits.pending_ops as usize {
            return Err(err(
                ErrorCode::PendingLimit,
                format!("pending_ops is {}", self.cfg.limits.pending_ops),
            ));
        }
        self.check_capacity(&op, size)?;
        let id = OpId(self.next_op);
        self.next_op += 1;
        let session = Self::session_of(&op);
        self.commit(id, op, size);
        if let Some(s) = session.and_then(|session| self.sessions.get_mut(&session)) {
            s.ops.insert(id.0);
        }
        Ok(id)
    }

    /// The session that an op acts on, when it names one.
    fn session_of(op: &Op) -> Option<SessionId> {
        match op {
            Op::Create { session, .. }
            | Op::SetSizePolicy { session, .. }
            | Op::SetColorProfile { session, .. }
            | Op::SetOutputTap { session, .. }
            | Op::ReadScreen { session, .. }
            | Op::ReadCursor { session }
            | Op::ReadModeFlags { session }
            | Op::CaptureSnapshot { session, .. }
            | Op::ReadFacts { session, .. }
            | Op::Resize { session, .. }
            | Op::WriteInput { session, .. }
            | Op::SetNotificationPolicy { session, .. } => Some(session.clone()),
            Op::Start { id }
            | Op::Stop { id }
            | Op::Signal { id, .. }
            | Op::Remove { id }
            | Op::UpdateMetadata { id, .. }
            | Op::Adopt { id } => Some(id.clone()),
            _ => None,
        }
    }

    fn pending(
        op: Op,
        session: Option<SessionId>,
        instance: Option<InstanceId>,
        step: Step,
    ) -> PendingOp {
        PendingOp {
            op,
            session,
            instance,
            step,
            pages: None,
            req: None,
            held_bytes: 0,
            created_path: false,
            fixed_timing: false,
            cancelled: false,
            rows: Default::default(),
        }
    }

    /// Changes the admission table and queues the first step (AM-1: at `begin`, in `begin` order).
    fn commit(&mut self, id: OpId, op: Op, size: u64) {
        let instance_of =
            |engine: &HostEngine, s: &SessionId| engine.sessions.get(s).map(|x| x.instance.clone());
        match op.clone() {
            Op::Create { session, request } => {
                let instance = self.mint_instance();
                let flow = Flow::Create(CreateFlow {
                    op: id,
                    phase: CreatePhase::WriteRow,
                });
                self.sessions.insert(
                    session.clone(),
                    new_session(session.clone(), instance.clone(), request, flow),
                );
                self.ops.insert(
                    id,
                    Self::pending(op, Some(session), Some(instance), Step::Await(Wait::Flow)),
                );
            }
            Op::Start { id: session } => {
                let instance = instance_of(self, &session);
                self.sessions.get_mut(&session).expect("checked").admit = Admit::Starting;
                self.enqueue_flow(
                    &session,
                    Flow::Start(StartFlow {
                        op: id,
                        phase: StartPhase::Token,
                        failure: None,
                        deadline: None,
                        error: None,
                        hello_seen: false,
                        adopted: false,
                    }),
                );
                self.ops.insert(
                    id,
                    Self::pending(op, Some(session), instance, Step::Await(Wait::Flow)),
                );
            }
            Op::Stop { id: session } => {
                let instance = instance_of(self, &session);
                let admit = self.sessions[&session].admit;
                let step = match admit {
                    Admit::Exited | Admit::Lost => {
                        let end = self.session_end(&session);
                        Step::Ready(Next::Complete(OpResult::Ok(OpOutput::End(end.public()))))
                    }
                    Admit::Running | Admit::Starting => {
                        self.sessions
                            .get_mut(&session)
                            .expect("checked")
                            .waiters
                            .push(id);
                        self.request_stop(&session);
                        Step::Await(Wait::Flow)
                    }
                    _ => {
                        // `Stopping`: join the stop that is running.
                        self.sessions
                            .get_mut(&session)
                            .expect("checked")
                            .waiters
                            .push(id);
                        Step::Await(Wait::Flow)
                    }
                };
                let mut pending = Self::pending(op, Some(session), instance, step);
                // LC-5, R-20: a `Stop` of a session whose payload already exited completes in the next `pump`.
                pending.fixed_timing = matches!(admit, Admit::Exited | Admit::Lost);
                self.ops.insert(id, pending);
            }
            Op::Signal { id: session, sig } => {
                let instance = instance_of(self, &session);
                let _ = sig;
                self.sessions.get_mut(&session).expect("checked").host_ended = true;
                self.ops.insert(
                    id,
                    Self::pending(op, Some(session), instance, Step::Ready(Next::Forward)),
                );
            }
            Op::Remove { id: session } => {
                let instance = instance_of(self, &session);
                self.sessions.get_mut(&session).expect("checked").admit = Admit::Removing;
                self.enqueue_flow(
                    &session,
                    Flow::Remove(RemoveFlow {
                        op: id,
                        phase: RemovePhase::CloseRoutes,
                        uploads: None,
                        worker_gone: false,
                        deadline: None,
                    }),
                );
                self.ops.insert(
                    id,
                    Self::pending(op, Some(session), instance, Step::Await(Wait::Flow)),
                );
            }
            Op::StopAll => {
                let targets: BTreeSet<SessionId> = self
                    .sessions
                    .values()
                    .filter(|s| !matches!(s.admit, Admit::Removing | Admit::Adopting))
                    .map(|s| s.id.clone())
                    .collect();
                self.ops.insert(
                    id,
                    Self::pending(op, None, None, Step::Ready(Next::StopAllStart(targets))),
                );
            }
            Op::UpdateMetadata { id: session, .. } => {
                let instance = instance_of(self, &session);
                self.ops.insert(
                    id,
                    Self::pending(op, Some(session), instance, Step::Ready(Next::MetaWrite)),
                );
            }
            Op::SetSizePolicy {
                session, policy, ..
            } => {
                let instance = instance_of(self, &session);
                let admit = self.sessions[&session].admit;
                let _ = policy;
                let step = if admit == Admit::Created {
                    self.sessions
                        .get_mut(&session)
                        .expect("checked")
                        .pending_setters += 1;
                    Step::Ready(Next::Setter)
                } else {
                    Step::Ready(Next::Forward)
                };
                self.ops
                    .insert(id, Self::pending(op, Some(session), instance, step));
            }
            Op::SetColorProfile {
                session, profile, ..
            } => {
                let instance = instance_of(self, &session);
                let admit = self.sessions[&session].admit;
                let _ = profile;
                let step = if admit == Admit::Created {
                    self.sessions
                        .get_mut(&session)
                        .expect("checked")
                        .pending_setters += 1;
                    Step::Ready(Next::Setter)
                } else {
                    Step::Ready(Next::Forward)
                };
                self.ops
                    .insert(id, Self::pending(op, Some(session), instance, step));
            }
            Op::SetNotificationPolicy { session, .. } => {
                let instance = instance_of(self, &session);
                let admit = self.sessions[&session].admit;
                let created = admit == Admit::Created;
                let step = if created {
                    self.sessions
                        .get_mut(&session)
                        .expect("checked")
                        .pending_setters += 1;
                    Step::Ready(Next::PolicyWrite)
                } else {
                    Step::Ready(Next::Forward)
                };
                let mut pending = Self::pending(op, Some(session), instance, step);
                pending.created_path = created;
                self.ops.insert(id, pending);
            }
            Op::Resize { session, size } => {
                let instance = instance_of(self, &session);
                let admit = self.sessions[&session].admit;
                // SZ-2, R-20: a `Resize` to the current size is `Applied` with no `SizeChanged`, in the next `pump`. While
                // another `Resize` of the session is in flight, the current size is not known yet (the in-flight one applies
                // later, SZ-3), so the resize goes to the worker.
                let same_size = admit == Admit::Running
                    && self.sessions[&session].size == size
                    && !self.resize_in_flight(&session);
                let step = if admit == Admit::Created {
                    self.sessions
                        .get_mut(&session)
                        .expect("checked")
                        .pending_setters += 1;
                    Step::Ready(Next::Setter)
                } else if same_size {
                    Step::Ready(Next::Complete(OpResult::Ok(OpOutput::Resize(
                        ResizeResult::Applied { actual: size },
                    ))))
                } else {
                    Step::Ready(Next::Forward)
                };
                let mut pending = Self::pending(op, Some(session), instance, step);
                // A2-1 (`Resize` in `Created`) and SZ-2: fixed timing, R-20.
                pending.fixed_timing = admit == Admit::Created || same_size;
                self.ops.insert(id, pending);
            }
            Op::WriteInput { session, .. } => {
                let instance = instance_of(self, &session);
                let s = self.sessions.get_mut(&session).expect("checked");
                s.input_ops += 1;
                s.input_bytes += size;
                let mut pending =
                    Self::pending(op, Some(session), instance, Step::Ready(Next::Forward));
                pending.held_bytes = size;
                self.ops.insert(id, pending);
            }
            Op::Detach { route, .. } => {
                let session = self.routes.get(&route).map(|r| r.session.clone());
                let instance = self.routes.get(&route).map(|r| r.instance.clone());
                self.ops.insert(
                    id,
                    Self::pending(op, session, instance, Step::Ready(Next::Detach)),
                );
            }
            Op::AdoptAll => {
                self.adopt_all_begun = true;
                self.ops.insert(
                    id,
                    Self::pending(op, None, None, Step::Ready(Next::AdoptRead)),
                );
            }
            Op::Adopt { id: session } => {
                let instance = instance_of(self, &session);
                // R-36: no intent; the worker's report alone decides the result.
                self.begin_adoption(id, &session, None);
                self.ops.insert(
                    id,
                    Self::pending(op, Some(session), instance, Step::Await(Wait::Flow)),
                );
            }
            Op::ReadScreen { session, .. }
            | Op::ReadCursor { session }
            | Op::ReadModeFlags { session }
            | Op::CaptureSnapshot { session, .. }
            | Op::ReadFacts { session, .. }
            | Op::SetOutputTap { session, .. } => {
                let instance = instance_of(self, &session);
                self.ops.insert(
                    id,
                    Self::pending(op, Some(session), instance, Step::Ready(Next::Forward)),
                );
            }
            // Every other row is refused by `check_arguments` in this build.
            _ => unreachable!("check_arguments refuses the rows that this build does not offer"),
        }
    }

    /// Starts `flow` now when the session is idle, and after the running flow otherwise (AM-1: `Create` then `Start` before a
    /// `pump`).
    pub(crate) fn enqueue_flow(&mut self, session: &SessionId, flow: Flow) {
        let s = self
            .sessions
            .get_mut(session)
            .expect("a flow has a session");
        if matches!(s.flow, Flow::Idle) {
            s.flow = flow;
        } else {
            s.queue.push_back(flow);
        }
    }

    /// The running flow ended: the next queued flow begins, or the session is idle.
    pub(crate) fn flow_done(&mut self, session: &SessionId) {
        if let Some(s) = self.sessions.get_mut(session) {
            s.flow = s.queue.pop_front().unwrap_or(Flow::Idle);
        }
    }

    /// How a session that has ended ended (A2-1: `SessionEnd`).
    ///
    /// A session reaches `Exited` or `Lost` in the same step that shows that state, so an ended session always shows its
    /// end.
    pub(crate) fn session_end(&self, session: &SessionId) -> End {
        self.sessions
            .get(session)
            .and_then(|s| s.shown)
            .and_then(End::of)
            .expect("an ended session shows its end")
    }

    /// `cancel` (IN-6, A2-1): only a `WriteInput` can be cancelled.
    pub fn cancel(&mut self, op: OpId) -> CancelResult {
        // An id that this handle never minted is `UnknownOp` (IN-6).
        if op.0 == 0 || op.0 >= self.next_op {
            return CancelResult::UnknownOp;
        }
        // An op of a removed instance is `UnknownOp` whether or not its completion is polled (ID-1). The identity is exact for
        // the whole life of the handle.
        if self.retired_ops.contains(op.0) {
            return CancelResult::UnknownOp;
        }
        let Some(pending) = self.ops.get(&op) else {
            // An op that completed and was polled is `TooLate` while its instance lives (A2-1).
            return CancelResult::TooLate;
        };
        if matches!(pending.step, Step::Done) {
            return CancelResult::TooLate;
        }
        if !matches!(pending.op, Op::WriteInput { .. }) {
            return CancelResult::Refused(CancelRefusal::NotCancellable);
        }
        if pending.cancelled {
            return CancelResult::AlreadyAdmitted;
        }
        let req = pending.req;
        let session = pending.session.clone();
        let sent = req.is_some();
        self.ops.get_mut(&op).expect("read above").cancelled = true;
        match (sent, session, req) {
            (true, Some(session), Some(req)) => {
                // Without a link the op completes through the link failure (IN-7); the cancel is admitted either way.
                self.send_msg(&session, botster_core_link::msg::HostMsg::Cancel { req });
                CancelResult::Admitted
            }
            _ => {
                // Not sent yet: nothing reached the PTY, so the cancel is exact (IN-6).
                self.set_step(
                    op,
                    Step::Ready(Next::Complete(OpResult::Ok(OpOutput::Input(InputResult {
                        outcome: WriteOutcome::Cancelled,
                        payload_bytes_written: 0,
                        pty_bytes_written: 0,
                        detail: "cancelled before it was sent".into(),
                    })))),
                );
                CancelResult::Admitted
            }
        }
    }

    /// The sync registration of a route (OU-1, DP-2). The handoff to the worker happens in a `pump` (OR-1, OU-9).
    pub fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        let _ = &client;
        // Every refusal is synchronous and hands the caller's transport back, untouched (DP-2, steward ruling R-19).
        let limits = match self.check_attach(&session, &transport, &options) {
            Ok(limits) => limits,
            Err(error) => return Err(AttachRefused::new(error, transport)),
        };
        let RouteTransport::Stream(endpoint) = transport else {
            unreachable!("check_attach refused every other transport");
        };
        let s = self.sessions.get(&session).expect("checked");
        let route = RouteId(self.next_route);
        self.next_route += 1;
        let instance = s.instance.clone();
        let route_tag = options.route_tag.clone();
        self.routes.insert(
            route,
            crate::engine::RouteEntry {
                session: session.clone(),
                instance,
                route_tag,
            },
        );
        let s = self.sessions.get_mut(&session).expect("checked");
        s.routes.insert(route);
        self.pending_handoffs.push((route, endpoint, options));
        if self.sessions[&session].terminal.is_some() {
            self.flush_handoffs(&session);
        }
        Ok(AttachResult { route, limits })
    }

    /// The refusals of `attach` (A2-1, OU-1): they leave no effect, and the transport is not touched.
    fn check_attach(
        &self,
        session: &SessionId,
        transport: &RouteTransport,
        options: &AttachOptions,
    ) -> Result<AppliedRouteLimits, CoreError> {
        self.require(
            "attach",
            session,
            &[Admit::Starting, Admit::Running, Admit::Exited],
        )?;
        if !matches!(transport, RouteTransport::Stream(_)) {
            return Err(invalid(
                "attach takes a connected stream; a WebRTC route uses AttachWebRtc",
            ));
        }
        let limits = self.applied_route_limits(options)?;
        let s = self.sessions.get(session).expect("checked");
        if s.routes.len() >= self.cfg.limits.routes_per_session as usize {
            return Err(err(
                ErrorCode::RouteLimit,
                format!(
                    "routes_per_session is {}",
                    self.cfg.limits.routes_per_session
                ),
            ));
        }
        Ok(limits)
    }

    fn applied_route_limits(
        &self,
        options: &AttachOptions,
    ) -> Result<AppliedRouteLimits, CoreError> {
        let limits = &self.cfg.limits;
        if !options.file_directory.starts_with('/') {
            return Err(invalid("file_directory is an absolute path"));
        }
        for (name, tag) in [("route_tag", &options.route_tag), ("owner", &options.owner)] {
            if tag
                .as_ref()
                .is_some_and(|t| t.len() > limits.max_route_tag_bytes as usize)
            {
                return Err(invalid_field(
                    name,
                    format!("{name} is over max_route_tag_bytes"),
                ));
            }
        }
        let choices = options.route_limits.unwrap_or_default();
        let frame_cap = limits.effective_max_route_frame_bytes();
        let max_frame_bytes = choices.max_frame_bytes.unwrap_or(frame_cap);
        if max_frame_bytes == 0 || max_frame_bytes > frame_cap {
            return Err(invalid_field(
                "route_limits.max_frame_bytes",
                "max_frame_bytes is from 1 to max_route_frame_bytes",
            ));
        }
        let screen_floor = limits.max_snapshot_bytes + 1;
        let max_screen_frame_bytes = choices.max_screen_frame_bytes.unwrap_or(screen_floor);
        if max_screen_frame_bytes < screen_floor {
            return Err(invalid_field(
                "route_limits.max_screen_frame_bytes",
                "max_screen_frame_bytes is at least max_snapshot_bytes + 1",
            ));
        }
        let query_deadline = match (options.answers_queries, options.query_deadline) {
            (true, None) => {
                return Err(invalid_field(
                    "query_deadline",
                    "answers_queries needs a query_deadline",
                ))
            }
            (true, Some(d)) if d < Duration::from_millis(1) || d > limits.max_query_deadline => {
                return Err(invalid_field(
                    "query_deadline",
                    "query_deadline is from 1 ms to max_query_deadline",
                ))
            }
            (true, Some(d)) => d,
            (false, _) => Duration::ZERO,
        };
        Ok(AppliedRouteLimits {
            max_frame_bytes,
            max_screen_frame_bytes,
            max_history_page_bytes: choices
                .max_history_page_bytes
                .unwrap_or(limits.max_history_page_bytes),
            max_chunk_bytes: choices
                .max_chunk_bytes
                .unwrap_or(limits.default_route_chunk_bytes),
            max_paste_bytes: limits.max_paste_bytes,
            max_query_bytes: limits.max_query_bytes,
            max_query_reply_bytes: limits.max_query_reply_bytes,
            max_file_bytes: limits.max_file_bytes,
            query_deadline,
            stall_deadline: options
                .stall_deadline
                .unwrap_or(limits.reader_progress_deadline),
            stall_close_after: limits.stall_close_after,
            route_input_queue_bytes: limits.route_input_queue_bytes,
        })
    }
}
