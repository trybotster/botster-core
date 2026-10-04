//! Core ST-6b and A8-2 through a fresh Core session at every byte offset.
//! R-30 permits independent native size measurement with framing computed from the format spec.

use botster_core_conformance::ControlError;
use botster_core_contract::prelude::{Size, SnapshotFormat};
use botster_terminal_ghostty::{
    snapshot_format, Continuation, History, Terminal, CONTINUATION_LIMIT,
};
use serde_json::{json, Value};

/// The adapter reads every Core capture page before it returns `Offered`.
pub enum CutCapture {
    Offered(Vec<u8>),
    SnapshotTooLarge,
    Other(String),
}

/// A fresh subject session. Production dispatch must drive real Core through injected edges.
/// A unit test adapter does not prove a conformance id.
pub trait CutSession {
    /// Verify no edge fault, allocation limit, or held snapshot is active.
    fn no_injected_failure(&self) -> bool;
    fn format(&self) -> SnapshotFormat;
    fn max_snapshot_bytes(&self) -> usize;
    /// Write the program output, run the fence, and return the exact consumed chunks in order.
    fn write_and_fence(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, ControlError>;
    /// Run real `CaptureSnapshot` to its completion and read every offered page.
    fn capture(&mut self) -> Result<CutCapture, ControlError>;
    fn model(&self) -> &Terminal;
}

/// An independent format source. Return the largest framing and per-capture field sizes from the published spec.
/// Return `None` for an unknown format or spec. Never obtain framing from the subject's capture.
pub trait FormatFraming {
    fn overhead(&self, format: &SnapshotFormat, native_bytes: usize) -> Option<usize>;
}

/// The current protocol spec has not supplied framing to this helper.
/// This source keeps refusal classification inconclusive until the protocol owner supplies it.
pub struct UnknownFraming;
impl FormatFraming for UnknownFraming {
    fn overhead(&self, _: &SnapshotFormat, _: usize) -> Option<usize> {
        None
    }
}

/// The dispatch adapter resolves `size` and decodes `corpus_hex` with the contract codec before it calls this helper.
pub struct EveryCutInput {
    pub size: Size,
    pub history: History,
    pub corpus: Vec<Vec<u8>>,
    pub beyond_limit: Vec<String>,
}

/// Check all offsets, including zero and the full length. Do not normalize terminal state or replace Core's pages.
pub fn oracle_resume_every_cut(
    input: &EveryCutInput,
    framing: &dyn FormatFraming,
    mut fresh: impl FnMut(&Size, History) -> Result<Box<dyn CutSession>, ControlError>,
) -> Result<Value, ControlError> {
    let mut corpus = input.corpus.clone();
    for kind in &input.beyond_limit {
        corpus.push(beyond_limit(kind)?);
    }
    let (mut cuts_checked, mut offered, mut refused_beyond_limit) = (0u64, 0u64, 0u64);
    let mut mismatches = Vec::new();
    let mut inconclusive = false;
    for (item, bytes) in corpus.iter().enumerate() {
        for offset in 0..=bytes.len() {
            cuts_checked += 1;
            let mut subject = fresh(&input.size, input.history)?;
            if !subject.no_injected_failure() {
                return Err(ControlError::Bad(
                    "every-cut requires no injected resource failure".into(),
                ));
            }
            let mut oracle = Terminal::new(&input.size, input.history).map_err(library_error)?;
            oracle
                .set_continuation_max_bytes(bytes.len())
                .map_err(library_error)?;
            let consumed_before = subject.write_and_fence(&bytes[..offset])?;
            feed(&mut oracle, &consumed_before);
            let pending = match oracle.continuation().map_err(library_error)? {
                Continuation::Retained(bytes) => Some(bytes.len()),
                Continuation::Unavailable => None,
            };
            let format = subject.format();
            let fits = fit(&oracle, &format, framing, subject.max_snapshot_bytes());
            let semantic_failure = subject
                .model()
                .vt_processing_error()
                .map_err(library_error)?;
            let retention = subject.model().continuation().map_err(library_error)?;
            let capture = subject.capture()?;
            if !subject.no_injected_failure() {
                return Err(ControlError::Bad(
                    "every-cut resource conditions changed during capture".into(),
                ));
            }
            if semantic_failure {
                mismatches
                    .push(json!({"item": item, "offset": offset, "reason": "semantic failure"}));
                continue;
            }
            match capture {
                CutCapture::Offered(bytes) => {
                    offered += 1;
                    let mut restored = match Terminal::from_snapshot(
                        &bytes,
                        input.history,
                        input.size.cell_px,
                    ) {
                        Ok(terminal) => terminal,
                        Err(error) => {
                            mismatches.push(json!({"item": item, "offset": offset, "reason": format!("decode: {error:?}")}));
                            continue;
                        }
                    };
                    let consumed_after = subject.write_and_fence(&corpus[item][offset..])?;
                    feed(&mut restored, &consumed_after);
                    match (restored.snapshot(), subject.model().snapshot()) {
                        (Ok(actual), Ok(expected)) if actual == expected => {}
                        _ => mismatches.push(
                            json!({"item": item, "offset": offset, "reason": "resume differs"}),
                        ),
                    }
                }
                CutCapture::SnapshotTooLarge => {
                    if fits != Some(true) || pending.is_none() {
                        inconclusive = true;
                    } else if pending.is_some_and(|length| length > CONTINUATION_LIMIT) {
                        refused_beyond_limit += 1;
                    } else {
                        let reason = if retention == Continuation::Unavailable {
                            "retention failure within limit"
                        } else {
                            "refused within limit"
                        };
                        mismatches.push(json!({"item": item, "offset": offset, "reason": reason}));
                    }
                }
                CutCapture::Other(error) => {
                    if fits == Some(true) {
                        mismatches.push(json!({"item": item, "offset": offset, "reason": error}));
                    } else {
                        inconclusive = true;
                    }
                }
            }
        }
    }
    Ok(json!({"cuts_checked": cuts_checked, "offered": offered,
        "refused_beyond_limit": refused_beyond_limit, "mismatches": mismatches, "inconclusive": inconclusive}))
}

fn library_error(error: botster_terminal_ghostty::Error) -> ControlError {
    ControlError::Bad(format!("libghostty: {error:?}"))
}

fn feed(terminal: &mut Terminal, chunks: &[Vec<u8>]) {
    for chunk in chunks {
        terminal.vt_write(chunk);
    }
}

fn fit(
    oracle: &Terminal,
    format: &SnapshotFormat,
    framing: &dyn FormatFraming,
    maximum: usize,
) -> Option<bool> {
    let native_format = snapshot_format();
    if format.name != native_format.name || format.version != native_format.version {
        return None;
    }
    if oracle.vt_processing_error().ok()? || oracle.image_storage_limit().ok()? != 0 {
        return None;
    }
    let native_bytes = oracle.snapshot().ok()?.len();
    let overhead = framing.overhead(format, native_bytes)?;
    Some(native_bytes.checked_add(overhead)? <= maximum)
}

fn beyond_limit(kind: &str) -> Result<Vec<u8>, ControlError> {
    let start: &[u8] = match kind {
        "osc" => b"\x1b]2;",
        "dcs" => b"\x1bP",
        "apc" => b"\x1b_",
        _ => {
            return Err(ControlError::Bad(
                "beyond_limit needs osc, dcs, or apc".into(),
            ))
        }
    };
    let mut bytes = start.to_vec();
    bytes.resize(CONTINUATION_LIMIT + 16, b'x');
    bytes.extend_from_slice(b"\x1b\\");
    Ok(bytes)
}
