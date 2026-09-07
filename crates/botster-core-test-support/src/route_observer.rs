//! Per-attachment observer over the scheme 2 terminal stream.
//!
//! One `RouteObserver` follows one attachment `(route, generation)`. It keeps
//! independent facts, not a phase list, and checks only what the protocol
//! states: `ATTACH_STATE attached` before `SNAPSHOT_READY`; no `OUTPUT`
//! before `SNAPSHOT_READY` on an epoch; nothing after
//! `PROCESS_EXIT` or `ATTACH_STATE failed`. `MODES`, `INPUT_RESULT`,
//! `ROUTE_RESYNC`, `SNAPSHOT_HISTORY`, and `OUTPUT` otherwise interleave
//! freely. No payload byte is retained.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::time::{Duration, Instant};

use botster_terminal_protocol::{
    AttachStateCode, HistoryUnavailableReason, InputResultBody, ModesBody, TerminalFrame,
    TerminalKind,
};
use botster_terminal_protocol_client::{decode_terminal_event, TerminalEvent};

use crate::diagnostics::StepFailure;

/// Most in-flight input operations one attachment may have outstanding.
pub const MAX_OUTSTANDING_INPUTS: usize = 32;
/// Frame kinds remembered for diagnostics.
pub const LAST_KINDS: usize = 16;

/// One delivered frame together with the epoch from its container header.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedFrame {
    /// Stream epoch stamped on the container, not in the body.
    pub stream_epoch: u32,
    /// The scheme 2 body.
    pub frame: TerminalFrame,
}

/// Incremental byte matcher that keeps only `needle.len() - 1` carry bytes.
#[derive(Debug, Clone)]
struct MarkerMatch {
    needle: Vec<u8>,
    carry: Vec<u8>,
    seen: bool,
}

impl MarkerMatch {
    fn feed(&mut self, bytes: &[u8]) {
        if self.seen || self.needle.is_empty() {
            return;
        }
        let mut window = std::mem::take(&mut self.carry);
        window.extend_from_slice(bytes);
        if window
            .windows(self.needle.len())
            .any(|candidate| candidate == self.needle.as_slice())
        {
            self.seen = true;
            return;
        }
        let keep = self.needle.len() - 1;
        let start = window.len().saturating_sub(keep);
        self.carry = window[start..].to_vec();
    }
}

/// Facts observed for one attachment.
#[derive(Debug, Clone)]
pub struct RouteState {
    /// Last `ATTACH_STATE` code seen.
    pub attach_state: Option<AttachStateCode>,
    /// `SNAPSHOT_READY` seen on the current epoch.
    pub ready_seen: bool,
    /// `SNAPSHOT_FINISH` seen on the current epoch.
    pub finish_seen: bool,
    /// Current stream epoch; changes only through `ROUTE_RESYNC`.
    pub stream_epoch: u32,
    /// Last `MODES` body.
    pub modes: Option<ModesBody>,
    /// `PROCESS_EXIT` code once seen.
    pub process_exit: Option<Option<i32>>,
    /// Last `HISTORY_UNAVAILABLE` reason.
    pub history_unavailable: Option<HistoryUnavailableReason>,
    /// Total `OUTPUT` bytes seen on the current epoch. Bytes are not kept.
    pub output_bytes: u64,
    /// Frames whose epoch did not equal the current epoch and were ignored.
    pub stale_frames: u64,
    /// Operation ids the test expects a result for.
    outstanding: BTreeSet<u64>,
    /// Results received for expected ids and not yet taken.
    results: BTreeMap<u64, InputResultBody>,
    /// Count of settled operations.
    pub settled: u64,
    marker: Option<MarkerMatch>,
    last_kinds: VecDeque<TerminalKind>,
    terminal_seen: bool,
}

impl RouteState {
    fn new() -> Self {
        Self {
            attach_state: None,
            ready_seen: false,
            finish_seen: false,
            stream_epoch: 0,
            modes: None,
            process_exit: None,
            history_unavailable: None,
            output_bytes: 0,
            stale_frames: 0,
            outstanding: BTreeSet::new(),
            results: BTreeMap::new(),
            settled: 0,
            marker: None,
            last_kinds: VecDeque::with_capacity(LAST_KINDS),
            terminal_seen: false,
        }
    }

    /// Whether the marker set by [`RouteObserver::set_marker`] has appeared
    /// in `OUTPUT` since it was set.
    #[must_use]
    pub fn marker_seen(&self) -> bool {
        self.marker.as_ref().is_some_and(|marker| marker.seen)
    }

    /// Whether the route reached `ATTACH_STATE attached`.
    #[must_use]
    pub fn attached(&self) -> bool {
        self.attach_state == Some(AttachStateCode::Attached)
    }

    /// Whether a result for `operation_id` has arrived and not been taken.
    #[must_use]
    pub fn has_result(&self, operation_id: u64) -> bool {
        self.results.contains_key(&operation_id)
    }

    /// Operation ids still awaiting a result.
    #[must_use]
    pub fn outstanding(&self) -> impl Iterator<Item = u64> + '_ {
        self.outstanding.iter().copied()
    }

    /// The last frame kinds seen, oldest first.
    #[must_use]
    pub fn last_kinds(&self) -> Vec<TerminalKind> {
        self.last_kinds.iter().copied().collect()
    }

    fn note_kind(&mut self, kind: TerminalKind) {
        if self.last_kinds.len() == LAST_KINDS {
            self.last_kinds.pop_front();
        }
        self.last_kinds.push_back(kind);
    }
}

/// Observer for one attachment.
#[derive(Debug, Clone)]
pub struct RouteObserver {
    layer: &'static str,
    session_id: String,
    subscription_id: String,
    generation: u64,
    state: RouteState,
}

impl RouteObserver {
    /// Follow the attachment `generation` of `subscription_id` on `session_id`.
    #[must_use]
    pub fn new(
        layer: &'static str,
        session_id: &str,
        subscription_id: &str,
        generation: u64,
    ) -> Self {
        Self {
            layer,
            session_id: session_id.to_string(),
            subscription_id: subscription_id.to_string(),
            generation,
            state: RouteState::new(),
        }
    }

    /// Attachment generation this observer follows.
    #[must_use]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Current facts.
    #[must_use]
    pub fn state(&self) -> &RouteState {
        &self.state
    }

    /// A failure record carrying this route's identity and last kinds.
    #[must_use]
    pub fn failure(&self, step: &'static str, cause: impl Into<String>) -> StepFailure {
        StepFailure::new(self.layer, step, cause)
            .on_route(
                &self.session_id,
                &self.subscription_id,
                self.generation,
                self.state.stream_epoch,
            )
            .with_last_kinds(self.state.last_kinds())
    }

    /// Register an operation id the test will send. Rejects a duplicate and
    /// a 33rd outstanding id.
    pub fn expect_result(&mut self, operation_id: u64) -> Result<(), StepFailure> {
        if self.state.outstanding.contains(&operation_id) {
            return Err(self.failure(
                "expect_result",
                format!("operation {operation_id} is already outstanding"),
            ));
        }
        if self.state.results.contains_key(&operation_id) {
            return Err(self.failure(
                "expect_result",
                format!("operation {operation_id} has a result that was not taken"),
            ));
        }
        if self.state.outstanding.len() + self.state.results.len() >= MAX_OUTSTANDING_INPUTS {
            return Err(self.failure(
                "expect_result",
                format!("{MAX_OUTSTANDING_INPUTS} operations or untaken results are tracked"),
            ));
        }
        self.state.outstanding.insert(operation_id);
        Ok(())
    }

    /// Take the result for `operation_id`, removing it.
    pub fn take_result(&mut self, operation_id: u64) -> Option<InputResultBody> {
        self.state.results.remove(&operation_id)
    }

    /// Start matching `needle` against `OUTPUT` from now on.
    pub fn set_marker(&mut self, needle: &[u8]) {
        self.state.marker = Some(MarkerMatch {
            needle: needle.to_vec(),
            carry: Vec::new(),
            seen: false,
        });
    }

    /// Observe one delivered frame with the epoch from its container header.
    pub fn observe(&mut self, stream_epoch: u32, frame: &TerminalFrame) -> Result<(), StepFailure> {
        let event = decode_terminal_event(frame)
            .map_err(|error| self.failure("decode", format!("undecodable body: {error}")))?;
        let kind = frame.kind();
        self.state.note_kind(kind);
        if self.state.terminal_seen {
            return Err(self.failure(
                "observe",
                format!("{kind:?} arrived after the route's terminal frame"),
            ));
        }
        // INPUT_RESULT is attachment-scoped and exempt from the epoch filter.
        // ROUTE_RESYNC changes the accepted epoch when both exact equalities hold.
        match &event {
            TerminalEvent::InputResult(_) => {}
            TerminalEvent::RouteResync(resync) => {
                if resync.from_epoch != self.state.stream_epoch {
                    return Err(self.failure(
                        "observe",
                        format!(
                            "ROUTE_RESYNC from_epoch {} does not equal accepted epoch {}",
                            resync.from_epoch, self.state.stream_epoch
                        ),
                    ));
                }
                if stream_epoch != resync.to_epoch {
                    return Err(self.failure(
                        "observe",
                        format!(
                            "ROUTE_RESYNC to_epoch {} does not equal container epoch {stream_epoch}",
                            resync.to_epoch
                        ),
                    ));
                }
            }
            _ if stream_epoch != self.state.stream_epoch => {
                self.state.stale_frames += 1;
                return Ok(());
            }
            _ => {}
        }
        match event {
            TerminalEvent::AttachState(code) => {
                self.state.attach_state = Some(code);
                if code == AttachStateCode::Failed {
                    self.state.terminal_seen = true;
                }
            }
            TerminalEvent::SnapshotReady(_) => {
                if !self.state.attached() {
                    return Err(self.failure("observe", "SNAPSHOT_READY before attached"));
                }
                self.state.ready_seen = true;
            }
            TerminalEvent::SnapshotHistory(_) => {}
            TerminalEvent::SnapshotFinish => self.state.finish_seen = true,
            TerminalEvent::Output(body) => {
                if !self.state.ready_seen {
                    return Err(self.failure(
                        "observe",
                        format!(
                            "OUTPUT before SNAPSHOT_READY on epoch {}",
                            self.state.stream_epoch
                        ),
                    ));
                }
                self.state.output_bytes += body.body().len() as u64;
                if let Some(marker) = self.state.marker.as_mut() {
                    marker.feed(body.body());
                }
            }
            TerminalEvent::ProcessExit(exit) => {
                self.state.process_exit = Some(exit.code);
                self.state.terminal_seen = true;
            }
            TerminalEvent::Modes(modes) => self.state.modes = Some(modes),
            TerminalEvent::InputResult(result) => {
                if !self.state.outstanding.remove(&result.operation_id) {
                    return Err(self.failure(
                        "observe",
                        format!(
                            "INPUT_RESULT for operation {} that is not outstanding",
                            result.operation_id
                        ),
                    ));
                }
                self.state.settled += 1;
                self.state.results.insert(result.operation_id, result);
            }
            TerminalEvent::HistoryUnavailable(reason) => {
                self.state.history_unavailable = Some(reason);
            }
            TerminalEvent::RouteResync(resync) => {
                self.state.stream_epoch = resync.to_epoch;
                self.state.ready_seen = false;
                self.state.finish_seen = false;
                self.state.output_bytes = 0;
            }
        }
        Ok(())
    }

    /// Feed frames from `feed` until `pred` holds or `deadline` passes.
    ///
    /// `feed` returns the next delivered frame, `Ok(None)` when none is
    /// available right now (the caller pumps its host between calls), or an
    /// error. `wait` runs between empty feeds so the caller can block on its
    /// host's wake source.
    pub fn wait_until(
        &mut self,
        step: &'static str,
        deadline: Duration,
        mut feed: impl FnMut() -> Result<Option<ObservedFrame>, StepFailure>,
        mut wait: impl FnMut(),
        pred: impl Fn(&RouteState) -> bool,
    ) -> Result<(), StepFailure> {
        let started = Instant::now();
        loop {
            if pred(&self.state) {
                return Ok(());
            }
            if started.elapsed() >= deadline {
                return Err(self
                    .failure(step, "deadline passed")
                    .with_timing(deadline, started));
            }
            match feed()? {
                Some(observed) => self
                    .observe(observed.stream_epoch, &observed.frame)
                    .map_err(|failure| failure.with_timing(deadline, started))?,
                None => wait(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use botster_terminal_protocol::{
        encode_attach_state, encode_input_result, encode_output, encode_process_exit,
        encode_route_resync, encode_snapshot_ready, InputOutcome,
    };

    fn observer() -> RouteObserver {
        RouteObserver::new("core", "session", "route", 7)
    }

    fn attached(observer: &mut RouteObserver) {
        observer
            .observe(
                0,
                &encode_attach_state(AttachStateCode::Attached).expect("attach"),
            )
            .expect("attached");
        observer
            .observe(0, &encode_snapshot_ready(b"GHOSTSNP").expect("ready"))
            .expect("ready");
    }

    #[test]
    fn output_before_ready_on_an_epoch_is_a_failure() {
        let mut observer = observer();
        observer
            .observe(
                0,
                &encode_attach_state(AttachStateCode::Attached).expect("attach"),
            )
            .expect("attached");
        let failure = observer
            .observe(0, &encode_output(b"x").expect("output"))
            .expect_err("output before ready");
        assert!(failure.cause.contains("OUTPUT before SNAPSHOT_READY"));
    }

    #[test]
    fn a_resync_changes_the_epoch_and_ignores_non_current_containers() {
        let mut observer = observer();
        attached(&mut observer);
        observer
            .observe(1, &encode_route_resync(0, 1).expect("resync"))
            .expect("resync");
        assert_eq!(observer.state().stream_epoch, 1);
        assert!(!observer.state().ready_seen);
        observer
            .observe(0, &encode_output(b"old").expect("output"))
            .expect("non-current container is ignored");
        assert_eq!(observer.state().stale_frames, 1);
        assert!(observer
            .observe(1, &encode_output(b"new").expect("output"))
            .is_err());
    }

    #[test]
    fn the_marker_matches_across_output_boundaries_without_retaining_bytes() {
        let mut observer = observer();
        attached(&mut observer);
        observer.set_marker(b"echo:MARK");
        observer
            .observe(0, &encode_output(b"...ech").expect("output"))
            .expect("first half");
        assert!(!observer.state().marker_seen());
        observer
            .observe(0, &encode_output(b"o:MARK\n").expect("output"))
            .expect("second half");
        assert!(observer.state().marker_seen());
        assert_eq!(observer.state().output_bytes, 13);
    }

    #[test]
    fn results_are_bounded_and_settle_by_removal() {
        let mut observer = observer();
        attached(&mut observer);
        for id in 1..=32 {
            observer.expect_result(id).expect("within the window");
        }
        assert!(observer.expect_result(33).is_err());
        assert!(observer.expect_result(5).is_err(), "duplicate");
        let result = InputResultBody {
            operation_id: 5,
            outcome: InputOutcome::Written,
            accepted_payload_bytes: Some(1),
            written_pty_bytes: Some(1),
            mode_bits: 0,
            detail: String::new(),
        };
        observer
            .observe(9, &encode_input_result(&result).expect("result"))
            .expect("results ignore the epoch filter");
        assert_eq!(observer.state().settled, 1);
        assert_eq!(observer.state().outstanding().count(), 31);
        assert_eq!(
            observer.take_result(5).map(|r| r.outcome),
            Some(InputOutcome::Written)
        );
        assert!(observer.take_result(5).is_none());
        assert!(
            observer
                .observe(0, &encode_input_result(&result).expect("result"))
                .is_err(),
            "a settled id is not outstanding"
        );
    }

    #[test]
    fn nothing_may_follow_process_exit() {
        let mut observer = observer();
        attached(&mut observer);
        observer
            .observe(0, &encode_process_exit(Some(0)).expect("exit"))
            .expect("exit");
        assert_eq!(observer.state().process_exit, Some(Some(0)));
        let failure = observer
            .observe(0, &encode_output(b"late").expect("output"))
            .expect_err("nothing after exit");
        assert!(failure.cause.contains("after the route's terminal frame"));
        assert!(failure
            .to_string()
            .starts_with("layer=core step=observe session_id=session"));
    }
}
