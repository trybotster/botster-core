//! The real form of `oracle_resume` (`docs/core-testkit-controls.md`, "The comparator and the real-tier form of
//! `oracle_resume`"; steward ruling R-62). The model is in the worker process, so the harness reads it through an
//! independent anchor and the shared comparator ([`crate::oracle_compare`]):
//!
//! - **O_raw** applies the session's output history: every `pty_output` that the session's probe acknowledged, in order
//!   ([`super::program`]). The harness never asks the subject for these bytes.
//! - **O_resume** loads the capture's pages, then applies every `pty_output` of the session that the transcript sent after the
//!   capture step: the outputs whose mark is not below the capture's.
//! - A **second capture**, taken when the control runs, is decoded into an oracle terminal: the session's model.
//!
//! `equal` is true only if O_resume equals O_raw and the second capture equals O_raw.
//!
//! [`CaptureLayer`] sits between the driver and Core's own composition. It passes every call through unchanged, and it
//! records each `CaptureSnapshot` that the transcript begins: the mark of the output history at the `begin` (the cut) and,
//! when Core completes it, every page that `read_page` gives, before the transcript can release it. For the second capture
//! the control begins its own `CaptureSnapshot` through the layer and pumps Core with the driver's latest `now` until it
//! completes. The events that those pumps take are held, and the driver gets them first, in order, with its next poll; its
//! next `pump` reports them in `events_posted`.
//!
//! Conditions (R-62):
//! 1. The output history must be known. A session whose program is not only `hold` steps, or whose probe has a `fake`, gives
//!    `unsupported_control` `unknown_output_history`.
//! 2. The transcript fences each capture (R-57 `await_title`); a wrong cut gives only a false failure.
//! 3. A session that a `Resize` or a `SetColorProfile` reached during its history needs its own ruling: plain
//!    `unsupported_control`, never a pass.

use super::RealCoreHarness;
use crate::controls::parse;
use crate::oracle_compare::{raw_oracle, resume_holds};
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::*;
use botster_terminal_ghostty::{History, Terminal};
use botster_test_process::Deadline;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

/// The owner of the control's own capture. No transcript names it.
const OWNER: &str = "oracle_resume";

/// A capture that Core completed for the transcript.
#[derive(Debug, Clone)]
struct Recorded {
    session: SessionId,
    /// The mark of the output history at its `begin`: the outputs of a lower mark are before its cut.
    mark: u64,
    /// Every page, in order, or why `read_page` refused one.
    pages: Result<Vec<u8>, String>,
}

/// What the layer shares with the harness.
pub(super) struct Shared {
    core: Box<dyn CoreApi>,
    marks: super::program::Marks,
    /// The `now` of the driver's latest `pump`.
    now: Option<Now>,
    /// The events that the control's pumps took, for the driver's next polls.
    held: VecDeque<Event>,
    /// How many of `held` no `pump` has reported yet.
    unreported: u32,
    /// The transcript's `CaptureSnapshot` begins that Core has not completed: their session and mark.
    begun: BTreeMap<OpId, (SessionId, u64)>,
    captures: BTreeMap<CaptureId, Recorded>,
    /// The sessions that a `Resize` or a `SetColorProfile` reached (condition 3).
    changed: BTreeSet<SessionId>,
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    // A panic while the lock was held leaves plain records behind, so a poisoned lock is still usable.
    shared.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Every page of a completed capture, in order.
fn pages(core: &dyn CoreApi, capture: &Capture) -> Result<Vec<u8>, String> {
    let mut bytes = Vec::new();
    for page in 0..capture.page_count {
        let page = core.read_page(capture.capture, page).map_err(|e| {
            format!(
                "read_page {page} of the capture {}: {e:?}",
                capture.capture.0
            )
        })?;
        bytes.extend_from_slice(&page.bytes.0);
    }
    Ok(bytes)
}

impl Shared {
    /// Records the transcript's completed captures among `events`.
    fn record(&mut self, events: &[Event]) {
        for event in events {
            let Event::Completed { op, result } = event else {
                continue;
            };
            let Some((session, mark)) = self.begun.remove(op) else {
                continue;
            };
            if let OpResult::Ok(OpOutput::Capture(capture)) = result {
                let pages = pages(self.core.as_ref(), capture);
                self.captures.insert(
                    capture.capture,
                    Recorded {
                        session,
                        mark,
                        pages,
                    },
                );
            }
        }
    }

    /// Takes a capture of `session` now and returns its pages, then releases it. The events that the pumps take are held
    /// for the driver. Core completes a capture after one round trip to the worker; the cleanup bound ends a wait that
    /// never ends.
    fn capture_now(&mut self, session: &SessionId) -> Result<Vec<u8>, ControlError> {
        let now = self
            .now
            .ok_or_else(|| ControlError::Bad("oracle_resume: Core was never pumped".into()))?;
        let op = self
            .core
            .begin(Op::CaptureSnapshot {
                session: session.clone(),
                owner: ClientId(OWNER.into()),
            })
            .map_err(|e| ControlError::Bad(format!("oracle_resume: the second capture: {e:?}")))?;
        let wake = self.core.wake_handle();
        let deadline = Deadline::cleanup();
        loop {
            self.core.pump(now);
            let events = self.core.poll_events(usize::MAX);
            self.record(&events);
            let mut done = None;
            for event in events {
                match event {
                    Event::Completed { op: id, result } if id == op => done = Some(result),
                    event => {
                        self.held.push_back(event);
                        self.unreported = self.unreported.saturating_add(1);
                    }
                }
            }
            match done {
                Some(OpResult::Ok(OpOutput::Capture(capture))) => {
                    let pages = pages(self.core.as_ref(), &capture);
                    self.core.release(capture.capture);
                    return pages.map_err(|e| ControlError::Bad(format!("oracle_resume: {e}")));
                }
                Some(result) => {
                    return Err(ControlError::Bad(format!(
                        "oracle_resume: the second capture completed {result:?}"
                    )))
                }
                None if deadline.expired() => {
                    return Err(ControlError::Bad(format!(
                        "oracle_resume: the second capture did not complete within {:?}",
                        deadline.limit()
                    )))
                }
                None => {
                    // timer: deadline — the cleanup bound of the second capture's round trip to the worker
                    wake.wait(deadline.remaining());
                }
            }
        }
    }
}

/// The layer between the driver and Core's own composition (see the module documentation).
pub(super) struct CaptureLayer(Arc<Mutex<Shared>>);

impl CaptureLayer {
    /// The layer over `core`, and the harness's handle of its records.
    pub(super) fn new(
        core: Box<dyn CoreApi>,
        marks: super::program::Marks,
    ) -> (CaptureLayer, Weak<Mutex<Shared>>) {
        let shared = Arc::new(Mutex::new(Shared {
            core,
            marks,
            now: None,
            held: VecDeque::new(),
            unreported: 0,
            begun: BTreeMap::new(),
            captures: BTreeMap::new(),
            changed: BTreeSet::new(),
        }));
        let weak = Arc::downgrade(&shared);
        (CaptureLayer(shared), weak)
    }

    fn shared(&self) -> MutexGuard<'_, Shared> {
        lock(&self.0)
    }
}

impl CoreApi for CaptureLayer {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        let mut shared = self.shared();
        let mark = shared.marks.next();
        let capture = match &op {
            Op::CaptureSnapshot { session, .. } => Some(session.clone()),
            Op::Resize { session, .. } | Op::SetColorProfile { session, .. } => {
                shared.changed.insert(session.clone());
                None
            }
            _ => None,
        };
        let id = shared.core.begin(op)?;
        if let Some(session) = capture {
            shared.begun.insert(id, (session, mark));
        }
        Ok(id)
    }

    fn pump(&mut self, now: Now) -> PumpReport {
        let mut shared = self.shared();
        shared.now = Some(now);
        let mut report = shared.core.pump(now);
        report.events_posted = report.events_posted.saturating_add(shared.unreported);
        shared.unreported = 0;
        report
    }

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let mut shared = self.shared();
        let held = shared.held.len().min(max);
        let mut events: Vec<Event> = shared.held.drain(..held).collect();
        if events.len() < max {
            let more = shared.core.poll_events(max - events.len());
            shared.record(&more);
            events.extend(more);
        }
        events
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        self.shared().core.wake_handle()
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.shared().core.next_deadline()
    }

    fn cancel(&mut self, op: OpId) -> CancelResult {
        self.shared().core.cancel(op)
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.shared().core.get(id)
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.shared().core.list()
    }

    fn status(&self) -> Status {
        self.shared().core.status()
    }

    fn diagnostics(&self) -> Value {
        self.shared().core.diagnostics()
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.shared().core.terminal_state(id)
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.shared().core.read_page(capture, page)
    }

    fn release(&mut self, capture: CaptureId) {
        self.shared().core.release(capture);
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.shared().core.release_owner(client);
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.shared().core.snapshot_formats(session)
    }

    fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        self.shared().core.shadow_answerable_kinds()
    }

    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        self.shared()
            .core
            .attach(client, session, transport, options)
    }

    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError> {
        self.shared().core.tap_read(session, max)
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        self.shared().core.set_silence_threshold(session, threshold)
    }

    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        self.shared().core.service_send(id, lane, frame)
    }

    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError> {
        self.shared().core.service_recv(id, lane)
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        self.shared().core.service_report(id)
    }

    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError> {
        self.shared().core.service_log_tail(id, max)
    }

    fn features(&self) -> Features {
        self.shared().core.features()
    }

    fn limits(&self) -> CoreLimits {
        self.shared().core.limits()
    }

    fn worker_protocol(&self) -> u8 {
        self.shared().core.worker_protocol()
    }

    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        self.shared().core.adoptable_worker_protocols()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.shared().core.worker_protocol_compatibility(protocol)
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        self.shared().core.terminal_identity()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Resume {
    session: SessionId,
    capture: CaptureId,
}

/// Condition 1: the program writes only its `pty_output` bytes. The driver puts the script in `argv[1]`
/// (`{"program": [...], "fake": {...}}`); only `hold` steps and no `fake` qualify.
fn known_output_history(request: &SpawnRequest) -> bool {
    let Some(script) = request
        .argv
        .get(1)
        .and_then(|script| serde_json::from_str::<Value>(script).ok())
    else {
        return false;
    };
    let holds_only = script
        .get("program")
        .and_then(Value::as_array)
        .is_some_and(|steps| {
            steps.iter().all(|step| {
                step.as_object()
                    .is_some_and(|step| step.len() == 1 && step.contains_key("hold"))
            })
        });
    let no_fake = script
        .get("fake")
        .is_none_or(|fake| fake.as_object().is_some_and(serde_json::Map::is_empty));
    holds_only && no_fake
}

/// An oracle terminal that loads `pages`.
fn decoded(pages: &[u8], size: &Size, what: &str) -> Result<Terminal, ControlError> {
    Terminal::from_snapshot(pages, History::On, size.cell_px)
        .map_err(|e| ControlError::Bad(format!("oracle_resume: {what} does not decode: {e:?}")))
}

impl RealCoreHarness {
    /// `oracle_resume {session, capture}` → `{equal}` (Core ST-6b, R-62): see the module documentation.
    pub(super) fn oracle_resume(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let Resume { session, capture } = parse(args)?;
        let (_, row) = self.stored(handle, &session)?;
        if !known_output_history(&row.request) {
            return Err(ControlError::UnsupportedFor(
                "unknown_output_history".into(),
            ));
        }
        let shared = self
            .handles
            .get(handle)
            .and_then(|open| open.resume.upgrade())
            .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
        let mut shared = lock(&shared);
        if shared.changed.contains(&session) {
            return Err(ControlError::Unsupported);
        }
        let recorded = shared
            .captures
            .get(&capture)
            .filter(|recorded| recorded.session == session)
            .cloned()
            .ok_or_else(|| {
                ControlError::Bad(format!(
                    "no completed capture {} of the session {}",
                    capture.0, session.0
                ))
            })?;
        let pages = recorded
            .pages
            .map_err(|e| ControlError::Bad(format!("oracle_resume: {e}")))?;
        let worker = row.worker.ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no worker process", session.0))
        })?;
        let history = self.programs.output(worker.identity());
        let all: Vec<u8> = history
            .iter()
            .flat_map(|(_, bytes)| bytes.clone())
            .collect();
        let after: Vec<u8> = history
            .iter()
            .filter(|(mark, _)| *mark >= recorded.mark)
            .flat_map(|(_, bytes)| bytes.clone())
            .collect();
        let second = shared.capture_now(&session)?;
        drop(shared);
        let size = row.request.size;
        let profile = row.request.color_profile.as_ref();
        let mut resumed = decoded(&pages, &size, "the capture")?;
        resumed.vt_write(&after);
        let model = decoded(&second, &size, "the second capture")?;
        let equal = resume_holds(resumed, model, || raw_oracle(&size, profile, &all))
            .map_err(|e| ControlError::Bad(format!("oracle_resume: {e:?}")))?;
        Ok(json!({ "equal": equal }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(script: Value) -> SpawnRequest {
        SpawnRequest {
            argv: vec!["probe".into(), script.to_string()],
            env: BTreeMap::new(),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            labels: BTreeMap::new(),
            color_profile: None,
            notification_policy: None,
            size_policy: None,
        }
    }

    #[test]
    fn only_a_hold_program_with_no_fake_has_a_known_output_history() {
        for (script, known) in [
            (json!({"program": [{"hold": {}}], "fake": {}}), true),
            (json!({"program": [{"hold": {}}]}), true),
            (json!({"program": [{"hold": {}}, {"hold": {}}]}), true),
            (json!({"program": [{"print": "x"}, {"hold": {}}]}), false),
            (json!({"program": [{"hold": {}, "print": "x"}]}), false),
            (json!({"program": ["hold"]}), false),
            (
                json!({"program": [{"hold": {}}], "fake": {"echo": true}}),
                false,
            ),
            (json!({"program": {"hold": {}}}), false),
            (json!({}), false),
        ] {
            assert_eq!(
                known_output_history(&request(script.clone())),
                known,
                "{script}"
            );
        }
        let mut no_script = request(json!({}));
        no_script.argv.truncate(1);
        assert!(!known_output_history(&no_script));
        let mut not_json = request(json!({}));
        not_json.argv[1] = "hold".into();
        assert!(!known_output_history(&not_json));
    }
}
