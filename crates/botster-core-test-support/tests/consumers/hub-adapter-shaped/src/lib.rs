//! Isolated Hub-shaped consumer that implements the published adapter contract.

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_core_test_support::terminal_adapter::TerminalAdapterHarnessDriver;
use botster_terminal_protocol::TerminalFrame;

/// Minimal external adapter. Not a published Core driver.
#[derive(Default)]
pub struct HubShapedTerminalAdapter {
    closed: bool,
    would_block: bool,
    active: Option<Vec<u8>>,
    delivered: Vec<Vec<u8>>,
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
        }
    }

    fn force_closed(&mut self) {
        self.closed = true;
        self.active = None;
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        &self.delivered
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
    };
    use botster_core_test_support::terminal_adapter::assert_terminal_adapter_conformance;

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
            .bind_terminal_adapter(
                client,
                session.clone(),
                subscription,
                generation,
                Box::new(adapter.clone()),
            )
            .expect("bind through public Core API");

        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            let _ = engine.drain_runtime_once(&session, 2).expect("drain");
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
}
