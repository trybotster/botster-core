//! Fake capability runtime helpers.

use std::collections::VecDeque;

use botster_core::{
    CapabilityOperationCompleted, CapabilityOperationFailure, CapabilityOperationId,
    CapabilityOperationResult, CapabilityResourceId, CapabilityRuntimeError,
    CapabilityRuntimeErrorKind, CapabilityRuntimeEvent, CapabilityRuntimeHandle,
    CapabilityRuntimeRequest, PluginCapabilityRuntime, PluginCleanupResult, PluginKey,
    PluginResourceRef, RequestId,
};

/// Deterministic fake implementation of the host capability runtime contract.
#[derive(Debug, Clone, Default)]
pub struct FakeCapabilityRuntime {
    pending: VecDeque<CapabilityRuntimeRequest>,
    events: Vec<CapabilityRuntimeEvent>,
    submitted: Vec<CapabilityRuntimeRequest>,
}

impl FakeCapabilityRuntime {
    /// Create an empty fake runtime.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return requests accepted by `submit`, in call order.
    #[must_use]
    pub fn submitted(&self) -> &[CapabilityRuntimeRequest] {
        &self.submitted
    }

    /// Return the number of accepted requests waiting for explicit completion.
    #[must_use]
    pub fn pending_len(&self) -> usize {
        self.pending.len()
    }

    /// Complete the oldest pending operation with a typed result.
    pub fn complete_next(
        &mut self,
        result: Option<CapabilityOperationResult>,
    ) -> Result<CapabilityOperationId, CapabilityRuntimeError> {
        let request = self.pending.pop_front().ok_or_else(|| {
            CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::OperationNotFound,
                "no pending capability operation",
            )
        })?;
        let operation_id = request.operation_id.clone();

        self.events.push(CapabilityRuntimeEvent::Completed(
            CapabilityOperationCompleted {
                plugin_key: request.plugin_key,
                operation_id: operation_id.clone(),
                result,
            },
        ));

        Ok(operation_id)
    }
}

impl PluginCapabilityRuntime for FakeCapabilityRuntime {
    fn submit(
        &mut self,
        request: CapabilityRuntimeRequest,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError> {
        let resource = request.resource_ref(CapabilityResourceId(request.operation_id.0.clone()));
        let handle = CapabilityRuntimeHandle {
            plugin_key: request.plugin_key.clone(),
            operation_id: request.operation_id.clone(),
            resource: Some(resource),
            required_capability: request.required_capability(),
        };

        self.submitted.push(request.clone());
        self.pending.push_back(request);
        Ok(handle)
    }

    fn cancel(
        &mut self,
        plugin_key: &PluginKey,
        operation_id: &CapabilityOperationId,
    ) -> Result<(), CapabilityRuntimeError> {
        let index = self
            .pending
            .iter()
            .position(|request| {
                &request.plugin_key == plugin_key && &request.operation_id == operation_id
            })
            .ok_or_else(|| {
                CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::OperationNotFound,
                    "capability operation not found",
                )
            })?;
        let request = self
            .pending
            .remove(index)
            .expect("pending capability operation at located index");

        self.events.push(CapabilityRuntimeEvent::Cancelled(
            CapabilityOperationFailure {
                plugin_key: request.plugin_key,
                operation_id: request.operation_id,
                error_kind: CapabilityRuntimeErrorKind::Cancelled,
                reason: "cancelled by fake capability runtime".to_string(),
            },
        ));
        Ok(())
    }

    fn release_resource(
        &mut self,
        resource: PluginResourceRef,
    ) -> Result<(), CapabilityRuntimeError> {
        self.events.push(CapabilityRuntimeEvent::ResourceReleased(
            botster_core::CapabilityResourceEvent {
                plugin_key: resource.plugin_key.clone(),
                operation_id: CapabilityOperationId(resource.resource_id.clone()),
                resource,
            },
        ));
        Ok(())
    }

    fn drain_events(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<Vec<CapabilityRuntimeEvent>, CapabilityRuntimeError> {
        let mut drained = Vec::new();
        self.events.retain(|event| {
            if event_plugin_key(event) == Some(plugin_key) {
                drained.push(event.clone());
                false
            } else {
                true
            }
        });
        Ok(drained)
    }

    fn cleanup_plugin(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<PluginCleanupResult, CapabilityRuntimeError> {
        self.pending
            .retain(|request| &request.plugin_key != plugin_key);
        self.events
            .retain(|event| event_plugin_key(event) != Some(plugin_key));

        Ok(PluginCleanupResult {
            request_id: RequestId("fake-capability-cleanup".to_string()),
            plugin_key: plugin_key.clone(),
            removed_descriptors: Vec::new(),
            removed_resources: Vec::new(),
        })
    }
}

fn event_plugin_key(event: &CapabilityRuntimeEvent) -> Option<&PluginKey> {
    match event {
        CapabilityRuntimeEvent::Completed(event) => Some(&event.plugin_key),
        CapabilityRuntimeEvent::ResourceOpened(event)
        | CapabilityRuntimeEvent::ResourceReleased(event) => Some(&event.plugin_key),
        CapabilityRuntimeEvent::TimedOut(event)
        | CapabilityRuntimeEvent::Cancelled(event)
        | CapabilityRuntimeEvent::Failed(event) => Some(&event.plugin_key),
        CapabilityRuntimeEvent::WebSocketMessage(event) => Some(&event.resource.plugin_key),
        CapabilityRuntimeEvent::Watch(event) => Some(&event.resource.plugin_key),
        CapabilityRuntimeEvent::TimerFired(event) => Some(&event.resource.plugin_key),
        CapabilityRuntimeEvent::CleanupCompleted(event) => Some(&event.plugin_key),
        CapabilityRuntimeEvent::Backpressure(event) => event.route.plugin_key.as_ref(),
    }
}
