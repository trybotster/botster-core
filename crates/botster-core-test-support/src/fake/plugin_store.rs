//! Fake plugin-store backend and capability runtime helpers.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

use botster_core::{
    apply_plugin_store_merge_patch, plugin_store_payload_bytes, CapabilityOperation,
    CapabilityOperationCompleted, CapabilityOperationFailure, CapabilityResourceId,
    CapabilityRuntimeError, CapabilityRuntimeErrorKind, CapabilityRuntimeEvent,
    CapabilityRuntimeHandle, CapabilityRuntimeRequest, CapabilitySet, PluginCapabilityRuntime,
    PluginCleanupResult, PluginKey, PluginResourceRef, PluginStoreBackend,
    PluginStoreCapabilityRequest, PluginStoreEntry, PluginStoreKey, PluginStoreLimits,
    PluginStoreOperation, PluginStoreRecord, PluginStoreResult, RequestId,
};

type NamespaceRecords = BTreeMap<PluginStoreKey, PluginStoreRecord>;

/// In-memory plugin-store backend for downstream consumer tests.
#[derive(Debug, Clone, Default)]
pub struct FakePluginStoreBackend {
    records: Arc<Mutex<HashMap<PluginKey, NamespaceRecords>>>,
}

impl FakePluginStoreBackend {
    /// Build an empty fake backend.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Return all records owned by one plugin namespace.
    #[must_use]
    pub fn records_for(&self, plugin_key: &PluginKey) -> Vec<PluginStoreRecord> {
        self.records
            .lock()
            .expect("fake plugin store lock")
            .get(plugin_key)
            .map(|records| records.values().cloned().collect())
            .unwrap_or_default()
    }

    fn enforce_limits(
        records: &NamespaceRecords,
        key: &PluginStoreKey,
        replacement_payload: &serde_json::Value,
        limits: PluginStoreLimits,
    ) -> Result<(), CapabilityRuntimeError> {
        let replacement_bytes = plugin_store_payload_bytes(replacement_payload);
        if replacement_bytes > limits.max_record_bytes {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::QuotaExceeded,
                "plugin-store record exceeds max_record_bytes",
            ));
        }

        if !records.contains_key(key) && records.len() + 1 > limits.max_plugin_keys {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::QuotaExceeded,
                "plugin-store namespace exceeds max_plugin_keys",
            ));
        }

        let current_bytes = records
            .iter()
            .filter(|(record_key, _)| *record_key != key)
            .map(|(_, record)| record.payload_bytes())
            .sum::<usize>();
        if current_bytes.saturating_add(replacement_bytes) > limits.max_plugin_bytes {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::QuotaExceeded,
                "plugin-store namespace exceeds max_plugin_bytes",
            ));
        }

        Ok(())
    }

    fn revision_for_write(
        current: Option<&PluginStoreRecord>,
        expected_revision: Option<u64>,
    ) -> Result<u64, CapabilityRuntimeError> {
        match (current, expected_revision) {
            (Some(record), Some(expected)) if record.revision != expected => {
                Err(CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::RevisionConflict,
                    "plugin-store revision did not match expected revision",
                ))
            }
            (None, Some(expected)) if expected != 0 => Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::RevisionConflict,
                "plugin-store create expected revision must be 0",
            )),
            (Some(record), _) => Ok(record.revision + 1),
            (None, _) => Ok(1),
        }
    }
}

impl PluginStoreBackend for FakePluginStoreBackend {
    fn get(
        &self,
        plugin_key: &PluginKey,
        key: &PluginStoreKey,
    ) -> Result<Option<PluginStoreRecord>, CapabilityRuntimeError> {
        Ok(self
            .records
            .lock()
            .expect("fake plugin store lock")
            .get(plugin_key)
            .and_then(|records| records.get(key).cloned()))
    }

    fn set(
        &self,
        plugin_key: &PluginKey,
        key: PluginStoreKey,
        schema_version: u64,
        payload: serde_json::Value,
        expected_revision: Option<u64>,
        limits: PluginStoreLimits,
    ) -> Result<PluginStoreRecord, CapabilityRuntimeError> {
        let mut namespaces = self.records.lock().expect("fake plugin store lock");
        let records = namespaces.entry(plugin_key.clone()).or_default();
        let revision = Self::revision_for_write(records.get(&key), expected_revision)?;
        Self::enforce_limits(records, &key, &payload, limits)?;

        let record = PluginStoreRecord {
            plugin_key: plugin_key.clone(),
            key: key.clone(),
            schema_version,
            revision,
            payload,
        };
        records.insert(key, record.clone());
        Ok(record)
    }

    fn delete(
        &self,
        plugin_key: &PluginKey,
        key: &PluginStoreKey,
    ) -> Result<PluginStoreRecord, CapabilityRuntimeError> {
        let mut namespaces = self.records.lock().expect("fake plugin store lock");
        namespaces
            .entry(plugin_key.clone())
            .or_default()
            .remove(key)
            .ok_or_else(|| {
                CapabilityRuntimeError::new(
                    CapabilityRuntimeErrorKind::StoreNotFound,
                    "plugin-store record was not found",
                )
            })
    }

    fn list(
        &self,
        plugin_key: &PluginKey,
        prefix: Option<&str>,
    ) -> Result<Vec<PluginStoreEntry>, CapabilityRuntimeError> {
        let namespaces = self.records.lock().expect("fake plugin store lock");
        Ok(namespaces
            .get(plugin_key)
            .into_iter()
            .flat_map(|records| records.values())
            .filter(|record| {
                prefix
                    .map(|prefix| record.key.0.starts_with(prefix))
                    .unwrap_or(true)
            })
            .map(PluginStoreEntry::from)
            .collect())
    }

    fn patch(
        &self,
        plugin_key: &PluginKey,
        key: &PluginStoreKey,
        patch: serde_json::Value,
        expected_revision: Option<u64>,
        limits: PluginStoreLimits,
    ) -> Result<PluginStoreRecord, CapabilityRuntimeError> {
        let mut namespaces = self.records.lock().expect("fake plugin store lock");
        let records = namespaces.entry(plugin_key.clone()).or_default();
        let current = records.get(key).cloned().ok_or_else(|| {
            CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::StoreNotFound,
                "plugin-store record was not found",
            )
        })?;
        let revision = Self::revision_for_write(Some(&current), expected_revision)?;
        let mut payload = current.payload.clone();
        apply_plugin_store_merge_patch(&mut payload, &patch)?;
        Self::enforce_limits(records, key, &payload, limits)?;

        let record = PluginStoreRecord {
            revision,
            payload,
            ..current
        };
        records.insert(key.clone(), record.clone());
        Ok(record)
    }
}

/// Deterministic plugin-store capability runtime backed by [`FakePluginStoreBackend`].
#[derive(Debug, Clone)]
pub struct FakePluginStoreCapabilityRuntime {
    backend: FakePluginStoreBackend,
    capabilities: CapabilitySet,
    limits: PluginStoreLimits,
    pending: Arc<Mutex<Vec<CapabilityRuntimeRequest>>>,
    events: Arc<Mutex<HashMap<PluginKey, Vec<CapabilityRuntimeEvent>>>>,
    resources: Arc<Mutex<Vec<PluginResourceRef>>>,
}

impl FakePluginStoreCapabilityRuntime {
    /// Build a fake runtime with the supplied capabilities.
    #[must_use]
    pub fn new(capabilities: CapabilitySet) -> Self {
        Self::with_backend_and_limits(
            FakePluginStoreBackend::new(),
            capabilities,
            PluginStoreLimits::default(),
        )
    }

    /// Build a fake runtime with explicit backend and limits.
    #[must_use]
    pub fn with_backend_and_limits(
        backend: FakePluginStoreBackend,
        capabilities: CapabilitySet,
        limits: PluginStoreLimits,
    ) -> Self {
        Self {
            backend,
            capabilities,
            limits,
            pending: Arc::new(Mutex::new(Vec::new())),
            events: Arc::new(Mutex::new(HashMap::new())),
            resources: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Access the backing fake backend.
    #[must_use]
    pub fn backend(&self) -> FakePluginStoreBackend {
        self.backend.clone()
    }

    fn validate_request(
        &self,
        request: &CapabilityRuntimeRequest,
    ) -> Result<(), CapabilityRuntimeError> {
        if !self.capabilities.contains(&request.required_capability()) {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::CapabilityDenied,
                "plugin-store capability is not declared for this namespace",
            ));
        }

        let CapabilityOperation::PluginStore(store_request) = &request.operation else {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "fake plugin-store runtime only accepts plugin-store operations",
            ));
        };

        if store_request.namespace != request.plugin_key.0 {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "plugin-store namespace must match the owning plugin key",
            ));
        }

        validate_store_operation(store_request)
    }

    fn complete(&self, request: CapabilityRuntimeRequest) -> CapabilityRuntimeEvent {
        match self.execute(&request) {
            Ok(plugin_store) => CapabilityRuntimeEvent::Completed(CapabilityOperationCompleted {
                plugin_key: request.plugin_key,
                operation_id: request.operation_id,
                response: None,
                plugin_store: Some(plugin_store),
            }),
            Err(error) => CapabilityRuntimeEvent::Failed(CapabilityOperationFailure {
                plugin_key: request.plugin_key,
                operation_id: request.operation_id,
                error_kind: error.kind,
                reason: error.message,
            }),
        }
    }

    fn execute(
        &self,
        request: &CapabilityRuntimeRequest,
    ) -> Result<PluginStoreResult, CapabilityRuntimeError> {
        let CapabilityOperation::PluginStore(PluginStoreCapabilityRequest { operation, .. }) =
            &request.operation
        else {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::InvalidRequest,
                "request was not a plugin-store operation",
            ));
        };

        match operation {
            PluginStoreOperation::Get { key } => self
                .backend
                .get(&request.plugin_key, key)?
                .map(|record| PluginStoreResult::Record { record })
                .ok_or_else(|| {
                    CapabilityRuntimeError::new(
                        CapabilityRuntimeErrorKind::StoreNotFound,
                        "plugin-store record was not found",
                    )
                }),
            PluginStoreOperation::Set {
                key,
                schema_version,
                payload,
                expected_revision,
            } => self
                .backend
                .set(
                    &request.plugin_key,
                    key.clone(),
                    *schema_version,
                    payload.clone(),
                    *expected_revision,
                    self.limits,
                )
                .map(|record| PluginStoreResult::Written { record }),
            PluginStoreOperation::Delete { key } => self
                .backend
                .delete(&request.plugin_key, key)
                .map(|record| PluginStoreResult::Deleted {
                    key: record.key,
                    revision: record.revision,
                }),
            PluginStoreOperation::List { prefix } => self
                .backend
                .list(&request.plugin_key, prefix.as_deref())
                .map(|entries| PluginStoreResult::List { entries }),
            PluginStoreOperation::Patch {
                key,
                patch,
                expected_revision,
            } => self
                .backend
                .patch(
                    &request.plugin_key,
                    key,
                    patch.clone(),
                    *expected_revision,
                    self.limits,
                )
                .map(|record| PluginStoreResult::Written { record }),
        }
    }
}

impl PluginCapabilityRuntime for FakePluginStoreCapabilityRuntime {
    fn submit(
        &mut self,
        request: CapabilityRuntimeRequest,
    ) -> Result<CapabilityRuntimeHandle, CapabilityRuntimeError> {
        self.validate_request(&request)?;
        let resource = request.resource_ref(CapabilityResourceId(request.operation_id.0.clone()));
        self.resources
            .lock()
            .expect("fake plugin store resources lock")
            .push(resource.clone());
        self.pending
            .lock()
            .expect("fake plugin store pending lock")
            .push(request.clone());
        let required_capability = request.required_capability();

        Ok(CapabilityRuntimeHandle {
            plugin_key: request.plugin_key,
            operation_id: request.operation_id,
            resource: Some(resource),
            required_capability,
        })
    }

    fn cancel(
        &mut self,
        plugin_key: &PluginKey,
        operation_id: &botster_core::CapabilityOperationId,
    ) -> Result<(), CapabilityRuntimeError> {
        let mut pending = self.pending.lock().expect("fake plugin store pending lock");
        let before = pending.len();
        pending.retain(|request| {
            &request.plugin_key != plugin_key || &request.operation_id != operation_id
        });
        if pending.len() == before {
            return Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::OperationNotFound,
                "plugin-store operation is not pending",
            ));
        }
        Ok(())
    }

    fn release_resource(
        &mut self,
        resource: PluginResourceRef,
    ) -> Result<(), CapabilityRuntimeError> {
        let mut resources = self
            .resources
            .lock()
            .expect("fake plugin store resources lock");
        let before = resources.len();
        resources.retain(|stored| stored != &resource);
        if resources.len() < before {
            Ok(())
        } else {
            Err(CapabilityRuntimeError::new(
                CapabilityRuntimeErrorKind::ResourceNotFound,
                "plugin-store resource was not found",
            ))
        }
    }

    fn drain_events(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<Vec<CapabilityRuntimeEvent>, CapabilityRuntimeError> {
        let mut ready = Vec::new();
        let mut retained = Vec::new();
        for request in self
            .pending
            .lock()
            .expect("fake plugin store pending lock")
            .drain(..)
        {
            if &request.plugin_key == plugin_key {
                ready.push(request);
            } else {
                retained.push(request);
            }
        }
        *self.pending.lock().expect("fake plugin store pending lock") = retained;

        let completed = ready.into_iter().map(|request| self.complete(request));
        let mut events = self.events.lock().expect("fake plugin store events lock");
        let plugin_events = events.entry(plugin_key.clone()).or_default();
        plugin_events.extend(completed);
        Ok(std::mem::take(plugin_events))
    }

    fn cleanup_plugin(
        &mut self,
        plugin_key: &PluginKey,
    ) -> Result<PluginCleanupResult, CapabilityRuntimeError> {
        self.pending
            .lock()
            .expect("fake plugin store pending lock")
            .retain(|request| &request.plugin_key != plugin_key);
        let mut resources = self
            .resources
            .lock()
            .expect("fake plugin store resources lock");
        let mut removed_resources = Vec::new();
        resources.retain(|resource| {
            if &resource.plugin_key == plugin_key {
                removed_resources.push(resource.clone());
                false
            } else {
                true
            }
        });

        Ok(PluginCleanupResult {
            request_id: RequestId("fake-plugin-store-cleanup".to_string()),
            plugin_key: plugin_key.clone(),
            removed_descriptors: Vec::new(),
            removed_resources,
        })
    }
}

fn validate_store_operation(
    request: &PluginStoreCapabilityRequest,
) -> Result<(), CapabilityRuntimeError> {
    let keys = match &request.operation {
        PluginStoreOperation::Get { key }
        | PluginStoreOperation::Delete { key }
        | PluginStoreOperation::Set { key, .. }
        | PluginStoreOperation::Patch { key, .. } => vec![key],
        PluginStoreOperation::List { .. } => Vec::new(),
    };
    if keys.iter().any(|key| !key.is_valid()) {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::InvalidRequest,
            "plugin-store key is invalid",
        ));
    }
    if matches!(
        &request.operation,
        PluginStoreOperation::Patch { patch, .. } if !patch.is_object()
    ) {
        return Err(CapabilityRuntimeError::new(
            CapabilityRuntimeErrorKind::PatchFailed,
            "plugin-store merge patch must be a JSON object",
        ));
    }
    Ok(())
}
