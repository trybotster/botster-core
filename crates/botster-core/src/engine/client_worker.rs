//! Synchronous ClientWorker: subscription queues, adapter pump, and teardown.
//!
//! This is the production bound-adapter egress owner. It is not
//! [`crate::contract::client_stream::ClientStreamHarness`]. Hosts advance it
//! through `wait_wakes` and `pump_woken`. There is no ClientWorker OS thread.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use botster_terminal_protocol::{
    TerminalCapabilitySet, TerminalFrame, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
    MAX_PASTE_BYTES, MAX_PASTE_CHUNK_DATA_BYTES,
};
use botster_terminal_protocol_client::{
    decode_terminal_input, AttachState, AttachStateKind, ProcessExit, Snapshot, SnapshotPhase,
    TerminalEvent, TerminalInputCommand, TerminalInputKind, TerminalInputRejection,
    TerminalInputResult, TerminalModeFlags, TerminalOutput,
};

use crate::actor::{QueueSource, TerminalAttachState};
use crate::client::ClientId;
use crate::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
};
use crate::contract::terminal_subscription::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, PasteOperation,
    TerminalInputDelivery, TerminalInputOperation, TerminalSubscriptionGeneration,
    TerminalSubscriptionRecord,
};
use crate::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeSource, WakingTerminalAdapter,
};
use crate::session::{SessionId, SubscriptionId};
use crate::transport::TransportEgress;
use crate::WorkerSnapshotPhase;

const WRITE_ATTEMPT_BUDGET: usize = 512;
/// Bounded per-subscription ingress backlog.
pub const INPUT_QUEUE_CAPACITY: usize = 256;
/// Stage A intake budget.
pub const INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK: usize = 64;
/// Stage B apply budget.
pub const APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK: usize = 16;
/// Maximum time between an accepted paste begin and complete commit.
pub const PASTE_ASSEMBLY_TIMEOUT: Duration = Duration::from_secs(5);

/// Routes that must be unsubscribed after ClientWorker ownership hard-stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientWorkerTeardown {
    /// Client that owned the torn-down subscription.
    pub client_id: ClientId,
    /// Session of the torn-down subscription.
    pub session_id: SessionId,
    /// Subscription identity that was removed.
    pub subscription_id: SubscriptionId,
    /// Generation that was removed.
    pub generation: TerminalSubscriptionGeneration,
    /// Outstanding gated request id, if this owner was parked.
    pub awaiting_gated: Option<String>,
}

/// Failure while enqueueing a client-facing `input_result`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueInputResultError {
    /// The owner was already removed.
    OwnerGone,
    /// The result could not be encoded as a terminal frame.
    EncodeFailed,
    /// The owner's egress queue is already at capacity.
    EgressFull,
}

/// Synchronous per-engine ClientWorker.
#[derive(Default)]
pub struct ClientWorker {
    live: HashMap<OwnerKey, SubscriptionOwner>,
    last_generation: HashMap<OwnerKey, TerminalSubscriptionGeneration>,
    next_snapshot_phase: HashMap<OwnerKey, SnapshotPhase>,
    expected_adapters: HashSet<(ClientId, OwnerKey)>,
    capacity_parked: HashMap<OwnerKey, TerminalSubscriptionGeneration>,
    input_cursor: usize,
    wake_source: TerminalWakeSource,
    bound_queue_wake_sessions: HashSet<SessionId>,
    #[cfg(test)]
    fail_next_encode: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OwnerKey {
    pub(crate) session_id: SessionId,
    pub(crate) subscription_id: SubscriptionId,
}

struct SubscriptionOwner {
    client_id: ClientId,
    generation: TerminalSubscriptionGeneration,
    adapter: Option<Box<dyn TerminalAdapter + Send>>,
    capabilities: Option<TerminalCapabilitySet>,
    queue: VecDeque<QueuedFrame>,
    held: VecDeque<(TransportEgress, Option<SnapshotPhase>)>,
    hold_until_bound: bool,
    unsuccessful_writes: usize,
    in_flight: bool,
    process_exit_enqueued: bool,
    process_exit_delivered: bool,
    input_queue: VecDeque<TerminalInputOperation>,
    awaiting_gated: Option<GatedWait>,
    paste: Option<PasteAssembly>,
    paste_in_flight: Option<u32>,
    last_paste_operation_id: Option<u32>,
}

struct PasteAssembly {
    operation_id: u32,
    mode_generation: u64,
    mode_revision: u64,
    total_len: usize,
    expected_chunks: usize,
    next_index: usize,
    data: Vec<u8>,
    deadline: Instant,
}

/// Outstanding mode-gated request for one owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatedWait {
    /// Correlated worker request id.
    pub request_id: String,
    /// When the parent wait expires.
    pub deadline: Instant,
    /// Client-visible input kind for the worker result.
    pub kind: TerminalInputKind,
    /// Paste operation id, when this wait belongs to a paste.
    pub operation_id: Option<u32>,
}

struct QueuedFrame {
    frame: TerminalFrame,
    kind: QueuedKind,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueuedKind {
    Snapshot,
    ProcessExit,
    Other,
}

impl ClientWorker {
    /// Build an empty ClientWorker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assign or reuse a generation on attach and publish the inventory row.
    ///
    /// A client that attaches a new subscription for the same session hard-stops
    /// the previous owner for that client and session.
    pub fn record_attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> (TerminalSubscriptionGeneration, Vec<ClientWorkerTeardown>) {
        let mut replacements =
            self.teardown_replaced_client_session(&client_id, &session_id, &subscription_id);
        let key = OwnerKey {
            session_id,
            subscription_id,
        };
        if let Some(generation) = self
            .live
            .get(&key)
            .and_then(|existing| (existing.client_id == client_id).then_some(existing.generation))
        {
            self.expected_adapters.remove(&(client_id, key));
            return (generation, replacements);
        }
        if self.live.contains_key(&key) {
            if let Some(stolen) = self.hard_stop_key(&key) {
                replacements.push(stolen);
            }
        }
        let generation = TerminalSubscriptionGeneration(
            self.last_generation
                .get(&key)
                .map(|generation| generation.0 + 1)
                .unwrap_or(1),
        );
        self.last_generation.insert(key.clone(), generation);
        let hold_until_bound = self
            .expected_adapters
            .remove(&(client_id.clone(), key.clone()));
        self.live.insert(
            key,
            SubscriptionOwner {
                client_id,
                generation,
                adapter: None,
                capabilities: None,
                queue: VecDeque::new(),
                held: VecDeque::new(),
                hold_until_bound,
                unsuccessful_writes: 0,
                in_flight: false,
                process_exit_enqueued: false,
                process_exit_delivered: false,
                input_queue: VecDeque::new(),
                awaiting_gated: None,
                paste: None,
                paste_in_flight: None,
                last_paste_operation_id: None,
            },
        );
        (generation, replacements)
    }

    /// Record that the next attach for this identity will bind an adapter.
    ///
    /// A matching [`Self::record_attach`] consumes the declaration, including
    /// an idempotent attach that reuses an existing owner. Only a new owner
    /// created by that attach holds initial frames until bind. A declaration
    /// for a different `client_id` is not consumed.
    pub fn expect_terminal_adapter(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.expected_adapters.insert((
            client_id,
            OwnerKey {
                session_id,
                subscription_id,
            },
        ));
    }

    /// Retire an unconsumed pre-attach adapter declaration.
    pub fn cancel_expected_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) {
        self.expected_adapters.remove(&(
            client_id.clone(),
            OwnerKey {
                session_id,
                subscription_id,
            },
        ));
    }

    fn teardown_replaced_client_session(
        &mut self,
        client_id: &ClientId,
        session_id: &SessionId,
        keep_subscription: &SubscriptionId,
    ) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self
            .live
            .iter()
            .filter(|(key, owner)| {
                &key.session_id == session_id
                    && &owner.client_id == client_id
                    && &key.subscription_id != keep_subscription
            })
            .map(|(key, _)| key.clone())
            .collect();
        keys.into_iter()
            .filter_map(|key| self.hard_stop_key(&key))
            .collect()
    }

    fn hard_stop_key(&mut self, key: &OwnerKey) -> Option<ClientWorkerTeardown> {
        self.wake_source
            .retire_route(&key.session_id, &key.subscription_id);
        self.next_snapshot_phase.remove(key);
        self.capacity_parked.remove(key);
        hard_stop(&mut self.live, key)
    }

    /// Replace the wake source. Construction-only; do not call after a waking bind.
    pub fn set_wake_source(&mut self, source: TerminalWakeSource) {
        self.wake_source = source;
    }

    /// Shared host wait source for this worker.
    #[must_use]
    pub fn wake_source(&self) -> &TerminalWakeSource {
        &self.wake_source
    }

    /// Bind a waking adapter after the live-generation rejection ladder.
    ///
    /// Allocation and registry insert happen only after every rejection returns.
    /// Rejected binds close and drop the adapter and allocate nothing.
    ///
    /// A plain [`TerminalAdapter`] cannot use the waking bind.
    ///
    /// ```compile_fail
    /// use botster_core::contract::terminal_adapter::{
    ///     TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
    /// };
    /// use botster_core::{ClientId, ClientWorker, SessionId, SubscriptionId, TerminalCapabilitySet};
    /// use botster_terminal_protocol::TerminalFrame;
    ///
    /// struct PollingAdapter;
    /// impl TerminalAdapter for PollingAdapter {
    ///     fn try_write(&mut self, _: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
    ///         Ok(())
    ///     }
    ///     fn close(&mut self) {}
    ///     fn pressure(&self) -> TerminalAdapterPressure { TerminalAdapterPressure::Ready }
    ///     fn try_read(&mut self) -> TerminalIngress { TerminalIngress::Empty }
    /// }
    ///
    /// let mut worker = ClientWorker::new();
    /// let client_id = ClientId("client".into());
    /// let session_id = SessionId("session".into());
    /// let subscription_id = SubscriptionId("subscription".into());
    /// let (generation, _) = worker.record_attach(
    ///     client_id.clone(),
    ///     session_id.clone(),
    ///     subscription_id.clone(),
    /// );
    /// worker.bind_waking_terminal_adapter(
    ///     &client_id,
    ///     session_id,
    ///     subscription_id,
    ///     generation,
    ///     TerminalCapabilitySet::empty(),
    ///     Box::new(PollingAdapter),
    /// );
    /// ```
    pub fn bind_waking_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        mut adapter: Box<dyn WakingTerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        let live_generation = {
            let Some(owner) = self.live.get_mut(&key) else {
                adapter.close();
                drop(adapter);
                return Err(if self.last_generation.contains_key(&key) {
                    BindTerminalAdapterError::UnknownSubscription {
                        session_id,
                        subscription_id,
                    }
                } else {
                    BindTerminalAdapterError::BindBeforeAttach {
                        session_id,
                        subscription_id,
                    }
                });
            };
            if &owner.client_id != client_id || owner.generation != generation {
                let live = Some(owner.generation);
                adapter.close();
                drop(adapter);
                return Err(BindTerminalAdapterError::StaleGeneration {
                    live,
                    requested: generation,
                });
            }
            if owner.adapter.is_some() {
                adapter.close();
                drop(adapter);
                return Err(BindTerminalAdapterError::AlreadyBound {
                    session_id,
                    subscription_id,
                    generation,
                });
            }
            owner.generation
        };
        let sink = self
            .wake_source
            .bind_route(session_id, subscription_id, live_generation);
        adapter.set_wake_sink(sink);
        let Some(owner) = self.live.get_mut(&key) else {
            adapter.close();
            drop(adapter);
            self.wake_source
                .retire_route(&key.session_id, &key.subscription_id);
            return Err(BindTerminalAdapterError::UnknownSubscription {
                session_id: key.session_id,
                subscription_id: key.subscription_id,
            });
        };
        owner.adapter = Some(Box::new(WakingAdapterHolder { inner: adapter }));
        owner.capabilities = Some(capabilities);
        Ok(())
    }

    /// Take session ids whose bound Ready queues grew since the last take.
    ///
    /// Non-pump drains notify these sessions. Pump paths discard the set so
    /// pump-time ingest cannot enqueue a second ingress wake.
    #[must_use]
    pub fn take_bound_queue_wake_sessions(&mut self) -> HashSet<SessionId> {
        std::mem::take(&mut self.bound_queue_wake_sessions)
    }

    /// Whether any live owner for `session_id` still holds undelivered frames.
    #[must_use]
    pub fn session_has_undelivered_frames(&self, session_id: &SessionId) -> bool {
        self.live.iter().any(|(key, owner)| {
            &key.session_id == session_id
                && (!owner.held.is_empty() || !owner.queue.is_empty() || owner.in_flight)
        })
    }

    /// Whether the live owner still holds frames that the next pump must flush.
    #[must_use]
    pub fn bound_owner_has_held_frames(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .is_some_and(|owner| owner.adapter.is_some() && !owner.held.is_empty())
    }

    fn owner_ready_for_bound_queue_wake(owner: &SubscriptionOwner) -> bool {
        owner.adapter.as_ref().is_some_and(|adapter| {
            !owner.in_flight && adapter.pressure() == TerminalAdapterPressure::Ready
        })
    }

    /// Return control-plane inventory rows without terminal state.
    #[must_use]
    pub fn list_terminal_subscriptions(&self) -> Vec<TerminalSubscriptionRecord> {
        let mut records: Vec<_> = self
            .live
            .iter()
            .map(|(key, owner)| TerminalSubscriptionRecord {
                client_id: owner.client_id.clone(),
                session_id: key.session_id.clone(),
                subscription_id: key.subscription_id.clone(),
                generation: owner.generation,
                adapter_bound: owner.adapter.is_some(),
                capabilities: owner.capabilities.clone(),
            })
            .collect();
        records.sort_by(|left, right| {
            left.session_id
                .0
                .cmp(&right.session_id.0)
                .then(left.subscription_id.0.cmp(&right.subscription_id.0))
                .then(left.generation.0.cmp(&right.generation.0))
        });
        records
    }

    /// Whether a live inventory row exists for this subscription.
    #[must_use]
    pub fn has_subscription(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.live.contains_key(&OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        })
    }

    /// Live generation for a subscription, if present.
    #[must_use]
    pub fn live_generation(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<TerminalSubscriptionGeneration> {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .map(|owner| owner.generation)
    }

    /// Remember the worker snapshot phase for the next ingested Snapshot frame.
    pub fn note_snapshot_phase(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        phase: WorkerSnapshotPhase,
    ) {
        self.next_snapshot_phase.insert(
            OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            },
            match phase {
                WorkerSnapshotPhase::Ready => SnapshotPhase::Ready,
                WorkerSnapshotPhase::History => SnapshotPhase::History,
                WorkerSnapshotPhase::Finish => SnapshotPhase::Finish,
            },
        );
    }

    /// Strip bound-route terminal frames from `egress` into ClientWorker queues.
    pub fn ingest_bound_terminal_frames(
        &mut self,
        egress: &mut Vec<(ClientId, TransportEgress)>,
    ) -> Vec<ClientWorkerTeardown> {
        let mut retained = Vec::with_capacity(egress.len());
        let mut teardowns = self.flush_held_after_bind();
        let mut failed_routes = HashSet::new();
        let mut unbound_process_exits = Vec::new();
        let mut bound_queue_wakes = Vec::new();
        for (client_id, frame) in egress.drain(..) {
            let Some((session_id, subscription_id)) = terminal_route(&frame) else {
                retained.push((client_id, frame));
                continue;
            };
            let key = OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            };
            if failed_routes.contains(&key) {
                continue;
            }
            let Some(owner) = self.live.get_mut(&key) else {
                self.next_snapshot_phase.remove(&key);
                if matches!(frame, TransportEgress::ProcessExit { .. }) {
                    retained.push((client_id, frame));
                }
                continue;
            };
            if owner.client_id != client_id {
                retained.push((client_id, frame));
                continue;
            }
            if owner.adapter.is_none() {
                if owner.hold_until_bound {
                    if matches!(
                        frame,
                        TransportEgress::AttachState {
                            state: TerminalAttachState::Detached,
                            ..
                        }
                    ) {
                        retained.push((client_id, frame));
                        continue;
                    }
                    if owner.held.len() >= QueueSource::ClientWorker.default_capacity() {
                        failed_routes.insert(key.clone());
                        if let Some(teardown) = self.hard_stop_key(&key) {
                            teardowns.push(teardown);
                        }
                        continue;
                    }
                    let phase = if matches!(frame, TransportEgress::Snapshot { .. }) {
                        self.next_snapshot_phase.remove(&key)
                    } else {
                        None
                    };
                    if matches!(frame, TransportEgress::ProcessExit { .. }) {
                        owner.process_exit_enqueued = true;
                    }
                    owner.held.push_back((frame, phase));
                    continue;
                }
                if matches!(frame, TransportEgress::Snapshot { .. }) {
                    self.next_snapshot_phase.remove(&key);
                }
                if matches!(frame, TransportEgress::ProcessExit { .. }) {
                    unbound_process_exits.push(key);
                }
                retained.push((client_id, frame));
                continue;
            }
            if owner.process_exit_enqueued {
                continue;
            }
            let capabilities = owner
                .capabilities
                .clone()
                .unwrap_or_else(TerminalCapabilitySet::empty);
            match encode_terminal_frame(
                &key,
                &frame,
                self.next_snapshot_phase.remove(&key),
                &capabilities,
            ) {
                Ok(Some(queued)) => {
                    if owner.queue.len() >= QueueSource::ClientWorker.default_capacity() {
                        failed_routes.insert(key.clone());
                        if let Some(teardown) = self.hard_stop_key(&key) {
                            teardowns.push(teardown);
                        }
                    } else {
                        let is_process_exit = queued.kind == QueuedKind::ProcessExit;
                        let ready = Self::owner_ready_for_bound_queue_wake(owner);
                        owner.queue.push_back(queued);
                        if is_process_exit {
                            owner.process_exit_enqueued = true;
                        }
                        if ready {
                            bound_queue_wakes.push(key.session_id.clone());
                        }
                    }
                }
                Ok(None) => {
                    if !matches!(frame, TransportEgress::Snapshot { .. }) {
                        retained.push((client_id, frame));
                    }
                }
                Err(()) => {
                    failed_routes.insert(key.clone());
                    if let Some(teardown) = self.hard_stop_key(&key) {
                        teardowns.push(teardown);
                    }
                }
            }
        }
        for key in unbound_process_exits {
            if let Some(teardown) = self.hard_stop_key(&key) {
                teardowns.push(teardown);
            }
        }
        self.bound_queue_wake_sessions.extend(bound_queue_wakes);
        *egress = retained;
        teardowns
    }

    fn flush_held_after_bind(&mut self) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self
            .live
            .iter()
            .filter(|(_, owner)| {
                owner.adapter.is_some() && (owner.hold_until_bound || !owner.held.is_empty())
            })
            .map(|(key, _)| key.clone())
            .collect();
        let mut teardowns = Vec::new();
        for key in keys {
            if let Some(teardown) = self.flush_held_owner(&key) {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    fn flush_held_owner(&mut self, key: &OwnerKey) -> Option<ClientWorkerTeardown> {
        loop {
            let (frame, phase, capabilities) = {
                let owner = self.live.get_mut(key)?;
                owner.adapter.as_ref()?;
                let Some(held) = owner.held.pop_front() else {
                    owner.hold_until_bound = false;
                    return None;
                };
                (
                    held.0,
                    held.1,
                    owner
                        .capabilities
                        .clone()
                        .unwrap_or_else(TerminalCapabilitySet::empty),
                )
            };
            #[cfg(test)]
            if self.fail_next_encode {
                self.fail_next_encode = false;
                return self.hard_stop_key(key);
            }
            match encode_terminal_frame(key, &frame, phase, &capabilities) {
                Ok(Some(queued)) => {
                    let ready;
                    {
                        let owner = self.live.get_mut(key)?;
                        if owner.queue.len() >= QueueSource::ClientWorker.default_capacity() {
                            return self.hard_stop_key(key);
                        }
                        let is_process_exit = queued.kind == QueuedKind::ProcessExit;
                        ready = Self::owner_ready_for_bound_queue_wake(owner);
                        owner.queue.push_back(queued);
                        if is_process_exit {
                            owner.process_exit_enqueued = true;
                        }
                    }
                    if ready {
                        self.bound_queue_wake_sessions
                            .insert(key.session_id.clone());
                    }
                }
                Ok(None) => {}
                Err(()) => return self.hard_stop_key(key),
            }
        }
    }

    /// Pump only routes named by a wake batch. Never scans unbound or unnamed routes.
    pub fn pump_woken(&mut self, batch: &TerminalWakeBatch) -> Vec<ClientWorkerTeardown> {
        let route_keys = self.adapter_route_keys(batch);
        let mut teardowns = self.expire_pastes_keys(&route_keys, Instant::now());
        let mut seen = HashSet::new();
        for route in &batch.adapter_routes {
            let key = OwnerKey {
                session_id: route.session_id.clone(),
                subscription_id: route.subscription_id.clone(),
            };
            if !seen.insert(key.clone()) {
                continue;
            }
            if let Some(teardown) = pump_one(
                &mut self.live,
                &mut self.next_snapshot_phase,
                &self.wake_source,
                &key,
            ) {
                teardowns.push(teardown);
            }
        }
        for session_id in &batch.ingress_sessions {
            let keys: Vec<_> = self
                .live
                .iter()
                .filter(|(key, owner)| &key.session_id == session_id && owner.adapter.is_some())
                .map(|(key, _)| key.clone())
                .collect();
            for key in keys {
                if !seen.insert(key.clone()) {
                    continue;
                }
                if let Some(teardown) = pump_one(
                    &mut self.live,
                    &mut self.next_snapshot_phase,
                    &self.wake_source,
                    &key,
                ) {
                    teardowns.push(teardown);
                }
            }
        }
        teardowns
    }

    /// Intake only routes named by a wake batch. Never `try_read`s an unnamed adapter.
    pub fn intake_woken(&mut self, batch: &TerminalWakeBatch) -> Vec<ClientWorkerTeardown> {
        let keys = self.adapter_route_keys(batch);
        self.intake_terminal_input_keys(keys)
    }

    /// Deduplicated exact routes named by adapter wakes.
    pub(crate) fn adapter_route_keys(&self, batch: &TerminalWakeBatch) -> Vec<OwnerKey> {
        let mut keys = Vec::new();
        let mut seen = HashSet::new();
        for route in &batch.adapter_routes {
            let key = OwnerKey {
                session_id: route.session_id.clone(),
                subscription_id: route.subscription_id.clone(),
            };
            if seen.insert(key.clone()) {
                keys.push(key);
            }
        }
        keys
    }

    /// Exact parked owners selected by a named session wake and live generation.
    pub(crate) fn parked_route_keys(&mut self, batch: &TerminalWakeBatch) -> Vec<OwnerKey> {
        let named_sessions: HashSet<_> = batch.ingress_sessions.iter().cloned().collect();
        self.capacity_parked.retain(|key, generation| {
            self.live
                .get(key)
                .is_some_and(|owner| owner.generation == *generation)
        });
        let mut keys: Vec<_> = self
            .capacity_parked
            .keys()
            .filter(|key| named_sessions.contains(&key.session_id))
            .cloned()
            .collect();
        keys.sort_by(|left, right| {
            left.session_id
                .0
                .cmp(&right.session_id.0)
                .then(left.subscription_id.0.cmp(&right.subscription_id.0))
        });
        keys
    }

    /// Park an exact live owner until its session reports capacity.
    pub(crate) fn park_for_capacity(&mut self, key: &OwnerKey) {
        if let Some(owner) = self.live.get(key) {
            self.capacity_parked.insert(key.clone(), owner.generation);
        }
    }

    /// Clear a capacity obligation after progress or hard-stop.
    pub(crate) fn clear_capacity_parked(&mut self, key: &OwnerKey) {
        self.capacity_parked.remove(key);
    }

    /// Whether one exact owner has accepted input awaiting Stage B.
    pub(crate) fn has_terminal_input(&self, key: &OwnerKey) -> bool {
        self.live
            .get(key)
            .is_some_and(|owner| !owner.input_queue.is_empty())
    }

    /// Hard-stop one exact owner selected by the targeted apply path.
    pub(crate) fn hard_stop_owner(&mut self, key: &OwnerKey) -> Option<ClientWorkerTeardown> {
        self.hard_stop_key(key)
    }

    fn intake_terminal_input_keys(&mut self, keys: Vec<OwnerKey>) -> Vec<ClientWorkerTeardown> {
        let mut teardowns = self.expire_pastes_keys(&keys, Instant::now());
        for key in keys {
            let reads = match self
                .live
                .get_mut(&key)
                .and_then(|owner| owner.adapter.as_mut())
            {
                Some(adapter) => {
                    let mut frames = Vec::new();
                    let mut hard_stop = false;
                    for _ in 0..INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK {
                        match adapter.try_read() {
                            TerminalIngress::Empty | TerminalIngress::Closed => break,
                            TerminalIngress::Lost => {
                                hard_stop = true;
                                break;
                            }
                            TerminalIngress::Frame(bytes) => frames.push(bytes),
                        }
                    }
                    Some((frames, hard_stop))
                }
                None => None,
            };
            let Some((frames, lost)) = reads else {
                continue;
            };
            let mut fail = lost;
            for bytes in frames {
                if fail {
                    break;
                }
                let decoded = botster_terminal_protocol::TerminalInputFrame::from_bytes(&bytes)
                    .ok()
                    .and_then(|frame| decode_terminal_input(&frame).ok());
                let Some(command) = decoded else {
                    fail = true;
                    break;
                };
                if self.intake_terminal_command(&key, command).is_err() {
                    fail = true;
                    break;
                }
            }
            if fail {
                if let Some(teardown) = self.hard_stop_key(&key) {
                    teardowns.push(teardown);
                }
            }
        }
        teardowns
    }

    fn intake_terminal_command(
        &mut self,
        key: &OwnerKey,
        command: TerminalInputCommand,
    ) -> Result<(), ()> {
        match command {
            TerminalInputCommand::PasteBegin {
                operation_id,
                mode_generation,
                mode_revision,
                total_len,
            } => {
                let rejection = {
                    let owner = self.live.get_mut(key).ok_or(())?;
                    if owner
                        .last_paste_operation_id
                        .is_some_and(|last| operation_id <= last)
                    {
                        return Ok(());
                    }
                    owner.last_paste_operation_id = Some(operation_id);
                    if owner.paste_in_flight.is_some() {
                        Some(TerminalInputRejection::OperationInFlight)
                    } else if total_len == 0 || total_len as usize > MAX_PASTE_BYTES {
                        Some(TerminalInputRejection::OperationOutOfBounds)
                    } else {
                        None
                    }
                };
                if let Some(rejection) = rejection {
                    return self.enqueue_paste_rejection(key, operation_id, rejection);
                }
                let owner = self.live.get_mut(key).ok_or(())?;
                let total_len = total_len as usize;
                owner.paste_in_flight = Some(operation_id);
                owner.paste = Some(PasteAssembly {
                    operation_id,
                    mode_generation,
                    mode_revision,
                    total_len,
                    expected_chunks: total_len.div_ceil(MAX_PASTE_CHUNK_DATA_BYTES),
                    next_index: 0,
                    data: Vec::with_capacity(total_len),
                    deadline: Instant::now() + PASTE_ASSEMBLY_TIMEOUT,
                });
                Ok(())
            }
            TerminalInputCommand::PasteChunk {
                operation_id,
                index,
                data,
            } => {
                let Some(owner) = self.live.get_mut(key) else {
                    return Err(());
                };
                let Some(assembly) = owner.paste.as_mut() else {
                    return Ok(());
                };
                if assembly.operation_id != operation_id {
                    return Ok(());
                }
                if index as usize != assembly.next_index
                    || assembly.next_index >= assembly.expected_chunks
                    || data.len() > assembly.total_len.saturating_sub(assembly.data.len())
                {
                    owner.paste = None;
                    owner.paste_in_flight = None;
                    return self.enqueue_paste_rejection(
                        key,
                        operation_id,
                        TerminalInputRejection::OperationIncomplete,
                    );
                }
                let expected_len = if assembly.next_index + 1 < assembly.expected_chunks {
                    MAX_PASTE_CHUNK_DATA_BYTES
                } else {
                    assembly.total_len - MAX_PASTE_CHUNK_DATA_BYTES * (assembly.expected_chunks - 1)
                };
                if data.len() != expected_len {
                    owner.paste = None;
                    owner.paste_in_flight = None;
                    return self.enqueue_paste_rejection(
                        key,
                        operation_id,
                        TerminalInputRejection::OperationIncomplete,
                    );
                }
                assembly.data.extend_from_slice(&data);
                assembly.next_index += 1;
                Ok(())
            }
            TerminalInputCommand::PasteCommit { operation_id } => {
                let Some(owner) = self.live.get_mut(key) else {
                    return Err(());
                };
                let Some(assembly) = owner.paste.take() else {
                    return Ok(());
                };
                if assembly.operation_id != operation_id {
                    owner.paste = Some(assembly);
                    return Ok(());
                }
                if assembly.next_index != assembly.expected_chunks
                    || assembly.data.len() != assembly.total_len
                {
                    owner.paste_in_flight = None;
                    return self.enqueue_paste_rejection(
                        key,
                        operation_id,
                        TerminalInputRejection::OperationIncomplete,
                    );
                }
                if owner.input_queue.len() >= INPUT_QUEUE_CAPACITY {
                    owner.paste_in_flight = None;
                    return self.enqueue_paste_rejection(
                        key,
                        operation_id,
                        TerminalInputRejection::OperationOutOfBounds,
                    );
                }
                owner
                    .input_queue
                    .push_back(TerminalInputOperation::Paste(PasteOperation {
                        operation_id,
                        mode_generation: assembly.mode_generation,
                        mode_revision: assembly.mode_revision,
                        data: assembly.data,
                    }));
                Ok(())
            }
            TerminalInputCommand::PasteAbort { operation_id } => {
                let Some(owner) = self.live.get_mut(key) else {
                    return Err(());
                };
                if owner
                    .awaiting_gated
                    .as_ref()
                    .is_some_and(|wait| wait.operation_id == Some(operation_id))
                {
                    return Ok(());
                }
                let assembling = owner
                    .paste
                    .as_ref()
                    .is_some_and(|paste| paste.operation_id == operation_id);
                let queued = owner.input_queue.iter().position(|operation| {
                    matches!(
                        operation,
                        TerminalInputOperation::Paste(paste)
                            if paste.operation_id == operation_id
                    )
                });
                if assembling {
                    owner.paste = None;
                } else if let Some(position) = queued {
                    owner.input_queue.remove(position);
                } else {
                    return Ok(());
                }
                owner.paste_in_flight = None;
                self.enqueue_paste_rejection(key, operation_id, TerminalInputRejection::Aborted)
            }
            command => {
                let owner = self.live.get_mut(key).ok_or(())?;
                if owner.input_queue.len() >= INPUT_QUEUE_CAPACITY {
                    return Err(());
                }
                owner
                    .input_queue
                    .push_back(TerminalInputOperation::Command(command));
                Ok(())
            }
        }
    }

    fn enqueue_paste_rejection(
        &mut self,
        key: &OwnerKey,
        operation_id: u32,
        rejection: TerminalInputRejection,
    ) -> Result<(), ()> {
        let subscription_id = key.subscription_id.clone();
        let result = TerminalInputResult {
            subscription_id: subscription_id.0.clone(),
            kind: TerminalInputKind::Paste,
            operation_id: Some(operation_id),
            admitted: false,
            bytes_written: 0,
            mode_generation: 0,
            mode_revision: 0,
            mode_flags: empty_mode_flags(),
            rejection: Some(rejection),
        };
        self.enqueue_input_result(&key.session_id, &subscription_id, &result)
            .map_err(|_| ())
    }

    fn expire_pastes_keys(&mut self, keys: &[OwnerKey], now: Instant) -> Vec<ClientWorkerTeardown> {
        let expired: Vec<_> = keys
            .iter()
            .filter_map(|key| {
                self.live.get(key).and_then(|owner| {
                    owner
                        .paste
                        .as_ref()
                        .filter(|paste| paste.deadline <= now)
                        .map(|paste| (key.clone(), paste.operation_id))
                })
            })
            .collect();
        let mut teardowns = Vec::new();
        for (key, operation_id) in expired {
            if let Some(owner) = self.live.get_mut(&key) {
                owner.paste = None;
                owner.paste_in_flight = None;
            }
            if self
                .enqueue_paste_rejection(&key, operation_id, TerminalInputRejection::Timeout)
                .is_err()
            {
                if let Some(teardown) = self.hard_stop_key(&key) {
                    teardowns.push(teardown);
                }
            }
        }
        teardowns
    }

    /// Earliest assembly deadline across live owners.
    #[must_use]
    pub fn next_paste_deadline(&self) -> Option<Instant> {
        self.live
            .values()
            .filter_map(|owner| owner.paste.as_ref().map(|paste| paste.deadline))
            .min()
    }

    /// Exact live routes whose assembly deadline has passed.
    #[must_use]
    pub fn expired_paste_routes(&self, now: Instant) -> Vec<crate::TerminalWakeRoute> {
        let mut routes: Vec<_> = self
            .live
            .iter()
            .filter(|(_, owner)| {
                owner
                    .paste
                    .as_ref()
                    .is_some_and(|paste| paste.deadline <= now)
            })
            .map(|(key, _)| crate::TerminalWakeRoute {
                session_id: key.session_id.clone(),
                subscription_id: key.subscription_id.clone(),
            })
            .collect();
        routes.sort_by(|left, right| {
            left.session_id
                .0
                .cmp(&right.session_id.0)
                .then(left.subscription_id.0.cmp(&right.subscription_id.0))
        });
        routes
    }

    /// Sessions that already have a parked gated owner.
    #[must_use]
    pub fn sessions_awaiting_gated(&self) -> HashSet<SessionId> {
        self.live
            .iter()
            .filter(|(_, owner)| owner.awaiting_gated.is_some())
            .map(|(key, _)| key.session_id.clone())
            .collect()
    }

    /// Stage B: dequeue apply-budget commands from unparked owners.
    pub fn take_terminal_input(
        &mut self,
        sessions_holding_gated: &HashSet<SessionId>,
    ) -> Vec<TerminalInputDelivery> {
        let mut held = sessions_holding_gated.clone();
        held.extend(self.sessions_awaiting_gated());
        let keys = self.rotated_live_keys();
        let mut deliveries = Vec::new();
        for key in keys {
            for _ in 0..APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK {
                let Some(delivery) = self.take_one_terminal_input(&key, &mut held) else {
                    break;
                };
                let gated = matches!(
                    delivery.command,
                    TerminalInputOperation::Command(TerminalInputCommand::ModeGatedInput { .. })
                        | TerminalInputOperation::Paste(_)
                );
                deliveries.push(delivery);
                if gated {
                    break;
                }
            }
        }
        deliveries
    }

    /// Dequeue at most one command from one exact live owner.
    pub(crate) fn take_one_terminal_input(
        &mut self,
        key: &OwnerKey,
        held: &mut HashSet<SessionId>,
    ) -> Option<TerminalInputDelivery> {
        let owner = self.live.get_mut(key)?;
        if owner.awaiting_gated.is_some() {
            return None;
        }
        let head = owner.input_queue.front()?;
        if matches!(
            head,
            TerminalInputOperation::Command(TerminalInputCommand::ModeGatedInput { .. })
                | TerminalInputOperation::Paste(_)
        ) && held.contains(&key.session_id)
        {
            return None;
        }
        let command = owner.input_queue.pop_front()?;
        if matches!(
            command,
            TerminalInputOperation::Command(TerminalInputCommand::ModeGatedInput { .. })
                | TerminalInputOperation::Paste(_)
        ) {
            held.insert(key.session_id.clone());
        }
        self.capacity_parked.remove(key);
        Some(TerminalInputDelivery {
            client_id: owner.client_id.clone(),
            session_id: key.session_id.clone(),
            subscription_id: key.subscription_id.clone(),
            generation: owner.generation,
            command,
        })
    }

    /// Record that this owner is parked on a submitted gated request.
    pub fn set_awaiting_gated(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        request_id: String,
        deadline: Instant,
        kind: TerminalInputKind,
        operation_id: Option<u32>,
    ) {
        if let Some(owner) = self.live.get_mut(&OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        }) {
            owner.awaiting_gated = Some(GatedWait {
                request_id,
                deadline,
                kind,
                operation_id,
            });
        }
    }

    /// Clear a parked gated wait after Ready, TimedOut, or teardown handling.
    pub fn clear_awaiting_gated(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<GatedWait> {
        self.live
            .get_mut(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .and_then(|owner| owner.awaiting_gated.take())
    }

    /// Outstanding gated wait for one owner, if any.
    #[must_use]
    pub fn awaiting_gated(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<&GatedWait> {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .and_then(|owner| owner.awaiting_gated.as_ref())
    }

    /// Enqueue an `input_result` onto the owner's egress queue.
    pub fn enqueue_input_result(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        result: &TerminalInputResult,
    ) -> Result<(), EnqueueInputResultError> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        let Some(owner) = self.live.get_mut(&key) else {
            return Err(EnqueueInputResultError::OwnerGone);
        };
        let frame = TerminalEvent::InputResult(result.clone())
            .to_frame()
            .map_err(|_| EnqueueInputResultError::EncodeFailed)?;
        if owner.queue.len() >= QueueSource::ClientWorker.default_capacity() {
            return Err(EnqueueInputResultError::EgressFull);
        }
        owner.queue.push_back(QueuedFrame {
            frame,
            kind: QueuedKind::Other,
        });
        Ok(())
    }

    /// Clear one paste's owner state when its authoritative result is ready.
    pub fn finish_paste_operation(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        operation_id: u32,
    ) {
        if let Some(owner) = self.live.get_mut(&OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        }) {
            if owner.paste_in_flight == Some(operation_id) {
                owner.paste_in_flight = None;
                owner.paste = None;
            }
        }
    }

    /// Current ingress queue length for one live owner.
    #[must_use]
    pub fn input_queue_len(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<usize> {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .map(|owner| owner.input_queue.len())
    }

    fn rotated_live_keys(&mut self) -> Vec<OwnerKey> {
        let mut keys: Vec<_> = self.live.keys().cloned().collect();
        keys.sort_by(|left, right| {
            left.session_id
                .0
                .cmp(&right.session_id.0)
                .then(left.subscription_id.0.cmp(&right.subscription_id.0))
        });
        if keys.is_empty() {
            return keys;
        }
        let start = self.input_cursor % keys.len();
        self.input_cursor = start.wrapping_add(1);
        keys.rotate_left(start);
        keys
    }

    /// Detach the live generation if present.
    pub fn detach_live(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<ClientWorkerTeardown> {
        self.hard_stop_key(&OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        })
    }

    /// Generation-aware detach. Mismatch does not delete a newer owner.
    pub fn detach_generation(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        generation: TerminalSubscriptionGeneration,
    ) -> DetachTerminalSubscriptionResult {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        match self.live.get(&key) {
            None => DetachTerminalSubscriptionResult::AlreadyGone,
            Some(owner) if owner.generation != generation => {
                DetachTerminalSubscriptionResult::GenerationMismatch {
                    live: owner.generation,
                    requested: generation,
                }
            }
            Some(_) => {
                let _ = self.hard_stop_key(&key);
                DetachTerminalSubscriptionResult::Detached { generation }
            }
        }
    }

    /// Ownership hard-stop for every live subscription on `session_id`.
    pub fn teardown_session(&mut self, session_id: &SessionId) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self
            .live
            .keys()
            .filter(|key| &key.session_id == session_id)
            .cloned()
            .collect();
        keys.into_iter()
            .filter_map(|key| self.hard_stop_key(&key))
            .collect()
    }

    /// Ownership hard-stop for every remaining bound subscription.
    pub fn teardown_all(&mut self) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self.live.keys().cloned().collect();
        keys.into_iter()
            .filter_map(|key| self.hard_stop_key(&key))
            .collect()
    }

    /// Whether any adapter is still held for tests and idle oracles.
    #[must_use]
    pub fn adapter_is_bound(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .is_some_and(|owner| owner.adapter.is_some())
    }
}

struct WakingAdapterHolder {
    inner: Box<dyn WakingTerminalAdapter + Send>,
}

impl TerminalAdapter for WakingAdapterHolder {
    fn try_write(&mut self, frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
        self.inner.try_write(frame)
    }

    fn close(&mut self) {
        self.inner.close();
    }

    fn pressure(&self) -> TerminalAdapterPressure {
        self.inner.pressure()
    }

    fn try_read(&mut self) -> TerminalIngress {
        self.inner.try_read()
    }
}

fn retire_and_hard_stop(
    live: &mut HashMap<OwnerKey, SubscriptionOwner>,
    phases: &mut HashMap<OwnerKey, SnapshotPhase>,
    wake_source: &TerminalWakeSource,
    key: &OwnerKey,
) -> Option<ClientWorkerTeardown> {
    wake_source.retire_route(&key.session_id, &key.subscription_id);
    phases.remove(key);
    hard_stop(live, key)
}

fn pump_one(
    live: &mut HashMap<OwnerKey, SubscriptionOwner>,
    phases: &mut HashMap<OwnerKey, SnapshotPhase>,
    wake_source: &TerminalWakeSource,
    key: &OwnerKey,
) -> Option<ClientWorkerTeardown> {
    let owner = live.get_mut(key)?;
    let adapter = owner.adapter.as_mut()?;

    if adapter.pressure() == TerminalAdapterPressure::Closed {
        return retire_and_hard_stop(live, phases, wake_source, key);
    }

    if owner.in_flight {
        match adapter.pressure() {
            TerminalAdapterPressure::Ready => {
                if let Some(completed) = owner.queue.pop_front() {
                    if completed.kind == QueuedKind::ProcessExit {
                        owner.process_exit_delivered = true;
                    }
                }
                owner.in_flight = false;
                owner.unsuccessful_writes = 0;
            }
            TerminalAdapterPressure::Closed => {
                return retire_and_hard_stop(live, phases, wake_source, key);
            }
            TerminalAdapterPressure::Full | TerminalAdapterPressure::WouldBlock => {
                owner.unsuccessful_writes = owner.unsuccessful_writes.saturating_add(1);
                if owner.unsuccessful_writes >= WRITE_ATTEMPT_BUDGET {
                    return retire_and_hard_stop(live, phases, wake_source, key);
                }
                return None;
            }
        }
    }

    loop {
        let owner = live.get_mut(key)?;
        if owner.process_exit_delivered {
            return retire_and_hard_stop(live, phases, wake_source, key);
        }
        let adapter = owner.adapter.as_mut()?;
        if adapter.pressure() == TerminalAdapterPressure::Closed {
            return retire_and_hard_stop(live, phases, wake_source, key);
        }
        let head = owner.queue.front()?;
        match adapter.try_write(&head.frame) {
            Ok(()) => {
                owner.in_flight = true;
                owner.unsuccessful_writes = 0;
                if adapter.pressure() == TerminalAdapterPressure::Ready {
                    if let Some(completed) = owner.queue.pop_front() {
                        if completed.kind == QueuedKind::ProcessExit {
                            owner.process_exit_delivered = true;
                        }
                    }
                    owner.in_flight = false;
                    continue;
                }
                return None;
            }
            Err(TerminalAdapterWriteError::WouldBlock | TerminalAdapterWriteError::Full) => {
                owner.unsuccessful_writes = owner.unsuccessful_writes.saturating_add(1);
                if owner.unsuccessful_writes >= WRITE_ATTEMPT_BUDGET {
                    return retire_and_hard_stop(live, phases, wake_source, key);
                }
                return None;
            }
            Err(TerminalAdapterWriteError::Closed) => {
                return retire_and_hard_stop(live, phases, wake_source, key);
            }
        }
    }
}

fn hard_stop(
    live: &mut HashMap<OwnerKey, SubscriptionOwner>,
    key: &OwnerKey,
) -> Option<ClientWorkerTeardown> {
    let mut owner = live.remove(key)?;
    owner.queue.clear();
    owner.held.clear();
    if let Some(mut adapter) = owner.adapter.take() {
        adapter.close();
        drop(adapter);
    }
    Some(ClientWorkerTeardown {
        client_id: owner.client_id,
        session_id: key.session_id.clone(),
        subscription_id: key.subscription_id.clone(),
        generation: owner.generation,
        awaiting_gated: owner.awaiting_gated.map(|wait| wait.request_id),
    })
}

fn empty_mode_flags() -> TerminalModeFlags {
    TerminalModeFlags {
        kitty_enabled: false,
        cursor_visible: false,
        bracketed_paste: false,
        mouse_mode: 0,
        alt_screen: false,
        focus_reporting: false,
        application_cursor: false,
    }
}

fn terminal_route(frame: &TransportEgress) -> Option<(&SessionId, &SubscriptionId)> {
    match frame {
        TransportEgress::TerminalOutput {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::Snapshot {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::Scrollback {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::ProcessExit {
            session_id,
            subscription_id,
            ..
        }
        | TransportEgress::AttachState {
            session_id,
            subscription_id,
            ..
        } => Some((session_id, subscription_id)),
        _ => None,
    }
}

fn encode_terminal_frame(
    key: &OwnerKey,
    frame: &TransportEgress,
    snapshot_phase: Option<SnapshotPhase>,
    capabilities: &TerminalCapabilitySet,
) -> Result<Option<QueuedFrame>, ()> {
    let event = match frame {
        TransportEgress::TerminalOutput { data, .. } | TransportEgress::Scrollback { data, .. } => {
            TerminalEvent::TerminalOutput(TerminalOutput::from_bytes(
                key.session_id.0.clone(),
                key.subscription_id.0.clone(),
                data,
            ))
        }
        TransportEgress::Snapshot { data, .. } => {
            if !capabilities.contains(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY) {
                return Ok(None);
            }
            TerminalEvent::Snapshot(Snapshot {
                session_id: key.session_id.0.clone(),
                subscription_id: key.subscription_id.0.clone(),
                payload_base64: base64::Engine::encode(
                    &base64::engine::general_purpose::STANDARD,
                    data,
                ),
                payload_encoding: botster_terminal_protocol_client::PayloadEncoding::Base64,
                bytes: data.len(),
                phase: snapshot_phase.unwrap_or(SnapshotPhase::Ready),
            })
        }
        TransportEgress::ProcessExit { code, .. } => TerminalEvent::ProcessExit(ProcessExit {
            session_id: key.session_id.0.clone(),
            subscription_id: key.subscription_id.0.clone(),
            code: *code,
        }),
        TransportEgress::AttachState { state, .. } => {
            let kind = match state {
                TerminalAttachState::Attaching => AttachStateKind::Attaching,
                TerminalAttachState::Attached => AttachStateKind::Attached,
                TerminalAttachState::SnapshotHistoryIncomplete => {
                    AttachStateKind::SnapshotHistoryIncomplete
                }
                TerminalAttachState::Detached => return Ok(None),
            };
            TerminalEvent::AttachState(AttachState {
                session_id: key.session_id.0.clone(),
                subscription_id: key.subscription_id.0.clone(),
                state: kind,
            })
        }
        _ => return Ok(None),
    };
    let encoded = event.to_frame().map_err(|_| ())?;
    let kind = match frame {
        TransportEgress::Snapshot { .. } => QueuedKind::Snapshot,
        TransportEgress::ProcessExit { .. } => QueuedKind::ProcessExit,
        _ => QueuedKind::Other,
    };
    Ok(Some(QueuedFrame {
        frame: encoded,
        kind,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::TerminalAttachState;
    use crate::contract::terminal_adapter::TerminalIngress;

    struct ReadyWakingAdapter;

    impl TerminalAdapter for ReadyWakingAdapter {
        fn try_write(&mut self, _frame: &TerminalFrame) -> Result<(), TerminalAdapterWriteError> {
            Ok(())
        }

        fn close(&mut self) {}

        fn pressure(&self) -> TerminalAdapterPressure {
            TerminalAdapterPressure::Ready
        }

        fn try_read(&mut self) -> TerminalIngress {
            TerminalIngress::Empty
        }
    }

    impl WakingTerminalAdapter for ReadyWakingAdapter {
        fn set_wake_sink(&mut self, _sink: crate::contract::terminal_wake::TerminalWakeSink) {}
    }

    fn ids() -> (ClientId, SessionId, SubscriptionId) {
        (
            ClientId("c".into()),
            SessionId("s".into()),
            SubscriptionId("sub".into()),
        )
    }

    fn paste_result(owner: &SubscriptionOwner, index: usize) -> TerminalInputResult {
        let event = TerminalEvent::from_frame(&owner.queue[index].frame).expect("input result");
        let TerminalEvent::InputResult(result) = event else {
            panic!("expected input result");
        };
        result
    }

    #[test]
    fn paste_assembly_is_ordered_bounded_and_single_result() {
        let mut worker = ClientWorker::new();
        let (client, session, subscription) = ids();
        worker.record_attach(client, session.clone(), subscription.clone());
        let key = OwnerKey {
            session_id: session.clone(),
            subscription_id: subscription.clone(),
        };
        let data = vec![0x5a; MAX_PASTE_BYTES];
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 3,
                    mode_generation: 4,
                    mode_revision: 5,
                    total_len: data.len() as u32,
                },
            )
            .expect("begin");
        for (index, chunk) in data.chunks(MAX_PASTE_CHUNK_DATA_BYTES).enumerate() {
            worker
                .intake_terminal_command(
                    &key,
                    TerminalInputCommand::PasteChunk {
                        operation_id: 3,
                        index: index as u32,
                        data: chunk.to_vec(),
                    },
                )
                .expect("chunk");
        }
        worker
            .intake_terminal_command(&key, TerminalInputCommand::PasteCommit { operation_id: 3 })
            .expect("commit");
        let owner = worker.live.get(&key).expect("owner");
        assert!(owner.paste.is_none());
        assert_eq!(owner.paste_in_flight, Some(3));
        assert_eq!(owner.input_queue.len(), 1);
        let TerminalInputOperation::Paste(paste) = owner.input_queue.front().expect("paste") else {
            panic!("expected queued paste");
        };
        assert_eq!(paste.operation_id, 3);
        assert_eq!(paste.mode_generation, 4);
        assert_eq!(paste.mode_revision, 5);
        assert_eq!(paste.data, data);

        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 4,
                    mode_generation: 4,
                    mode_revision: 5,
                    total_len: 1,
                },
            )
            .expect("in-flight rejection");
        let owner = worker.live.get(&key).expect("owner");
        let result = paste_result(owner, 0);
        assert_eq!(result.subscription_id, subscription.0);
        assert_eq!(result.operation_id, Some(4));
        assert_eq!(
            result.rejection,
            Some(TerminalInputRejection::OperationInFlight)
        );
        assert_eq!(result.bytes_written, 0);
        assert_eq!(owner.paste_in_flight, Some(3));

        worker
            .intake_terminal_command(&key, TerminalInputCommand::PasteAbort { operation_id: 3 })
            .expect("abort queued paste");
        let owner = worker.live.get(&key).expect("owner");
        assert!(owner.input_queue.is_empty());
        assert!(owner.paste_in_flight.is_none());
        let result = paste_result(owner, 1);
        assert_eq!(result.operation_id, Some(3));
        assert_eq!(result.rejection, Some(TerminalInputRejection::Aborted));

        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 4,
                    mode_generation: 4,
                    mode_revision: 5,
                    total_len: 1,
                },
            )
            .expect("rejected operation replay is ignored");
        let owner = worker.live.get(&key).expect("owner");
        assert_eq!(owner.queue.len(), 2);
        assert!(owner.paste.is_none());
        assert!(owner.paste_in_flight.is_none());
    }

    #[test]
    fn paste_validation_timeout_and_commit_capacity_preserve_owner() {
        let mut worker = ClientWorker::new();
        let (client, session, subscription) = ids();
        worker.record_attach(client, session.clone(), subscription.clone());
        let key = OwnerKey {
            session_id: session.clone(),
            subscription_id: subscription.clone(),
        };
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 7,
                    mode_generation: 1,
                    mode_revision: 1,
                    total_len: 2,
                },
            )
            .expect("begin");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 7,
                    mode_generation: 1,
                    mode_revision: 1,
                    total_len: 2,
                },
            )
            .expect("duplicate begin is ignored");
        let owner = worker.live.get(&key).expect("owner");
        assert!(owner.paste.is_some());
        assert_eq!(owner.paste_in_flight, Some(7));
        assert!(owner.queue.is_empty());

        worker
            .live
            .get_mut(&key)
            .expect("owner")
            .paste
            .as_mut()
            .expect("paste")
            .deadline = Instant::now() - Duration::from_millis(1);
        assert_eq!(worker.expired_paste_routes(Instant::now()).len(), 1);
        assert_eq!(
            worker
                .expire_pastes_keys(std::slice::from_ref(&key), Instant::now())
                .len(),
            0
        );
        let owner = worker.live.get(&key).expect("owner remains live");
        assert!(owner.paste.is_none());
        assert!(owner.paste_in_flight.is_none());
        assert_eq!(
            paste_result(owner, 0).rejection,
            Some(TerminalInputRejection::Timeout)
        );
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 7,
                    mode_generation: 1,
                    mode_revision: 1,
                    total_len: 2,
                },
            )
            .expect("completed operation replay is ignored");
        assert_eq!(
            worker
                .live
                .get(&key)
                .expect("owner remains live")
                .queue
                .len(),
            1,
            "one operation id must produce one result"
        );

        let owner = worker.live.get_mut(&key).expect("owner");
        for _ in 0..INPUT_QUEUE_CAPACITY {
            owner.input_queue.push_back(TerminalInputOperation::Command(
                TerminalInputCommand::Input { data: vec![1] },
            ));
        }
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 8,
                    mode_generation: 1,
                    mode_revision: 2,
                    total_len: 1,
                },
            )
            .expect("begin while queue full");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 8,
                    index: 0,
                    data: vec![9],
                },
            )
            .expect("chunk while queue full");
        worker
            .intake_terminal_command(&key, TerminalInputCommand::PasteCommit { operation_id: 8 })
            .expect("commit rejection");
        let owner = worker.live.get(&key).expect("owner remains live");
        assert_eq!(owner.input_queue.len(), INPUT_QUEUE_CAPACITY);
        assert_eq!(
            paste_result(owner, 1).rejection,
            Some(TerminalInputRejection::OperationOutOfBounds)
        );
    }

    #[test]
    fn paste_incomplete_sequence_fails_cleanly_and_replacement_resets_identity() {
        let mut worker = ClientWorker::new();
        let (_, session, subscription) = ids();
        let (generation, _) = worker.record_attach(
            ClientId("first".into()),
            session.clone(),
            subscription.clone(),
        );
        let key = OwnerKey {
            session_id: session.clone(),
            subscription_id: subscription.clone(),
        };
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 9,
                    mode_generation: 1,
                    mode_revision: 1,
                    total_len: (MAX_PASTE_CHUNK_DATA_BYTES + 1) as u32,
                },
            )
            .expect("begin");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 9,
                    index: 1,
                    data: vec![0; MAX_PASTE_CHUNK_DATA_BYTES],
                },
            )
            .expect("incomplete result");
        let owner = worker.live.get(&key).expect("owner remains");
        assert!(owner.paste.is_none());
        assert!(owner.paste_in_flight.is_none());
        let result = paste_result(owner, 0);
        assert_eq!(result.subscription_id, subscription.0);
        assert_eq!(result.operation_id, Some(9));
        assert_eq!(result.bytes_written, 0);
        assert_eq!(
            result.rejection,
            Some(TerminalInputRejection::OperationIncomplete)
        );

        let (replacement_generation, teardowns) = worker.record_attach(
            ClientId("replacement".into()),
            session.clone(),
            subscription.clone(),
        );
        assert_eq!(teardowns.len(), 1);
        assert!(replacement_generation > generation);
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 9,
                    mode_generation: 2,
                    mode_revision: 1,
                    total_len: 1,
                },
            )
            .expect("replacement can reuse operation id");
        let owner = worker.live.get(&key).expect("replacement owner");
        assert_eq!(owner.paste_in_flight, Some(9));
        assert!(owner.queue.is_empty());
    }

    #[test]
    fn paste_rejects_a_chunk_past_the_declared_chunk_count() {
        let mut worker = ClientWorker::new();
        let (client, session, subscription) = ids();
        worker.record_attach(client, session.clone(), subscription.clone());
        let key = OwnerKey {
            session_id: session,
            subscription_id: subscription,
        };
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 10,
                    mode_generation: 1,
                    mode_revision: 1,
                    total_len: 5,
                },
            )
            .expect("begin");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 10,
                    index: 0,
                    data: vec![1; 5],
                },
            )
            .expect("declared chunk");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 10,
                    index: 1,
                    data: vec![2; 5],
                },
            )
            .expect("extra chunk rejection");

        let owner = worker.live.get(&key).expect("owner remains");
        assert!(owner.paste.is_none());
        assert!(owner.paste_in_flight.is_none());
        assert!(owner.input_queue.is_empty());
        let result = paste_result(owner, 0);
        assert_eq!(result.operation_id, Some(10));
        assert_eq!(result.bytes_written, 0);
        assert_eq!(
            result.rejection,
            Some(TerminalInputRejection::OperationIncomplete)
        );
    }

    #[test]
    fn paste_rejects_a_repeated_final_chunk() {
        let mut worker = ClientWorker::new();
        let (client, session, subscription) = ids();
        worker.record_attach(client, session.clone(), subscription.clone());
        let key = OwnerKey {
            session_id: session,
            subscription_id: subscription,
        };
        let total_len = MAX_PASTE_CHUNK_DATA_BYTES + 1;
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 11,
                    mode_generation: 1,
                    mode_revision: 1,
                    total_len: total_len as u32,
                },
            )
            .expect("begin");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 11,
                    index: 0,
                    data: vec![1; MAX_PASTE_CHUNK_DATA_BYTES],
                },
            )
            .expect("first chunk");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 11,
                    index: 1,
                    data: vec![2],
                },
            )
            .expect("final chunk");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 11,
                    index: 1,
                    data: vec![2],
                },
            )
            .expect("repeated final chunk rejection");

        let owner = worker.live.get(&key).expect("owner remains");
        assert!(owner.paste.is_none());
        assert!(owner.paste_in_flight.is_none());
        assert!(owner.input_queue.is_empty());
        let result = paste_result(owner, 0);
        assert_eq!(result.operation_id, Some(11));
        assert_eq!(result.bytes_written, 0);
        assert_eq!(
            result.rejection,
            Some(TerminalInputRejection::OperationIncomplete)
        );
    }

    #[test]
    fn adapter_route_does_not_select_capacity_parked_sibling() {
        let mut worker = ClientWorker::new();
        let session = SessionId("shared-session".into());
        let route_a = OwnerKey {
            session_id: session.clone(),
            subscription_id: SubscriptionId("route-a".into()),
        };
        let route_b = OwnerKey {
            session_id: session.clone(),
            subscription_id: SubscriptionId("route-b".into()),
        };
        worker.record_attach(
            ClientId("client-a".into()),
            session.clone(),
            route_a.subscription_id.clone(),
        );
        worker.record_attach(
            ClientId("client-b".into()),
            session.clone(),
            route_b.subscription_id.clone(),
        );
        worker.park_for_capacity(&route_b);

        let route_only = TerminalWakeBatch {
            adapter_routes: vec![crate::contract::terminal_wake::TerminalWakeRoute {
                session_id: session.clone(),
                subscription_id: route_a.subscription_id.clone(),
            }],
            ingress_sessions: Vec::new(),
        };
        assert!(worker.parked_route_keys(&route_only).is_empty());

        let capacity_wake = TerminalWakeBatch {
            adapter_routes: Vec::new(),
            ingress_sessions: vec![session],
        };
        assert_eq!(worker.parked_route_keys(&capacity_wake), vec![route_b]);
    }

    #[test]
    fn stale_parked_generation_does_not_select_replacement_owner() {
        let mut worker = ClientWorker::new();
        let session = SessionId("parked-generation-session".into());
        let subscription = SubscriptionId("parked-generation-sub".into());
        let key = OwnerKey {
            session_id: session.clone(),
            subscription_id: subscription.clone(),
        };
        let (old_generation, _) = worker.record_attach(
            ClientId("parked-generation-old".into()),
            session.clone(),
            subscription.clone(),
        );
        worker.park_for_capacity(&key);
        let _ = worker.detach_generation(&session, &subscription, old_generation);
        let (new_generation, _) = worker.record_attach(
            ClientId("parked-generation-new".into()),
            session.clone(),
            subscription,
        );
        assert_ne!(old_generation, new_generation);
        worker.capacity_parked.insert(key, old_generation);

        let capacity_wake = TerminalWakeBatch {
            adapter_routes: Vec::new(),
            ingress_sessions: vec![session],
        };
        assert!(worker.parked_route_keys(&capacity_wake).is_empty());
    }

    #[test]
    fn waking_bind_after_attach_inserts_registry_and_rejection_does_not() {
        let mut worker = ClientWorker::new();
        let (client, session, subscription) = ids();
        let before = worker.wake_source().registry_len();
        let err = worker.bind_waking_terminal_adapter(
            &client,
            session.clone(),
            subscription.clone(),
            TerminalSubscriptionGeneration(1),
            TerminalCapabilitySet::empty(),
            Box::new(ReadyWakingAdapter),
        );
        assert!(err.is_err());
        assert_eq!(worker.wake_source().registry_len(), before);
        worker.record_attach(client.clone(), session.clone(), subscription.clone());
        let generation = worker
            .live_generation(&session, &subscription)
            .expect("generation");
        worker
            .bind_waking_terminal_adapter(
                &client,
                session.clone(),
                subscription.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(ReadyWakingAdapter),
            )
            .expect("bind");
        assert_eq!(worker.wake_source().registry_len(), 1);
        worker.teardown_session(&session);
        assert_eq!(worker.wake_source().registry_len(), 0);
    }

    #[test]
    fn flush_encode_failure_returns_teardown_from_ingest() {
        let mut worker = ClientWorker::new();
        let (client, session, subscription) = ids();
        worker.expect_terminal_adapter(client.clone(), session.clone(), subscription.clone());
        worker.record_attach(client.clone(), session.clone(), subscription.clone());
        let generation = worker
            .live_generation(&session, &subscription)
            .expect("generation");
        let mut frames = vec![(
            client.clone(),
            TransportEgress::AttachState {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
                state: TerminalAttachState::Attached,
            },
        )];
        assert!(worker.ingest_bound_terminal_frames(&mut frames).is_empty());
        assert!(
            frames.is_empty(),
            "held attach state must not leak: {frames:?}"
        );
        worker
            .bind_waking_terminal_adapter(
                &client,
                session.clone(),
                subscription.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(ReadyWakingAdapter),
            )
            .expect("bind");
        worker.fail_next_encode = true;
        let mut empty = Vec::new();
        let teardowns = worker.ingest_bound_terminal_frames(&mut empty);
        assert_eq!(
            teardowns.len(),
            1,
            "encode failure must teardown: {teardowns:?}"
        );
        assert!(!worker.has_subscription(&session, &subscription));
        assert_eq!(teardowns[0].client_id, client);
        assert_eq!(teardowns[0].session_id, session);
        assert_eq!(teardowns[0].subscription_id, subscription);
    }
}
