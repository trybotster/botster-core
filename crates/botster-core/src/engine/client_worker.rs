//! Synchronous ClientWorker: per-route binary egress queues, scheme 2 input
//! admission, and teardown.
//!
//! This is the production bound-adapter egress owner. Hosts advance it through
//! `wait_wakes` and `pump_woken`. There is no ClientWorker OS thread.
//!
//! Every frame that leaves a bound route is a [`RoutedTerminalFrame`] whose
//! body is a shared scheme 2 `TerminalBody`. Session-wide frames are encoded
//! once and shared by `Arc` across every route on the session. Route-personal
//! frames (attach state, snapshot pages, input results, resync) are encoded per
//! route.

use std::collections::{HashMap, HashSet, VecDeque};
use std::time::{Duration, Instant};

use botster_terminal_protocol::{
    encode_attach_state, encode_input_result, encode_modes, encode_output, encode_process_exit,
    encode_route_resync, AttachStateCode, InputOutcome, InputResultBody, ModesBody, RouteId,
    RoutedTerminalFrame, TerminalCapabilitySet, TerminalFrame, TerminalInputFrame,
    TerminalInputKind, TerminalKind, INPUT_HEADER_BYTES, MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION,
    MAX_INPUT_OPERATIONS_PER_CLIENT, MAX_INPUT_OPERATIONS_PER_SESSION,
    MAX_INPUT_RESULT_DETAIL_BYTES, MAX_PASTE_BYTES, MAX_PASTE_CHUNK_DATA_BYTES,
    MAX_RETAINED_INPUT_BYTES_PER_CLIENT, MAX_RETAINED_INPUT_BYTES_PER_SESSION,
    MAX_ROUTE_EGRESS_BYTES, MAX_ROUTE_EGRESS_FRAMES,
};
use botster_terminal_protocol_client::{decode_terminal_input, TerminalInputCommand};

use crate::client::ClientId;
use crate::contract::terminal_adapter::{
    TerminalAdapter, TerminalAdapterPressure, TerminalAdapterWriteError, TerminalIngress,
};
use crate::contract::terminal_subscription::{
    AttachTerminalRouteError, BindTerminalAdapterError, DetachTerminalSubscriptionResult,
    StagedTerminalInput, TerminalSubscriptionGeneration, TerminalSubscriptionRecord,
};
use crate::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeSource, WakingTerminalAdapter,
};
use crate::session::{SessionId, SubscriptionId};
use crate::session_protocol::WorkerInputKind;
use crate::transport::TransportEgress;

const WRITE_ATTEMPT_BUDGET: usize = 512;
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
    /// Worker operation keys still in flight for this route. The host asks
    /// the worker to cancel them; their results are dropped.
    pub in_flight_keys: Vec<u64>,
}

/// One route that needs a fresh worker capture after egress overflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteResyncRequest {
    /// Client that owns the route.
    pub client_id: ClientId,
    /// Session of the route.
    pub session_id: SessionId,
    /// Subscription identity of the route.
    pub subscription_id: SubscriptionId,
    /// Fixed attachment generation of the route.
    pub generation: TerminalSubscriptionGeneration,
    /// Stream epoch the route entered with the `ROUTE_RESYNC` frame.
    pub stream_epoch: u32,
}

/// Failure while enqueueing a route-personal frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueRouteFrameError {
    /// The owner was already removed.
    OwnerGone,
    /// The frame could not be encoded.
    EncodeFailed,
}

/// Monotonic generation source shared by attach generations and resync epochs.
///
/// Values never wrap. The seed puts one daemon incarnation above earlier ones
/// on the same clock, but clients never compare generations numerically: they
/// adopt the value carried by `ATTACH_STATE` or `ROUTE_RESYNC` and drop frames
/// that do not match it.
#[derive(Debug)]
pub struct GenerationAllocator {
    next: u64,
}

impl Default for GenerationAllocator {
    fn default() -> Self {
        Self::new()
    }
}

impl GenerationAllocator {
    /// Seed from the wall clock so generations rise across restarts.
    #[must_use]
    pub fn new() -> Self {
        let seconds = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);
        // 2^20 attach or resync events per second before a restart overlaps.
        Self {
            next: (seconds << 20).max(1),
        }
    }

    /// Allocate the next generation, or `None` at exhaustion.
    pub fn allocate(&mut self) -> Option<TerminalSubscriptionGeneration> {
        if self.next == u64::MAX {
            return None;
        }
        let value = self.next;
        self.next += 1;
        Some(TerminalSubscriptionGeneration(value))
    }
}

/// Synchronous per-engine ClientWorker.
pub struct ClientWorker {
    live: HashMap<OwnerKey, SubscriptionOwner>,
    known: HashSet<OwnerKey>,
    expected_adapters: HashSet<(ClientId, OwnerKey)>,
    capacity_parked: HashMap<OwnerKey, TerminalSubscriptionGeneration>,
    input_cursor: usize,
    wake_source: TerminalWakeSource,
    bound_queue_wake_sessions: HashSet<SessionId>,
    generations: GenerationAllocator,
    resync_requests: Vec<RouteResyncRequest>,
    cancel_requests: Vec<(SessionId, u64)>,
    session_modes: HashMap<SessionId, ModesBody>,
    session_lanes: HashMap<SessionId, LaneUsage>,
    client_lanes: HashMap<ClientId, LaneUsage>,
    next_operation_key: u64,
    in_flight: HashMap<u64, InFlightOperation>,
}

impl Default for ClientWorker {
    fn default() -> Self {
        Self {
            live: HashMap::new(),
            known: HashSet::new(),
            expected_adapters: HashSet::new(),
            capacity_parked: HashMap::new(),
            input_cursor: 0,
            wake_source: TerminalWakeSource::new(),
            bound_queue_wake_sessions: HashSet::new(),
            generations: GenerationAllocator::new(),
            resync_requests: Vec::new(),
            cancel_requests: Vec::new(),
            session_modes: HashMap::new(),
            session_lanes: HashMap::new(),
            client_lanes: HashMap::new(),
            next_operation_key: 1,
            in_flight: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct OwnerKey {
    pub(crate) session_id: SessionId,
    pub(crate) subscription_id: SubscriptionId,
}

#[derive(Debug, Default, Clone, Copy)]
struct LaneUsage {
    operations: usize,
    bytes: usize,
}

struct SubscriptionOwner {
    client_id: ClientId,
    generation: TerminalSubscriptionGeneration,
    route: RouteId,
    adapter: Option<Box<dyn TerminalAdapter + Send>>,
    capabilities: Option<TerminalCapabilitySet>,
    queue: VecDeque<QueuedFrame>,
    queued_bytes: usize,
    hold_until_bound: bool,
    unsuccessful_writes: usize,
    in_flight: bool,
    /// A terminal frame (`PROCESS_EXIT` or `ATTACH_STATE failed`) is queued;
    /// nothing may follow it and the route hard-stops after delivery.
    terminal_enqueued: bool,
    terminal_delivered: bool,
    /// Stream epoch inside the fixed attachment generation. Starts at 0 and
    /// advances only through `ROUTE_RESYNC`.
    stream_epoch: u32,
    /// Live output is suppressed until the route's `SNAPSHOT_READY` lands.
    awaiting_capture: bool,
    input_queue: VecDeque<AdmittedInput>,
    last_operation_id: u64,
    paste: Option<PasteAssembly>,
    /// Lane usage this owner currently holds, released on hard-stop.
    lane: LaneUsage,
}

struct AdmittedInput {
    operation_id: u64,
    kind: WorkerInputKind,
    body: Vec<u8>,
    accepted_payload_bytes: u64,
}

struct InFlightOperation {
    key: OwnerKey,
    client_id: ClientId,
    operation_id: u64,
    retained_bytes: usize,
}

struct PasteAssembly {
    operation_id: u64,
    allow_unsafe: bool,
    total_len: usize,
    expected_chunks: usize,
    next_index: usize,
    data: Vec<u8>,
    deadline: Instant,
}

/// One queued frame with the small route descriptor captured at enqueue.
struct QueuedFrame {
    frame: TerminalFrame,
    kind: QueuedKind,
    /// Epoch current when this frame was queued. Never restamped.
    stream_epoch: u32,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum QueuedKind {
    /// `PROCESS_EXIT` or `ATTACH_STATE failed`: last frame on the route.
    Terminal,
    /// `INPUT_RESULT`: preserved across resync in order.
    InputResult,
    Other,
}

impl ClientWorker {
    /// Build an empty ClientWorker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Assign a fresh generation on attach and publish the inventory row.
    ///
    /// A client that attaches a new subscription for the same session hard-stops
    /// the previous owner for that client and session. A repeated attach for a
    /// live identity reuses its generation. The new owner awaits a capture:
    /// live output is suppressed until its `SNAPSHOT_READY` lands.
    ///
    /// # Errors
    ///
    /// Returns [`AttachTerminalRouteError::InvalidRoute`] when the subscription
    /// id is not a valid scheme 2 route id, and
    /// [`AttachTerminalRouteError::GenerationExhausted`] when the shared
    /// allocator has no value left. Neither creates an owner.
    pub fn record_attach(
        &mut self,
        client_id: ClientId,
        session_id: SessionId,
        subscription_id: SubscriptionId,
    ) -> Result<(TerminalSubscriptionGeneration, Vec<ClientWorkerTeardown>), AttachTerminalRouteError>
    {
        let route = RouteId::new(&subscription_id.0).map_err(|_| {
            AttachTerminalRouteError::InvalidRoute {
                subscription_id: subscription_id.clone(),
            }
        })?;
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
            return Ok((generation, replacements));
        }
        if self.live.contains_key(&key) {
            if let Some(stolen) = self.hard_stop_key(&key) {
                replacements.push(stolen);
            }
        }
        let Some(generation) = self.generations.allocate() else {
            return Err(AttachTerminalRouteError::GenerationExhausted);
        };
        self.known.insert(key.clone());
        let hold_until_bound = self
            .expected_adapters
            .remove(&(client_id.clone(), key.clone()));
        self.live.insert(
            key,
            SubscriptionOwner {
                client_id,
                generation,
                route,
                adapter: None,
                capabilities: None,
                queue: VecDeque::new(),
                queued_bytes: 0,
                hold_until_bound,
                unsuccessful_writes: 0,
                in_flight: false,
                terminal_enqueued: false,
                terminal_delivered: false,
                stream_epoch: 0,
                awaiting_capture: true,
                input_queue: VecDeque::new(),
                last_operation_id: 0,
                paste: None,
                lane: LaneUsage::default(),
            },
        );
        Ok((generation, replacements))
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
        self.capacity_parked.remove(key);
        let owner = self.live.remove(key)?;
        self.release_lane(&key.session_id, &owner.client_id, owner.lane);
        let in_flight_keys: Vec<u64> = self
            .in_flight
            .iter()
            .filter(|(_, operation)| &operation.key == key)
            .map(|(operation_key, _)| *operation_key)
            .collect();
        for operation_key in &in_flight_keys {
            self.in_flight.remove(operation_key);
        }
        Some(hard_stop(owner, key, in_flight_keys))
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
                return Err(if self.known.contains(&key) {
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
        owner.hold_until_bound = false;
        if !owner.queue.is_empty() {
            self.bound_queue_wake_sessions.insert(key.session_id);
        }
        Ok(())
    }

    /// Take session ids whose bound Ready queues grew since the last take.
    #[must_use]
    pub fn take_bound_queue_wake_sessions(&mut self) -> HashSet<SessionId> {
        std::mem::take(&mut self.bound_queue_wake_sessions)
    }

    /// Whether any live owner for `session_id` still holds undelivered frames.
    #[must_use]
    pub fn session_has_undelivered_frames(&self, session_id: &SessionId) -> bool {
        self.live.iter().any(|(key, owner)| {
            &key.session_id == session_id && (!owner.queue.is_empty() || owner.in_flight)
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
            .is_some_and(|owner| owner.adapter.is_some() && !owner.queue.is_empty())
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

    /// Whether this route is a bound or pre-bind held route.
    ///
    /// Such routes receive binary frames directly and are excluded from the
    /// unbound `TransportEgress` drain path.
    #[must_use]
    pub fn route_is_bound(
        &self,
        client_id: &ClientId,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .is_some_and(|owner| {
                &owner.client_id == client_id && (owner.adapter.is_some() || owner.hold_until_bound)
            })
    }

    /// Latest worker modes for a session as seen by this worker.
    #[must_use]
    pub fn session_modes(&self, session_id: &SessionId) -> Option<ModesBody> {
        self.session_modes.get(session_id).copied()
    }

    /// Current stream epoch of one live route.
    #[must_use]
    pub fn route_stream_epoch(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<u32> {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .map(|owner| owner.stream_epoch)
    }

    /// Remove bound-route frames from `egress` so only unbound drain consumers
    /// receive them. Bound routes already received the binary frame.
    ///
    /// An unbound owner that sees `ProcessExit` is hard-stopped: its client
    /// receives the exit through the drain path and the row must not outlive it.
    pub fn filter_bound_terminal_frames(
        &mut self,
        egress: &mut Vec<(ClientId, TransportEgress)>,
    ) -> Vec<ClientWorkerTeardown> {
        let mut retained = Vec::with_capacity(egress.len());
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
            let Some(owner) = self.live.get(&key) else {
                retained.push((client_id, frame));
                continue;
            };
            if owner.client_id != client_id {
                retained.push((client_id, frame));
                continue;
            }
            if owner.adapter.is_some() || owner.hold_until_bound {
                continue;
            }
            if matches!(frame, TransportEgress::ProcessExit { .. }) {
                unbound_process_exits.push(key);
            }
            retained.push((client_id, frame));
        }
        *egress = retained;
        unbound_process_exits
            .into_iter()
            .filter_map(|key| self.hard_stop_key(&key))
            .collect()
    }

    /// Enqueue one session-wide frame onto every receiving route of `session_id`.
    ///
    /// The frame body is shared by `Arc`; no route copies it. Routes that are
    /// still awaiting their capture skip live output because the capture
    /// already contains those bytes.
    pub fn push_session_frame(
        &mut self,
        session_id: &SessionId,
        frame: &TerminalFrame,
    ) -> Vec<ClientWorkerTeardown> {
        let keys = self.receiving_keys(session_id, false);
        let mut teardowns = Vec::new();
        for key in keys {
            if let Some(teardown) = self.enqueue_owner_frame(&key, frame.clone(), QueuedKind::Other)
            {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    /// Encode live PTY output once and share it across the session's routes.
    pub fn push_session_output(
        &mut self,
        session_id: &SessionId,
        data: &[u8],
    ) -> Vec<ClientWorkerTeardown> {
        if !self.session_has_receivers(session_id, false) {
            return Vec::new();
        }
        match encode_output(data) {
            Ok(frame) => self.push_session_frame(session_id, &frame),
            Err(_) => self.teardown_session(session_id),
        }
    }

    /// Record worker modes for a session and share the `MODES` frame.
    pub fn push_session_modes(
        &mut self,
        session_id: &SessionId,
        modes: ModesBody,
    ) -> Vec<ClientWorkerTeardown> {
        self.session_modes.insert(session_id.clone(), modes);
        if !self.session_has_receivers(session_id, false) {
            return Vec::new();
        }
        match encode_modes(modes) {
            Ok(frame) => self.push_session_frame(session_id, &frame),
            Err(_) => self.teardown_session(session_id),
        }
    }

    /// End one route with `ATTACH_STATE failed`. The frame is the last on the
    /// route; the route hard-stops after it is delivered.
    pub fn fail_route(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<ClientWorkerTeardown> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        let bound = self
            .live
            .get(&key)
            .is_some_and(|owner| owner.adapter.is_some() || owner.hold_until_bound);
        if !bound {
            return self.hard_stop_key(&key);
        }
        match encode_attach_state(AttachStateCode::Failed) {
            Ok(frame) => self.enqueue_owner_frame(&key, frame, QueuedKind::Terminal),
            Err(_) => self.hard_stop_key(&key),
        }
    }

    /// Share `PROCESS_EXIT` with every route on the session, including routes
    /// still awaiting a capture that will never arrive.
    pub fn push_session_process_exit(
        &mut self,
        session_id: &SessionId,
        code: Option<i32>,
    ) -> Vec<ClientWorkerTeardown> {
        let mut teardowns = self.fail_queued_input_for_session(session_id);
        let keys = self.receiving_keys(session_id, true);
        if keys.is_empty() {
            return teardowns;
        }
        let frame = match encode_process_exit(code) {
            Ok(frame) => frame,
            Err(_) => {
                teardowns.extend(self.teardown_session(session_id));
                return teardowns;
            }
        };
        for key in keys {
            if let Some(teardown) =
                self.enqueue_owner_frame(&key, frame.clone(), QueuedKind::Terminal)
            {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    /// Enqueue one route-personal frame.
    ///
    /// `SNAPSHOT_READY` ends the route's capture wait so later live output
    /// flows. Returns the teardown when the enqueue hard-stopped the route.
    pub fn push_route_frame(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        frame: TerminalFrame,
    ) -> Result<Option<ClientWorkerTeardown>, EnqueueRouteFrameError> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        let Some(owner) = self.live.get_mut(&key) else {
            return Err(EnqueueRouteFrameError::OwnerGone);
        };
        if owner.adapter.is_none() && !owner.hold_until_bound {
            // Unbound owners are served by the drain path.
            return Ok(None);
        }
        if frame.kind() == TerminalKind::SnapshotReady {
            owner.awaiting_capture = false;
        }
        let kind = if frame.kind() == TerminalKind::InputResult {
            QueuedKind::InputResult
        } else {
            QueuedKind::Other
        };
        Ok(self.enqueue_owner_frame(&key, frame, kind))
    }

    /// Enqueue an `ATTACH_STATE` frame for one route.
    pub fn push_attach_state(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        state: AttachStateCode,
    ) -> Result<Option<ClientWorkerTeardown>, EnqueueRouteFrameError> {
        let frame = encode_attach_state(state).map_err(|_| EnqueueRouteFrameError::EncodeFailed)?;
        self.push_route_frame(session_id, subscription_id, frame)
    }

    /// Mark a route as awaiting a fresh capture without changing its generation.
    ///
    /// Used when the host restarts a capture for a live route (attach takeover).
    pub fn begin_route_capture(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) {
        if let Some(owner) = self.live.get_mut(&OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        }) {
            owner.awaiting_capture = true;
        }
    }

    /// Take routes whose egress overflowed and now need a fresh capture.
    #[must_use]
    pub fn take_resync_requests(&mut self) -> Vec<RouteResyncRequest> {
        std::mem::take(&mut self.resync_requests)
    }

    /// Worker key of one in-flight operation addressed by route identity.
    ///
    /// The route is the subscription id; `generation` must match the live
    /// attachment generation.
    #[must_use]
    pub fn in_flight_key_for_route(
        &self,
        route: &RouteId,
        generation: u64,
        operation_id: u64,
    ) -> Option<(SessionId, u64)> {
        self.in_flight
            .iter()
            .find(|(_, operation)| {
                operation.key.subscription_id.0 == route.as_str()
                    && operation.operation_id == operation_id
                    && self
                        .live
                        .get(&operation.key)
                        .is_some_and(|owner| owner.generation.0 == generation)
            })
            .map(|(key, operation)| (operation.key.session_id.clone(), *key))
    }

    /// Take worker operation keys the host must cancel at the worker.
    #[must_use]
    pub fn take_cancel_requests(&mut self) -> Vec<(SessionId, u64)> {
        std::mem::take(&mut self.cancel_requests)
    }

    fn session_has_receivers(&self, session_id: &SessionId, include_awaiting: bool) -> bool {
        self.live.iter().any(|(key, owner)| {
            &key.session_id == session_id
                && (owner.adapter.is_some() || owner.hold_until_bound)
                && (include_awaiting || !owner.awaiting_capture)
                && !owner.terminal_enqueued
        })
    }

    fn receiving_keys(&self, session_id: &SessionId, include_awaiting: bool) -> Vec<OwnerKey> {
        let mut keys: Vec<_> = self
            .live
            .iter()
            .filter(|(key, owner)| {
                &key.session_id == session_id
                    && (owner.adapter.is_some() || owner.hold_until_bound)
                    && (include_awaiting || !owner.awaiting_capture)
                    && !owner.terminal_enqueued
            })
            .map(|(key, _)| key.clone())
            .collect();
        keys.sort_by(|left, right| left.subscription_id.0.cmp(&right.subscription_id.0));
        keys
    }

    /// Enqueue one frame on one owner with the route egress ceiling.
    ///
    /// Overflow recovers only this route: unsent obsolete frames are dropped,
    /// the route enters a new stream epoch, `ROUTE_RESYNC` is queued under it
    /// followed by any preserved `INPUT_RESULT` frames, and the host is asked
    /// for a new capture. Epoch exhaustion ends the route with
    /// `ATTACH_STATE failed`.
    fn enqueue_owner_frame(
        &mut self,
        key: &OwnerKey,
        frame: TerminalFrame,
        kind: QueuedKind,
    ) -> Option<ClientWorkerTeardown> {
        let over = {
            let owner = self.live.get_mut(key)?;
            if owner.terminal_enqueued {
                return None;
            }
            owner.queue.len() >= MAX_ROUTE_EGRESS_FRAMES
                || owner.queued_bytes.saturating_add(frame.len()) > MAX_ROUTE_EGRESS_BYTES
        };
        if over {
            return self.overflow_route(key, frame, kind);
        }
        let ready = {
            let owner = self.live.get_mut(key)?;
            let ready = Self::owner_ready_for_bound_queue_wake(owner);
            owner.queued_bytes += frame.len();
            if kind == QueuedKind::Terminal {
                owner.terminal_enqueued = true;
            }
            let stream_epoch = owner.stream_epoch;
            owner.queue.push_back(QueuedFrame {
                frame,
                kind,
                stream_epoch,
            });
            ready
        };
        if ready {
            self.bound_queue_wake_sessions
                .insert(key.session_id.clone());
        }
        None
    }

    fn overflow_route(
        &mut self,
        key: &OwnerKey,
        overflowing: TerminalFrame,
        overflowing_kind: QueuedKind,
    ) -> Option<ClientWorkerTeardown> {
        let from_epoch = self.live.get(key)?.stream_epoch;
        let Some(to_epoch) = from_epoch.checked_add(1) else {
            // Epoch exhausted: end the route explicitly. `fail_route` needs
            // queue room, so drop the unsent frames first.
            let owner = self.live.get_mut(key)?;
            let keep = usize::from(owner.in_flight);
            owner.queue.truncate(keep);
            owner.queued_bytes = owner.queue.iter().map(|queued| queued.frame.len()).sum();
            return self.fail_route(&key.session_id, &key.subscription_id);
        };
        let resync = match encode_route_resync(from_epoch, to_epoch) {
            Ok(frame) => frame,
            Err(_) => return self.hard_stop_key(key),
        };
        let owner = self.live.get_mut(key)?;
        // The in-flight head is already copied by the adapter; keep its slot so
        // completion bookkeeping stays exact. Unsent obsolete frames are
        // dropped; accepted input results are preserved in order behind the
        // transition under the new epoch.
        let keep = usize::from(owner.in_flight);
        let mut preserved: VecDeque<QueuedFrame> = owner.queue.drain(keep..).collect();
        preserved.retain(|queued| queued.kind == QueuedKind::InputResult);
        if overflowing_kind == QueuedKind::InputResult {
            preserved.push_back(QueuedFrame {
                frame: overflowing,
                kind: overflowing_kind,
                stream_epoch: from_epoch,
            });
        }
        owner.stream_epoch = to_epoch;
        owner.awaiting_capture = true;
        owner.queue.push_back(QueuedFrame {
            frame: resync,
            kind: QueuedKind::Other,
            stream_epoch: to_epoch,
        });
        for mut queued in preserved {
            queued.stream_epoch = to_epoch;
            owner.queue.push_back(queued);
        }
        owner.queued_bytes = owner.queue.iter().map(|queued| queued.frame.len()).sum();
        let ready = Self::owner_ready_for_bound_queue_wake(owner);
        let request = RouteResyncRequest {
            client_id: owner.client_id.clone(),
            session_id: key.session_id.clone(),
            subscription_id: key.subscription_id.clone(),
            generation: owner.generation,
            stream_epoch: to_epoch,
        };
        if ready {
            self.bound_queue_wake_sessions
                .insert(key.session_id.clone());
        }
        self.resync_requests.push(request);
        None
    }

    /// Pump only routes named by a wake batch. Never scans unbound or unnamed routes.
    pub fn pump_woken(&mut self, batch: &TerminalWakeBatch) -> Vec<ClientWorkerTeardown> {
        let route_keys = self.adapter_route_keys(batch);
        let mut teardowns = self.expire_pastes_keys(&route_keys, Instant::now());
        let mut seen = HashSet::new();
        let mut keys = Vec::new();
        for route in &batch.adapter_routes {
            let key = OwnerKey {
                session_id: route.session_id.clone(),
                subscription_id: route.subscription_id.clone(),
            };
            if seen.insert(key.clone()) {
                keys.push(key);
            }
        }
        for session_id in &batch.ingress_sessions {
            let mut session_keys: Vec<_> = self
                .live
                .iter()
                .filter(|(key, owner)| &key.session_id == session_id && owner.adapter.is_some())
                .map(|(key, _)| key.clone())
                .collect();
            session_keys
                .sort_by(|left, right| left.subscription_id.0.cmp(&right.subscription_id.0));
            for key in session_keys {
                if seen.insert(key.clone()) {
                    keys.push(key);
                }
            }
        }
        for key in keys {
            if let Some(teardown) = self.pump_one(&key) {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    fn pump_one(&mut self, key: &OwnerKey) -> Option<ClientWorkerTeardown> {
        loop {
            let owner = self.live.get_mut(key)?;
            let adapter = owner.adapter.as_mut()?;
            if adapter.pressure() == TerminalAdapterPressure::Closed {
                return self.hard_stop_key(key);
            }
            if owner.in_flight {
                match adapter.pressure() {
                    TerminalAdapterPressure::Ready => {
                        Self::complete_head(owner);
                    }
                    TerminalAdapterPressure::Closed => return self.hard_stop_key(key),
                    TerminalAdapterPressure::Full | TerminalAdapterPressure::WouldBlock => {
                        owner.unsuccessful_writes = owner.unsuccessful_writes.saturating_add(1);
                        if owner.unsuccessful_writes >= WRITE_ATTEMPT_BUDGET {
                            return self.hard_stop_key(key);
                        }
                        return None;
                    }
                }
            }
            if owner.terminal_delivered {
                return self.hard_stop_key(key);
            }
            let Some(head) = owner.queue.front() else {
                return None;
            };
            let routed = RoutedTerminalFrame {
                route: owner.route.clone(),
                generation: owner.generation.0,
                stream_epoch: head.stream_epoch,
                frame: head.frame.clone(),
            };
            let adapter = owner.adapter.as_mut()?;
            match adapter.try_write(&routed) {
                Ok(()) => {
                    owner.in_flight = true;
                    owner.unsuccessful_writes = 0;
                    if adapter.pressure() == TerminalAdapterPressure::Ready {
                        Self::complete_head(owner);
                        continue;
                    }
                    return None;
                }
                Err(TerminalAdapterWriteError::WouldBlock | TerminalAdapterWriteError::Full) => {
                    owner.unsuccessful_writes = owner.unsuccessful_writes.saturating_add(1);
                    if owner.unsuccessful_writes >= WRITE_ATTEMPT_BUDGET {
                        return self.hard_stop_key(key);
                    }
                    return None;
                }
                Err(TerminalAdapterWriteError::Closed) => return self.hard_stop_key(key),
            }
        }
    }

    fn complete_head(owner: &mut SubscriptionOwner) {
        if let Some(completed) = owner.queue.pop_front() {
            owner.queued_bytes = owner.queued_bytes.saturating_sub(completed.frame.len());
            if completed.kind == QueuedKind::Terminal {
                owner.terminal_delivered = true;
            }
        }
        owner.in_flight = false;
        owner.unsuccessful_writes = 0;
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

    /// Whether the owner's next Stage B operation is a resize.
    pub(crate) fn terminal_input_head_is_resize(&self, key: &OwnerKey) -> bool {
        self.live
            .get(key)
            .and_then(|owner| owner.input_queue.front())
            .is_some_and(|input| input.kind == WorkerInputKind::Resize)
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
                let Ok(frame) = TerminalInputFrame::from_bytes(&bytes) else {
                    fail = true;
                    break;
                };
                let Ok(command) = decode_terminal_input(&frame) else {
                    fail = true;
                    break;
                };
                let body = frame.as_bytes()[INPUT_HEADER_BYTES..].to_vec();
                if self.intake_terminal_command(&key, command, body).is_err() {
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

    /// Admit one decoded command. `Err` means the route must hard-stop.
    fn intake_terminal_command(
        &mut self,
        key: &OwnerKey,
        command: TerminalInputCommand,
        body: Vec<u8>,
    ) -> Result<(), ()> {
        let operation_id = command_operation_id(&command);
        let continues_paste = matches!(
            command,
            TerminalInputCommand::PasteChunk { .. }
                | TerminalInputCommand::PasteCommit { .. }
                | TerminalInputCommand::PasteAbort { .. }
        );
        {
            let owner = self.live.get_mut(key).ok_or(())?;
            if continues_paste {
                let matches_active = owner
                    .paste
                    .as_ref()
                    .is_some_and(|paste| paste.operation_id == operation_id);
                if !matches_active {
                    // Abort may name a queued or in-flight paste; the others may not.
                    if !matches!(command, TerminalInputCommand::PasteAbort { .. })
                        || operation_id > owner.last_operation_id
                    {
                        return self.reject(
                            key,
                            operation_id,
                            InputOutcome::RejectedProtocol,
                            "unknown paste operation",
                        );
                    }
                }
            } else {
                if operation_id == 0 || operation_id <= owner.last_operation_id {
                    return self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedProtocol,
                        "operation id is not strictly increasing",
                    );
                }
                owner.last_operation_id = operation_id;
            }
        }
        match command {
            TerminalInputCommand::RawBytes { data, .. } => {
                let payload = data.len() as u64;
                self.admit(key, operation_id, WorkerInputKind::RawBytes, body, payload)
            }
            TerminalInputCommand::Key { text, .. } => {
                let payload = text.len() as u64;
                self.admit(key, operation_id, WorkerInputKind::Key, body, payload)
            }
            TerminalInputCommand::Mouse { .. } => {
                self.admit(key, operation_id, WorkerInputKind::Mouse, body, 0)
            }
            TerminalInputCommand::Focus { .. } => {
                self.admit(key, operation_id, WorkerInputKind::Focus, body, 0)
            }
            TerminalInputCommand::Resize { .. } => {
                self.admit(key, operation_id, WorkerInputKind::Resize, body, 0)
            }
            TerminalInputCommand::PasteBegin {
                total_len,
                allow_unsafe,
                ..
            } => {
                let owner = self.live.get_mut(key).ok_or(())?;
                if owner.paste.is_some() {
                    // The protocol allows one assembling paste per route.
                    debug_assert!(MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION >= 1);
                    return self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedLaneFull,
                        "a paste is already assembling on this route",
                    );
                }
                let total_len = total_len as usize;
                if total_len == 0 || total_len > MAX_PASTE_BYTES {
                    return self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedTooLarge,
                        "paste length is zero or exceeds the paste ceiling",
                    );
                }
                owner.paste = Some(PasteAssembly {
                    operation_id,
                    allow_unsafe,
                    total_len,
                    expected_chunks: total_len.div_ceil(MAX_PASTE_CHUNK_DATA_BYTES),
                    next_index: 0,
                    data: Vec::with_capacity(total_len),
                    deadline: Instant::now() + PASTE_ASSEMBLY_TIMEOUT,
                });
                Ok(())
            }
            TerminalInputCommand::PasteChunk { index, data, .. } => {
                let owner = self.live.get_mut(key).ok_or(())?;
                let Some(assembly) = owner.paste.as_mut() else {
                    return Ok(());
                };
                let expected_len = if assembly.next_index + 1 < assembly.expected_chunks {
                    MAX_PASTE_CHUNK_DATA_BYTES
                } else {
                    assembly.total_len - MAX_PASTE_CHUNK_DATA_BYTES * (assembly.expected_chunks - 1)
                };
                if index as usize != assembly.next_index
                    || assembly.next_index >= assembly.expected_chunks
                    || data.len() != expected_len
                {
                    owner.paste = None;
                    return self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedProtocol,
                        "paste chunk is out of order or mis-sized",
                    );
                }
                assembly.data.extend_from_slice(&data);
                assembly.next_index += 1;
                Ok(())
            }
            TerminalInputCommand::PasteCommit { .. } => {
                let owner = self.live.get_mut(key).ok_or(())?;
                let Some(assembly) = owner.paste.take() else {
                    return Ok(());
                };
                if assembly.next_index != assembly.expected_chunks
                    || assembly.data.len() != assembly.total_len
                {
                    return self.reject(
                        key,
                        operation_id,
                        InputOutcome::RejectedProtocol,
                        "paste committed before every chunk arrived",
                    );
                }
                let mut worker_body = Vec::with_capacity(assembly.data.len() + 1);
                worker_body.push(u8::from(assembly.allow_unsafe));
                worker_body.extend_from_slice(&assembly.data);
                let payload = assembly.data.len() as u64;
                self.admit(
                    key,
                    operation_id,
                    WorkerInputKind::Paste,
                    worker_body,
                    payload,
                )
            }
            TerminalInputCommand::PasteAbort { .. } => {
                let owner = self.live.get_mut(key).ok_or(())?;
                if owner
                    .paste
                    .as_ref()
                    .is_some_and(|paste| paste.operation_id == operation_id)
                {
                    owner.paste = None;
                    return self.reject(key, operation_id, InputOutcome::Cancelled, "");
                }
                if let Some(position) = owner
                    .input_queue
                    .iter()
                    .position(|input| input.operation_id == operation_id)
                {
                    let removed = owner.input_queue.remove(position).ok_or(())?;
                    let usage = LaneUsage {
                        operations: 1,
                        bytes: removed.body.len(),
                    };
                    owner.lane.operations = owner.lane.operations.saturating_sub(1);
                    owner.lane.bytes = owner.lane.bytes.saturating_sub(removed.body.len());
                    let client_id = owner.client_id.clone();
                    self.release_lane(&key.session_id, &client_id, usage);
                    return self.reject(key, operation_id, InputOutcome::Cancelled, "");
                }
                let in_flight = self
                    .in_flight
                    .iter()
                    .find(|(_, operation)| {
                        &operation.key == key && operation.operation_id == operation_id
                    })
                    .map(|(operation_key, _)| *operation_key);
                if let Some(operation_key) = in_flight {
                    self.cancel_requests
                        .push((key.session_id.clone(), operation_key));
                    return Ok(());
                }
                self.reject(
                    key,
                    operation_id,
                    InputOutcome::RejectedProtocol,
                    "abort names no active paste",
                )
            }
        }
    }

    fn admit(
        &mut self,
        key: &OwnerKey,
        operation_id: u64,
        kind: WorkerInputKind,
        body: Vec<u8>,
        accepted_payload_bytes: u64,
    ) -> Result<(), ()> {
        let client_id = self.live.get(key).ok_or(())?.client_id.clone();
        let session_lane = self
            .session_lanes
            .get(&key.session_id)
            .copied()
            .unwrap_or_default();
        let client_lane = self
            .client_lanes
            .get(&client_id)
            .copied()
            .unwrap_or_default();
        let bytes = body.len();
        let session_full = session_lane.operations >= MAX_INPUT_OPERATIONS_PER_SESSION
            || session_lane.bytes.saturating_add(bytes) > MAX_RETAINED_INPUT_BYTES_PER_SESSION;
        let client_full = client_lane.operations >= MAX_INPUT_OPERATIONS_PER_CLIENT
            || client_lane.bytes.saturating_add(bytes) > MAX_RETAINED_INPUT_BYTES_PER_CLIENT;
        if session_full || client_full {
            return self.reject(
                key,
                operation_id,
                InputOutcome::RejectedLaneFull,
                if session_full {
                    "session input lane is full"
                } else {
                    "client input lane is full"
                },
            );
        }
        let usage = LaneUsage {
            operations: 1,
            bytes,
        };
        self.reserve_lane(&key.session_id, &client_id, usage);
        let owner = self.live.get_mut(key).ok_or(())?;
        owner.lane.operations += 1;
        owner.lane.bytes += bytes;
        owner.input_queue.push_back(AdmittedInput {
            operation_id,
            kind,
            body,
            accepted_payload_bytes,
        });
        Ok(())
    }

    fn reserve_lane(&mut self, session_id: &SessionId, client_id: &ClientId, usage: LaneUsage) {
        let session = self.session_lanes.entry(session_id.clone()).or_default();
        session.operations += usage.operations;
        session.bytes += usage.bytes;
        let client = self.client_lanes.entry(client_id.clone()).or_default();
        client.operations += usage.operations;
        client.bytes += usage.bytes;
    }

    fn release_lane(&mut self, session_id: &SessionId, client_id: &ClientId, usage: LaneUsage) {
        if let Some(session) = self.session_lanes.get_mut(session_id) {
            session.operations = session.operations.saturating_sub(usage.operations);
            session.bytes = session.bytes.saturating_sub(usage.bytes);
            if session.operations == 0 && session.bytes == 0 {
                self.session_lanes.remove(session_id);
            }
        }
        if let Some(client) = self.client_lanes.get_mut(client_id) {
            client.operations = client.operations.saturating_sub(usage.operations);
            client.bytes = client.bytes.saturating_sub(usage.bytes);
            if client.operations == 0 && client.bytes == 0 {
                self.client_lanes.remove(client_id);
            }
        }
    }

    /// Enqueue a Core-originated result. `Err` means the route hard-stopped.
    fn reject(
        &mut self,
        key: &OwnerKey,
        operation_id: u64,
        outcome: InputOutcome,
        detail: &str,
    ) -> Result<(), ()> {
        let mode_bits = self
            .session_modes
            .get(&key.session_id)
            .map(|modes| modes.mode_bits)
            .unwrap_or(0);
        let result = InputResultBody {
            operation_id,
            outcome,
            accepted_payload_bytes: Some(0),
            written_pty_bytes: Some(0),
            mode_bits,
            detail: bounded_detail(detail),
        };
        self.enqueue_result(key, &result)
    }

    fn enqueue_result(&mut self, key: &OwnerKey, result: &InputResultBody) -> Result<(), ()> {
        let frame = encode_input_result(result).map_err(|_| ())?;
        match self.push_route_frame(&key.session_id, &key.subscription_id, frame) {
            Ok(None) => Ok(()),
            Ok(Some(_)) | Err(_) => Err(()),
        }
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
            }
            if self
                .reject(
                    &key,
                    operation_id,
                    InputOutcome::RejectedProtocol,
                    "paste assembly timed out",
                )
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

    /// Stage B: take at most one admitted operation from one exact live owner
    /// and move it to the in-flight map under a unique worker key.
    pub(crate) fn take_one_terminal_input(
        &mut self,
        key: &OwnerKey,
    ) -> Option<StagedTerminalInput> {
        let owner = self.live.get_mut(key)?;
        let input = owner.input_queue.pop_front()?;
        let operation_key = self.next_operation_key;
        self.next_operation_key = self.next_operation_key.checked_add(1)?;
        let staged = StagedTerminalInput {
            client_id: owner.client_id.clone(),
            session_id: key.session_id.clone(),
            subscription_id: key.subscription_id.clone(),
            generation: owner.generation,
            operation_key,
            operation_id: input.operation_id,
            kind: input.kind,
            body: input.body,
            accepted_payload_bytes: input.accepted_payload_bytes,
        };
        self.in_flight.insert(
            operation_key,
            InFlightOperation {
                key: key.clone(),
                client_id: owner.client_id.clone(),
                operation_id: input.operation_id,
                retained_bytes: staged.body.len(),
            },
        );
        self.capacity_parked.remove(key);
        Some(staged)
    }

    /// Stage B across every live owner in rotated order.
    pub fn take_terminal_input(&mut self) -> Vec<StagedTerminalInput> {
        let keys = self.rotated_live_keys();
        let mut staged = Vec::new();
        for key in keys {
            for _ in 0..APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK {
                let Some(input) = self.take_one_terminal_input(&key) else {
                    break;
                };
                staged.push(input);
            }
        }
        staged
    }

    /// Deliver the worker's result for one in-flight operation.
    ///
    /// Releases lane usage and enqueues `INPUT_RESULT` on the route. A result
    /// for a route that was torn down is dropped. Returns a teardown when the
    /// enqueue hard-stopped the route.
    pub fn complete_operation(
        &mut self,
        operation_key: u64,
        result: InputResultBody,
    ) -> Option<ClientWorkerTeardown> {
        let operation = self.in_flight.remove(&operation_key)?;
        let usage = LaneUsage {
            operations: 1,
            bytes: operation.retained_bytes,
        };
        self.release_lane(&operation.key.session_id, &operation.client_id, usage);
        if let Some(owner) = self.live.get_mut(&operation.key) {
            owner.lane.operations = owner.lane.operations.saturating_sub(1);
            owner.lane.bytes = owner.lane.bytes.saturating_sub(operation.retained_bytes);
        }
        let mut result = result;
        result.operation_id = operation.operation_id;
        result.detail = bounded_detail(&result.detail);
        match self.enqueue_result(&operation.key, &result) {
            Ok(()) => None,
            Err(()) => self.hard_stop_key(&operation.key),
        }
    }

    /// Fail every in-flight operation on `session_id` with one outcome.
    ///
    /// Used for worker link failure (`OutcomeUnknown`) and session end.
    pub fn fail_in_flight_for_session(
        &mut self,
        session_id: &SessionId,
        outcome: InputOutcome,
        detail: &str,
    ) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<u64> = self
            .in_flight
            .iter()
            .filter(|(_, operation)| &operation.key.session_id == session_id)
            .map(|(key, _)| *key)
            .collect();
        let mode_bits = self
            .session_modes
            .get(session_id)
            .map(|modes| modes.mode_bits)
            .unwrap_or(0);
        let mut teardowns = Vec::new();
        for key in keys {
            let result = InputResultBody {
                operation_id: 0,
                outcome,
                accepted_payload_bytes: None,
                written_pty_bytes: None,
                mode_bits,
                detail: bounded_detail(detail),
            };
            if let Some(teardown) = self.complete_operation(key, result) {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    /// Reject every queued but unsent operation on `session_id` as ended.
    fn fail_queued_input_for_session(
        &mut self,
        session_id: &SessionId,
    ) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self
            .live
            .iter()
            .filter(|(key, owner)| &key.session_id == session_id && !owner.input_queue.is_empty())
            .map(|(key, _)| key.clone())
            .collect();
        let mut teardowns = Vec::new();
        for key in keys {
            let Some(owner) = self.live.get_mut(&key) else {
                continue;
            };
            let client_id = owner.client_id.clone();
            let drained: Vec<_> = owner.input_queue.drain(..).collect();
            let released = LaneUsage {
                operations: drained.len(),
                bytes: drained.iter().map(|input| input.body.len()).sum(),
            };
            owner.lane.operations = owner.lane.operations.saturating_sub(released.operations);
            owner.lane.bytes = owner.lane.bytes.saturating_sub(released.bytes);
            owner.paste = None;
            self.release_lane(session_id, &client_id, released);
            for input in drained {
                if self
                    .reject(
                        &key,
                        input.operation_id,
                        InputOutcome::SessionEnded,
                        "session ended before the operation ran",
                    )
                    .is_err()
                {
                    if let Some(teardown) = self.hard_stop_key(&key) {
                        teardowns.push(teardown);
                    }
                    break;
                }
            }
        }
        teardowns
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

    /// Operations admitted for a session and not yet resulted.
    #[must_use]
    pub fn session_in_flight_operations(&self, session_id: &SessionId) -> usize {
        self.session_lanes
            .get(session_id)
            .map(|lane| lane.operations)
            .unwrap_or(0)
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
        self.session_modes.remove(session_id);
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
    fn try_write(&mut self, frame: &RoutedTerminalFrame) -> Result<(), TerminalAdapterWriteError> {
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

fn hard_stop(
    mut owner: SubscriptionOwner,
    key: &OwnerKey,
    in_flight_keys: Vec<u64>,
) -> ClientWorkerTeardown {
    owner.queue.clear();
    owner.input_queue.clear();
    if let Some(mut adapter) = owner.adapter.take() {
        adapter.close();
        drop(adapter);
    }
    ClientWorkerTeardown {
        client_id: owner.client_id,
        session_id: key.session_id.clone(),
        subscription_id: key.subscription_id.clone(),
        generation: owner.generation,
        in_flight_keys,
    }
}

fn command_operation_id(command: &TerminalInputCommand) -> u64 {
    match command {
        TerminalInputCommand::RawBytes { operation_id, .. }
        | TerminalInputCommand::Key { operation_id, .. }
        | TerminalInputCommand::Mouse { operation_id, .. }
        | TerminalInputCommand::Focus { operation_id, .. }
        | TerminalInputCommand::Resize { operation_id, .. }
        | TerminalInputCommand::PasteBegin { operation_id, .. }
        | TerminalInputCommand::PasteChunk { operation_id, .. }
        | TerminalInputCommand::PasteCommit { operation_id }
        | TerminalInputCommand::PasteAbort { operation_id } => *operation_id,
    }
}

fn bounded_detail(detail: &str) -> String {
    if detail.len() <= MAX_INPUT_RESULT_DETAIL_BYTES {
        return detail.to_owned();
    }
    let mut end = MAX_INPUT_RESULT_DETAIL_BYTES;
    while !detail.is_char_boundary(end) {
        end -= 1;
    }
    detail[..end].to_owned()
}

/// Client-visible kind of one worker input kind, for diagnostics.
#[must_use]
pub fn terminal_input_kind_of(kind: WorkerInputKind) -> TerminalInputKind {
    match kind {
        WorkerInputKind::RawBytes => TerminalInputKind::RawBytes,
        WorkerInputKind::Key => TerminalInputKind::Key,
        WorkerInputKind::Mouse => TerminalInputKind::Mouse,
        WorkerInputKind::Focus => TerminalInputKind::Focus,
        WorkerInputKind::Resize => TerminalInputKind::Resize,
        WorkerInputKind::Paste => TerminalInputKind::PasteCommit,
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
