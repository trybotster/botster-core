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
    /// Include consumed output after the snapshot revision but before the suffix write.
    /// This also supports a worker that chooses a preceding ground state and replays pending output.
    Offered {
        pages: Vec<u8>,
        after_capture: Vec<Vec<u8>>,
    },
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
    fn history(&self) -> History;
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
            if u32::from(subject.model().rows()) != input.size.rows
                || u32::from(subject.model().cols()) != input.size.cols
                || subject.history() != input.history
            {
                return Err(ControlError::Bad(
                    "every-cut subject configuration differs".into(),
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
            if fits.is_none() {
                inconclusive = true;
            }
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
                CutCapture::Offered {
                    pages,
                    after_capture,
                } => {
                    offered += 1;
                    let mut restored = match Terminal::from_snapshot(
                        &pages,
                        input.history,
                        input.size.cell_px,
                    ) {
                        Ok(terminal) => terminal,
                        Err(error) => {
                            mismatches.push(json!({"item": item, "offset": offset, "reason": format!("decode: {error:?}")}));
                            continue;
                        }
                    };
                    feed(&mut restored, &after_capture);
                    let consumed_after = subject.write_and_fence(&corpus[item][offset..])?;
                    feed(&mut restored, &consumed_after);
                    if !subject.no_injected_failure() {
                        return Err(ControlError::Bad(
                            "every-cut resource conditions changed during suffix".into(),
                        ));
                    }
                    if subject
                        .model()
                        .vt_processing_error()
                        .map_err(library_error)?
                    {
                        mismatches.push(json!({"item": item, "offset": offset, "reason": "semantic failure after suffix"}));
                        continue;
                    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::rc::Rc;

    struct NativeSession {
        terminal: Terminal,
        history: History,
        refuse: bool,
        corrupt: bool,
        fault: bool,
        at_ground: bool,
        prefix: Vec<u8>,
        trace: Rc<RefCell<Vec<&'static str>>>,
    }

    impl CutSession for NativeSession {
        fn no_injected_failure(&self) -> bool {
            !self.fault
        }
        fn format(&self) -> SnapshotFormat {
            snapshot_format()
        }
        fn max_snapshot_bytes(&self) -> usize {
            usize::MAX
        }
        fn history(&self) -> History {
            self.history
        }
        fn write_and_fence(&mut self, bytes: &[u8]) -> Result<Vec<Vec<u8>>, ControlError> {
            self.trace.borrow_mut().push("write/fence");
            self.terminal.vt_write(bytes);
            self.prefix.extend_from_slice(bytes);
            Ok(vec![bytes.to_vec()])
        }
        fn capture(&mut self) -> Result<CutCapture, ControlError> {
            self.trace.borrow_mut().push("capture/pages");
            if self.refuse {
                return Ok(CutCapture::SnapshotTooLarge);
            }
            if self.corrupt {
                return Ok(CutCapture::Offered {
                    pages: Vec::new(),
                    after_capture: Vec::new(),
                });
            }
            if self.at_ground {
                let model = Terminal::new(&input().size, self.history).unwrap();
                return Ok(CutCapture::Offered {
                    pages: model.snapshot().unwrap(),
                    after_capture: vec![self.prefix.clone()],
                });
            }
            Ok(CutCapture::Offered {
                pages: self.terminal.snapshot().unwrap(),
                after_capture: Vec::new(),
            })
        }
        fn model(&self) -> &Terminal {
            &self.terminal
        }
    }

    // This synthetic source tests the measurement interface. It is never conformance fit evidence.
    struct TestFraming(usize);
    impl FormatFraming for TestFraming {
        fn overhead(&self, _: &SnapshotFormat, _: usize) -> Option<usize> {
            Some(self.0)
        }
    }

    fn input() -> EveryCutInput {
        EveryCutInput {
            size: Size {
                rows: 2,
                cols: 8,
                cell_px: None,
            },
            history: History::On,
            corpus: vec![b"A\x1b[31mB".to_vec(), b"\x1b]2;t\x1b\\".to_vec()],
            beyond_limit: Vec::new(),
        }
    }

    fn session(
        size: &Size,
        history: History,
        trace: Rc<RefCell<Vec<&'static str>>>,
    ) -> NativeSession {
        NativeSession {
            terminal: Terminal::new(size, history).unwrap(),
            history,
            refuse: false,
            corrupt: false,
            fault: false,
            at_ground: false,
            prefix: Vec::new(),
            trace,
        }
    }

    #[test]
    fn every_offset_gets_a_fresh_session_and_capture_before_the_suffix() {
        let input = input();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let mut sessions = 0;
        let result = oracle_resume_every_cut(&input, &TestFraming(0), |size, history| {
            sessions += 1;
            Ok(Box::new(session(size, history, trace.clone())))
        })
        .unwrap();
        let cuts = input
            .corpus
            .iter()
            .map(|bytes| bytes.len() + 1)
            .sum::<usize>();
        assert_eq!(sessions, cuts);
        assert_eq!(result["cuts_checked"], cuts);
        assert_eq!(result["offered"], cuts);
        assert_eq!(result["refused_beyond_limit"], 0);
        assert_eq!(result["mismatches"], json!([]));
        assert_eq!(result["inconclusive"], false);
        assert_eq!(
            *trace.borrow(),
            ["write/fence", "capture/pages", "write/fence"].repeat(cuts)
        );
    }

    #[test]
    fn refusals_need_independent_fit_evidence() {
        let input = input();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let run = |framing: &dyn FormatFraming| {
            oracle_resume_every_cut(&input, framing, |size, history| {
                let mut subject = session(size, history, trace.clone());
                subject.refuse = true;
                Ok(Box::new(subject))
            })
            .unwrap()
        };
        let unknown = run(&UnknownFraming);
        assert_eq!(unknown["inconclusive"], true);
        assert_eq!(unknown["mismatches"], json!([]));
        let known = run(&TestFraming(0));
        assert_eq!(known["inconclusive"], false);
        assert_eq!(
            known["mismatches"].as_array().unwrap().len(),
            known["cuts_checked"].as_u64().unwrap() as usize
        );
    }

    #[test]
    fn a_ground_cut_replays_consumed_output_after_the_capture_revision() {
        let input = input();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let result = oracle_resume_every_cut(&input, &TestFraming(0), |size, history| {
            let mut subject = session(size, history, trace.clone());
            subject.at_ground = true;
            Ok(Box::new(subject))
        })
        .unwrap();
        assert_eq!(result["mismatches"], json!([]));
        assert_eq!(result["inconclusive"], false);
    }

    #[test]
    fn malformed_pages_and_injected_failures_do_not_pass() {
        let input = input();
        let trace = Rc::new(RefCell::new(Vec::new()));
        let result = oracle_resume_every_cut(&input, &TestFraming(0), |size, history| {
            let mut subject = session(size, history, trace.clone());
            subject.corrupt = true;
            Ok(Box::new(subject))
        })
        .unwrap();
        assert_eq!(
            result["mismatches"].as_array().unwrap().len(),
            result["cuts_checked"].as_u64().unwrap() as usize
        );
        assert!(
            oracle_resume_every_cut(&input, &UnknownFraming, |size, history| {
                let mut subject = session(size, history, trace.clone());
                subject.fault = true;
                Ok(Box::new(subject))
            })
            .is_err()
        );
        assert!(
            oracle_resume_every_cut(&input, &UnknownFraming, |_, _| Err(ControlError::Bad(
                "factory failed".into()
            )))
            .is_err()
        );
    }

    #[test]
    fn fit_counts_native_bytes_and_independent_framing_at_the_boundary() {
        let input = input();
        let oracle = Terminal::new(&input.size, input.history).unwrap();
        let native_bytes = oracle.snapshot().unwrap().len();
        let format = snapshot_format();
        assert_eq!(
            fit(&oracle, &format, &TestFraming(7), native_bytes + 7),
            Some(true)
        );
        assert_eq!(
            fit(&oracle, &format, &TestFraming(7), native_bytes + 6),
            Some(false)
        );
        assert_eq!(
            fit(&oracle, &format, &TestFraming(usize::MAX), usize::MAX),
            None
        );
        assert_eq!(fit(&oracle, &format, &UnknownFraming, usize::MAX), None);
        let mut unknown = format.clone();
        unknown.version += 1;
        assert_eq!(fit(&oracle, &unknown, &TestFraming(0), usize::MAX), None);
        unknown = format;
        unknown.name.push('x');
        assert_eq!(fit(&oracle, &unknown, &TestFraming(0), usize::MAX), None);
    }

    #[test]
    fn generated_strings_exceed_the_native_continuation_limit() {
        let input = input();
        for kind in ["osc", "dcs", "apc"] {
            let bytes = beyond_limit(kind).unwrap();
            let mut oracle = Terminal::new(&input.size, input.history).unwrap();
            oracle.set_continuation_max_bytes(bytes.len()).unwrap();
            oracle.vt_write(&bytes[..bytes.len() - 2]);
            match oracle.continuation().unwrap() {
                Continuation::Retained(pending) => assert!(pending.len() > CONTINUATION_LIMIT),
                Continuation::Unavailable => panic!("diagnostic oracle lost continuation"),
            }
            oracle.vt_write(&bytes[bytes.len() - 2..]);
            assert_eq!(
                oracle.continuation().unwrap(),
                Continuation::Retained(Vec::new())
            );
        }
        assert!(beyond_limit("unknown").is_err());
    }
}
