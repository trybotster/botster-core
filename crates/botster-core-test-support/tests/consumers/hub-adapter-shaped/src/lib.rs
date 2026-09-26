//! Isolated Hub-shaped consumer that implements the published adapter contract.

use std::collections::VecDeque;

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    MIN_ADAPTER_INGRESS_BUFFER_FRAMES,
};
use botster_core::{TerminalWakeKind, TerminalWakeSink, WakingTerminalAdapter};
use botster_core_test_support::terminal_adapter::TerminalAdapterHarnessDriver;
use botster_terminal_protocol::RoutedTerminalFrame;

/// Minimal external adapter. Not a published Core driver.
#[derive(Default)]
pub struct HubShapedTerminalAdapter {
    closed: bool,
    would_block: bool,
    active: Option<Vec<u8>>,
    delivered: Vec<Vec<u8>>,
    ingress: VecDeque<Vec<u8>>,
    ingress_partial: Option<Vec<u8>>,
    lost_pending: bool,
    wake_sink: Option<TerminalWakeSink>,
}

impl TerminalAdapter for HubShapedTerminalAdapter {
    fn try_write(&mut self, frame: &RoutedTerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        if self.closed {
            return Err(TerminalAdapterWriteError::Closed);
        }
        if self.active.is_some() {
            return Err(TerminalAdapterWriteError::Full);
        }
        if self.would_block {
            return Err(TerminalAdapterWriteError::WouldBlock);
        }
        self.active = Some(frame.frame.as_bytes().to_vec());
        Ok(())
    }

    fn close(&mut self) {
        self.closed = true;
        self.active = None;
        self.ingress.clear();
        self.ingress_partial = None;
        self.lost_pending = false;
        if let Some(sink) = &self.wake_sink {
            let _ = sink.wake(TerminalWakeKind::Closed);
        }
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        if self.closed {
            TerminalAdapterPressure::Closed
        } else if self.active.is_some() {
            TerminalAdapterPressure::Full
        } else if self.would_block {
            TerminalAdapterPressure::WouldBlock
        } else {
            TerminalAdapterPressure::Ready
        }
    }

    fn try_read(&mut self) -> TerminalIngress {
        if self.closed {
            return TerminalIngress::Closed;
        }
        if self.lost_pending {
            self.lost_pending = false;
            return TerminalIngress::Lost;
        }
        match self.ingress.pop_front() {
            Some(frame) => TerminalIngress::Frame(frame),
            None => TerminalIngress::Empty,
        }
    }
}

impl WakingTerminalAdapter for HubShapedTerminalAdapter {
    fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
        self.wake_sink = Some(sink);
    }
}

impl TerminalAdapterHarnessDriver for HubShapedTerminalAdapter {
    type Adapter = Self;

    fn adapter(&mut self) -> &mut Self::Adapter {
        self
    }

    fn force_would_block(&mut self) {
        self.would_block = true;
    }

    fn clear_would_block(&mut self) {
        self.would_block = false;
    }

    fn complete_active_write(&mut self) {
        if self.closed {
            return;
        }
        if let Some(bytes) = self.active.take() {
            self.delivered.push(bytes);
            if let Some(sink) = &self.wake_sink {
                let _ = sink.wake(TerminalWakeKind::Writable);
            }
        }
    }

    fn force_closed(&mut self) {
        self.closed = true;
        self.active = None;
        self.ingress.clear();
        self.ingress_partial = None;
        self.lost_pending = false;
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        &self.delivered
    }

    fn inject_ingress_frame(&mut self, bytes: Vec<u8>) {
        if self.closed {
            return;
        }
        if self.ingress.len() >= MIN_ADAPTER_INGRESS_BUFFER_FRAMES {
            self.lost_pending = true;
            return;
        }
        self.ingress.push_back(bytes);
        if let Some(sink) = &self.wake_sink {
            let _ = sink.wake(TerminalWakeKind::Writable);
        }
    }

    fn inject_ingress_partial(&mut self, bytes: Vec<u8>) {
        if !self.closed {
            self.ingress_partial = Some(bytes);
        }
    }

    fn complete_ingress_partial(&mut self) {
        if let Some(bytes) = self.ingress_partial.take() {
            self.inject_ingress_frame(bytes);
        }
    }

    fn drop_buffered_ingress_frame(&mut self) {
        if self.closed {
            return;
        }
        if self.ingress.pop_back().is_some() {
            self.lost_pending = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use std::time::{Duration, Instant};

    use botster_core::{
        ClientId, CoreSessionMetadata, DefaultBotsterEngine, RequestId, ResizePayload, SessionId,
        SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
        TerminalCapabilitySet, WorkerBackedBotsterEngine,
    };
    use botster_core_test_support::real_worker::WorkerBinary;
    use botster_core_test_support::terminal_adapter::assert_terminal_adapter_conformance;
    use botster_terminal_protocol::{
        TerminalFrame, TerminalKind, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
    };

    #[derive(Clone, Default)]
    struct SharedHubAdapter {
        inner: Arc<Mutex<HubShapedTerminalAdapter>>,
    }

    impl TerminalAdapter for SharedHubAdapter {
        fn try_write(
            &mut self,
            frame: &RoutedTerminalFrame,
        ) -> Result<(), TerminalAdapterWriteError> {
            let mut inner = self.inner.lock().expect("hub adapter lock");
            let result = inner.try_write(frame);
            if result.is_ok() {
                inner.complete_active_write();
            }
            result
        }

        fn close(&mut self) {
            self.inner.lock().expect("hub adapter lock").close();
        }

        fn pressure(&self) -> TerminalAdapterPressure {
            self.inner.lock().expect("hub adapter lock").pressure()
        }

        fn try_read(&mut self) -> TerminalIngress {
            self.inner.lock().expect("hub adapter lock").try_read()
        }
    }

    impl WakingTerminalAdapter for SharedHubAdapter {
        fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
            self.inner
                .lock()
                .expect("hub adapter lock")
                .set_wake_sink(sink);
        }
    }

    #[test]
    fn hub_shaped_consumer_adapter_passes_published_harness() {
        let mut driver = HubShapedTerminalAdapter::default();
        assert_terminal_adapter_conformance(&mut driver);
    }

    /// A Hub-shaped session: a worker-owned PTY that echoes its input.
    struct HubShapedSession {
        engine: WorkerBackedBotsterEngine,
        session: SessionId,
        client: ClientId,
        subscription: SubscriptionId,
    }

    impl HubShapedSession {
        /// Spawn one worker session and declare the Hub adapter route.
        fn declared(name: &str) -> Self {
            let worker = WorkerBinary::from_env().unwrap_or_else(|failure| panic!("{failure}"));
            let mut engine = DefaultBotsterEngine::worker_backed(worker.path);
            let session = SessionId(format!("{name}-session"));
            let client = ClientId(format!("{name}-client"));
            let subscription = SubscriptionId(format!("{name}-sub"));
            engine
                .spawn_session(
                    SessionSpawnRequest {
                        request_id: RequestId(format!("{name}-spawn")),
                        session_id: session.clone(),
                        executable: "cat".to_string(),
                        arguments: Vec::new(),
                        working_directory: SpawnWorkingDirectory {
                            path: ".".to_string(),
                        },
                        environment: SpawnEnvironment::default(),
                        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                    },
                    CoreSessionMetadata::new(),
                )
                .expect("spawn");
            engine.expect_terminal_adapter(client.clone(), session.clone(), subscription.clone());
            let attached = engine
                .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
                .expect("attach");
            assert!(
                attached.client_egress.iter().all(|(routed, frame)| {
                    routed != &client
                        || !matches!(
                            frame,
                            botster_core::TransportEgress::TerminalOutput { .. }
                                | botster_core::TransportEgress::Snapshot { .. }
                                | botster_core::TransportEgress::AttachState { .. }
                        )
                }),
                "declared attach must not extract route frames: {:?}",
                attached.client_egress
            );
            Self {
                engine,
                session,
                client,
                subscription,
            }
        }

        fn bind(
            &mut self,
            capabilities: TerminalCapabilitySet,
            adapter: Box<dyn WakingTerminalAdapter + Send>,
        ) {
            let generation = self
                .engine
                .terminal_subscription_generation(&self.session, &self.subscription)
                .expect("generation after attach");
            self.engine
                .bind_waking_terminal_adapter(
                    self.client.clone(),
                    self.session.clone(),
                    self.subscription.clone(),
                    generation,
                    capabilities.clone(),
                    adapter,
                )
                .expect("bind through public Core API");
            let row = self
                .engine
                .list_terminal_subscriptions(1024 * 1024)
                .expect("test inventory allowance")
                .records
                .into_iter()
                .find(|row| row.subscription_id == self.subscription)
                .expect("bound inventory row");
            assert!(row.adapter_bound);
            assert_eq!(row.capabilities, Some(capabilities));
        }

        fn type_line(&mut self, line: &[u8]) {
            self.engine
                .write_bytes(self.client.clone(), self.session.clone(), line.to_vec(), 2)
                .expect("write input");
        }

        /// Pump engine wakes until `done` holds. `wait_wakes` blocks on the
        /// engine's wake source; each adapter write completion wakes it.
        fn pump_until(&mut self, bound: Duration, mut done: impl FnMut() -> bool) -> bool {
            // timer: deadline — the condition must arrive through engine wakes; expiry fails the test
            let deadline = Instant::now() + bound;
            while !done() {
                let Some(remaining) = deadline.checked_duration_since(Instant::now()) else {
                    return false;
                };
                let batch = self.engine.wait_wakes(remaining);
                self.engine.pump_woken(&batch, 3).expect("targeted pump");
            }
            true
        }
    }

    /// Opaque frame kinds, read from the scheme 2 header only.
    fn frame_kinds(delivered: &[Vec<u8>]) -> Vec<TerminalKind> {
        delivered
            .iter()
            .map(|bytes| {
                TerminalFrame::from_bytes(bytes)
                    .expect("scheme 2 frame")
                    .kind()
            })
            .collect()
    }

    fn assert_live_output_reaches_the_adapter(capabilities: TerminalCapabilitySet, name: &str) {
        let mut hub = HubShapedSession::declared(name);
        let adapter = SharedHubAdapter::default();
        hub.bind(capabilities, Box::new(adapter.clone()));
        hub.type_line(b"hub-shaped-live\n");
        let delivered = || {
            adapter
                .inner
                .lock()
                .expect("lock")
                .delivered_frame_bytes()
                .to_vec()
        };
        assert!(
            hub.pump_until(Duration::from_secs(5), || {
                frame_kinds(&delivered()).contains(&TerminalKind::Output)
            }),
            "hub-shaped consumer never observed opaque live frames: {:?}",
            frame_kinds(&delivered())
        );
    }

    #[test]
    fn hub_shaped_consumer_binds_through_public_core_api_with_an_empty_set() {
        assert_live_output_reaches_the_adapter(TerminalCapabilitySet::empty(), "hub-shaped-empty");
    }

    #[test]
    fn hub_shaped_consumer_binds_ready_then_history_and_reads_inventory_tokens() {
        let capabilities =
            TerminalCapabilitySet::from_tokens([FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY])
                .expect("Hub constructs an opaque set from protocol tokens");
        assert_live_output_reaches_the_adapter(capabilities, "hub-shaped-rth");
    }

    #[derive(Clone, Default)]
    struct SharedOneSlotHubAdapter {
        inner: Arc<Mutex<HubShapedTerminalAdapter>>,
    }

    impl TerminalAdapter for SharedOneSlotHubAdapter {
        fn try_write(
            &mut self,
            frame: &RoutedTerminalFrame,
        ) -> Result<(), TerminalAdapterWriteError> {
            self.inner
                .lock()
                .expect("hub adapter lock")
                .try_write(frame)
        }

        fn close(&mut self) {
            self.inner.lock().expect("hub adapter lock").close();
        }

        fn pressure(&self) -> TerminalAdapterPressure {
            self.inner.lock().expect("hub adapter lock").pressure()
        }

        fn try_read(&mut self) -> TerminalIngress {
            self.inner.lock().expect("hub adapter lock").try_read()
        }
    }

    impl WakingTerminalAdapter for SharedOneSlotHubAdapter {
        fn set_wake_sink(&mut self, sink: TerminalWakeSink) {
            self.inner
                .lock()
                .expect("hub adapter lock")
                .set_wake_sink(sink);
        }
    }

    impl SharedOneSlotHubAdapter {
        fn complete_write(&self) {
            self.inner
                .lock()
                .expect("hub adapter lock")
                .complete_active_write();
        }

        fn delivered(&self) -> Vec<Vec<u8>> {
            self.inner
                .lock()
                .expect("hub adapter lock")
                .delivered_frame_bytes()
                .to_vec()
        }

        fn pressure(&self) -> TerminalAdapterPressure {
            self.inner.lock().expect("hub adapter lock").pressure()
        }
    }

    #[test]
    fn held_dump_drains_one_frame_per_ready_then_live_output_follows() {
        let mut hub = HubShapedSession::declared("hub-shaped-hold");
        let adapter = SharedOneSlotHubAdapter::default();
        hub.bind(
            TerminalCapabilitySet::from_tokens([FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY])
                .expect("optional token"),
            Box::new(adapter.clone()),
        );
        hub.type_line(b"hub-shaped-hold-live\n");
        let saw_live = hub.pump_until(Duration::from_secs(8), || {
            // Completing the one-slot write wakes the engine for the next frame.
            if adapter.pressure() == TerminalAdapterPressure::Full {
                adapter.complete_write();
            }
            frame_kinds(&adapter.delivered()).contains(&TerminalKind::Output)
        });
        let kinds = frame_kinds(&adapter.delivered());
        assert!(
            saw_live,
            "one-slot adapter must drain the held dump then live output: {kinds:?}"
        );
        let live_at = kinds
            .iter()
            .position(|kind| *kind == TerminalKind::Output)
            .expect("live output");
        assert!(
            live_at >= 2,
            "live output must follow a held dump of at least two frames: {kinds:?}"
        );
    }
}
