//! Fake plugin worker runtime helpers.

use std::sync::{Arc, Condvar, Mutex};

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
    /// A slow handler that ignores cancellation: hold the invocation until
    /// the runtime is stopped or [`FakePluginRuntime::release`] is called,
    /// then complete with `payload`.
    Held {
        /// Payload returned when the hold ends.
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
    cancellations_observed: Arc<(Mutex<usize>, Condvar)>,
    /// Set when held invocations may finish: by `stop` or `release`.
    released: Arc<(Mutex<bool>, Condvar)>,
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

    /// Build a fake runtime whose invocations are held (ignoring
    /// cancellation) until the runtime is stopped or released, then return
    /// `{"value": "late"}`.
    #[must_use]
    pub fn held() -> Self {
        Self::new(FakePluginBehavior::Held {
            payload: BoundaryJson(serde_json::json!({ "value": "late" })),
        })
    }

    /// Let held invocations finish.
    pub fn release(&self) {
        let (released, changed) = &*self.released;
        *released.lock().expect("fake plugin runtime release lock") = true;
        changed.notify_all();
    }

    /// Build a fake runtime with explicit behavior.
    #[must_use]
    pub fn new(behavior: FakePluginBehavior) -> Self {
        Self {
            behavior: Arc::new(Mutex::new(behavior)),
            invocations: Arc::new(Mutex::new(Vec::new())),
            stopped: Arc::new(Mutex::new(Vec::new())),
            cancellations_observed: Arc::new((Mutex::new(0), Condvar::new())),
            released: Arc::new((Mutex::new(false), Condvar::new())),
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
            .0
            .lock()
            .expect("fake plugin runtime cancellations lock")
    }

    /// Wait until the fake has observed at least `count` cancellations, or
    /// `timeout` passes. Returns the number observed.
    #[must_use]
    pub fn wait_for_cancellations(&self, count: usize, timeout: std::time::Duration) -> usize {
        let (observed, changed) = &*self.cancellations_observed;
        let observed = observed
            .lock()
            .expect("fake plugin runtime cancellations lock");
        let (observed, _) = changed
            // timer: deadline — the caller's bound; a recorded cancellation ends the wait
            .wait_timeout_while(observed, timeout, |observed| *observed < count)
            .expect("fake plugin runtime cancellations wait");
        *observed
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
            FakePluginBehavior::Held { payload } => {
                let (released, changed) = &*self.released;
                let mut released = released.lock().expect("fake plugin runtime release lock");
                while !*released {
                    released = changed
                        .wait(released)
                        .expect("fake plugin runtime release wait");
                }
                PluginInvocationResult::Completed(PluginInvocationSuccess {
                    request_id: request.request_id,
                    handler: request.handler,
                    payload: Some(payload),
                })
            }
            FakePluginBehavior::WaitForCancellation => {
                let cancelled = Arc::new((Mutex::new(false), Condvar::new()));
                let signal = Arc::clone(&cancelled);
                cancellation.on_cancel(move || {
                    let (flag, changed) = &*signal;
                    *flag.lock().expect("fake plugin runtime cancel lock") = true;
                    changed.notify_all();
                });
                let (flag, changed) = &*cancelled;
                let mut flag = flag.lock().expect("fake plugin runtime cancel lock");
                while !*flag && !cancellation.is_cancelled() {
                    flag = changed.wait(flag).expect("fake plugin runtime cancel wait");
                }
                drop(flag);
                let (observed, changed) = &*self.cancellations_observed;
                *observed
                    .lock()
                    .expect("fake plugin runtime cancellations lock") += 1;
                changed.notify_all();
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
        self.release();
    }
}
