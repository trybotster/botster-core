//! The resume oracle of the testkit (`docs/core-testkit-controls.md`): `oracle_resume` (Core ST-6b, the resume invariant).
//!
//! Steward ruling R-67: the testkit uses the two comparisons of the real form (R-62), through the shared comparator
//! ([`crate::oracle_compare`]). O_raw is a fresh oracle terminal that applies the output that the session's model consumed,
//! from the model log, which the testkit fills at the program edge (`worker.rs`, before `Input::PtyOutput`); the worker crate
//! never fills it. (a) O_resume, a fresh oracle terminal that loads Core's pages of the capture and applies the output that the
//! model consumed after the capture's `model_rev`, equals O_raw. (b) The live model's snapshot (`Worker::model_snapshot`),
//! decoded into an oracle terminal, equals O_raw. The model has then applied every logged byte that it consumed. So the
//! expected side never passes through the encoder, and the comparator touches only oracle terminals, never the live model.
//! An independent libghostty terminal replays the output that the worker read with the worker's own step rule only to find
//! the consumed part (R-7).
//!
//! Condition 3 (R-62) holds on this tier too: a session that a `Resize` or a `SetColorProfile` reached gives
//! `unsupported_control` (`crate::oracle_controls::Changed`). Before it, the control replayed with the size and the profile
//! of the create request and gave a wrong `{"equal": false}`.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use crate::oracle_compare::{raw_oracle, resume_holds};
use crate::oracle_controls::Changed;
use crate::snapshot_controls::CapturePages;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{CaptureId, ColorProfile, ModelRev, Op, SessionId, Size};
use botster_terminal_ghostty::{History, Terminal};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

pub(crate) fn register_controls(registry: &mut ControlRegistry) {
    registry.register("oracle_resume", oracle_resume);
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The testkit has no panic that leaves its state half written, so a poisoned lock still holds usable state.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// What reached the terminal model of one worker: the payload's size, every output byte that the worker read, in order,
/// and the position in that output at which the worker's `model_rev` first had each value.
#[derive(Debug, Clone, Default)]
pub(crate) struct ModelLog {
    size: Option<Size>,
    output: Vec<u8>,
    revs: Vec<(ModelRev, usize)>,
}

impl ModelLog {
    /// The log of a payload of `size` that has written nothing yet.
    pub(crate) fn new(size: Size) -> ModelLog {
        ModelLog {
            size: Some(size),
            ..ModelLog::default()
        }
    }

    /// The worker read these output bytes.
    pub(crate) fn read(&mut self, bytes: &[u8]) {
        self.output.extend_from_slice(bytes);
    }

    /// The worker's `model_rev` after its last input. Only a new value is kept, at the output read so far.
    pub(crate) fn rev(&mut self, rev: ModelRev) {
        if self.revs.last().map(|(r, _)| *r) != Some(rev) {
            self.revs.push((rev, self.output.len()));
        }
    }

    /// The output that the worker had read when its `model_rev` first was `rev`.
    fn read_at(&self, rev: ModelRev) -> Option<usize> {
        self.revs.iter().find(|(r, _)| *r == rev).map(|(_, at)| *at)
    }

    /// The payload's size, and how many bytes of the output the worker's model consumed: the step rule over all of it.
    fn consumed(&self, session: &SessionId) -> Result<(Size, usize), ControlError> {
        let size = self.size.ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no payload", session.0))
        })?;
        Ok((size, replay(&size, &self.output)?))
    }

    /// O_raw (R-67): a fresh oracle terminal of the payload's size and `profile` that applied every byte that the worker's
    /// model consumed (`oracle_screen`).
    pub(crate) fn raw(
        &self,
        session: &SessionId,
        profile: Option<&ColorProfile>,
    ) -> Result<Terminal, ControlError> {
        let (size, after) = self.consumed(session)?;
        raw_oracle(&size, profile, &self.output[..after])
            .map_err(|e| ControlError::Bad(format!("no oracle terminal: {e:?}")))
    }
}

/// A capture as the host completed it: its `model_rev` and every page that `read_page` gave, in order.
#[derive(Debug, Clone)]
pub(crate) struct CaptureRecord {
    pub(crate) model_rev: ModelRev,
    pub(crate) bytes: Vec<u8>,
}

/// The captures of one handle, recorded as the host's `Completed` passes through the testkit's `poll_events`, and the
/// sessions that a `Resize` or a `SetColorProfile` reached, recorded as the host begins the op (condition 3).
#[derive(Debug, Clone, Default)]
pub(crate) struct CaptureLog(Arc<Mutex<Captures>>);

#[derive(Debug, Default)]
struct Captures {
    records: BTreeMap<CaptureId, CaptureRecord>,
    changed: Changed,
}

impl CaptureLog {
    pub(crate) fn insert(&self, capture: CaptureId, record: CaptureRecord) {
        lock(&self.0).records.insert(capture, record);
    }

    fn get(&self, capture: CaptureId) -> Option<CaptureRecord> {
        lock(&self.0).records.get(&capture).cloned()
    }

    /// The host of this handle begins `op`.
    pub(crate) fn begun(&self, op: &Op) {
        lock(&self.0).changed.begun(op);
    }

    /// Condition 3 for `session` ([`Changed::check`]).
    pub(crate) fn check(&self, session: &SessionId) -> Result<(), ControlError> {
        lock(&self.0).changed.check(session)
    }
}

/// The worker's step rule over `output`: `vt_write_until_query` until a step consumes nothing, the unconsumed suffix kept.
/// It returns the bytes it consumed.
fn replay(size: &Size, output: &[u8]) -> Result<usize, ControlError> {
    let mut terminal = Terminal::new(size, History::On)
        .map_err(|e| ControlError::Bad(format!("the replay: no oracle terminal: {e:?}")))?;
    // Each step that does not end the replay consumes at least one byte, so `rest` is shorter after it.
    let mut rest = output;
    loop {
        let step = terminal.vt_write_until_query(rest).map_err(|e| {
            ControlError::Bad(format!("the replay: the oracle refused a step: {e:?}"))
        })?;
        terminal.drain_events();
        if step.consumed == 0 && step.query.is_none() {
            break;
        }
        rest = rest.get(step.consumed..).unwrap_or_default();
    }
    Ok(output.len() - rest.len())
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    session: SessionId,
    capture: CaptureId,
}

/// `oracle_resume` (Core ST-6b, R-67): `{equal}`. Core's pages of `capture`, loaded in a fresh oracle terminal, with every
/// byte that the session's model consumed after the capture's `model_rev`, and the live session model now, each equal O_raw. A
/// session that a `Resize` or a `SetColorProfile` reached gives `unsupported_control` (condition 3). A capture that the
/// host did not complete, a `model_rev` that the worker never had, or a worker with no model is `Bad`.
fn oracle_resume(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Resume = parse(args)?;
    let captures = harness
        .captures_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    captures.check(&args.session)?;
    let record = captures
        .get(args.capture)
        .ok_or_else(|| ControlError::Bad(format!("no completed capture {}", args.capture.0)))?;
    let row = session_row(harness, handle, &args.session)?;
    let worker = row.worker.ok_or_else(|| {
        ControlError::Bad(format!(
            "the session {} has no worker process",
            args.session.0
        ))
    })?;
    let log = harness
        .workers()
        .model_log(worker.identity())
        .map_err(ControlError::Bad)?;
    let (size, after) = log.consumed(&args.session)?;
    let cut = log.read_at(record.model_rev).ok_or_else(|| {
        ControlError::Bad(format!(
            "the worker never had the capture's model_rev {}",
            record.model_rev.0
        ))
    })?;
    // The worker had consumed what the step rule consumes of the output before the cut; the rest it consumed later.
    let before = replay(&size, &log.output[..cut])?;
    let live = harness
        .workers()
        .model_snapshot(worker.identity())
        .map_err(ControlError::Bad)?
        .ok_or_else(|| ControlError::Bad(format!("the session {} has no model", args.session.0)))?;
    let pages = CapturePages {
        bytes: &record.bytes,
        history: History::On,
        cell_px: size.cell_px,
    };
    let mut restored = pages
        .restore()
        .map_err(|e| ControlError::Bad(format!("oracle_resume: {e:?}")))?;
    restored.vt_write(&log.output[before..after]);
    let live = CapturePages {
        bytes: &live,
        history: History::On,
        cell_px: size.cell_px,
    }
    .restore()
    .map_err(|e| ControlError::Bad(format!("oracle_resume: the live model's snapshot: {e:?}")))?;
    let profile = row.request.color_profile.as_ref();
    let equal = resume_holds(restored, live, || {
        raw_oracle(&size, profile, &log.output[..after])
    })
    .map_err(|e| ControlError::Bad(format!("oracle_resume: {e:?}")))?;
    Ok(json!({ "equal": equal }))
}

#[cfg(test)]
mod tests;
