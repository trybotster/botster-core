//! In-memory one-slot sink used as the baseline published driver.

use std::sync::{Arc, Mutex};

use botster_core::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError,
};
use botster_terminal_protocol::TerminalFrame;

use super::core::OneSlotCore;
use super::TerminalAdapterHarnessDriver;

/// In-memory one-slot terminal adapter.
///
/// This is a Core-owned test adapter. It is not a production transport.
#[derive(Debug, Default)]
pub struct FakeTerminalAdapter {
    inner: OneSlotCore,
}

impl TerminalAdapter for FakeTerminalAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        self.inner.try_write(frame)
    }

    fn close(&mut self) {
        self.inner.close();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.inner.pressure()
    }
}

impl TerminalAdapterHarnessDriver for FakeTerminalAdapter {
    type Adapter = Self;

    fn adapter(&mut self) -> &mut Self::Adapter {
        self
    }

    fn force_would_block(&mut self) {
        self.inner.force_would_block();
    }

    fn clear_would_block(&mut self) {
        self.inner.clear_would_block();
    }

    fn complete_active_write(&mut self) {
        if self.inner.is_closed() {
            return;
        }
        if let Some(bytes) = self.inner.take_active() {
            self.inner.push_delivered(bytes);
        }
    }

    fn force_closed(&mut self) {
        self.inner.close();
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        self.inner.delivered()
    }
}

/// Shared Fake adapter handle so tests can complete writes after bind.
#[derive(Clone, Debug, Default)]
pub struct SharedFakeTerminalAdapter {
    inner: Arc<Mutex<FakeTerminalAdapter>>,
    auto_complete: bool,
}

impl SharedFakeTerminalAdapter {
    /// Build a shared Fake that occupies the one-slot write until completed.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Build a shared Fake that completes each accepted write immediately.
    #[must_use]
    pub fn auto_complete() -> Self {
        Self {
            inner: Arc::new(Mutex::new(FakeTerminalAdapter::default())),
            auto_complete: true,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, FakeTerminalAdapter> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }
}

impl TerminalAdapter for SharedFakeTerminalAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        let result = self.lock().try_write(frame);
        if result.is_ok() && self.auto_complete {
            self.complete_active_write();
        }
        result
    }

    fn close(&mut self) {
        self.lock().close();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.lock().pressure()
    }
}

impl TerminalAdapterHarnessDriver for SharedFakeTerminalAdapter {
    type Adapter = Self;

    fn adapter(&mut self) -> &mut Self::Adapter {
        self
    }

    fn force_would_block(&mut self) {
        self.lock().force_would_block();
    }

    fn clear_would_block(&mut self) {
        self.lock().clear_would_block();
    }

    fn complete_active_write(&mut self) {
        self.lock().complete_active_write();
    }

    fn force_closed(&mut self) {
        self.lock().force_closed();
    }

    fn delivered_frame_bytes(&self) -> &[Vec<u8>] {
        // The driver trait returns a borrow. Shared observation uses
        // [`Self::snapshot_delivered_frame_bytes`].
        &[]
    }
}

impl SharedFakeTerminalAdapter {
    /// Copy of completed frame bytes after bind.
    #[must_use]
    pub fn snapshot_delivered_frame_bytes(&self) -> Vec<Vec<u8>> {
        self.lock().delivered_frame_bytes().to_vec()
    }

    /// Current adapter pressure after bind.
    #[must_use]
    pub fn snapshot_pressure(&self) -> TerminalAdapterPressure {
        self.lock().pressure()
    }

    /// Complete the active write after bind without needing `&mut self`.
    pub fn complete_write(&self) {
        self.lock().complete_active_write();
    }

    /// Force transport-side close after bind without needing `&mut self`.
    pub fn close_transport(&self) {
        self.lock().force_closed();
    }

    /// Force would-block after bind without needing `&mut self`.
    pub fn block_writes(&self) {
        self.lock().force_would_block();
    }
}

/// Hub-shaped adapter: `try_write` accepts and reports Ready, but `close`
/// abandons frames that have not been flushed to the consumer sink.
///
/// This is not a one-slot conformance driver. Use it to prove ClientWorker
/// close ordering when accepted writes are not yet consumer-visible.
#[derive(Clone, Debug, Default)]
pub struct DeferredFlushTerminalAdapter {
    inner: Arc<Mutex<DeferredFlushInner>>,
}

#[derive(Debug, Default)]
struct DeferredFlushInner {
    closed: bool,
    accepted: Vec<Vec<u8>>,
    delivered: Vec<Vec<u8>>,
    events: Vec<&'static str>,
}

impl DeferredFlushTerminalAdapter {
    /// Build a shared deferred-flush adapter.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, DeferredFlushInner> {
        self.inner.lock().unwrap_or_else(|error| error.into_inner())
    }

    /// Move accepted frames into the consumer-visible sink unless closed.
    pub fn flush(&self) {
        let mut inner = self.lock();
        if inner.closed {
            return;
        }
        let accepted = std::mem::take(&mut inner.accepted);
        inner.delivered.extend(accepted);
    }

    /// Copy of flushed frame bytes.
    #[must_use]
    pub fn snapshot_delivered_frame_bytes(&self) -> Vec<Vec<u8>> {
        self.lock().delivered.clone()
    }

    /// Accept/close event log for ordering assertions.
    #[must_use]
    pub fn snapshot_events(&self) -> Vec<&'static str> {
        self.lock().events.clone()
    }

    /// Whether `close()` has run.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.lock().closed
    }
}

fn deferred_event_for(bytes: &[u8]) -> &'static str {
    let kind = serde_json::from_slice::<serde_json::Value>(bytes)
        .ok()
        .and_then(|value| {
            value
                .get("type")
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        })
        .unwrap_or_default();
    match kind.as_str() {
        "terminal_output" => "accept_terminal_output",
        "process_exit" => "accept_process_exit",
        _ => "accept_other",
    }
}

impl TerminalAdapter for DeferredFlushTerminalAdapter {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        let mut inner = self.lock();
        if inner.closed {
            return Err(TerminalAdapterWriteError::Closed);
        }
        let bytes = frame.to_bytes().expect("fixture TerminalFrame serializes");
        inner.events.push(deferred_event_for(&bytes));
        inner.accepted.push(bytes);
        Ok(())
    }

    fn close(&mut self) {
        let mut inner = self.lock();
        inner.closed = true;
        let abandoned = !inner.accepted.is_empty();
        inner
            .events
            .push(if abandoned { "close_abandon" } else { "close" });
        inner.accepted.clear();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        if self.lock().closed {
            TerminalAdapterPressure::Closed
        } else {
            TerminalAdapterPressure::Ready
        }
    }
}
