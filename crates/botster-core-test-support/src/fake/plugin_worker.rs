//! Fake plugin worker runtime helpers.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use botster_core::{
    BoundaryJson, PluginCancellationToken, PluginInvocationFailure, PluginInvocationFailureKind,
    PluginInvocationRequest, PluginInvocationResult, PluginInvocationSuccess, PluginKey,
    PluginRuntime,
};

/// Behavior returned by a fake plugin runtime.
#[derive(Debug, Clone)]
pub enum FakePluginBehavior {
    /// Complete the invocation with a payload.
    Success(BoundaryJson),
    /// Fail the invocation with a handler error.
    Failure(String),
    /// Sleep before completing the invocation.
    Delay {
        /// Delay duration.
        duration: Duration,
        /// Payload returned after the delay.
        payload: BoundaryJson,
    },
    /// Wait until core signals cancellation, then fail as cancelled.
    WaitForCancellation,
}

/// Shared fake plugin runtime for public API and conformance tests.
#[derive(Debug, Clone)]
pub struct FakePluginRuntime {
    behavior: Arc<Mutex<FakePluginBehavior>>,
    invocations: Arc<Mutex<Vec<PluginInvocationRequest>>>,
    stopped: Arc<Mutex<Vec<PluginKey>>>,
    cancellations_observed: Arc<Mutex<usize>>,
}

impl FakePluginRuntime {
    /// Build a fake runtime that completes with `{"value": value}`.
    #[must_use]
    pub fn success(value: &str) -> Self {
        Self::new(FakePluginBehavior::Success(BoundaryJson(
            serde_json::json!({ "value": value }),
        )))
    }

    /// Build a fake runtime that fails handler invocation.
    #[must_use]
    pub fn failure(reason: &str) -> Self {
        Self::new(FakePluginBehavior::Failure(reason.to_string()))
    }

    /// Build a fake runtime that delays before returning `{"value": "late"}`.
    #[must_use]
    pub fn delayed(duration: Duration) -> Self {
        Self::new(FakePluginBehavior::Delay {
            duration,
            payload: BoundaryJson(serde_json::json!({ "value": "late" })),
        })
    }

    /// Build a fake runtime with explicit behavior.
    #[must_use]
    pub fn new(behavior: FakePluginBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            invocations: Arc::new(Mutex::new(Vec::new())),
            stopped: Arc::new(Mutex::new(Vec::new())),
            cancellations_observed: Arc::new(Mutex::new(0)),
        }
    }

    /// Invocation requests recorded by the fake runtime.
    #[must_use]
    pub fn invocations(&self) -> Vec<PluginInvocationRequest> {
        self.invocations
            .lock()
            .expect("fake plugin runtime invocations lock")
            .clone()
    }

    /// Plugin keys stopped by the fake runtime.
    #[must_use]
    pub fn stopped(&self) -> Vec<PluginKey> {
        self.stopped
            .lock()
            .expect("fake plugin runtime stopped lock")
            .clone()
    }

    /// Number of invocations where the fake observed cancellation.
    #[must_use]
    pub fn cancellations_observed(&self) -> usize {
        *self
            .cancellations_observed
            .lock()
            .expect("fake plugin runtime cancellations lock")
    }
}

impl PluginRuntime for FakePluginRuntime {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.invocations
            .lock()
            .expect("fake plugin runtime invocations lock")
            .push(request.clone());

        match self
            .behavior
            .lock()
            .expect("fake plugin runtime behavior lock")
            .clone()
        {
            FakePluginBehavior::Success(payload) => {
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler: request.handler,
                    payload: Some(payload),
                })
            }
            FakePluginBehavior::Failure(reason) => {
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: request.request_id,
                    handler: request.handler,
                    kind: PluginInvocationFailureKind::HandlerFailed,
                    timeout_ms: None,
                    reason,
                })
            }
            FakePluginBehavior::Delay { duration, payload } => {
                std::thread::sleep(duration);
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler: request.handler,
                    payload: Some(payload),
                })
            }
            FakePluginBehavior::WaitForCancellation => {
                while !cancellation.is_cancelled() {
                    std::thread::sleep(Duration::from_millis(1));
                }
                *self
                    .cancellations_observed
                    .lock()
                    .expect("fake plugin runtime cancellations lock") += 1;
                PluginInvocationResult::Failed(PluginInvocationFailure {
                    request_id: request.request_id,
                    handler: request.handler,
                    kind: PluginInvocationFailureKind::Cancelled,
                    timeout_ms: None,
                    reason: "cancelled by test fake".to_string(),
                })
            }
        }
    }

    fn stop(&self, plugin_key: &PluginKey) {
        self.stopped
            .lock()
            .expect("fake plugin runtime stopped lock")
            .push(plugin_key.clone());
    }
}
