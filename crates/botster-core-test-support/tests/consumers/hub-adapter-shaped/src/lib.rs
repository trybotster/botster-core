//! Isolated Hub-shaped consumer that implements the published adapter contract.

use std::collections::VecDeque;

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    MIN_ADAPTER_INGRESS_BUFFER_FRAMES,
};
use botster_core::{TerminalWakeKind, TerminalWakeSink, WakingTerminalAdapter};
use botster_core_test_support::terminal_adapter::TerminalAdapterHarnessDriver;
use botster_terminal_protocol::TerminalFrame;

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
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        if self.closed {
            return Err(TerminalAdapterWriteError::Closed);
        }
        if self.active.is_some() {
            return Err(TerminalAdapterWriteError::Full);
        }
        if self.would_block {
            return Err(TerminalAdapterWriteError::WouldBlock);
        }
        self.active = Some(frame.to_bytes().expect("serialize accepted frame"));
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
    use std::thread;
    use std::time::{Duration, Instant};

    use botster_core::{
        ClientId, CoreSessionMetadata, DefaultBotsterEngine, RequestId, ResizePayload, SessionId,
        SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
        TerminalCapabilitySet,
    };
    use botster_core_test_support::terminal_adapter::assert_terminal_adapter_conformance;
    use botster_terminal_protocol::FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY;

    #[derive(Clone, Default)]
    struct SharedHubAdapter {
        inner: Arc<Mutex<HubShapedTerminalAdapter>>,
    }

    impl TerminalAdapter for SharedHubAdapter {
        fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
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

    #[test]
    fn hub_shaped_consumer_binds_through_public_core_api_without_decoding_snapshots() {
        let mut engine = DefaultBotsterEngine::new();
        let session = SessionId("hub-shaped-bind".to_string());
        let client = ClientId("hub-shaped-client".to_string());
        let subscription = SubscriptionId("hub-shaped-sub".to_string());
        engine
            .spawn_session(
                SessionSpawnRequest {
                    request_id: RequestId("hub-shaped-spawn".to_string()),
                    session_id: session.clone(),
                    executable: "sh".to_string(),
                    arguments: vec![
                        "-c".to_string(),
                        "printf 'hub-shaped-live\\n'; sleep 30".to_string(),
                    ],
                    working_directory: SpawnWorkingDirectory {
                        path: ".".to_string(),
                    },
                    environment: SpawnEnvironment::default(),
                    initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                },
                CoreSessionMetadata::new(),
            )
            .expect("spawn");
        engine
            .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
            .expect("attach");
        let generation = engine
            .terminal_subscription_generation(&session, &subscription)
            .expect("generation after attach");
        let adapter = SharedHubAdapter::default();
        engine
            .bind_waking_terminal_adapter(
                client,
                session.clone(),
                subscription.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(adapter.clone()),
            )
            .expect("bind empty set through public Core API");
        let empty_row = engine
            .list_terminal_subscriptions()
            .into_iter()
            .find(|row| row.subscription_id == subscription)
            .expect("empty-set inventory");
        assert!(empty_row.adapter_bound);
        let empty_caps = empty_row.capabilities.expect("bound empty is Some");
        assert!(empty_caps.is_empty());
        assert_eq!(empty_caps.iter().count(), 0);

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let batch = engine.wait_wakes(Duration::from_secs(5));
            let _ = engine.pump_woken(&batch, 2).expect("targeted pump");
            let delivered = adapter
                .inner
                .lock()
                .expect("lock")
                .delivered_frame_bytes()
                .to_vec();
            let saw_opaque_live = delivered.iter().any(|bytes| {
                let text = String::from_utf8_lossy(bytes);
                text.contains("terminal_output")
            });
            if saw_opaque_live {
                for bytes in &delivered {
                    let text = String::from_utf8_lossy(bytes);
                    assert!(
                        !text.contains("\"type\":\"snapshot\""),
                        "empty set must not emit snapshot event tags"
                    );
                    assert!(
                        !bytes.windows(b"GHOSTSNP".len()).any(|window| window == b"GHOSTSNP"),
                        "hub-shaped consumer must not decode Snapshot bodies"
                    );
                }
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("hub-shaped consumer never observed opaque live frames");
    }

    #[test]
    fn hub_shaped_consumer_binds_ready_then_history_and_reads_inventory_tokens() {
        let mut engine = DefaultBotsterEngine::new();
        let session = SessionId("hub-shaped-rth".to_string());
        let client = ClientId("hub-shaped-rth-client".to_string());
        let subscription = SubscriptionId("hub-shaped-rth-sub".to_string());
        engine
            .spawn_session(
                SessionSpawnRequest {
                    request_id: RequestId("hub-shaped-rth-spawn".to_string()),
                    session_id: session.clone(),
                    executable: "sh".to_string(),
                    arguments: vec![
                        "-c".to_string(),
                        "printf 'hub-shaped-rth\\n'; sleep 30".to_string(),
                    ],
                    working_directory: SpawnWorkingDirectory {
                        path: ".".to_string(),
                    },
                    environment: SpawnEnvironment::default(),
                    initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                },
                CoreSessionMetadata::new(),
            )
            .expect("spawn");
        engine
            .attach_client(client.clone(), session.clone(), subscription.clone(), 1)
            .expect("attach");
        let generation = engine
            .terminal_subscription_generation(&session, &subscription)
            .expect("generation after attach");
        let adapter = SharedHubAdapter::default();
        let capabilities = TerminalCapabilitySet::from_tokens([
            FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
        ])
        .expect("Hub constructs an opaque set from protocol tokens");
        engine
            .bind_waking_terminal_adapter(
                client,
                session.clone(),
                subscription.clone(),
                generation,
                capabilities.clone(),
                Box::new(adapter.clone()),
            )
            .expect("bind optional-token set");
        let row = engine
            .list_terminal_subscriptions()
            .into_iter()
            .find(|row| row.subscription_id == subscription)
            .expect("optional-token inventory");
        assert!(row.adapter_bound);
        assert_eq!(row.capabilities, Some(capabilities));

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let batch = engine.wait_wakes(Duration::from_secs(5));
            let _ = engine.pump_woken(&batch, 2).expect("targeted pump");
            let delivered = adapter
                .inner
                .lock()
                .expect("lock")
                .delivered_frame_bytes()
                .to_vec();
            let saw_live = delivered.iter().any(|bytes| {
                String::from_utf8_lossy(bytes).contains("terminal_output")
            });
            if saw_live {
                for bytes in &delivered {
                    assert!(
                        !bytes.windows(b"GHOSTSNP".len()).any(|window| window == b"GHOSTSNP"),
                        "hub-shaped consumer must not decode Snapshot bodies"
                    );
                }
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        panic!("hub-shaped ready-then-history never observed opaque live frames");
    }

    #[derive(Clone, Default)]
    struct SharedOneSlotHubAdapter {
        inner: Arc<Mutex<HubShapedTerminalAdapter>>,
    }

    impl TerminalAdapter for SharedOneSlotHubAdapter {
        fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
            self.inner.lock().expect("hub adapter lock").try_write(frame)
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

    fn frame_type(bytes: &[u8]) -> String {
        let text = String::from_utf8_lossy(bytes);
        for kind in ["terminal_output", "snapshot", "attach_state", "process_exit"] {
            if text.contains(&format!("\"type\":\"{kind}\""))
                || text.contains(&format!("\"type\": \"{kind}\""))
            {
                return kind.to_string();
            }
        }
        String::new()
    }

    #[test]
    fn held_dump_drains_one_frame_per_ready_then_live_output_follows() {
        let mut engine = DefaultBotsterEngine::new();
        let session = SessionId("hub-shaped-hold".to_string());
        let client = ClientId("hub-shaped-hold-client".to_string());
        let subscription = SubscriptionId("hub-shaped-hold-sub".to_string());
        engine
            .spawn_session(
                SessionSpawnRequest {
                    request_id: RequestId("hub-shaped-hold-spawn".to_string()),
                    session_id: session.clone(),
                    executable: "sh".to_string(),
                    arguments: vec![
                        "-c".to_string(),
                        "printf 'hub-shaped-hold-live\\n'; sleep 30".to_string(),
                    ],
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
        let generation = engine
            .terminal_subscription_generation(&session, &subscription)
            .expect("generation after attach");
        let adapter = SharedOneSlotHubAdapter::default();
        engine
            .bind_waking_terminal_adapter(
                client,
                session.clone(),
                subscription,
                generation,
                TerminalCapabilitySet::from_tokens([FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY])
                    .expect("optional token"),
                Box::new(adapter.clone()),
            )
            .expect("bind one-slot");

        let deadline = Instant::now() + Duration::from_secs(8);
        let mut saw_live_after_dump = false;
        while Instant::now() < deadline {
            let batch = engine.wait_wakes(Duration::from_secs(5));
            let _ = engine.pump_woken(&batch, 2).expect("targeted pump");
            if adapter.pressure() == TerminalAdapterPressure::Full {
                adapter.complete_write();
            }
            let delivered = adapter.delivered();
            let types: Vec<String> = delivered.iter().map(|bytes| frame_type(bytes)).collect();
            if let Some(live_at) = types.iter().position(|kind| kind == "terminal_output") {
                assert!(
                    live_at >= 2,
                    "live output must follow a held dump of at least two frames: {types:?}"
                );
                assert!(
                    types[..live_at]
                        .iter()
                        .all(|kind| kind != "terminal_output"),
                    "live output must not interleave the dump: {types:?}"
                );
                saw_live_after_dump = true;
                break;
            }
            thread::sleep(Duration::from_millis(20));
        }
        assert!(
            saw_live_after_dump,
            "one-slot adapter must drain the held dump then live output: {:?}",
            adapter.delivered().iter().map(|bytes| frame_type(bytes)).collect::<Vec<_>>()
        );
    }
}
