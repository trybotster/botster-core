//! The real tier's record of the ops that Core admits, for `scheduler_hold` (Core A5-2; steward ruling R-88, group E).
//!
//! The capture layer holds the driver behind `Box<dyn CoreApi>`, so a control cannot ask the engine about an op. This layer
//! is the driver's own, in front of it: at each `begin` it records the op, and whether a clause fixes its timing (R-20), in
//! the holds that the edge tap's scheduler answers from. Every other call goes to the driver unchanged.

use crate::op_holds::{lock, SharedHolds};
use botster_core_contract::prelude::*;
use botster_core_host::driver::{HostDriver, HostEdges};
use std::collections::BTreeSet;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub(crate) struct Timed<E: HostEdges> {
    driver: HostDriver<E>,
    holds: SharedHolds,
}

impl<E: HostEdges> Timed<E> {
    pub(crate) fn new(driver: HostDriver<E>, holds: SharedHolds) -> Timed<E> {
        Timed { driver, holds }
    }
}

impl<E: HostEdges + Send> CoreApi for Timed<E> {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        let id = self.driver.begin(op)?;
        let fixed = self.driver.engine().fixed_timing(id) == Some(true);
        lock(&self.holds).begun(id, fixed);
        Ok(id)
    }

    fn pump(&mut self, now: Now) -> PumpReport {
        self.driver.pump(now)
    }

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.driver.poll_events(max)
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        self.driver.wake_handle()
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.driver.next_deadline()
    }

    fn cancel(&mut self, op: OpId) -> CancelResult {
        self.driver.cancel(op)
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.driver.get(id)
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.driver.list()
    }

    fn status(&self) -> Status {
        self.driver.status()
    }

    fn diagnostics(&self) -> serde_json::Value {
        self.driver.diagnostics()
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.driver.terminal_state(id)
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.driver.read_page(capture, page)
    }

    fn release(&mut self, capture: CaptureId) {
        self.driver.release(capture);
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.driver.release_owner(client);
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.driver.snapshot_formats(session)
    }

    fn shadow_answerable_kinds(&self) -> Vec<QueryKind> {
        self.driver.shadow_answerable_kinds()
    }

    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        self.driver.attach(client, session, transport, options)
    }

    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError> {
        self.driver.tap_read(session, max)
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        self.driver.set_silence_threshold(session, threshold)
    }

    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        self.driver.service_send(id, lane, frame)
    }

    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError> {
        self.driver.service_recv(id, lane)
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        self.driver.service_report(id)
    }

    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError> {
        self.driver.service_log_tail(id, max)
    }

    fn features(&self) -> Features {
        self.driver.features()
    }

    fn limits(&self) -> CoreLimits {
        self.driver.limits()
    }

    fn worker_protocol(&self) -> u8 {
        self.driver.worker_protocol()
    }

    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        self.driver.adoptable_worker_protocols()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.driver.worker_protocol_compatibility(protocol)
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        self.driver.terminal_identity()
    }
}
