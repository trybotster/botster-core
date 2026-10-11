//! The capture layer of the real `oracle_resume` (steward ruling R-62). The control itself is the real harness's
//! (`real/resume.rs`, the `slow` feature).
//!
//! [`CaptureLayer`] sits between the driver and Core's own composition. It passes every call through unchanged, and it
//! records each `CaptureSnapshot` that the transcript begins: the mark of the output history at the `begin` (the cut) and,
//! when Core completes it, every page that `read_page` gives, before the transcript can release it. For the second capture
//! the control begins its own `CaptureSnapshot` through the layer and pumps Core with the driver's latest `now` until it
//! completes. The events that those pumps take are held, and the driver gets them first, in order, with its next poll; its
//! next `pump` reports them in `events_posted`. It also records each session that a `Resize` or a `SetColorProfile` reached
//! (R-62 condition 3).
//!
//! Only the real harness composes it. It is outside that module so that its tests run in the default tier too: a layer that
//! drops a call would otherwise stall every real transcript before a test could fail.

use botster_core_conformance::ControlError;
use botster_core_contract::prelude::*;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

/// The mark that the next acknowledged `pty_output` gets (the real harness's output history).
pub(crate) type Marks = Box<dyn Fn() -> u64 + Send + Sync>;

/// The owner of the control's own capture. No transcript names it.
const OWNER: &str = "oracle_resume";

/// A capture that Core completed for the transcript.
#[derive(Debug, Clone)]
pub(crate) struct Recorded {
    pub(crate) session: SessionId,
    /// The mark of the output history at its `begin`: the outputs of a lower mark are before its cut.
    pub(crate) mark: u64,
    /// Every page, in order, or why `read_page` refused one.
    pub(crate) pages: Result<Vec<u8>, String>,
}

/// What the layer shares with the harness.
pub(crate) struct Shared {
    core: Box<dyn CoreApi>,
    marks: Marks,
    /// The `now` of the driver's latest `pump`.
    now: Option<Now>,
    /// The events that the control's pumps took, for the driver's next polls.
    held: VecDeque<Event>,
    /// How many of `held` no `pump` has reported yet.
    unreported: u32,
    /// The transcript's `CaptureSnapshot` begins that Core has not completed: their session and mark.
    begun: BTreeMap<OpId, (SessionId, u64)>,
    pub(crate) captures: BTreeMap<CaptureId, Recorded>,
    /// The sessions that a `Resize` or a `SetColorProfile` reached (condition 3).
    pub(crate) changed: BTreeSet<SessionId>,
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

    /// Takes a capture of `session` now and returns its pages, then releases it. The events that the pumps take are held
    /// for the driver. Core completes a capture after one round trip to the worker; `bound` ends a wait that never ends.
    pub(crate) fn capture_now(
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
            held: VecDeque::new(),
            unreported: 0,
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
        let mut report = shared.core.pump(now);
        report.events_posted = report.events_posted.saturating_add(shared.unreported);
        shared.unreported = 0;
        report
    }

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let mut shared = self.shared();
        let held = shared.held.len().min(max);
        let mut events: Vec<Event> = shared.held.drain(..held).collect();
        // Core reads at most the room that is left: none when the held events filled `max`.
        let more = shared.core.poll_events(max - events.len());
        shared.record(&more);
        events.extend(more);
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

    /// A Core that posts an unrelated event for each `CaptureSnapshot` and, when it `completes`, the completion after it, at
    /// the next `pump`: two pages of two bytes, the capture's id and the page's index. It records the captures that it
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

    /// An event that is not the control's.
    fn other() -> Event {
        Event::Completed {
            op: OpId(999),
            result: OpResult::Ok(OpOutput::Unit),
        }
    }

    impl CoreApi for Fake {
        fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
            self.next += 1;
            let id = OpId(self.next);
            if let Op::CaptureSnapshot { .. } = op {
                self.posted.push(other());
            }
            if let (Op::CaptureSnapshot { .. }, true) = (&op, self.completes) {
                self.posted.push(Event::Completed {
                    op: id,
                    result: OpResult::Ok(OpOutput::Capture(Capture {
                        capture: CaptureId(id.0),
                        page_count: 2,
                        total_bytes: 4,
                        model_rev: ModelRev(0),
                    })),
                });
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

    /// The layer records the transcript's capture with its mark and pages; the control's own capture is not recorded, is
    /// released, and the events that its pumps took reach the driver first, in order, and in the next `events_posted`.
    #[test]
    fn the_layer_records_captures_and_holds_the_controls_events_for_the_driver() {
        let (fake, released) = Fake::new(true);
        let (marks, _next) = marks_from(4);
        let (mut layer, shared) = CaptureLayer::new(Box::new(fake), marks);
        let shared = shared.upgrade().unwrap();
        assert!(
            matches!(
                lock(&shared).capture_now(&sid("s1"), Duration::from_secs(10)),
                Err(ControlError::Bad(_))
            ),
            "no pump yet"
        );

        let op = layer
            .begin(Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId("c1".into()),
            })
            .unwrap();
        assert_eq!(layer.pump(now()).events_posted, 2);
        let events = layer.poll_events(8);
        assert_eq!(events.len(), 2);
        {
            let shared = lock(&shared);
            let recorded = &shared.captures[&CaptureId(op.0)];
            assert_eq!(recorded.session, sid("s1"));
            assert_eq!(recorded.mark, 4);
            assert_eq!(recorded.pages, Ok(vec![1, 0, 1, 1]));
            assert!(shared.begun.is_empty());
        }

        // A transcript capture that Core has not completed yet: the control's pump takes its two events too.
        let pending = layer
            .begin(Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId("c1".into()),
            })
            .unwrap();
        let second = lock(&shared)
            .capture_now(&sid("s1"), Duration::from_secs(10))
            .unwrap();
        assert_eq!(second, vec![3, 0, 3, 1]);
        assert_eq!(*released.lock().unwrap(), vec![CaptureId(3)]);
        {
            let shared = lock(&shared);
            assert!(
                !shared.captures.contains_key(&CaptureId(3)),
                "not the transcript's"
            );
            assert_eq!(shared.captures[&CaptureId(pending.0)].mark, 5);
        }
        let inner = layer
            .begin(Op::CaptureSnapshot {
                session: sid("s1"),
                owner: ClientId("c1".into()),
            })
            .unwrap();
        assert_eq!(
            layer.pump(now()).events_posted,
            2 + 3,
            "the held events are reported with the new ones"
        );
        assert_eq!(layer.pump(now()).events_posted, 0, "once");
        let held = [
            other(),
            Event::Completed {
                op: pending,
                result: OpResult::Ok(OpOutput::Capture(Capture {
                    capture: CaptureId(pending.0),
                    page_count: 2,
                    total_bytes: 4,
                    model_rev: ModelRev(0),
                })),
            },
            other(),
        ];
        assert_eq!(layer.poll_events(0), vec![]);
        assert_eq!(layer.poll_events(2), held[..2].to_vec());
        assert_eq!(
            layer.poll_events(2),
            vec![held[2].clone(), other()],
            "the held event first, then Core's"
        );
        let last = layer.poll_events(8);
        assert_eq!(last.len(), 1);
        assert!(matches!(last[0], Event::Completed { op, .. } if op == inner));
        assert_eq!(layer.poll_events(8), vec![]);

        for (op, session) in [
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
            layer.begin(op).unwrap();
            assert!(lock(&shared).changed.contains(&sid(session)));
        }
        assert!(!lock(&shared).changed.contains(&sid("s1")));
    }

    /// A second capture that Core never completes gives `Bad` at the bound; the events of its pumps are still held.
    #[test]
    fn a_second_capture_that_never_completes_is_bad_at_the_bound() {
        let (fake, released) = Fake::new(false);
        let (marks, _next) = marks();
        let (mut layer, shared) = CaptureLayer::new(Box::new(fake), marks);
        let shared = shared.upgrade().unwrap();
        assert_eq!(layer.pump(now()).events_posted, 0);
        assert!(matches!(
            lock(&shared).capture_now(&sid("s1"), Duration::ZERO),
            Err(ControlError::Bad(message)) if message.contains("did not complete")
        ));
        assert!(released.lock().unwrap().is_empty());
        assert_eq!(layer.poll_events(8), vec![other()]);
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
        assert_eq!(layer.poll_events(8), vec![other()]);
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
