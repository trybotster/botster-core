//! The crash slot of a real handle (`host_crash`, steward ruling R-55 (1), contracts `main` `70dff23` and `af17b97`).
//!
//! The conformance driver owns the `CoreApi` of each handle until its `drop` step, and a control of the harness cannot drop
//! it. So the real harness gives the driver a [`Crashable`] in front of Core, and keeps its [`CrashHandle`]. A crash takes
//! Core out of the slot and drops it: the edges close their descriptors and the directory lock, as the kernel does at the
//! death of a host process. Every call that reaches the slot before the crash passes through unchanged. A transcript uses
//! no handle after its crash, so a call after it is a fault of the run, and it panics.

use botster_core_contract::prelude::*;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

type Slot = Arc<Mutex<Option<Box<dyn CoreApi>>>>;

/// A `CoreApi` in a slot that its [`CrashHandle`] can empty.
pub struct Crashable {
    slot: Slot,
}

/// Empties the slot of a [`Crashable`].
#[derive(Clone)]
pub struct CrashHandle {
    slot: Slot,
}

fn lock(slot: &Slot) -> MutexGuard<'_, Option<Box<dyn CoreApi>>> {
    slot.lock().unwrap_or_else(PoisonError::into_inner)
}

impl Crashable {
    pub fn new(core: Box<dyn CoreApi>) -> (Crashable, CrashHandle) {
        let slot: Slot = Arc::new(Mutex::new(Some(core)));
        (
            Crashable {
                slot: Arc::clone(&slot),
            },
            CrashHandle { slot },
        )
    }

    fn with<R>(&self, call: impl FnOnce(&mut dyn CoreApi) -> R) -> R {
        let mut slot = lock(&self.slot);
        let core = slot
            .as_mut()
            .expect("the host of this handle crashed (host_crash): a transcript uses no handle after its crash");
        call(core.as_mut())
    }
}

impl CrashHandle {
    /// Drops the Core of the slot, outside the slot's lock. False when it was dropped already.
    pub fn crash(&self) -> bool {
        let core = lock(&self.slot).take();
        let crashed = core.is_some();
        drop(core);
        crashed
    }
}

impl CoreApi for Crashable {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        self.with(|core| core.begin(op))
    }
    fn pump(&mut self, now: Now) -> PumpReport {
        self.with(|core| core.pump(now))
    }
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.with(|core| core.poll_events(max))
    }
    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        self.with(|core| core.wake_handle())
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.with(|core| core.next_deadline())
    }
    fn cancel(&mut self, op: OpId) -> CancelResult {
        self.with(|core| core.cancel(op))
    }
    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.with(|core| core.get(id))
    }
    fn list(&self) -> Vec<SessionRecord> {
        self.with(|core| core.list())
    }
    fn status(&self) -> Status {
        self.with(|core| core.status())
    }
    fn diagnostics(&self) -> serde_json::Value {
        self.with(|core| core.diagnostics())
    }
    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.with(|core| core.terminal_state(id))
    }
    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.with(|core| core.read_page(capture, page))
    }
    fn release(&mut self, capture: CaptureId) {
        self.with(|core| core.release(capture));
    }
    fn release_owner(&mut self, client: &ClientId) {
        self.with(|core| core.release_owner(client));
    }
    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.with(|core| core.snapshot_formats(session))
    }
    fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        self.with(|core| core.shadow_answerable_kinds())
    }
    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        self.with(|core| core.attach(client, session, transport, options))
    }
    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError> {
        self.with(|core| core.tap_read(session, max))
    }
    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        self.with(|core| core.set_silence_threshold(session, threshold))
    }
    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        self.with(|core| core.service_send(id, lane, frame))
    }
    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError> {
        self.with(|core| core.service_recv(id, lane))
    }
    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        self.with(|core| core.service_report(id))
    }
    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError> {
        self.with(|core| core.service_log_tail(id, max))
    }
    fn features(&self) -> Features {
        self.with(|core| core.features())
    }
    fn limits(&self) -> CoreLimits {
        self.with(|core| core.limits())
    }
    fn worker_protocol(&self) -> u8 {
        self.with(|core| core.worker_protocol())
    }
    fn adoptable_worker_protocols(&self) -> std::collections::BTreeSet<u8> {
        self.with(|core| core.adoptable_worker_protocols())
    }
    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.with(|core| core.worker_protocol_compatibility(protocol))
    }
    fn terminal_identity(&self) -> TerminalIdentity {
        self.with(|core| core.terminal_identity())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::refusal::tests::behind;

    /// R-55 (1): a crash drops the Core of the slot once; a call before it reaches the Core.
    #[test]
    fn a_crash_drops_the_core_once() {
        let (behind, reached) = behind();
        let (slot, crash) = Crashable::new(Box::new(behind));
        assert!(slot.list().is_empty());
        assert_eq!(*reached.lock().unwrap(), ["list"]);
        assert_eq!(Arc::strong_count(&reached), 2, "the Core holds the record");
        assert!(crash.crash());
        assert_eq!(Arc::strong_count(&reached), 1, "the Core is dropped");
        assert!(!crash.crash(), "once");
    }

    /// A transcript uses no handle after its crash: a call then is a fault of the run.
    #[test]
    #[should_panic(expected = "crashed (host_crash)")]
    fn a_call_after_the_crash_panics() {
        let (behind, _reached) = behind();
        let (slot, crash) = Crashable::new(Box::new(behind));
        crash.crash();
        slot.list();
    }
}
