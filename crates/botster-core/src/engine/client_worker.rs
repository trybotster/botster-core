//! Synchronous ClientWorker: subscription queues, adapter pump, and teardown.
//!
//! This is the production bound-adapter egress owner. It is not
//! [`crate::contract::client_stream::ClientStreamHarness`]. Hosts pump it from
//! the existing drain tick. There is no ClientWorker OS thread.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::Instant;

use botster_terminal_protocol::{
    TerminalCapabilitySet, TerminalFrame, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
};
use botster_terminal_protocol_client::{
    decode_terminal_input, AttachState, AttachStateKind, ProcessExit, Snapshot, SnapshotPhase,
    TerminalEvent, TerminalInputCommand, TerminalInputResult, TerminalOutput,
};

use crate::actor::{QueueSource, TerminalAttachState};
use crate::client::ClientId;
use crate::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
};
use crate::contract::terminal_subscription::{
    BindTerminalAdapterError, DetachTerminalSubscriptionResult, TerminalInputDelivery,
    TerminalSubscriptionGeneration, TerminalSubscriptionRecord,
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
    input_cursor: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct OwnerKey {
    session_id: SessionId,
    subscription_id: SubscriptionId,
}

struct SubscriptionOwner {
    client_id: ClientId,
    generation: TerminalSubscriptionGeneration,
    adapter: Option<Box<dyn TerminalAdapter + Send>>,
    capabilities: Option<TerminalCapabilitySet>,
    queue: VecDeque<QueuedFrame>,
    unsuccessful_writes: usize,
    in_flight: bool,
    process_exit_enqueued: bool,
    process_exit_delivered: bool,
    input_queue: VecDeque<TerminalInputCommand>,
    awaiting_gated: Option<GatedWait>,
}

/// Outstanding mode-gated request for one owner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatedWait {
    /// Correlated worker request id.
    pub request_id: String,
    /// When the parent wait expires.
    pub deadline: Instant,
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
        if let Some(existing) = self.live.get(&key) {
            if existing.client_id == client_id {
                return (existing.generation, replacements);
            }
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
        self.live.insert(
            key,
            SubscriptionOwner {
                client_id,
                generation,
                adapter: None,
                capabilities: None,
                queue: VecDeque::new(),
                unsuccessful_writes: 0,
                in_flight: false,
                process_exit_enqueued: false,
                process_exit_delivered: false,
                input_queue: VecDeque::new(),
                awaiting_gated: None,
            },
        );
        (generation, replacements)
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
        self.next_snapshot_phase.remove(key);
        hard_stop(&mut self.live, key)
    }

    /// Bind a content-blind adapter to a live attach generation.
    ///
    /// `capabilities` is required. Omission does not compile. An empty set is
    /// valid. Core does not inspect token contents at bind. On rejection the
    /// presented adapter is closed and dropped on this stack.
    ///
    /// ```compile_fail
    /// use botster_core::ClientWorker;
    /// use botster_core::{ClientId, SessionId, SubscriptionId, TerminalSubscriptionGeneration};
    /// fn omit(worker: &mut ClientWorker) {
    ///     let adapter: Box<dyn botster_core::contract::terminal_adapter::TerminalAdapter + Send> =
    ///         unimplemented!();
    ///     let _ = worker.bind_terminal_adapter(
    ///         &ClientId("c".into()),
    ///         SessionId("s".into()),
    ///         SubscriptionId("sub".into()),
    ///         TerminalSubscriptionGeneration(1),
    ///         adapter,
    ///     );
    /// }
    /// ```
    pub fn bind_terminal_adapter(
        &mut self,
        client_id: &ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
        capabilities: TerminalCapabilitySet,
        mut adapter: Box<dyn TerminalAdapter + Send>,
    ) -> Result<(), BindTerminalAdapterError> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
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
        owner.adapter = Some(adapter);
        owner.capabilities = Some(capabilities);
        Ok(())
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
        let mut teardowns = Vec::new();
        let mut failed_routes = HashSet::new();
        let mut unbound_process_exits = Vec::new();
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
                        owner.queue.push_back(queued);
                        if is_process_exit {
                            owner.process_exit_enqueued = true;
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
        *egress = retained;
        teardowns
    }

    /// Pump bound adapters once per host tick.
    ///
    /// A WouldBlock/Full on the head frame counts once. Completing an in-flight
    /// write is observed as pressure returning to Ready.
    ///
    /// After `process_exit` is delivered (the write completed and pressure is
    /// Ready), Core hard-stops on that host tick. Close stays non-blocking.
    pub fn pump(&mut self) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self.live.keys().cloned().collect();
        let mut teardowns = Vec::new();
        for key in keys {
            if let Some(teardown) = pump_one(&mut self.live, &mut self.next_snapshot_phase, &key) {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    /// Stage A: intake complete frames from bound adapters.
    pub fn intake_terminal_input(&mut self) -> Vec<ClientWorkerTeardown> {
        let keys = self.rotated_live_keys();
        let mut teardowns = Vec::new();
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
                let Some(owner) = self.live.get_mut(&key) else {
                    fail = true;
                    break;
                };
                if owner.input_queue.len() >= INPUT_QUEUE_CAPACITY {
                    fail = true;
                    break;
                }
                owner.input_queue.push_back(command);
            }
            if fail {
                if let Some(teardown) = self.hard_stop_key(&key) {
                    teardowns.push(teardown);
                }
            }
        }
        teardowns
    }

    /// Stage B: dequeue apply-budget commands from unparked owners.
    pub fn take_terminal_input(
        &mut self,
        sessions_holding_gated: &HashSet<SessionId>,
    ) -> Vec<TerminalInputDelivery> {
        let keys = self.rotated_live_keys();
        let mut deliveries = Vec::new();
        for key in keys {
            let Some(owner) = self.live.get_mut(&key) else {
                continue;
            };
            if owner.awaiting_gated.is_some() {
                continue;
            }
            for _ in 0..APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK {
                let Some(head) = owner.input_queue.front() else {
                    break;
                };
                if matches!(head, TerminalInputCommand::ModeGatedInput { .. })
                    && sessions_holding_gated.contains(&key.session_id)
                {
                    break;
                }
                let Some(command) = owner.input_queue.pop_front() else {
                    break;
                };
                let gated = matches!(command, TerminalInputCommand::ModeGatedInput { .. });
                deliveries.push(TerminalInputDelivery {
                    client_id: owner.client_id.clone(),
                    session_id: key.session_id.clone(),
                    subscription_id: key.subscription_id.clone(),
                    generation: owner.generation,
                    command,
                });
                if gated {
                    break;
                }
            }
        }
        deliveries
    }

    /// Record that this owner is parked on a submitted gated request.
    pub fn set_awaiting_gated(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        request_id: String,
        deadline: Instant,
    ) {
        if let Some(owner) = self.live.get_mut(&OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        }) {
            owner.awaiting_gated = Some(GatedWait {
                request_id,
                deadline,
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

fn pump_one(
    live: &mut HashMap<OwnerKey, SubscriptionOwner>,
    phases: &mut HashMap<OwnerKey, SnapshotPhase>,
    key: &OwnerKey,
) -> Option<ClientWorkerTeardown> {
    let owner = live.get_mut(key)?;
    let adapter = owner.adapter.as_mut()?;

    if adapter.pressure() == TerminalAdapterPressure::Closed {
        phases.remove(key);
        return hard_stop(live, key);
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
                phases.remove(key);
                return hard_stop(live, key);
            }
            TerminalAdapterPressure::Full | TerminalAdapterPressure::WouldBlock => {
                owner.unsuccessful_writes = owner.unsuccessful_writes.saturating_add(1);
                if owner.unsuccessful_writes >= WRITE_ATTEMPT_BUDGET {
                    phases.remove(key);
                    return hard_stop(live, key);
                }
                return None;
            }
        }
    }

    loop {
        let owner = live.get_mut(key)?;
        if owner.process_exit_delivered {
            phases.remove(key);
            return hard_stop(live, key);
        }
        let adapter = owner.adapter.as_mut()?;
        if adapter.pressure() == TerminalAdapterPressure::Closed {
            phases.remove(key);
            return hard_stop(live, key);
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
                    phases.remove(key);
                    return hard_stop(live, key);
                }
                return None;
            }
            Err(TerminalAdapterWriteError::Closed) => {
                phases.remove(key);
                return hard_stop(live, key);
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
