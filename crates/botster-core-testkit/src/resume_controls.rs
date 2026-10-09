//! The resume oracle of the testkit (`docs/core-testkit-controls.md`): `oracle_resume` (Core ST-6b, the resume invariant).
//!
//! A fresh libghostty terminal loads Core's pages of a capture and applies the output that the session's model consumed
//! after the capture's `model_rev`. It is compared with the session's model: an independent libghostty terminal that replays
//! every output byte the worker read, with the worker's own step rule (R-7). The subject's model is never read.

use crate::controls::{parse, session_row, ControlRegistry};
use crate::harness::TestkitHarness;
use crate::snapshot_controls::{oracle_resume as resume, CapturePages};
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{CaptureId, ModelRev, SessionId, Size};
use botster_terminal_ghostty::{History, Terminal};
use serde::Deserialize;
use serde_json::Value;
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
}

/// A capture as the host completed it: its `model_rev` and every page that `read_page` gave, in order.
#[derive(Debug, Clone)]
pub(crate) struct CaptureRecord {
    pub(crate) model_rev: ModelRev,
    pub(crate) bytes: Vec<u8>,
}

/// The captures of one handle, recorded as the host's `Completed` passes through the testkit's `poll_events`.
#[derive(Debug, Clone, Default)]
pub(crate) struct CaptureLog(Arc<Mutex<BTreeMap<CaptureId, CaptureRecord>>>);

impl CaptureLog {
    pub(crate) fn insert(&self, capture: CaptureId, record: CaptureRecord) {
        lock(&self.0).insert(capture, record);
    }

    fn get(&self, capture: CaptureId) -> Option<CaptureRecord> {
        lock(&self.0).get(&capture).cloned()
    }
}

/// The worker's step rule over `output`: `vt_write_until_query` until a step consumes nothing, the unconsumed suffix kept.
/// It returns the terminal and the bytes it consumed.
fn replay(size: &Size, output: &[u8]) -> Result<(Terminal, usize), ControlError> {
    let mut terminal = Terminal::new(size, History::On)
        .map_err(|e| ControlError::Bad(format!("oracle_resume: no oracle terminal: {e:?}")))?;
    // Each step that does not end the replay consumes at least one byte, so `rest` is shorter after it.
    let mut rest = output;
    loop {
        let step = terminal.vt_write_until_query(rest).map_err(|e| {
            ControlError::Bad(format!("oracle_resume: the oracle refused a step: {e:?}"))
        })?;
        terminal.drain_events();
        if step.consumed == 0 && step.query.is_none() {
            break;
        }
        rest = rest.get(step.consumed..).unwrap_or_default();
    }
    let consumed = output.len() - rest.len();
    Ok((terminal, consumed))
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    session: SessionId,
    capture: CaptureId,
}

/// `oracle_resume` (Core ST-6b): `{equal}`. Core's pages of `capture`, loaded in a fresh oracle terminal, with every byte
/// that the session's model consumed after the capture's `model_rev`, give the session's model now. A capture that the
/// host did not complete, or a `model_rev` that the worker never had, is `Bad`.
fn oracle_resume(
    harness: &mut TestkitHarness,
    handle: &str,
    args: &Value,
) -> Result<Value, ControlError> {
    let args: Resume = parse(args)?;
    let record = harness
        .captures_of(handle)
        .and_then(|captures| captures.get(args.capture))
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
    let size = log.size.ok_or_else(|| {
        ControlError::Bad(format!("the session {} has no payload", args.session.0))
    })?;
    let cut = log.read_at(record.model_rev).ok_or_else(|| {
        ControlError::Bad(format!(
            "the worker never had the capture's model_rev {}",
            record.model_rev.0
        ))
    })?;
    // The worker had consumed what the step rule consumes of the output before the cut; the rest it consumed later.
    let (_, before) = replay(&size, &log.output[..cut])?;
    let (model, after) = replay(&size, &log.output)?;
    let pages = CapturePages {
        bytes: &record.bytes,
        history: History::On,
        cell_px: size.cell_px,
    };
    resume(&model, &pages, &[&log.output[before..after]])
        .map_err(|e| ControlError::Bad(format!("oracle_resume: {e:?}")))
}

#[cfg(test)]
mod tests;
