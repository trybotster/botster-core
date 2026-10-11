//! The capture layer of the real `oracle_resume` (steward rulings R-62 and R-69). The control itself is the real harness's
//! (`real/resume.rs`, the `slow` feature).
//!
//! [`CaptureLayer`] sits between the driver and Core's own composition. It passes every call and its result through
//! unchanged, and it records each `CaptureSnapshot` that the transcript begins: the mark of the output history at the
//! `begin` (the cut) and, when Core completes it, every page that `read_page` gives, before the transcript can release it.
//! It also records each session that a `Resize` or a `SetColorProfile` reached (R-62 condition 3).
//!
//! The second capture runs only at a quiet point (R-69). The control begins its own `CaptureSnapshot` through the layer,
//! pumps Core with the driver's latest `now` (no clock step) and polls. A poll that returns any event other than the
//! capture's own `Completed` gives `unsupported_control` `events_pending`: the control never holds or re-injects an event,
//! and the transcript ends there.
//!
//! Only the real harness composes it. It is outside that module so that its tests run in the default tier too: a layer that
//! drops a call would otherwise stall every real transcript before a test could fail.

use botster_core_conformance::ControlError;
use botster_core_contract::prelude::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

/// The mark that the next acknowledged `pty_output` gets (the real harness's output history).
pub(crate) type Marks = Box<dyn Fn() -> u64 + Send + Sync>;

/// The owner of the control's own capture. No transcript names it.
const OWNER: &str = "oracle_resume";

/// A capture that Core completed for the transcript.
#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    session: SessionId,
    /// The mark of the output history at its `begin`: the outputs of a lower mark are before its cut.
    mark: u64,
    /// Every page, in order, or why `read_page` refused one.
    pub(crate) pages: Result<Vec<u8>, String>,
}

/// What `oracle_resume {session, capture}` reads from the layer (R-62, R-69).
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Resumed {
    /// The mark of the output history at the capture's `begin`: the outputs of a lower mark are before its cut.
    pub(crate) mark: u64,
    /// The pages of the transcript's capture.
    pub(crate) pages: Vec<u8>,
    /// The pages of the second capture, taken now.
    pub(crate) second: Vec<u8>,
}

/// What the layer shares with the harness.
pub(crate) struct Shared {
    core: Box<dyn CoreApi>,
    marks: Marks,
    /// The `now` of the driver's latest `pump`.
    now: Option<Now>,
    /// The transcript's `CaptureSnapshot` begins that Core has not completed: their session and mark.
    begun: BTreeMap<OpId, (SessionId, u64)>,
    /// The transcript's completed captures. Only a test's fault injection writes them from outside.
    pub(crate) captures: BTreeMap<CaptureId, Recorded>,
    /// The sessions that a `Resize` or a `SetColorProfile` reached (condition 3).
    changed: BTreeSet<SessionId>,
    /// The pages that replace the next second capture: the negative proof through comparison 2.
    #[cfg(test)]
    pub(crate) second: Option<Vec<u8>>,
}

pub(crate) fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
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

    /// The transcript's capture `capture` of `session`, with its mark and pages, and a second capture of `session` taken now
    /// ([`Shared::capture_now`]). A session that a `Resize` or a `SetColorProfile` reached gives plain `unsupported_control`
    /// (R-62 condition 3); a capture that is not a completed capture of `session` gives `Bad`.
    pub(crate) fn resume(
        &mut self,
        session: &SessionId,
        capture: CaptureId,
        bound: Duration,
    ) -> Result<Resumed, ControlError> {
        if self.changed.contains(session) {
            return Err(ControlError::Unsupported);
        }
        let recorded = self
            .captures
            .get(&capture)
            .filter(|recorded| recorded.session == *session)
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
        let second = self.capture_now(session, bound)?;
        #[cfg(test)]
        let second = self.second.take().unwrap_or(second);
        Ok(Resumed {
            mark: recorded.mark,
            pages,
            second,
        })
    }

    /// Takes a capture of `session` at a quiet point and returns its pages, then releases it (R-69). Each poll must return
    /// nothing or only the capture's own `Completed`; any other event gives `unsupported_control` `events_pending`. Core
    /// completes a capture after one round trip to the worker; `bound` ends a wait that never ends.
    fn capture_now(
        &mut self,
        session: &SessionId,
        bound: Duration,
    ) -> Result<Vec<u8>, ControlError> {
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
        let end = Instant::now() + bound;
        loop {
            self.core.pump(now);
            let mut done = None;
            let mut pending = false;
            for event in self.core.poll_events(usize::MAX) {
                match event {
                    Event::Completed { op: id, result } if id == op => done = Some(result),
                    _ => pending = true,
                }
            }
            if pending {
                if let Some(OpResult::Ok(OpOutput::Capture(capture))) = done {
                    self.core.release(capture.capture);
                }
                return Err(ControlError::UnsupportedFor("events_pending".into()));
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
                None => {}
            }
            let left = end.saturating_duration_since(Instant::now());
            if left.is_zero() {
                return Err(ControlError::Bad(format!(
                    "oracle_resume: the second capture did not complete within {bound:?}"
                )));
            }
            // timer: deadline — the bound of the second capture's round trip to the worker
            wake.wait(left);
        }
    }
}

/// The layer between the driver and Core's own composition (see the module documentation).
pub(crate) struct CaptureLayer(Arc<Mutex<Shared>>);

impl CaptureLayer {
    /// The layer over `core`, and the harness's handle of its records.
    pub(crate) fn new(core: Box<dyn CoreApi>, marks: Marks) -> (CaptureLayer, Weak<Mutex<Shared>>) {
        let shared = Arc::new(Mutex::new(Shared {
            core,
            marks,
            now: None,
            begun: BTreeMap::new(),
            captures: BTreeMap::new(),
            changed: BTreeSet::new(),
            #[cfg(test)]
            second: None,
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
        let mark = (shared.marks)();
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
        shared.core.pump(now)
    }

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let mut shared = self.shared();
        let events = shared.core.poll_events(max);
        shared.record(&events);
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
#[cfg(test)]
mod tests {
    use super::*;
    use botster_route_codec::prelude::HexBytes;
    use serde_json::json;
    use std::collections::VecDeque;
    use std::os::fd::RawFd;
    use std::sync::atomic::{AtomicU64, Ordering};

    fn sid(name: &str) -> SessionId {
        SessionId(name.into())
    }

    /// A count of marks that starts at `first`, and a view of the next one.
    fn marks_from(first: u64) -> (Marks, Arc<AtomicU64>) {
        let next = Arc::new(AtomicU64::new(first));
        let view = Arc::clone(&next);
        (Box::new(move || next.fetch_add(1, Ordering::SeqCst)), view)
    }

    fn marks() -> (Marks, Arc<AtomicU64>) {
        marks_from(0)
    }

    impl Fake {
        /// The Core and the captures that it released.
        fn new(completes: bool) -> (Fake, Arc<Mutex<Vec<CaptureId>>>) {
            let released = Arc::new(Mutex::new(vec![]));
            let fake = Fake {
                next: 0,
                completes,
                stall: 0,
                pumps: 0,
                posted: vec![],
                queue: VecDeque::new(),
                released: Arc::clone(&released),
            };
            (fake, released)
        }
    }

    /// A Core that completes each op at the next `pump`: a `CaptureSnapshot` only when it `completes`, with two pages of two
    /// bytes (the capture's id and the page's index), and every other op with `Unit`. It records the captures that it
    /// released.
    struct Fake {
        next: u64,
        completes: bool,
        /// The pumps that take nothing from `posted`, before the first one that does.
        stall: u32,
        pumps: u32,
        posted: Vec<Event>,
        queue: VecDeque<Event>,
        released: Arc<Mutex<Vec<CaptureId>>>,
    }

    struct NoWake;

    impl WakeHandle for NoWake {
        fn wait(&self, _timeout: Duration) -> Wake {
            Wake::TimedOut
        }
        fn fd(&self) -> RawFd {
            -1
        }
    }

    /// The completion of an op that is not a capture: an event that is not the control's.
    fn other(op: OpId) -> Event {
        Event::Completed {
            op,
            result: OpResult::Ok(OpOutput::Unit),
        }
    }

    /// The completion of the capture of `op`.
    fn captured(op: OpId) -> Event {
        Event::Completed {
            op,
            result: OpResult::Ok(OpOutput::Capture(Capture {
                capture: CaptureId(op.0),
                page_count: 2,
                total_bytes: 4,
                model_rev: ModelRev(0),
            })),
        }
    }

    impl CoreApi for Fake {
        fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
            self.next += 1;
            let id = OpId(self.next);
            match (&op, self.completes) {
                (Op::CaptureSnapshot { .. }, true) => self.posted.push(captured(id)),
                (Op::CaptureSnapshot { .. }, false) => {}
                _ => self.posted.push(other(id)),
            }
            Ok(id)
        }
        fn pump(&mut self, _now: Now) -> PumpReport {
            // A wait that never ends fails here, not at the test's timeout.
            self.pumps += 1;
            assert!(self.pumps <= 100, "the pumps never end");
            if self.stall > 0 {
                self.stall -= 1;
                return PumpReport {
                    more: false,
                    events_posted: 0,
                };
            }
            let posted = u32::try_from(self.posted.len()).unwrap();
            self.queue.extend(self.posted.drain(..));
            PumpReport {
                more: false,
                events_posted: posted,
            }
        }
        fn poll_events(&mut self, max: usize) -> Vec<Event> {
            let n = self.queue.len().min(max);
            self.queue.drain(..n).collect()
        }
        fn wake_handle(&self) -> Arc<dyn WakeHandle> {
            Arc::new(NoWake)
        }
        fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
            Ok(Page {
                index: page,
                bytes: HexBytes(vec![
                    u8::try_from(capture.0).unwrap(),
                    u8::try_from(page).unwrap(),
                ]),
                last: page == 1,
            })
        }
        fn release(&mut self, capture: CaptureId) {
            self.released.lock().unwrap().push(capture);
        }
        fn next_deadline(&self) -> Option<Instant> {
            unreachable!()
        }
        fn cancel(&mut self, _op: OpId) -> CancelResult {
            unreachable!()
        }
        fn get(&self, _id: &SessionId) -> Result<SessionRecord, CoreError> {
            unreachable!()
        }
        fn list(&self) -> Vec<SessionRecord> {
            unreachable!()
        }
        fn status(&self) -> Status {
            unreachable!()
        }
        fn diagnostics(&self) -> Value {
            unreachable!()
        }
        fn terminal_state(&self, _id: &SessionId) -> Result<TerminalState, CoreError> {
            unreachable!()
        }
        fn release_owner(&mut self, _client: &ClientId) {
            unreachable!()
        }
        fn snapshot_formats(&self, _session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
            unreachable!()
        }
        fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
            unreachable!()
        }
        fn attach(
            &mut self,
            _client: ClientId,
            _session: SessionId,
            _transport: RouteTransport,
            _options: AttachOptions,
        ) -> Result<AttachResult, AttachRefused> {
            unreachable!()
        }
        fn tap_read(&mut self, _session: &SessionId, _max: usize) -> Result<TapChunk, CoreError> {
            unreachable!()
        }
        fn set_silence_threshold(
            &mut self,
            _session: &SessionId,
            _threshold: Option<Duration>,
        ) -> Result<(), CoreError> {
            unreachable!()
        }
        fn service_send(
            &mut self,
            _id: &ServiceId,
            _lane: u8,
            _frame: &OutboundFrame,
        ) -> Result<(), SendError> {
            unreachable!()
        }
        fn service_recv(&mut self, _id: &ServiceId, _lane: u8) -> Result<Option<Frame>, RecvError> {
            unreachable!()
        }
        fn service_report(&self, _id: &ServiceId) -> Result<SpawnReport, CoreError> {
            unreachable!()
        }
        fn service_log_tail(&self, _id: &ServiceId, _max: usize) -> Result<Vec<u8>, CoreError> {
            unreachable!()
        }
        fn features(&self) -> Features {
            unreachable!()
        }
        fn limits(&self) -> CoreLimits {
            unreachable!()
        }
        fn worker_protocol(&self) -> u8 {
            unreachable!()
        }
        fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
            unreachable!()
        }
        fn worker_protocol_compatibility(&self, _protocol: Option<u8>) -> WorkerCompatibility {
            unreachable!()
        }
        fn terminal_identity(&self) -> TerminalIdentity {
            unreachable!()
        }
    }

    fn now() -> Now {
        Now {
            monotonic: Instant::now(),
            unix: 1,
        }
    }

    /// The control reads the transcript's capture with its mark and pages, and a second capture that Core then releases.
    /// Only a completed capture of the same session qualifies, and only a `Resize` or a `SetColorProfile` of that session
    /// makes it unsupported. The layer passes Core's events through as they are.
    #[test]
    fn the_control_reads_the_transcripts_capture_and_a_second_one() {
        let (fake, released) = Fake::new(true);
        let (marks, _next) = marks_from(4);
        let (mut layer, shared) = CaptureLayer::new(Box::new(fake), marks);
        let shared = shared.upgrade().unwrap();
        let bound = Duration::from_secs(10);
        let op = layer
            .begin(Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId("c1".into()),
            })
            .unwrap();
        let start = layer.begin(Op::Start { id: sid("s1") }).unwrap();
        assert!(
            matches!(
                lock(&shared).resume(&sid("s1"), CaptureId(op.0), bound),
                Err(ControlError::Bad(_))
            ),
            "not completed yet"
        );
        assert_eq!(layer.pump(now()).events_posted, 2);
        assert_eq!(layer.poll_events(1), vec![captured(op)]);
        assert_eq!(layer.poll_events(8), vec![other(start)]);

        assert_eq!(
            lock(&shared).resume(&sid("s1"), CaptureId(op.0), bound),
            Ok(Resumed {
                mark: 4,
                pages: vec![1, 0, 1, 1],
                second: vec![3, 0, 3, 1],
            })
        );
        assert_eq!(*released.lock().unwrap(), vec![CaptureId(3)]);
        assert_eq!(
            lock(&shared).resume(&sid("s1"), CaptureId(op.0), bound),
            Ok(Resumed {
                mark: 4,
                pages: vec![1, 0, 1, 1],
                second: vec![4, 0, 4, 1],
            }),
            "the second capture is not the transcript's, and it is taken again"
        );
        for (session, capture) in [("s2", op.0), ("s1", start.0), ("s1", 99)] {
            assert!(
                matches!(
                    lock(&shared).resume(&sid(session), CaptureId(capture), bound),
                    Err(ControlError::Bad(_))
                ),
                "{session} {capture}"
            );
        }

        let capture = CaptureId(op.0);
        for (change, session) in [
            (
                Op::Resize {
                    session: sid("s2"),
                    size: Size {
                        rows: 2,
                        cols: 2,
                        cell_px: None,
                    },
                },
                "s2",
            ),
            (
                Op::SetColorProfile {
                    session: sid("s3"),
                    profile: ColorProfile {
                        foreground: Rgb { r: 1, g: 2, b: 3 },
                        background: Rgb { r: 4, g: 5, b: 6 },
                        cursor: None,
                        palette: None,
                    },
                },
                "s3",
            ),
        ] {
            let id = layer.begin(change).unwrap();
            assert_eq!(layer.pump(now()).events_posted, 1);
            assert_eq!(layer.poll_events(8), vec![other(id)]);
            assert_eq!(
                lock(&shared).resume(&sid(session), capture, bound),
                Err(ControlError::Unsupported)
            );
        }
        assert!(
            lock(&shared).resume(&sid("s1"), capture, bound).is_ok(),
            "only the changed sessions"
        );
    }

    /// At a quiet point, the second capture returns its pages and releases the capture. It is not the transcript's, and it
    /// leaves no event for the driver (R-69 item 3).
    #[test]
    fn a_quiet_second_capture_returns_its_pages_and_releases_it() {
        let (fake, released) = Fake::new(true);
        let (marks, _next) = marks();
        let (mut layer, shared) = CaptureLayer::new(Box::new(fake), marks);
        let shared = shared.upgrade().unwrap();
        assert_eq!(layer.pump(now()).events_posted, 0);
        let second = lock(&shared)
            .capture_now(&sid("s1"), Duration::from_secs(10))
            .unwrap();
        assert_eq!(second, vec![1, 0, 1, 1]);
        assert_eq!(*released.lock().unwrap(), vec![CaptureId(1)]);
        assert_eq!(layer.pump(now()).events_posted, 0);
        assert_eq!(layer.poll_events(8), vec![]);
    }

    /// An event that the driver has not polled gives `events_pending`, also when it arrives with the capture's completion.
    /// The control releases a completed capture and never gives the drained event back (R-69 item 2).
    #[test]
    fn an_event_that_the_driver_did_not_poll_gives_events_pending() {
        for completes in [true, false] {
            let (fake, released) = Fake::new(completes);
            let (marks, _next) = marks();
            let (mut layer, shared) = CaptureLayer::new(Box::new(fake), marks);
            let shared = shared.upgrade().unwrap();
            layer.begin(Op::Start { id: sid("s1") }).unwrap();
            assert_eq!(layer.pump(now()).events_posted, 1);
            assert!(
                matches!(
                    lock(&shared).capture_now(&sid("s1"), Duration::from_secs(10)),
                    Err(ControlError::UnsupportedFor(name)) if name == "events_pending"
                ),
                "completes: {completes}"
            );
            let expected = if completes {
                vec![CaptureId(2)]
            } else {
                vec![]
            };
            assert_eq!(*released.lock().unwrap(), expected);
            assert_eq!(layer.pump(now()).events_posted, 0);
            assert_eq!(layer.poll_events(8), vec![], "never given back");
        }
    }

    /// A second capture before the driver's first `pump` gives `Bad`, and one that Core never completes gives `Bad` at the
    /// bound.
    #[test]
    fn a_second_capture_that_never_completes_is_bad_at_the_bound() {
        let (fake, released) = Fake::new(false);
        let (marks, _next) = marks();
        let (mut layer, shared) = CaptureLayer::new(Box::new(fake), marks);
        let shared = shared.upgrade().unwrap();
        assert!(matches!(
            lock(&shared).capture_now(&sid("s1"), Duration::ZERO),
            Err(ControlError::Bad(message)) if message.contains("never pumped")
        ));
        assert_eq!(layer.pump(now()).events_posted, 0);
        assert!(matches!(
            lock(&shared).capture_now(&sid("s1"), Duration::ZERO),
            Err(ControlError::Bad(message)) if message.contains("did not complete")
        ));
        assert!(released.lock().unwrap().is_empty());
    }

    /// The second capture pumps Core again after each wait until Core completes it.
    #[test]
    fn the_second_capture_pumps_until_core_completes_it() {
        let (mut fake, released) = Fake::new(true);
        fake.stall = 3;
        let (marks, _next) = marks();
        let (mut layer, shared) = CaptureLayer::new(Box::new(fake), marks);
        let shared = shared.upgrade().unwrap();
        assert_eq!(layer.pump(now()).events_posted, 0);
        let second = lock(&shared)
            .capture_now(&sid("s1"), Duration::from_secs(10))
            .unwrap();
        assert_eq!(second, vec![1, 0, 1, 1]);
        assert_eq!(*released.lock().unwrap(), vec![CaptureId(1)]);
    }

    /// Every call reaches the Core behind the layer, and every result comes back unchanged: the layer only records
    /// (`crate::refusal::tests::behind`, which also stands behind the refusal layer and the crash slot in their tests).
    #[test]
    fn every_call_and_its_result_pass_through_the_layer() {
        let (behind, reached) = crate::refusal::tests::behind();
        let deadline = behind.next_deadline();
        let (marks, _mark) = marks();
        let (mut layer, _shared) = CaptureLayer::new(Box::new(behind), marks);
        let session = sid("s1");
        let service = ServiceId([1; 32]);
        let frame = OutboundFrame {
            frame_type: 1,
            payload: Default::default(),
        };
        assert_eq!(
            layer.begin(Op::Start {
                id: session.clone()
            }),
            Ok(OpId(7))
        );
        assert_eq!(
            layer.pump(now()),
            PumpReport {
                more: true,
                events_posted: 3
            }
        );
        assert!(layer.poll_events(9).is_empty());
        assert_eq!(layer.next_deadline(), deadline);
        assert_eq!(layer.cancel(OpId(1)), CancelResult::TooLate);
        assert_eq!(layer.get(&session).unwrap_err().detail, "behind");
        assert!(layer.list().is_empty());
        assert!(layer.status().sessions.is_empty());
        assert_eq!(layer.diagnostics(), json!({"behind": true}));
        assert_eq!(layer.terminal_state(&session).unwrap_err().detail, "behind");
        assert_eq!(
            layer.read_page(CaptureId(1), 0).unwrap_err().detail,
            "behind"
        );
        layer.release(CaptureId(5));
        layer.release_owner(&ClientId("c9".into()));
        assert_eq!(layer.snapshot_formats(&session), Ok(vec![]));
        assert!(layer.shadow_answerable_kinds().is_empty());
        let (end, _client) = std::os::unix::net::UnixStream::pair().unwrap();
        let refused = layer
            .attach(
                ClientId("c1".into()),
                session.clone(),
                RouteTransport::Stream(StreamEndpoint::new(end)),
                serde_json::from_value(json!({"file_directory": "/tmp"})).unwrap(),
            )
            .unwrap_err();
        assert_eq!(refused.error.detail, "behind");
        assert_eq!(layer.tap_read(&session, 1).unwrap_err().detail, "behind");
        assert_eq!(layer.set_silence_threshold(&session, None), Ok(()));
        assert_eq!(layer.service_send(&service, 0, &frame), Ok(()));
        assert_eq!(layer.service_recv(&service, 0), Ok(None));
        assert_eq!(layer.service_report(&service).unwrap_err().detail, "behind");
        assert_eq!(layer.service_log_tail(&service, 1), Ok(Vec::new()));
        assert_eq!(layer.features().service_preamble_versions, vec![9]);
        assert_eq!(layer.limits().max_sessions, 7);
        assert_eq!(layer.worker_protocol(), 3);
        assert_eq!(layer.adoptable_worker_protocols(), BTreeSet::from([7]));
        assert_eq!(
            layer.worker_protocol_compatibility(Some(1)),
            WorkerCompatibility::Compatible
        );
        assert_eq!(layer.terminal_identity().term, "t");
        assert_eq!(
            *reached.lock().unwrap(),
            [
                "Start",
                "pump",
                "poll_events 9",
                "cancel",
                "get",
                "list",
                "status",
                "terminal_state",
                "read_page",
                "release 5",
                "release_owner c9",
                "snapshot_formats",
                "shadow_answerable_kinds",
                "attach",
                "tap_read",
                "set_silence_threshold",
                "service_send",
                "service_recv",
                "service_report",
                "service_log_tail"
            ]
        );
    }
}
