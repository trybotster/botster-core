//! Host input: `WriteInput` admission, the guards and the typed outcomes (Core 5.1, 5.1A, 9A, A2-2).
//!
//! FakeCore never encodes (R-5). The session script says what each semantic event writes; this file implements the clauses that
//! decide whether it is written: validation, the admission bound, the guards, the mode checks and the zero outcomes.

use crate::state::*;
use crate::Inner;
use botster_core_contract::prelude::*;
use botster_fake_route::script::{Outcome, SessionScript};
use botster_fake_route::{HostOutcome, Planned, StartView};
use botster_route_codec::prelude::{
    Key, KeyEvent, Modifier, MouseAction, MouseButton, MouseEncoding, MouseTracking,
};

const LANE_FULL: u32 = 1;
const SESSION_ENDED: u32 = 3;
const STALE: u32 = 5;
const MODE_PRECONDITION: u32 = 7;
const NOT_REPORTED: u32 = 8;
const UNSUPPORTED: u32 = 100;

const WHATS: [UnsupportedWhat; 6] = [
    UnsupportedWhat::PixelPosition,
    UnsupportedWhat::ProducedText,
    UnsupportedWhat::NamedKey,
    UnsupportedWhat::WheelDirection,
    UnsupportedWhat::Coordinate,
    UnsupportedWhat::ReleaseEvent,
];

fn invalid(why: &str) -> CoreError {
    CoreError::new(ErrorCode::InvalidInput { field: None }, why)
}

/// The reason of a refusal code that the worker carries for the host.
fn reason_of(code: u32) -> NotWrittenReason {
    match code {
        LANE_FULL => NotWrittenReason::LaneFull,
        SESSION_ENDED => NotWrittenReason::SessionEnded,
        STALE => NotWrittenReason::Stale,
        MODE_PRECONDITION => NotWrittenReason::ModePreconditionFailed,
        NOT_REPORTED => NotWrittenReason::NotReported,
        n => NotWrittenReason::Unsupported {
            what: WHATS[(n - UNSUPPORTED) as usize],
        },
    }
}

/// What a script outcome says for a semantic event, `times` times.
fn scripted(outcome: &Outcome, times: u64) -> Result<Planned, u32> {
    match outcome {
        Outcome::Bytes(b) => Ok(Planned {
            payload_bytes: b.len() as u64 * times,
            bytes: b.repeat(times as usize),
        }),
        Outcome::NotReported => Err(NOT_REPORTED),
        Outcome::Unsupported(what) => {
            let what: UnsupportedWhat =
                serde_json::from_value(serde_json::json!(what)).unwrap_or(UnsupportedWhat::Other);
            Err(UNSUPPORTED + WHATS.iter().position(|w| *w == what).unwrap_or(0) as u32)
        }
    }
}

/// Whether the tracking mode reports this mouse event (Core IN-9 rule 3).
fn mouse_reported(mode: MouseTracking, action: MouseAction, button: MouseButton) -> bool {
    match mode {
        MouseTracking::X10 => action == MouseAction::Press,
        MouseTracking::Normal => action != MouseAction::Move,
        MouseTracking::ButtonEvent => action != MouseAction::Move || button != MouseButton::None,
        MouseTracking::AnyEvent => true,
        _ => false,
    }
}

impl Inner {
    /// Core IN-1, IN-5, IN-9: the refusals of `WriteInput` at `begin`.
    pub(crate) fn admit_write(
        &mut self,
        session: &SessionId,
        payload: &InputPayload,
        _guard: &Option<Guard>,
    ) -> Result<(Lane, Option<InstanceId>), CoreError> {
        let row = self
            .adm
            .get(session)
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownSession, "no such session"))?;
        if !matches!(
            row.state,
            Adm::Running | Adm::Stopping | Adm::Exited | Adm::Lost
        ) {
            return Self::err(ErrorCode::WrongState, "not admitted in this state");
        }
        self.validate_input(payload)?;
        let cost = self.worst_case(session, payload);
        // Core IN-5: a payload over `max_paste_bytes` is permanent; a write over a lane bound is `LaneFull`.
        if cost > self.limits.max_paste_bytes {
            return Self::err(ErrorCode::PayloadTooLarge, "over max_paste_bytes");
        }
        let row = self.adm.get_mut(session).expect("row");
        if row.io_ops + 1 > self.limits.input_ops_per_session as usize
            || row.io_bytes + cost > self.limits.input_retained_bytes
        {
            return Self::err(ErrorCode::LaneFull, "a lane bound is reached");
        }
        row.io_ops += 1;
        row.io_bytes += cost;
        Ok((Lane::Io(session.clone()), Some(row.instance.clone())))
    }

    /// Core IN-9, IN-9: a semantic input that can never be written is `InvalidInput`.
    fn validate_input(&self, payload: &InputPayload) -> Result<(), CoreError> {
        match payload {
            InputPayload::Key(k) => {
                // Core IN-9: an explicit `repeat` is valid only with `press`, is at least 1 and at most `max_key_repeat`.
                if let Some(n) = k.repeat {
                    if k.event != KeyEvent::Press
                        || n == 0
                        || u32::from(n) > self.limits.max_key_repeat
                    {
                        return Err(invalid("repeat is not valid"));
                    }
                }
                // Core IN-9: `shifted_key` is valid only when `shift` is in `mods`.
                if k.shifted_key.is_some() && !k.mods.contains(&Modifier::Shift) {
                    return Err(invalid("shifted_key needs shift"));
                }
                if matches!(&k.key, Key::Char(c) if c.chars().count() != 1) {
                    return Err(invalid("a char key is one scalar"));
                }
                Ok(())
            }
            InputPayload::Mouse(m) => {
                let wheel_button = matches!(
                    m.button,
                    MouseButton::WheelUp
                        | MouseButton::WheelDown
                        | MouseButton::WheelLeft
                        | MouseButton::WheelRight
                );
                // Core IN-9: `action` is `wheel` exactly when `button` is a `wheel_*` value; `notches` is at least 1 and only with a wheel.
                if (m.action == MouseAction::Wheel) != wheel_button
                    || m.notches == Some(0)
                    || (m.notches.is_some() && m.action != MouseAction::Wheel)
                {
                    return Err(invalid("action, button or notches are not valid"));
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// Core IN-9, IN-9: the worst-case size of the write, counted before the mode is known. The script supplies the per-event size.
    pub(crate) fn worst_case(&self, session: &SessionId, payload: &InputPayload) -> u64 {
        let admission = self
            .sessions
            .get(session)
            .map(|s| s.script.fake.admission)
            .unwrap_or_default();
        match payload {
            InputPayload::Bytes { bytes } | InputPayload::Paste { bytes, .. } => {
                bytes.0.len() as u64
            }
            InputPayload::Text { text } => text.len() as u64,
            InputPayload::Key(k) => u64::from(k.repeat.unwrap_or(1)) * admission.key,
            InputPayload::Mouse(m) => u64::from(m.notches.unwrap_or(1)) * admission.mouse,
            InputPayload::Focus { .. } => admission.focus,
            _ => 0,
        }
    }

    /// The decision at the start of the write transaction (Core IN-8, IN-9, IN-10, AM-2). The worker calls it at the one admission point.
    pub(crate) fn decision(
        payload: InputPayload,
        guard: Option<Guard>,
        script: SessionScript,
    ) -> crate::Decider {
        Box::new(move |view: &StartView<'_>| -> Result<Planned, u32> {
            // Core IN-7: a write to an ended session writes nothing.
            if view.exited {
                return Err(SESSION_ENDED);
            }
            if let Some(g) = guard {
                // Core IN-10: the input guard passes iff no input of that class was received or admitted after `rev`.
                let rev = |class| match class {
                    SourceClass::Client => view.client_rev,
                    _ => view.host_rev,
                };
                let input_ok = g.input.is_none_or(|i| rev(i.source_class) == i.rev.0);
                // Core IN-10: the terminal guard passes iff the model revision still equals the token.
                let model_ok = g.model_rev.is_none_or(|m| m.0 == view.model_rev);
                if !(input_ok && model_ok) {
                    return Err(STALE);
                }
            }
            let writes = &script.writes;
            match &payload {
                InputPayload::Bytes { bytes } => Ok(Planned {
                    payload_bytes: bytes.0.len() as u64,
                    bytes: bytes.0.clone(),
                }),
                InputPayload::Text { text } => Ok(Planned {
                    payload_bytes: text.len() as u64,
                    bytes: text.clone().into_bytes(),
                }),
                InputPayload::Paste {
                    bytes,
                    require_bracketed,
                } => {
                    // Core IN-8: decided with the mode at the start of the write; a failed precondition writes zero bytes.
                    if *require_bracketed && !view.modes.bracketed_paste {
                        return Err(MODE_PRECONDITION);
                    }
                    let mut out = bytes.0.clone();
                    if view.modes.bracketed_paste {
                        // Core IN-2, IN-8: the markers that Core adds are script data (R-5).
                        out = writes.paste.clone();
                    }
                    Ok(Planned {
                        payload_bytes: bytes.0.len() as u64,
                        bytes: out,
                    })
                }
                InputPayload::Key(k) => scripted(&writes.key, u64::from(k.repeat.unwrap_or(1))),
                InputPayload::Mouse(m) => {
                    // Core IN-9 rule 3: reported only when the tracking mode reports that kind.
                    if !mouse_reported(view.modes.mouse_tracking, m.action, m.button) {
                        return Err(NOT_REPORTED);
                    }
                    // Core IN-9: under `SgrPixels` a mouse event without `x` and `y` is typed zero.
                    if view.modes.mouse_encoding == MouseEncoding::SgrPixels
                        && (m.x.is_none() || m.y.is_none())
                    {
                        return Err(UNSUPPORTED);
                    }
                    scripted(&writes.mouse, u64::from(m.notches.unwrap_or(1)))
                }
                InputPayload::Focus { focused } => {
                    // Core IN-9 rule 4: focus is reported only when `focus_reporting` is on.
                    if !view.modes.focus_reporting {
                        return Err(NOT_REPORTED);
                    }
                    scripted(
                        if *focused {
                            &writes.focus_in
                        } else {
                            &writes.focus_out
                        },
                        1,
                    )
                }
                _ => Err(NOT_REPORTED),
            }
        })
    }

    /// The `InputResult` of a finished host write (Core IN-2, A2-2).
    pub(crate) fn input_result(outcome: &HostOutcome) -> InputResult {
        let result = |outcome, payload, pty| InputResult {
            outcome,
            payload_bytes_written: payload,
            pty_bytes_written: pty,
            detail: String::new(),
        };
        match outcome {
            HostOutcome::Written { pty, payload } => result(WriteOutcome::Written, *payload, *pty),
            HostOutcome::Refused(code) => result(WriteOutcome::NotWritten(reason_of(*code)), 0, 0),
            HostOutcome::Failed { pty, payload } => result(WriteOutcome::Failed, *payload, *pty),
            // Core IN-6: a cancel after progress leaves a certain partial write; before any progress the write is cancelled.
            HostOutcome::Cancelled { pty, payload } if *pty > 0 => {
                result(WriteOutcome::Partial, *payload, *pty)
            }
            HostOutcome::Cancelled { .. } => result(WriteOutcome::Cancelled, 0, 0),
            // Core IN-2: a write that was sent and not acknowledged is never reported as a certain zero.
            HostOutcome::Unknown { max_payload } => result(
                WriteOutcome::Unknown {
                    max_payload_bytes: *max_payload,
                },
                0,
                0,
            ),
            // Core IN-7, A2-2: a write that was never sent completes `NotWritten(SessionEnded)`.
            HostOutcome::NeverSent => result(
                WriteOutcome::NotWritten(NotWrittenReason::SessionEnded),
                0,
                0,
            ),
        }
    }
}

impl Inner {
    /// A host write that reaches its turn: it joins the worker's input queue, and the worker decides at the start of the transaction.
    pub(crate) fn submit_write(
        &mut self,
        id: OpId,
        session: &SessionId,
        payload: InputPayload,
        guard: Option<Guard>,
        _stage: u8,
    ) -> crate::pump::Step {
        use crate::pump::{ok, Step};
        let Some(s) = self.sessions.get_mut(session) else {
            return Step::Retry;
        };
        let never = |reason| {
            ok(OpOutput::Input(InputResult {
                outcome: WriteOutcome::NotWritten(reason),
                payload_bytes_written: 0,
                pty_bytes_written: 0,
                detail: String::new(),
            }))
        };
        // Core IN-6: a write that is cancelled before it started completes `Cancelled` with exact zero.
        if self.cancels.contains(&id) {
            return ok(OpOutput::Input(InputResult {
                outcome: WriteOutcome::Cancelled,
                payload_bytes_written: 0,
                pty_bytes_written: 0,
                detail: String::new(),
            }));
        }
        let script = s.script.fake.clone();
        match s.worker.as_mut() {
            // Core IN-7, A2-2: a write that cannot reach a worker is certain zero.
            None => never(NotWrittenReason::SessionEnded),
            Some(w) => {
                w.host_submit(id.0, Self::decision(payload, guard, script));
                Step::Wait
            }
        }
    }
}
