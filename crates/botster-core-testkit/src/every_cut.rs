//! Core ST-6b and A8-2 through a fresh Core session at every byte offset.
//! R-30 (contracts-v0.1.14) permits independent native size measurement with framing computed from the format spec.

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

#[derive(Default)]
struct Refusals {
    beyond_limit: u64,
    mismatches: Vec<Value>,
    inconclusive: bool,
}

impl Refusals {
    fn record(
        &mut self,
        item: usize,
        offset: usize,
        pending: Option<usize>,
        fits: Option<bool>,
        unavailable: bool,
    ) {
        if fits != Some(true) || pending.is_none() {
            self.inconclusive = true;
        } else if pending.is_some_and(|length| length > CONTINUATION_LIMIT) {
            self.beyond_limit += 1;
        } else {
            let reason = if unavailable {
                "retention failure within limit"
            } else {
                "refused within limit"
            };
            self.mismatches
                .push(json!({"item": item, "offset": offset, "reason": reason}));
        }
    }
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
    let (mut cuts_checked, mut offered) = (0u64, 0u64);
    let mut refusals = Refusals::default();
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
            let maximum = subject.max_snapshot_bytes();
            let fits = fit(&oracle, &format, framing, maximum);
            if fits != Some(true) {
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
            let semantic_failure_after_capture = subject
                .model()
                .vt_processing_error()
                .map_err(library_error)?;
            let retention_after_capture = subject.model().continuation().map_err(library_error)?;
            if either_failure(semantic_failure, semantic_failure_after_capture) {
                refusals
                    .mismatches
                    .push(json!({"item": item, "offset": offset, "reason": "semantic failure"}));
                continue;
            }
            let retention_failed = either_failure(
                retention == Continuation::Unavailable,
                retention_after_capture == Continuation::Unavailable,
            );
            if pending.is_none() {
                inconclusive = true;
            } else if failed_retention_within_limit(pending, retention_failed) {
                refusals.mismatches.push(json!({"item": item, "offset": offset, "reason": "retention failure within limit"}));
            }
            match capture {
                CutCapture::Offered {
                    pages,
                    after_capture,
                } => {
                    offered += 1;
                    if framing
                        .overhead(&format, pages.len())
                        .and_then(|overhead| pages.len().checked_add(overhead))
                        .is_some_and(|total| total > maximum)
                    {
                        refusals.mismatches.push(json!({"item": item, "offset": offset, "reason": "offered capture exceeds maximum"}));
                    }
                    let mut restored = match Terminal::from_snapshot(
                        &pages,
                        input.history,
                        input.size.cell_px,
                    ) {
                        Ok(terminal) => terminal,
                        Err(error) => {
                            refusals.mismatches.push(json!({"item": item, "offset": offset, "reason": format!("decode: {error:?}")}));
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
                        refusals.mismatches.push(json!({"item": item, "offset": offset, "reason": "semantic failure after suffix"}));
                        continue;
                    }
                    match (restored.snapshot(), subject.model().snapshot()) {
                        (Ok(actual), Ok(expected)) if actual == expected => {}
                        _ => refusals.mismatches.push(
                            json!({"item": item, "offset": offset, "reason": "resume differs"}),
                        ),
                    }
                }
                CutCapture::SnapshotTooLarge => {
                    refusals.record(
                        item,
                        offset,
                        pending,
                        fits,
                        retention == Continuation::Unavailable,
                    );
                }
                CutCapture::Other(error) => {
                    if fits == Some(true) {
                        refusals
                            .mismatches
                            .push(json!({"item": item, "offset": offset, "reason": error}));
                    } else {
                        inconclusive = true;
                    }
                }
            }
        }
    }
    Ok(json!({"cuts_checked": cuts_checked, "offered": offered,
        "refused_beyond_limit": refusals.beyond_limit, "mismatches": refusals.mismatches, "inconclusive": inconclusive || refusals.inconclusive}))
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
    if !valid_oracle(
        oracle.vt_processing_error().ok()?,
        oracle.image_storage_limit().ok()?,
    ) {
        return None;
    }
    let native_bytes = oracle.snapshot().ok()?.len();
    let overhead = framing.overhead(format, native_bytes)?;
    Some(native_bytes.checked_add(overhead)? <= maximum)
}

fn valid_oracle(semantic_failure: bool, image_limit: u64) -> bool {
    !semantic_failure && image_limit == 0
}

fn either_failure(before: bool, after: bool) -> bool {
    before || after
}

fn failed_retention_within_limit(pending: Option<usize>, unavailable: bool) -> bool {
    pending.is_some_and(|length| length <= CONTINUATION_LIMIT) && unavailable
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
    bytes.resize(
        CONTINUATION_LIMIT
            .checked_add(16)
            .expect("continuation limit fits usize"),
        b'x',
    );
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
        wrong: bool,
        other: bool,
        maximum: usize,
        exact_maximum: bool,
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
            if self.exact_maximum {
                self.terminal.snapshot().unwrap().len()
            } else {
                self.maximum
            }
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
            if self.other {
                return Ok(CutCapture::Other("capture failed".into()));
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
            if self.wrong {
                let mut model = Terminal::new(&input().size, self.history).unwrap();
                model.vt_write(b"different");
                return Ok(CutCapture::Offered {
                    pages: model.snapshot().unwrap(),
                    after_capture: Vec::new(),
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
            wrong: false,
            other: false,
            maximum: usize::MAX,
            exact_maximum: false,
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
        for mismatch in known["mismatches"].as_array().unwrap() {
            assert_eq!(mismatch["reason"], "refused within limit");
        }
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
    fn offered_ground_cuts_cannot_hide_unavailable_retention() {
        let input = input();
        let result = oracle_resume_every_cut(&input, &TestFraming(0), |size, history| {
            let mut subject = session(size, history, Rc::new(RefCell::new(Vec::new())));
            subject.at_ground = true;
            // This unit adapter deliberately loses retention. It is not a conformance run.
            subject.terminal.set_continuation_max_bytes(0).unwrap();
            Ok(Box::new(subject))
        })
        .unwrap();
        let mut failed_cuts = Vec::new();
        for (item, bytes) in input.corpus.iter().enumerate() {
            for offset in 0..=bytes.len() {
                let mut diagnostic = Terminal::new(&input.size, input.history).unwrap();
                diagnostic.vt_write(&bytes[..offset]);
                let mut failed = Terminal::new(&input.size, input.history).unwrap();
                failed.set_continuation_max_bytes(0).unwrap();
                failed.vt_write(&bytes[..offset]);
                if let Continuation::Retained(pending) = diagnostic.continuation().unwrap() {
                    if pending.len() <= CONTINUATION_LIMIT
                        && failed.continuation().unwrap() == Continuation::Unavailable
                    {
                        failed_cuts.push((item, offset));
                    }
                }
            }
        }
        let mismatches = result["mismatches"].as_array().unwrap();
        assert_eq!(mismatches.len(), failed_cuts.len());
        assert!(!mismatches.is_empty());
        for (mismatch, (item, offset)) in mismatches.iter().zip(failed_cuts) {
            assert_eq!(mismatch["item"], item);
            assert_eq!(mismatch["offset"], offset);
            assert_eq!(mismatch["reason"], "retention failure within limit");
        }
        assert_eq!(result["inconclusive"], false);
    }

    #[test]
    fn an_offered_capture_above_the_known_size_cannot_report_success() {
        let input = input();
        let result = oracle_resume_every_cut(&input, &TestFraming(0), |size, history| {
            let mut subject = session(size, history, Rc::new(RefCell::new(Vec::new())));
            subject.maximum = 0;
            Ok(Box::new(subject))
        })
        .unwrap();
        assert_eq!(result["inconclusive"], true);
        assert_eq!(
            result["mismatches"].as_array().unwrap().len(),
            result["cuts_checked"].as_u64().unwrap() as usize
        );
        for mismatch in result["mismatches"].as_array().unwrap() {
            assert_eq!(mismatch["reason"], "offered capture exceeds maximum");
        }
    }

    #[test]
    fn an_offered_capture_at_the_known_maximum_is_valid() {
        let input = input();
        let result = oracle_resume_every_cut(&input, &TestFraming(0), |size, history| {
            let mut subject = session(size, history, Rc::new(RefCell::new(Vec::new())));
            subject.exact_maximum = true;
            Ok(Box::new(subject))
        })
        .unwrap();
        assert_eq!(result["inconclusive"], false);
        assert_eq!(result["mismatches"], json!([]));
    }

    #[test]
    fn either_observation_keeps_failures_and_the_retention_limit_is_inclusive() {
        for (before, after, failed) in [
            (false, false, false),
            (true, false, true),
            (false, true, true),
            (true, true, true),
        ] {
            assert_eq!(either_failure(before, after), failed);
        }
        for unavailable in [false, true] {
            for pending in [
                Some(0),
                Some(CONTINUATION_LIMIT - 1),
                Some(CONTINUATION_LIMIT),
            ] {
                assert_eq!(
                    failed_retention_within_limit(pending, unavailable),
                    unavailable
                );
            }
            for pending in [None, Some(CONTINUATION_LIMIT + 1)] {
                assert!(!failed_retention_within_limit(pending, unavailable));
            }
        }
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

    #[test]
    fn refusal_classification_checks_both_sides_of_the_limit_and_retention() {
        let mut result = Refusals::default();
        result.record(0, 0, Some(CONTINUATION_LIMIT), Some(true), false);
        assert_eq!(result.beyond_limit, 0);
        assert_eq!(result.mismatches[0]["reason"], "refused within limit");
        result.record(0, 1, Some(CONTINUATION_LIMIT - 1), Some(true), true);
        assert_eq!(
            result.mismatches[1]["reason"],
            "retention failure within limit"
        );
        result.record(0, 2, Some(CONTINUATION_LIMIT + 1), Some(true), true);
        assert_eq!(result.beyond_limit, 1);
        result.record(0, 3, Some(CONTINUATION_LIMIT + 2), Some(true), false);
        assert_eq!(result.beyond_limit, 2);
        assert_eq!(result.mismatches.len(), 2);
        assert!(!result.inconclusive);
        for (pending, fits) in [(None, Some(true)), (Some(0), None), (Some(0), Some(false))] {
            let mut result = Refusals::default();
            result.record(0, 0, pending, fits, false);
            assert!(result.inconclusive);
            assert!(result.mismatches.is_empty());
            assert_eq!(result.beyond_limit, 0);
        }
        for (failed, limit, valid) in [
            (false, 0, true),
            (true, 0, false),
            (false, 1, false),
            (true, 1, false),
        ] {
            assert_eq!(valid_oracle(failed, limit), valid);
        }
    }

    #[test]
    fn a_valid_snapshot_of_different_state_and_other_capture_errors_fail() {
        let input = input();
        for framing in [&TestFraming(0) as &dyn FormatFraming, &UnknownFraming] {
            for wrong in [true, false] {
                let result = oracle_resume_every_cut(&input, framing, |size, history| {
                    let mut subject = session(size, history, Rc::new(RefCell::new(Vec::new())));
                    subject.wrong = wrong;
                    subject.other = !wrong;
                    Ok(Box::new(subject))
                })
                .unwrap();
                if wrong || framing.overhead(&snapshot_format(), 0).is_some() {
                    assert_eq!(
                        result["mismatches"].as_array().unwrap().len(),
                        result["cuts_checked"].as_u64().unwrap() as usize
                    );
                } else {
                    assert_eq!(result["mismatches"], json!([]));
                    assert_eq!(result["inconclusive"], true);
                }
            }
        }
    }

    #[test]
    fn each_subject_configuration_field_must_match() {
        let input = input();
        for field in ["rows", "cols", "history"] {
            assert!(
                oracle_resume_every_cut(&input, &UnknownFraming, |size, history| {
                    let mut different = *size;
                    let mut history = history;
                    match field {
                        "rows" => different.rows += 1,
                        "cols" => different.cols += 1,
                        _ => history = History::Off,
                    }
                    Ok(Box::new(session(
                        &different,
                        history,
                        Rc::new(RefCell::new(Vec::new())),
                    )))
                })
                .is_err()
            );
        }
    }
}
