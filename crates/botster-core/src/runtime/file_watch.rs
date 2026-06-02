//! Capability-scoped file watch runtime mechanics.
//!
//! Core owns registration state, scoped-path validation, deterministic
//! debounce/coalescing, bounded event delivery, and plugin cleanup. Host
//! profiles still own concrete OS watcher tasks, watched-directory selection,
//! root and symlink resolution, and capacity values supplied through config.

use std::collections::{HashMap, HashSet};
use std::error::Error;
use std::fmt;

use crate::actor::{
    BackpressureRoute, BackpressureSummary, PluginCleanupResult, PluginHandlerRef, PluginKey,
    PluginResourceKind, PluginResourceRef, QueueSource,
};
use crate::package::{Capability, CapabilitySet};
use crate::runtime::{
    CapabilityOperation, CapabilityOperationId, CapabilityResourceEvent, CapabilityResourceId,
    CapabilityRuntimeError, CapabilityRuntimeErrorKind, CapabilityRuntimeEvent,
    CapabilityRuntimeHandle, CapabilityRuntimeRequest, CapabilityWatchEvent,
    PluginCapabilityRuntime, ScopedRelativePath, WatchCapabilityRequest, WatchChangeKind,
};
use crate::RequestId;

/// Default debounce window for file-watch events.
pub const DEFAULT_FILE_WATCH_DEBOUNCE_MS: u64 = 50;

/// Runtime configuration for capability-scoped file watches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWatchRuntimeConfig {
    /// Maximum active watch registrations.
    pub registration_capacity: usize,
    /// Maximum queued events retained per plugin before pressure is reported.
    pub event_capacity: usize,
    /// Debounce window used before coalesced events are delivered.
    pub debounce_ms: u64,
}

impl Default for FileWatchRuntimeConfig {
    fn default() -> Self {
        Self {
            registration_capacity: QueueSource::PluginWorker.default_capacity(),
            event_capacity: QueueSource::PluginWorker.default_capacity(),
            debounce_ms: DEFAULT_FILE_WATCH_DEBOUNCE_MS,
        }
    }
}

impl FileWatchRuntimeConfig {
    fn validate(&self) -> Result<(), CapabilityRuntimeError> {
        if self.registration_capacity == 0 {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "file watch registration capacity must be positive",
            ));
        }
        if self.event_capacity == 0 {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "file watch event capacity must be positive",
            ));
        }
        Ok(())
    }
}

/// Host-provided source for concrete file-watch backends.
pub trait FileWatchEventSource {
    /// Register one already-validated watch with the host backend.
    fn register(&mut self, registration: FileWatchRegistration)
        -> Result<(), FileWatchSourceError>;

    /// Release one watch from the host backend.
    fn unregister(&mut self, resource: &PluginResourceRef) -> Result<(), FileWatchSourceError>;

    /// Drain currently available backend events without blocking.
    fn drain_events(&mut self) -> Result<Vec<FileWatchSourceEvent>, FileWatchSourceError>;
}

/// Validated watch registration sent to a host source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWatchRegistration {
    /// Owning plugin.
    pub plugin_key: PluginKey,
    /// Operation that opened the watch.
    pub operation_id: CapabilityOperationId,
    /// Runtime resource for this watch.
    pub resource: PluginResourceRef,
    /// Opaque host-owned filesystem scope id.
    pub scope_id: String,
    /// Scoped relative path under the host-owned root.
    pub path: ScopedRelativePath,
    /// Include recursive descendants.
    pub recursive: bool,
    /// Optional same-plugin callback handler.
    pub callback: Option<PluginHandlerRef>,
}

/// Event emitted by a host source into the core watch runtime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWatchSourceEvent {
    /// Watch resource that observed the change.
    pub resource: PluginResourceRef,
    /// Path affected inside the watched scope, except for backend overflow.
    pub path: Option<ScopedRelativePath>,
    /// Stable change kind.
    pub change: WatchChangeKind,
    /// Injected monotonic timestamp in milliseconds for deterministic debounce.
    pub observed_at_ms: u64,
}

impl FileWatchSourceEvent {
    /// Build a path-specific backend event.
    #[must_use]
    pub fn path(
        resource: PluginResourceRef,
        path: ScopedRelativePath,
        change: WatchChangeKind,
        observed_at_ms: u64,
    ) -> Self {
        Self {
            resource,
            path: Some(path),
            change,
            observed_at_ms,
        }
    }

    /// Build a backend-overflow event.
    #[must_use]
    pub fn overflow(resource: PluginResourceRef, observed_at_ms: u64) -> Self {
        Self {
            resource,
            path: None,
            change: WatchChangeKind::Overflow,
            observed_at_ms,
        }
    }
}

/// Host source error mapped into the typed capability runtime surface.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileWatchSourceError {
    /// Stable capability runtime error category.
    pub kind: CapabilityRuntimeErrorKind,
    /// Human-readable detail.
    pub message: String,
}

impl FileWatchSourceError {
    /// Build a source error.
    #[must_use]
    pub fn new(kind: CapabilityRuntimeErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl fmt::Display for FileWatchSourceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.message)
    }
}

impl Error for FileWatchSourceError {}

impl From<FileWatchSourceError> for CapabilityRuntimeError {
    fn from(error: FileWatchSourceError) -> Self {
        Self::new(error.kind, error.message)
    }
}

/// Core-owned file watch runtime over a host-provided event source.
pub struct FileWatchRuntime<S> {
    config: FileWatchRuntimeConfig,
    source: S,
    grants: HashMap<PluginKey, CapabilitySet>,
    registrations: HashMap<PluginResourceRef, WatchRegistrationState>,
    events: HashMap<PluginKey, Vec<CapabilityRuntimeEvent>>,
    pending: HashMap<CoalescingKey, PendingWatchEvent>,
    pressure: HashMap<PluginKey, BackpressureSummary>,
    next_resource_id: u64,
    now_ms: u64,
}

impl<S> FileWatchRuntime<S>
where
    S: FileWatchEventSource,
{
    /// Build a runtime with default config.
    pub fn new(source: S) -> Self {
        Self::with_config(source, FileWatchRuntimeConfig::default())
            .expect("default file watch runtime config is valid")
    }

    /// Build a runtime with explicit config.
    pub fn with_config(
        source: S,
        config: FileWatchRuntimeConfig,
    ) -> Result<Self, CapabilityRuntimeError> {
        config.validate()?;
        Ok(Self {
            config,
            source,
            grants: HashMap::new(),
            registrations: HashMap::new(),
            events: HashMap::new(),
            pending: HashMap::new(),
            pressure: HashMap::new(),
            next_resource_id: 1,
            now_ms: 0,
        })
    }

    /// Grant one capability to a plugin.
    pub fn grant_capability(&mut self, plugin_key: PluginKey, capability: Capability) {
        self.grants
            .entry(plugin_key)
            .or_default()
            .insert(capability);
    }

    /// Advance the injected monotonic clock used for deterministic debounce.
    pub fn advance_to(&mut self, now_ms: u64) {
        self.now_ms = self.now_ms.max(now_ms);
    }

    /// Borrow the host source, useful for fake-source assertions.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Mutably borrow the host source, useful for fake-source event injection.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    fn submit_register(
        &mut self,
        request: CapabilityRuntimeRequest,
        scope_id: String,
        path: ScopedRelativePath,
        recursive: bool,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError> {
        self.validate_grant(&request)?;
        self.validate_callback(&request)?;
        if !path.is_scoped_relative() {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "watch path must be relative to the granted filesystem scope",
            ));
        }
        if self.registrations.len() >= self.config.registration_capacity {
            let pressure = self.backpressure_for(&request.plugin_key, self.registrations.len());
            self.pressure
                .insert(request.plugin_key.clone(), pressure.clone());
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::Backpressured,
                format!(
                    "file watch registration queue is at capacity {} for plugin {}",
                    pressure.capacity, request.plugin_key.0
                ),
            ));
        }

        let resource = request.resource_ref(self.next_resource_id());
        let registration = FileWatchRegistration {
            plugin_key: request.plugin_key.clone(),
            operation_id: request.operation_id.clone(),
            resource: resource.clone(),
            scope_id,
            path: path.clone(),
            recursive,
            callback: request.callback.clone(),
        };
        self.source.register(registration.clone())?;
        self.registrations.insert(
            resource.clone(),
            WatchRegistrationState {
                operation_id: request.operation_id.clone(),
                scope_id: registration.scope_id,
                path,
            },
        );
        self.enqueue_event(
            &request.plugin_key,
            CapabilityRuntimeEvent::ResourceOpened(CapabilityResourceEvent {
                plugin_key: request.plugin_key.clone(),
                operation_id: request.operation_id.clone(),
                resource: resource.clone(),
            }),
        );

        let required_capability = request.required_capability();
        Ok(CapabilityRuntimeHandle {
            plugin_key: request.plugin_key,
            operation_id: request.operation_id,
            resource: Some(resource),
            required_capability,
        })
    }

    fn submit_unregister(
        &mut self,
        request: CapabilityRuntimeRequest,
        scope_id: String,
        resource_id: CapabilityResourceId,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError> {
        self.validate_grant(&request)?;
        let resource = request.resource_ref(resource_id);
        let registration = self
            .registrations
            .get(&resource)
            .ok_or_else(|| {
                CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::ResourceNotFound,
                    "watch resource is not registered",
                )
            })?
            .clone();
        if registration.scope_id != scope_id {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "watch unregister scope does not match the registered resource",
            ));
        }

        self.release_registered_resource(resource.clone(), request.operation_id.clone())?;
        let required_capability = request.required_capability();
        Ok(CapabilityRuntimeHandle {
            plugin_key: request.plugin_key,
            operation_id: request.operation_id,
            resource: Some(resource),
            required_capability,
        })
    }

    fn validate_grant(
        &self,
        request: &CapabilityRuntimeRequest,
    ) -> Result<(), CapabilityRuntimeError> {
        let required = request.required_capability();
        if self
            .grants
            .get(&request.plugin_key)
            .is_some_and(|grants| grants.contains(&required))
        {
            return Ok(());
        }

        Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::CapabilityDenied,
            "plugin is missing the required filesystem capability grant",
        ))
    }

    fn validate_callback(
        &self,
        request: &CapabilityRuntimeRequest,
    ) -> Result<(), CapabilityRuntimeError> {
        if let Some(callback) = &request.callback {
            if callback.plugin_key != request.plugin_key {
                return Err(CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::InvalidRequest,
                    "watch callback must belong to the requesting plugin",
                ));
            }
        }
        Ok(())
    }

    fn release_registered_resource(
        &mut self,
        resource: PluginResourceRef,
        operation_id: CapabilityOperationId,
    ) -> Result<(), CapabilityRuntimeError> {
        let registration = self.registrations.remove(&resource).ok_or_else(|| {
            CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::ResourceNotFound,
                "watch resource is not registered",
            )
        })?;
        self.source.unregister(&resource)?;
        self.pending
            .retain(|key, _| key.resource != resource || key.plugin_key != resource.plugin_key);
        self.enqueue_event(
            &resource.plugin_key.clone(),
            CapabilityRuntimeEvent::ResourceReleased(CapabilityResourceEvent {
                plugin_key: resource.plugin_key.clone(),
                operation_id,
                resource: resource.clone(),
            }),
        );
        let _ = registration;
        Ok(())
    }

    fn poll_source(&mut self) -> Result<(), CapabilityRuntimeError> {
        for event in self.source.drain_events()? {
            self.now_ms = self.now_ms.max(event.observed_at_ms);
            let Some(registration) = self.registrations.get(&event.resource).cloned() else {
                continue;
            };
            let path = event.path.unwrap_or_else(|| registration.path.clone());
            let key = CoalescingKey {
                plugin_key: event.resource.plugin_key.clone(),
                resource: event.resource.clone(),
                path: if event.change == WatchChangeKind::Overflow {
                    None
                } else {
                    Some(path.clone())
                },
                overflow: event.change == WatchChangeKind::Overflow,
            };
            self.pending.insert(
                key,
                PendingWatchEvent {
                    path,
                    change: event.change,
                    due_at_ms: event.observed_at_ms + self.config.debounce_ms,
                },
            );
        }
        Ok(())
    }

    fn flush_due(&mut self, plugin_key: &PluginKey) {
        let due_keys = self
            .pending
            .iter()
            .filter(|(key, pending)| {
                &key.plugin_key == plugin_key && pending.due_at_ms <= self.now_ms
            })
            .map(|(key, _)| key.clone())
            .collect::<Vec<_>>();

        for key in due_keys {
            let Some(pending) = self.pending.remove(&key) else {
                continue;
            };
            self.enqueue_event(
                plugin_key,
                CapabilityRuntimeEvent::Watch(CapabilityWatchEvent {
                    resource: key.resource,
                    path: pending.path,
                    change: pending.change,
                }),
            );
        }
    }

    fn enqueue_event(&mut self, plugin_key: &PluginKey, event: CapabilityRuntimeEvent) {
        let queue_len = self.events.get(plugin_key).map_or(0, Vec::len);
        if queue_len >= self.config.event_capacity {
            self.pressure.insert(
                plugin_key.clone(),
                self.backpressure_for(plugin_key, queue_len),
            );
            return;
        }
        self.events
            .entry(plugin_key.clone())
            .or_default()
            .push(event);
    }

    fn backpressure_for(&self, plugin_key: &PluginKey, depth: usize) -> BackpressureSummary {
        BackpressureSummary {
            source: QueueSource::PluginWorker,
            capacity: self.config.event_capacity,
            depth,
            route: BackpressureRoute {
                session_id: None,
                client_id: None,
                subscription_id: None,
                plugin_key: Some(plugin_key.clone()),
            },
        }
    }

    fn next_resource_id(&mut self) -> CapabilityResourceId {
        let id = self.next_resource_id;
        self.next_resource_id += 1;
        CapabilityResourceId(format!("watch-{id}"))
    }
}

impl<S> PluginCapabilityRuntime for FileWatchRuntime<S>
where
    S: FileWatchEventSource,
{
    fn submit(
        &mut self,
        request: CapabilityRuntimeRequest,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError> {
        match request.operation.clone() {
            CapabilityOperation::Watch(WatchCapabilityRequest::Register {
                scope_id,
                path,
                recursive,
            }) => self.submit_register(request, scope_id, path, recursive),
            CapabilityOperation::Watch(WatchCapabilityRequest::Unregister {
                scope_id,
                resource_id,
            }) => self.submit_unregister(request, scope_id, resource_id),
            _ => Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "file watch runtime only accepts watch capability requests",
            )),
        }
    }

    fn cancel(
        &mut self,
        plugin_key: &PluginKey,
        operation_id: &CapabilityOperationId,
    ) -> Result<(), CapabilityRuntimeError> {
        let resources = self
            .registrations
            .iter()
            .filter(|(resource, registration)| {
                &resource.plugin_key == plugin_key && &registration.operation_id == operation_id
            })
            .map(|(resource, _)| resource.clone())
            .collect::<Vec<_>>();

        if resources.is_empty() {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::OperationNotFound,
                "watch operation is not registered",
            ));
        }

        for resource in resources {
            self.release_registered_resource(resource, operation_id.clone())?;
        }
        Ok(())
    }

    fn release_resource(
        &mut self,
        resource: PluginResourceRef,
    ) -> Result<(), CapabilityRuntimeError> {
        if resource.kind != PluginResourceKind::Watch {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "file watch runtime can only release watch resources",
            ));
        }
        let operation_id = self
            .registrations
            .get(&resource)
            .map(|registration| registration.operation_id.clone())
            .ok_or_else(|| {
                CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::ResourceNotFound,
                    "watch resource is not registered",
                )
            })?;
        self.release_registered_resource(resource, operation_id)
    }

    fn drain_events(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<Vec<CapabilityRuntimeEvent>, CapabilityRuntimeError> {
        self.poll_source()?;
        self.flush_due(plugin_key);
        let mut events = self.events.remove(plugin_key).unwrap_or_default();
        if let Some(pressure) = self.pressure.remove(plugin_key) {
            events.push(CapabilityRuntimeEvent::Backpressure(pressure));
        }
        Ok(events)
    }

    fn cleanup_plugin(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<PluginCleanupResult, CapabilityRuntimeError> {
        let resources = self
            .registrations
            .keys()
            .filter(|resource| &resource.plugin_key == plugin_key)
            .cloned()
            .collect::<Vec<_>>();
        for resource in &resources {
            self.source.unregister(resource)?;
        }
        let resources_set = resources.iter().cloned().collect::<HashSet<_>>();
        self.registrations
            .retain(|resource, _| !resources_set.contains(resource));
        self.pending
            .retain(|key, _| !resources_set.contains(&key.resource));

        let cleanup = PluginCleanupResult {
            request_id: RequestId(format!("file-watch-cleanup:{}", plugin_key.0)),
            plugin_key: plugin_key.clone(),
            removed_descriptors: Vec::new(),
            removed_resources: resources,
        };
        self.enqueue_event(
            plugin_key,
            CapabilityRuntimeEvent::CleanupCompleted(cleanup.clone()),
        );
        Ok(cleanup)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct WatchRegistrationState {
    operation_id: CapabilityOperationId,
    scope_id: String,
    path: ScopedRelativePath,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CoalescingKey {
    plugin_key: PluginKey,
    resource: PluginResourceRef,
    path: Option<ScopedRelativePath>,
    overflow: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PendingWatchEvent {
    path: ScopedRelativePath,
    change: WatchChangeKind,
    due_at_ms: u64,
}
