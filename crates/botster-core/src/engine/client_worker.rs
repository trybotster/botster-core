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
    TerminalRouteCloseReason,
};
use crate::contract::terminal_subscription::{
    AttachTerminalRouteError, BindTerminalAdapterError, DetachTerminalSubscriptionResult,
    StagedTerminalInput, TerminalSubscriptionGeneration, TerminalSubscriptionInventory,
    TerminalSubscriptionInventoryError, TerminalSubscriptionRecord,
};
use crate::contract::terminal_wake::{
    TerminalWakeBatch, TerminalWakeSource, WakingTerminalAdapter,
};
use crate::session::{SessionId, SubscriptionId};
use crate::session_protocol::WorkerInputKind;
use crate::transport::TransportEgress;

const WRITE_ATTEMPT_BUDGET: usize = 512;

/// How long a bound route's adapter may keep refusing or holding writes,
/// while the route has frames pending, before Core ends the route as a dead
/// reader.
///
/// - The clock runs only while the adapter refuses or holds writes with
///   frames pending: it starts when the adapter refuses the head, or accepts
///   a frame and has not finished it. It never runs for a route with nothing
///   pending.
/// - Only progress clears it: a write the transport completes, or an empty
///   queue. A reader that falls behind but completes a write within the
///   deadline stays attached, however fast the producer is.
/// - The deadline does not delay a real transport close: an adapter that
///   reports `Closed` ends the route at once.
///
/// A reader that never reads never drains its transport, so its adapter
/// posts no writable wake and nothing else can observe it. The deadline is
/// part of the host wait (see [`ClientWorker::next_reader_deadline`]), so a
/// dead reader is closed even when the session produces nothing more.
pub(crate) const READER_PROGRESS_DEADLINE: Duration = Duration::from_secs(10);

/// What woke a route pump.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PumpOrigin {
    /// The route's adapter posted a wake. A busy adapter here counts toward
    /// [`WRITE_ATTEMPT_BUDGET`], which guards against spurious-wake storms.
    AdapterWake,
    /// The session produced output. It never counts: it follows the
    /// producer's rate, not the reader's liveness.
    SessionOutput,
}

fn inventory_add_bytes(
    total: usize,
    bytes: usize,
) -> Result<usize, TerminalSubscriptionInventoryError> {
    total
        .checked_add(bytes)
        .ok_or(TerminalSubscriptionInventoryError::SizeOverflow)
}

fn inventory_row_storage_bytes(rows: usize) -> Result<usize, TerminalSubscriptionInventoryError> {
    let bytes = rows
        .checked_mul(std::mem::size_of::<TerminalSubscriptionRecord>())
        .ok_or(TerminalSubscriptionInventoryError::SizeOverflow)?;
    inventory_add_bytes(std::mem::size_of::<TerminalSubscriptionInventory>(), bytes)
}
/// Stage A intake budget.
pub const INTAKE_FRAMES_PER_SUBSCRIPTION_PER_TICK: usize = 64;
/// Stage B apply budget.
pub const APPLY_COMMANDS_PER_SUBSCRIPTION_PER_TICK: usize = 16;
/// Maximum time between an accepted paste begin and complete commit.
pub const PASTE_ASSEMBLY_TIMEOUT: Duration = Duration::from_secs(5);
/// Maximum Core-originated rejection results queued on one route. Rejections
/// hold no input-lane reservation, so they get their own hard cap.
pub const MAX_QUEUED_REJECTIONS_PER_ROUTE: usize = 16;

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
    /// Why Core ended the route; the same value reached the adapter's close.
    pub reason: TerminalRouteCloseReason,
}

/// Why an input step failed. `Some` holds the teardown of a route that the
/// step already ended, such as a result that overflowed the route's egress,
/// so the caller reports that teardown instead of losing it. `None` means
/// the route is still live and the caller ends it.
type RouteEnded = Option<ClientWorkerTeardown>;

/// Identity a capture is bound to: the attachment generation and the
/// route's capture fence at request time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CaptureIdentity {
    /// Fixed attachment generation of the route.
    pub generation: TerminalSubscriptionGeneration,
    /// Route capture fence; advances when an overflow lost visual frames.
    pub capture_fence: u64,
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
    /// Capture fence the fresh capture must carry.
    pub capture_fence: u64,
}

/// Failure while enqueueing a route-personal frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EnqueueRouteFrameError {
    /// The owner was already removed.
    OwnerGone,
    /// The frame could not be encoded.
    EncodeFailed,
    /// The frame belongs to a capture that an overflow resync superseded.
    EpochSuperseded,
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
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
    /// When the reader stopped making progress with frames pending: the
    /// adapter refused the head, or accepted it and has not finished it.
    /// Only a completed write or an empty queue clears it;
    /// [`READER_PROGRESS_DEADLINE`] after it the route ends.
    blocked_since: Option<Instant>,
    /// A stall resync ran and no write succeeded after it. The next
    /// exhausted attempt budget ends the route.
    stall_resynced: bool,
    in_flight: bool,
    /// A terminal frame (`PROCESS_EXIT` or `ATTACH_STATE failed`) is queued;
    /// nothing may follow it and the route hard-stops after delivery.
    terminal_enqueued: bool,
    terminal_delivered: bool,
    /// Stream epoch inside the fixed attachment generation. Starts at 0 and
    /// advances only through `ROUTE_RESYNC`.
    stream_epoch: u32,
    /// Capture fence. Advances whenever an overflow lost visual frames, so a
    /// capture started before that overflow can never add pages afterwards,
    /// even when the epoch did not change.
    capture_fence: u64,
    /// Live output is suppressed until the route's `SNAPSHOT_READY` lands.
    awaiting_capture: bool,
    /// A capture was requested for the route and the engine has not ended
    /// it: `PROCESS_EXIT` waits for it, so it follows the snapshot and the
    /// output after the capture fence.
    capture_open: bool,
    /// `PROCESS_EXIT` held while `capture_open`.
    deferred_exit: Option<TerminalFrame>,
    /// An unserved capture ended with its FINISH queued and no exit held
    /// yet: the later PROCESS_EXIT is released only while that FINISH is
    /// intact and the exit fits, and the route ends typed otherwise.
    unserved_finish: Option<CaptureIdentity>,
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
    /// `INPUT_RESULT`: preserved across resync in order. Carries the input
    /// lane reservation released when the adapter completes the write.
    InputResult(LaneUsage),
    /// `ROUTE_RESYNC`: an unsent transition is preserved across a later
    /// overflow so the receiver always sees a transition from its epoch.
    Resync,
    /// Output, modes, snapshot pages, attach state: dropped on overflow.
    Visual,
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
            if let Some(stolen) = self.hard_stop_key(&key, TerminalRouteCloseReason::Replaced) {
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
                blocked_since: None,
                stall_resynced: false,
                in_flight: false,
                terminal_enqueued: false,
                terminal_delivered: false,
                stream_epoch: 0,
                capture_fence: 0,
                awaiting_capture: true,
                // Opened by the engine when it queues a capture for the
                // route; an engine that captures synchronously never opens it.
                capture_open: false,
                deferred_exit: None,
                unserved_finish: None,
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
            .filter_map(|key| self.hard_stop_key(&key, TerminalRouteCloseReason::Replaced))
            .collect()
    }

    fn hard_stop_key(
        &mut self,
        key: &OwnerKey,
        reason: TerminalRouteCloseReason,
    ) -> Option<ClientWorkerTeardown> {
        self.wake_source
            .retire_route(&key.session_id, &key.subscription_id);
        self.capacity_parked.remove(key);
        // A route that ends can no longer ask for a fresh capture.
        self.resync_requests.retain(|pending| {
            pending.session_id != key.session_id || pending.subscription_id != key.subscription_id
        });
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
        Some(hard_stop(owner, key, in_flight_keys, reason))
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
                adapter.close(TerminalRouteCloseReason::BindRejected);
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
                adapter.close(TerminalRouteCloseReason::BindRejected);
                drop(adapter);
                return Err(BindTerminalAdapterError::StaleGeneration {
                    live,
                    requested: generation,
                });
            }
            if owner.adapter.is_some() {
                adapter.close(TerminalRouteCloseReason::BindRejected);
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
            adapter.close(TerminalRouteCloseReason::BindRejected);
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

    /// Return complete inventory only when its logical size fits the caller's
    /// retained reservation. Sizing borrows owners before cloning any output.
    pub fn list_terminal_subscriptions(
        &self,
        max_logical_bytes: usize,
    ) -> Result<TerminalSubscriptionInventory, TerminalSubscriptionInventoryError> {
        let mut logical_bytes = inventory_row_storage_bytes(self.live.len())?;
        for (key, owner) in &self.live {
            for bytes in [
                owner.client_id.0.len(),
                key.session_id.0.len(),
                key.subscription_id.0.len(),
            ] {
                logical_bytes = inventory_add_bytes(logical_bytes, bytes)?;
            }
            if let Some(capabilities) = &owner.capabilities {
                for token in capabilities.iter() {
                    logical_bytes =
                        inventory_add_bytes(logical_bytes, std::mem::size_of::<String>())?;
                    logical_bytes = inventory_add_bytes(logical_bytes, token.len())?;
                }
            }
        }
        if logical_bytes > max_logical_bytes {
            return Err(TerminalSubscriptionInventoryError::BudgetTooSmall {
                required_bytes: logical_bytes,
                max_bytes: max_logical_bytes,
            });
        }
        let mut records = Vec::with_capacity(self.live.len());
        for (key, owner) in &self.live {
            records.push(TerminalSubscriptionRecord {
                client_id: owner.client_id.clone(),
                session_id: key.session_id.clone(),
                subscription_id: key.subscription_id.clone(),
                generation: owner.generation,
                adapter_bound: owner.adapter.is_some(),
                capabilities: owner.capabilities.clone(),
            });
        }
        // The map's unique (session, subscription) keys make ties impossible.
        // Unstable in-place sorting preserves the old observable total order
        // without the stable sort's temporary allocation.
        records.sort_unstable_by(|left, right| {
            left.session_id
                .0
                .cmp(&right.session_id.0)
                .then(left.subscription_id.0.cmp(&right.subscription_id.0))
                .then(left.generation.0.cmp(&right.generation.0))
        });
        Ok(TerminalSubscriptionInventory {
            records,
            logical_bytes,
        })
    }

    /// Borrow the live client identity and generation for one exact route.
    ///
    /// Scans live owners without allocating or cloning identifiers. This is
    /// O(n) lookup work, not a constant-time index. Historical routes and
    /// expected-but-unattached adapters are not live owners.
    #[must_use]
    pub fn terminal_subscription_owner(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<(&ClientId, TerminalSubscriptionGeneration)> {
        self.live
            .iter()
            .find(|(key, _)| {
                &key.session_id == session_id && &key.subscription_id == subscription_id
            })
            .map(|(_, owner)| (&owner.client_id, owner.generation))
    }

    /// Compare a live owner without materializing inventory or cloning IDs.
    #[must_use]
    pub fn terminal_subscription_matches(
        &self,
        session_id: &SessionId,
        client_id: &ClientId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.live.iter().any(|(key, owner)| {
            &key.session_id == session_id
                && &key.subscription_id == subscription_id
                && &owner.client_id == client_id
        })
    }

    pub(crate) fn terminal_subscription_owners(
        &self,
    ) -> impl Iterator<Item = (&SessionId, &ClientId, &SubscriptionId)> {
        self.live
            .iter()
            .map(|(key, owner)| (&key.session_id, &owner.client_id, &key.subscription_id))
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
            .filter_map(|key| self.hard_stop_key(&key, TerminalRouteCloseReason::SessionEnded))
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
            if let Some(teardown) =
                self.enqueue_owner_frame(&key, frame.clone(), QueuedKind::Visual)
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
            Err(_) => self.teardown_session(session_id, TerminalRouteCloseReason::Failed),
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
            Err(_) => self.teardown_session(session_id, TerminalRouteCloseReason::Failed),
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
            return self.hard_stop_key(&key, TerminalRouteCloseReason::Failed);
        }
        match encode_attach_state(AttachStateCode::Failed) {
            Ok(frame) => self.enqueue_owner_frame(&key, frame, QueuedKind::Terminal),
            Err(_) => self.hard_stop_key(&key, TerminalRouteCloseReason::Failed),
        }
    }

    /// Share `PROCESS_EXIT` with every route on the session. A route whose
    /// capture is still open holds it until the engine ends that capture
    /// ([`Self::end_route_capture`]): the exit never overtakes a snapshot.
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
                teardowns
                    .extend(self.teardown_session(session_id, TerminalRouteCloseReason::Failed));
                return teardowns;
            }
        };
        for key in keys {
            if let Some(owner) = self.live.get_mut(&key) {
                if owner.capture_open {
                    owner.deferred_exit = Some(frame.clone());
                    continue;
                }
                if let Some(finished) = owner.unserved_finish.take() {
                    if let Some(teardown) =
                        self.release_after_unserved_finish(&key, frame.clone(), Some(finished))
                    {
                        teardowns.push(teardown);
                    }
                    continue;
                }
            }
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
        if frame.kind() == TerminalKind::SnapshotReady {
            // Also for an unbound owner: a later bind must not suppress live
            // output behind a capture that already completed.
            owner.awaiting_capture = false;
        }
        if owner.adapter.is_none() && !owner.hold_until_bound {
            // Unbound owners are served by the drain path.
            return Ok(None);
        }
        let kind = if frame.kind() == TerminalKind::InputResult {
            QueuedKind::InputResult(LaneUsage::default())
        } else {
            QueuedKind::Visual
        };
        Ok(self.enqueue_owner_frame(&key, frame, kind))
    }

    /// Enqueue one capture page under the fence the capture started with.
    ///
    /// A page from a capture that an overflow superseded is refused with
    /// [`EnqueueRouteFrameError::EpochSuperseded`]; the host cancels that
    /// capture and the pending resync request starts a fresh one.
    pub fn push_capture_frame(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        identity: CaptureIdentity,
        frame: TerminalFrame,
    ) -> Result<Option<ClientWorkerTeardown>, EnqueueRouteFrameError> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        let Some(owner) = self.live.get(&key) else {
            return Err(EnqueueRouteFrameError::OwnerGone);
        };
        if owner.generation != identity.generation || owner.capture_fence != identity.capture_fence
        {
            return Err(EnqueueRouteFrameError::EpochSuperseded);
        }
        self.push_route_frame(session_id, subscription_id, frame)
    }

    /// Complete capture identity of one live route right now: the fixed
    /// attachment generation plus the capture fence. A capture records it
    /// when its request is created and carries it unchanged to every page.
    #[must_use]
    pub fn capture_identity(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<CaptureIdentity> {
        self.live
            .get(&OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            })
            .map(|owner| CaptureIdentity {
                generation: owner.generation,
                capture_fence: owner.capture_fence,
            })
    }

    /// Whether `identity` still names the live attachment and fence.
    #[must_use]
    pub fn capture_identity_is_live(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        identity: CaptureIdentity,
    ) -> bool {
        self.capture_identity(session_id, subscription_id) == Some(identity)
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
            owner.capture_open = true;
            owner.unserved_finish = None;
        }
    }

    /// The engine queued a capture for the route: a `PROCESS_EXIT` now waits
    /// for it.
    pub fn open_route_capture(&mut self, session_id: &SessionId, subscription_id: &SubscriptionId) {
        if let Some(owner) = self.live.get_mut(&OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        }) {
            owner.capture_open = true;
            owner.unserved_finish = None;
        }
    }

    /// The engine ended the route's capture, after its snapshot and the
    /// output behind its fence were routed. A `PROCESS_EXIT` held for the
    /// capture is queued now, last.
    pub fn end_route_capture(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<ClientWorkerTeardown> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        let owner = self.live.get_mut(&key)?;
        owner.capture_open = false;
        let frame = owner.deferred_exit.take()?;
        if owner.terminal_enqueued {
            return None;
        }
        self.enqueue_owner_frame(&key, frame, QueuedKind::Terminal)
    }

    /// End a route capture that will never be served: the session is shutting
    /// down, its worker is lost, or its control link ended.
    ///
    /// PROCESS_EXIT never reaches the route ahead of its owed snapshot, and
    /// the snapshot is never lost silently. The held exit is released only
    /// when `finished` names the capture whose FINISH is queued, that capture
    /// is still the route's current one (no overflow resync since), and the
    /// exit fits without an overflow that would drop the queued FINISH.
    /// Otherwise the route ends with ATTACH_STATE failed, its terminal frame,
    /// which an overflow preserves. An unbound route hard-stops (`Failed`).
    /// Returns the teardown when the route hard-stopped.
    pub fn end_unserved_capture(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        finished: Option<CaptureIdentity>,
    ) -> Option<ClientWorkerTeardown> {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        let owner = self.live.get_mut(&key)?;
        owner.capture_open = false;
        let exit = owner.deferred_exit.take();
        if owner.terminal_enqueued {
            return None;
        }
        if owner.adapter.is_none() && !owner.hold_until_bound {
            return self.hard_stop_key(&key, TerminalRouteCloseReason::Failed);
        }
        match exit {
            Some(exit) => self.release_after_unserved_finish(&key, exit, finished),
            None if finished.is_some() => {
                // FINISH is queued and the exit has not arrived: it is
                // released later under the same rule.
                owner.unserved_finish = finished;
                None
            }
            None => self.fail_unserved_route(&key),
        }
    }

    /// Release `exit` after an unserved capture's FINISH only while that
    /// FINISH is intact (same generation and capture fence, no pending
    /// resync) and the exit fits without an overflow that would drop it.
    /// Otherwise end the route with the typed ATTACH_STATE failed.
    fn release_after_unserved_finish(
        &mut self,
        key: &OwnerKey,
        exit: TerminalFrame,
        finished: Option<CaptureIdentity>,
    ) -> Option<ClientWorkerTeardown> {
        let owner = self.live.get(key)?;
        if owner.terminal_enqueued {
            return None;
        }
        let intact = finished.is_some_and(|identity| {
            owner.generation == identity.generation
                && owner.capture_fence == identity.capture_fence
                && !owner.awaiting_capture
        });
        let fits = owner.queue.len() < MAX_ROUTE_EGRESS_FRAMES
            && owner.queued_bytes.saturating_add(exit.len()) <= MAX_ROUTE_EGRESS_BYTES;
        if intact && fits {
            return self.enqueue_owner_frame(key, exit, QueuedKind::Terminal);
        }
        self.fail_unserved_route(key)
    }

    /// End a route whose owed snapshot cannot be completed: ATTACH_STATE
    /// failed, a Terminal frame that an overflow preserves.
    fn fail_unserved_route(&mut self, key: &OwnerKey) -> Option<ClientWorkerTeardown> {
        match encode_attach_state(AttachStateCode::Failed) {
            Ok(frame) => self.enqueue_owner_frame(key, frame, QueuedKind::Terminal),
            Err(_) => self.hard_stop_key(key, TerminalRouteCloseReason::Failed),
        }
    }

    /// Take routes whose egress overflowed and now need a fresh capture.
    #[must_use]
    pub fn take_resync_requests(&mut self) -> Vec<RouteResyncRequest> {
        std::mem::take(&mut self.resync_requests)
    }

    /// Queue one resync request directly. Crate tests use it to model an
    /// overflow that happened before a later route event.
    #[cfg(test)]
    pub(crate) fn queue_resync_request(&mut self, request: RouteResyncRequest) {
        self.resync_requests.push(request);
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
            if matches!(kind, QueuedKind::InputResult(usage) if usage.operations == 0) {
                let queued_rejections = owner
                    .queue
                    .iter()
                    .filter(|queued| {
                        matches!(queued.kind, QueuedKind::InputResult(usage) if usage.operations == 0)
                    })
                    .count();
                if queued_rejections >= MAX_QUEUED_REJECTIONS_PER_ROUTE {
                    // Rejection traffic has no lane reservation; end the
                    // route explicitly instead of retaining it unbounded.
                    return self.hard_stop_key(key, TerminalRouteCloseReason::Overflowed);
                }
            }
            owner.queue.len() >= MAX_ROUTE_EGRESS_FRAMES
                || owner.queued_bytes.saturating_add(frame.len()) > MAX_ROUTE_EGRESS_BYTES
        };
        if over {
            return self.overflow_route(key, Some((frame, kind)));
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

    /// Recover one overflowing route.
    ///
    /// Order of preference: the in-flight head is untouched; unsent visual
    /// frames are dropped; terminal frames, input results, and an unsent
    /// transition are preserved. When a visual frame was lost the route needs
    /// a transition: an unsent transition already in the queue is kept (it
    /// still names the receiver's epoch), otherwise the route enters the next
    /// epoch and queues `ROUTE_RESYNC` first. Preserved results are re-stamped
    /// with the route epoch. If the preserved frames still exceed the ceiling
    /// the route ends explicitly. A stall resync has no overflowing frame.
    fn overflow_route(
        &mut self,
        key: &OwnerKey,
        overflowing: Option<(TerminalFrame, QueuedKind)>,
    ) -> Option<ClientWorkerTeardown> {
        // A declared route that has never bound cannot drain, so resync
        // recovery would only start another capture it cannot receive. Its
        // first overflow ends it explicitly; bound routes recover by resync.
        if self
            .live
            .get(key)
            .is_some_and(|owner| owner.adapter.is_none() && owner.hold_until_bound)
        {
            return self.hard_stop_key(key, TerminalRouteCloseReason::Overflowed);
        }
        let mut lost_visual = matches!(overflowing, Some((_, QueuedKind::Visual)));
        let (needs_transition, epoch_exhausted, ready) = {
            let owner = self.live.get_mut(key)?;
            let keep = usize::from(owner.in_flight);
            let mut kept: VecDeque<QueuedFrame> = VecDeque::new();
            let mut unsent_transition: Option<QueuedFrame> = None;
            for queued in owner.queue.drain(keep..) {
                match queued.kind {
                    QueuedKind::Visual => lost_visual = true,
                    QueuedKind::Resync => unsent_transition = Some(queued),
                    QueuedKind::Terminal | QueuedKind::InputResult(_) => kept.push_back(queued),
                }
            }
            let terminal_incoming = matches!(overflowing, Some((_, QueuedKind::Terminal)));
            if let Some((frame, kind)) = overflowing.filter(|(_, kind)| *kind != QueuedKind::Visual)
            {
                kept.push_back(QueuedFrame {
                    frame,
                    kind,
                    stream_epoch: owner.stream_epoch,
                });
                if terminal_incoming {
                    owner.terminal_enqueued = true;
                }
            }
            // A terminal frame ends the route; no capture can follow it.
            let needs_transition = lost_visual && !terminal_incoming && !owner.terminal_enqueued;
            let mut epoch_exhausted = false;
            if let Some(transition) = unsent_transition {
                // The receiver still sits at the epoch this transition leaves.
                kept.push_front(transition);
            } else if needs_transition {
                match owner.stream_epoch.checked_add(1) {
                    Some(to_epoch) => match encode_route_resync(owner.stream_epoch, to_epoch) {
                        Ok(frame) => {
                            owner.stream_epoch = to_epoch;
                            kept.push_front(QueuedFrame {
                                frame,
                                kind: QueuedKind::Resync,
                                stream_epoch: to_epoch,
                            });
                        }
                        Err(_) => epoch_exhausted = true,
                    },
                    None => epoch_exhausted = true,
                }
            }
            for queued in &mut kept {
                if matches!(queued.kind, QueuedKind::InputResult(_)) {
                    queued.stream_epoch = owner.stream_epoch;
                }
            }
            owner.queue.append(&mut kept);
            owner.queued_bytes = owner.queue.iter().map(|queued| queued.frame.len()).sum();
            if needs_transition {
                owner.awaiting_capture = true;
                owner.capture_fence = owner.capture_fence.wrapping_add(1);
            }
            let ready = Self::owner_ready_for_bound_queue_wake(owner);
            (needs_transition, epoch_exhausted, ready)
        };
        if epoch_exhausted {
            return self.fail_route(&key.session_id, &key.subscription_id);
        }
        let still_over = self.live.get(key).is_some_and(|owner| {
            owner.queue.len() > MAX_ROUTE_EGRESS_FRAMES
                || owner.queued_bytes > MAX_ROUTE_EGRESS_BYTES
        });
        if still_over {
            // Bounded retention is impossible: end the route explicitly.
            return self.hard_stop_key(key, TerminalRouteCloseReason::Overflowed);
        }
        if ready {
            self.bound_queue_wake_sessions
                .insert(key.session_id.clone());
        }
        if needs_transition {
            let owner = self.live.get(key)?;
            let request = RouteResyncRequest {
                client_id: owner.client_id.clone(),
                session_id: key.session_id.clone(),
                subscription_id: key.subscription_id.clone(),
                generation: owner.generation,
                stream_epoch: owner.stream_epoch,
                capture_fence: owner.capture_fence,
            };
            self.resync_requests.retain(|pending| {
                pending.session_id != request.session_id
                    || pending.subscription_id != request.subscription_id
            });
            self.resync_requests.push(request);
        }
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
                keys.push((key, PumpOrigin::AdapterWake));
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
                    keys.push((key, PumpOrigin::SessionOutput));
                }
            }
        }
        for (key, origin) in keys {
            if let Some(teardown) = self.pump_one(&key, origin) {
                teardowns.push(teardown);
            }
        }
        teardowns
    }

    /// Pump one route. Only the adapter's own wake counts an attempt that
    /// finds it busy: output wakes follow the producer's rate, not the
    /// reader's, so a session-output pump writes only into a ready adapter.
    fn pump_one(&mut self, key: &OwnerKey, origin: PumpOrigin) -> Option<ClientWorkerTeardown> {
        loop {
            let owner = self.live.get_mut(key)?;
            let adapter = owner.adapter.as_mut()?;
            if adapter.pressure() == TerminalAdapterPressure::Closed {
                return self.hard_stop_key(key, TerminalRouteCloseReason::AdapterClosed);
            }
            if owner.in_flight {
                match adapter.pressure() {
                    TerminalAdapterPressure::Ready => {
                        self.complete_head(key);
                        continue;
                    }
                    TerminalAdapterPressure::Closed => {
                        return self.hard_stop_key(key, TerminalRouteCloseReason::AdapterClosed)
                    }
                    TerminalAdapterPressure::Full | TerminalAdapterPressure::WouldBlock => {
                        return self.head_refused(key, origin);
                    }
                }
            }
            if owner.terminal_delivered {
                return self.hard_stop_key(key, TerminalRouteCloseReason::TerminalDelivered);
            }
            let Some(head) = owner.queue.front() else {
                // Nothing is pending, so the reader is not blocked.
                owner.blocked_since = None;
                return None;
            };
            let routed = RoutedTerminalFrame {
                route: owner.route.clone(),
                generation: owner.generation.0,
                stream_epoch: head.stream_epoch,
                frame: head.frame.clone(),
            };
            let adapter = owner.adapter.as_mut()?;
            if origin == PumpOrigin::SessionOutput
                && adapter.pressure() != TerminalAdapterPressure::Ready
            {
                // The adapter's writable wake retries this head.
                return self.head_refused(key, origin);
            }
            match adapter.try_write(&routed) {
                Ok(()) => {
                    owner.in_flight = true;
                    owner.unsuccessful_writes = 0;
                    owner.stall_resynced = false;
                    if adapter.pressure() == TerminalAdapterPressure::Ready {
                        self.complete_head(key);
                        continue;
                    }
                    // Acceptance is not progress: the frame must complete.
                    // Arm the deadline so a transport that holds the frame
                    // forever, with no further wake, still ends the route.
                    owner.blocked_since.get_or_insert_with(Instant::now);
                    return None;
                }
                Err(TerminalAdapterWriteError::WouldBlock | TerminalAdapterWriteError::Full) => {
                    return self.head_refused(key, origin);
                }
                Err(TerminalAdapterWriteError::Closed) => {
                    return self.hard_stop_key(key, TerminalRouteCloseReason::AdapterClosed)
                }
            }
        }
    }

    /// The adapter did not accept the route's head.
    ///
    /// Starts the reader-progress deadline if it is not running, and ends
    /// the route when the reader has accepted nothing for the whole
    /// deadline. An adapter wake also counts one unsuccessful attempt toward
    /// the budget that guards against spurious-wake storms; a session-output
    /// pump does not.
    fn head_refused(&mut self, key: &OwnerKey, origin: PumpOrigin) -> Option<ClientWorkerTeardown> {
        let owner = self.live.get_mut(key)?;
        let since = *owner.blocked_since.get_or_insert_with(Instant::now);
        if since.elapsed() >= READER_PROGRESS_DEADLINE {
            return self.hard_stop_key(key, TerminalRouteCloseReason::Stalled);
        }
        if origin == PumpOrigin::SessionOutput {
            return None;
        }
        owner.unsuccessful_writes = owner.unsuccessful_writes.saturating_add(1);
        if owner.unsuccessful_writes >= WRITE_ATTEMPT_BUDGET {
            return self.stall_route(key);
        }
        None
    }

    /// The bound adapter refused writes for a full attempt budget.
    ///
    /// A reader that falls behind a sustained producer recovers like an
    /// overflow: unsent visual frames are dropped and the route resyncs to a
    /// fresh capture. A reader with no successful write across a full budget
    /// after that resync is dead, and the route ends.
    fn stall_route(&mut self, key: &OwnerKey) -> Option<ClientWorkerTeardown> {
        let owner = self.live.get_mut(key)?;
        if owner.stall_resynced {
            return self.hard_stop_key(key, TerminalRouteCloseReason::Stalled);
        }
        owner.stall_resynced = true;
        owner.unsuccessful_writes = 0;
        self.overflow_route(key, None)
    }

    /// The adapter finished the head frame. Input lane reservations held by
    /// a delivered result are released here, not at worker completion.
    fn complete_head(&mut self, key: &OwnerKey) {
        let Some(owner) = self.live.get_mut(key) else {
            return;
        };
        let mut released = None;
        if let Some(completed) = owner.queue.pop_front() {
            owner.queued_bytes = owner.queued_bytes.saturating_sub(completed.frame.len());
            match completed.kind {
                QueuedKind::Terminal => owner.terminal_delivered = true,
                QueuedKind::InputResult(usage) if usage.operations > 0 => {
                    owner.lane.operations = owner.lane.operations.saturating_sub(usage.operations);
                    owner.lane.bytes = owner.lane.bytes.saturating_sub(usage.bytes);
                    released = Some((owner.client_id.clone(), usage));
                }
                _ => {}
            }
        }
        owner.in_flight = false;
        owner.unsuccessful_writes = 0;
        owner.blocked_since = None;
        owner.stall_resynced = false;
        if let Some((client_id, usage)) = released {
            self.release_lane(&key.session_id, &client_id, usage);
        }
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
    pub(crate) fn hard_stop_owner(
        &mut self,
        key: &OwnerKey,
        reason: TerminalRouteCloseReason,
    ) -> Option<ClientWorkerTeardown> {
        self.hard_stop_key(key, reason)
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
            let mut ended = None;
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
                if let Err(route_ended) = self.intake_terminal_command(&key, command, body) {
                    fail = true;
                    ended = route_ended;
                    break;
                }
            }
            if fail {
                if let Some(teardown) = ended
                    .or_else(|| self.hard_stop_key(&key, TerminalRouteCloseReason::InputFailed))
                {
                    teardowns.push(teardown);
                }
            }
        }
        teardowns
    }

    /// Admit one decoded command. `Err` means the route must hard-stop;
    /// see [`RouteEnded`].
    fn intake_terminal_command(
        &mut self,
        key: &OwnerKey,
        command: TerminalInputCommand,
        body: Vec<u8>,
    ) -> Result<(), RouteEnded> {
        let operation_id = command_operation_id(&command);
        let continues_paste = matches!(
            command,
            TerminalInputCommand::PasteChunk { .. }
                | TerminalInputCommand::PasteCommit { .. }
                | TerminalInputCommand::PasteAbort { .. }
        );
        {
            let owner = self.live.get_mut(key).ok_or(None)?;
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
                let owner = self.live.get_mut(key).ok_or(None)?;
                if owner.paste.is_some() {
                    // The protocol allows one assembling paste per route.
                    const { assert!(MAX_ASSEMBLING_PASTES_PER_SUBSCRIPTION >= 1) };
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
                let owner = self.live.get_mut(key).ok_or(None)?;
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
                let owner = self.live.get_mut(key).ok_or(None)?;
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
                let owner = self.live.get_mut(key).ok_or(None)?;
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
                    let removed = owner.input_queue.remove(position).ok_or(None)?;
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
    ) -> Result<(), RouteEnded> {
        let client_id = self.live.get(key).ok_or(None)?.client_id.clone();
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
        let owner = self.live.get_mut(key).ok_or(None)?;
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
    ) -> Result<(), RouteEnded> {
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

    fn enqueue_result(
        &mut self,
        key: &OwnerKey,
        result: &InputResultBody,
    ) -> Result<(), RouteEnded> {
        self.enqueue_result_with_reservation(key, result, LaneUsage::default())
    }

    /// Enqueue a result that keeps `reservation` until the adapter delivers it.
    fn enqueue_result_with_reservation(
        &mut self,
        key: &OwnerKey,
        result: &InputResultBody,
        reservation: LaneUsage,
    ) -> Result<(), RouteEnded> {
        let frame = encode_input_result(result).map_err(|_| None)?;
        let Some(owner) = self.live.get(key) else {
            return Err(None);
        };
        if owner.adapter.is_none() && !owner.hold_until_bound {
            // Unbound owners never receive results; the reservation ends now.
            if reservation.operations > 0 {
                let client_id = owner.client_id.clone();
                if let Some(owner) = self.live.get_mut(key) {
                    owner.lane.operations =
                        owner.lane.operations.saturating_sub(reservation.operations);
                    owner.lane.bytes = owner.lane.bytes.saturating_sub(reservation.bytes);
                }
                self.release_lane(&key.session_id, &client_id, reservation);
            }
            return Ok(());
        }
        match self.enqueue_owner_frame(key, frame, QueuedKind::InputResult(reservation)) {
            None => Ok(()),
            Some(teardown) => Err(Some(teardown)),
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
            if let Err(ended) = self.reject(
                &key,
                operation_id,
                InputOutcome::RejectedProtocol,
                "paste assembly timed out",
            ) {
                if let Some(teardown) = ended
                    .or_else(|| self.hard_stop_key(&key, TerminalRouteCloseReason::InputFailed))
                {
                    teardowns.push(teardown);
                }
            }
        }
        teardowns
    }

    /// Earliest reader-progress deadline across bound routes.
    ///
    /// The host clamps its wait to this so a dead reader is closed without
    /// any other traffic.
    #[must_use]
    pub fn next_reader_deadline(&self) -> Option<Instant> {
        self.live
            .values()
            .filter_map(|owner| owner.blocked_since)
            .min()
            .map(|since| since + READER_PROGRESS_DEADLINE)
    }

    /// Exact live routes whose reader-progress deadline has passed.
    #[must_use]
    pub fn expired_reader_routes(&self, now: Instant) -> Vec<crate::TerminalWakeRoute> {
        let mut routes: Vec<_> = self
            .live
            .iter()
            .filter(|(_, owner)| {
                owner
                    .blocked_since
                    .is_some_and(|since| since + READER_PROGRESS_DEADLINE <= now)
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

    /// Start a route's reader-progress deadline at `since`, for host tests.
    #[cfg(test)]
    pub(crate) fn test_start_reader_block(
        &mut self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
        since: Instant,
    ) {
        let key = OwnerKey {
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
        };
        self.live.get_mut(&key).expect("live route").blocked_since = Some(since);
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
        if !self.live.contains_key(&operation.key) {
            // The route is gone; its owner lane was released at hard-stop.
            return None;
        }
        let mut result = result;
        result.operation_id = operation.operation_id;
        result.detail = bounded_detail(&result.detail);
        // The reservation moves from the in-flight map onto the queued result
        // and is released when the adapter completes the write.
        match self.enqueue_result_with_reservation(&operation.key, &result, usage) {
            Ok(()) => None,
            Err(ended) => ended.or_else(|| {
                self.hard_stop_key(&operation.key, TerminalRouteCloseReason::InputFailed)
            }),
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
                if let Err(ended) = self.reject(
                    &key,
                    input.operation_id,
                    InputOutcome::SessionEnded,
                    "session ended before the operation ran",
                ) {
                    if let Some(teardown) = ended.or_else(|| {
                        self.hard_stop_key(&key, TerminalRouteCloseReason::SessionEnded)
                    }) {
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
        self.hard_stop_key(
            &OwnerKey {
                session_id: session_id.clone(),
                subscription_id: subscription_id.clone(),
            },
            TerminalRouteCloseReason::Detached,
        )
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
                let _ = self.hard_stop_key(&key, TerminalRouteCloseReason::Detached);
                DetachTerminalSubscriptionResult::Detached { generation }
            }
        }
    }

    /// Ownership hard-stop for every live subscription on `session_id`.
    pub fn teardown_session(
        &mut self,
        session_id: &SessionId,
        reason: TerminalRouteCloseReason,
    ) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self
            .live
            .keys()
            .filter(|key| &key.session_id == session_id)
            .cloned()
            .collect();
        self.session_modes.remove(session_id);
        keys.into_iter()
            .filter_map(|key| self.hard_stop_key(&key, reason))
            .collect()
    }

    /// Ownership hard-stop for every remaining bound subscription.
    pub fn teardown_all(&mut self) -> Vec<ClientWorkerTeardown> {
        let keys: Vec<_> = self.live.keys().cloned().collect();
        keys.into_iter()
            .filter_map(|key| self.hard_stop_key(&key, TerminalRouteCloseReason::Shutdown))
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

    fn close(&mut self, reason: TerminalRouteCloseReason) {
        self.inner.close(reason);
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
    reason: TerminalRouteCloseReason,
) -> ClientWorkerTeardown {
    owner.queue.clear();
    owner.input_queue.clear();
    if let Some(mut adapter) = owner.adapter.take() {
        adapter.close(reason);
        drop(adapter);
    }
    ClientWorkerTeardown {
        client_id: owner.client_id,
        session_id: key.session_id.clone(),
        subscription_id: key.subscription_id.clone(),
        generation: owner.generation,
        in_flight_keys,
        reason,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::terminal_wake::TerminalWakeSink;
    use botster_terminal_protocol::{
        decode_input_result, encode_snapshot_history, encode_snapshot_ready,
    };

    #[test]
    fn terminal_inventory_size_overflow_is_explicit() {
        assert_eq!(
            inventory_add_bytes(usize::MAX, 1),
            Err(TerminalSubscriptionInventoryError::SizeOverflow)
        );
        assert_eq!(
            inventory_row_storage_bytes(usize::MAX),
            Err(TerminalSubscriptionInventoryError::SizeOverflow)
        );
        assert_eq!(inventory_add_bytes(usize::MAX - 1, 1), Ok(usize::MAX));
    }

    /// Adapter whose write slot never drains, so frames stay queued in Core.
    struct StuckAdapter;

    impl TerminalAdapter for StuckAdapter {
        fn try_write(
            &mut self,
            _frame: &RoutedTerminalFrame,
        ) -> Result<(), TerminalAdapterWriteError> {
            Err(TerminalAdapterWriteError::WouldBlock)
        }

        fn close(&mut self, _reason: TerminalRouteCloseReason) {}

        fn pressure(&self) -> TerminalAdapterPressure {
            TerminalAdapterPressure::WouldBlock
        }

        fn try_read(&mut self) -> TerminalIngress {
            TerminalIngress::Empty
        }
    }

    impl WakingTerminalAdapter for StuckAdapter {
        fn set_wake_sink(&mut self, _sink: TerminalWakeSink) {}
    }

    fn bound_route() -> (ClientWorker, OwnerKey) {
        let mut worker = ClientWorker::new();
        let client = ClientId("client".into());
        let session = SessionId("session".into());
        let subscription = SubscriptionId("route".into());
        let (generation, _) = worker
            .record_attach(client.clone(), session.clone(), subscription.clone())
            .expect("valid route");
        worker
            .bind_waking_terminal_adapter(
                &client,
                session.clone(),
                subscription.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(StuckAdapter),
            )
            .expect("bind");
        // The route has its capture; live output flows.
        worker
            .push_route_frame(
                &session,
                &subscription,
                encode_snapshot_ready(b"GHOSTSNP").expect("ready"),
            )
            .expect("ready enqueue");
        (
            worker,
            OwnerKey {
                session_id: session,
                subscription_id: subscription,
            },
        )
    }

    fn fill_route(worker: &mut ClientWorker, key: &OwnerKey) {
        while worker.live[key].queue.len() < MAX_ROUTE_EGRESS_FRAMES {
            let teardowns = worker.push_session_output(&key.session_id, b"x");
            assert!(teardowns.is_empty());
        }
    }

    fn kinds(worker: &ClientWorker, key: &OwnerKey) -> Vec<&'static str> {
        worker.live[key]
            .queue
            .iter()
            .map(|queued| match queued.kind {
                QueuedKind::Terminal => "terminal",
                QueuedKind::InputResult(_) => "result",
                QueuedKind::Resync => "resync",
                QueuedKind::Visual => "visual",
            })
            .collect()
    }

    fn input_results(worker: &ClientWorker, key: &OwnerKey) -> Vec<InputResultBody> {
        worker.live[key]
            .queue
            .iter()
            .filter(|queued| queued.frame.kind() == TerminalKind::InputResult)
            .map(|queued| decode_input_result(&queued.frame).expect("input result"))
            .collect()
    }

    fn assert_no_output(worker: &ClientWorker, key: &OwnerKey) {
        assert!(worker.live[key]
            .queue
            .iter()
            .all(|queued| queued.frame.kind() != TerminalKind::Output));
    }

    fn send_unmatched_paste_continuations(
        worker: &mut ClientWorker,
        key: &OwnerKey,
        known_operation_id: u64,
        unknown_operation_id: u64,
    ) {
        for command in [
            TerminalInputCommand::PasteChunk {
                operation_id: known_operation_id,
                index: 0,
                data: vec![b'x'],
            },
            TerminalInputCommand::PasteCommit {
                operation_id: known_operation_id,
            },
            TerminalInputCommand::PasteAbort {
                operation_id: unknown_operation_id,
            },
            TerminalInputCommand::PasteAbort {
                operation_id: known_operation_id,
            },
        ] {
            worker
                .intake_terminal_command(key, command, Vec::new())
                .expect("rejection queues");
        }
    }

    fn assert_unmatched_paste_results(
        results: &[InputResultBody],
        known_operation_id: u64,
        unknown_operation_id: u64,
    ) {
        assert_eq!(results.len(), 4, "one result per continuation");
        assert_eq!(
            results
                .iter()
                .map(|result| (result.operation_id, result.outcome, result.detail.as_str()))
                .collect::<Vec<_>>(),
            vec![
                (
                    known_operation_id,
                    InputOutcome::RejectedProtocol,
                    "unknown paste operation"
                ),
                (
                    known_operation_id,
                    InputOutcome::RejectedProtocol,
                    "unknown paste operation"
                ),
                (
                    unknown_operation_id,
                    InputOutcome::RejectedProtocol,
                    "unknown paste operation"
                ),
                (
                    known_operation_id,
                    InputOutcome::RejectedProtocol,
                    "abort names no active paste"
                ),
            ]
        );
    }

    #[test]
    fn unmatched_paste_continuations_without_an_active_paste_are_rejected() {
        let (mut worker, key) = bound_route();

        send_unmatched_paste_continuations(&mut worker, &key, 0, 1);

        assert_unmatched_paste_results(&input_results(&worker, &key), 0, 1);
        assert_no_output(&worker, &key);
    }

    #[test]
    fn unmatched_paste_continuations_after_a_completed_paste_are_rejected() {
        let (mut worker, key) = bound_route();
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 51,
                    total_len: 1,
                    allow_unsafe: false,
                },
                Vec::new(),
            )
            .expect("paste begins");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 51,
                    index: 0,
                    data: vec![b'x'],
                },
                Vec::new(),
            )
            .expect("paste chunk");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteCommit { operation_id: 51 },
                Vec::new(),
            )
            .expect("paste commit");
        let staged = worker
            .take_one_terminal_input(&key)
            .expect("committed paste stages");
        assert!(worker
            .complete_operation(
                staged.operation_key,
                InputResultBody {
                    operation_id: 0,
                    outcome: InputOutcome::Written,
                    accepted_payload_bytes: Some(1),
                    written_pty_bytes: Some(1),
                    mode_bits: 0,
                    detail: String::new(),
                },
            )
            .is_none());

        send_unmatched_paste_continuations(&mut worker, &key, 51, 52);

        let results = input_results(&worker, &key);
        assert_eq!(results[0].operation_id, 51);
        assert_eq!(results[0].outcome, InputOutcome::Written);
        assert_unmatched_paste_results(&results[1..], 51, 52);
        assert_no_output(&worker, &key);
    }

    #[test]
    fn unmatched_paste_continuations_preserve_a_different_active_paste() {
        let (mut worker, key) = bound_route();
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteBegin {
                    operation_id: 60,
                    total_len: 3,
                    allow_unsafe: false,
                },
                Vec::new(),
            )
            .expect("paste begins");
        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteChunk {
                    operation_id: 60,
                    index: 0,
                    data: b"abc".to_vec(),
                },
                Vec::new(),
            )
            .expect("paste chunk");

        send_unmatched_paste_continuations(&mut worker, &key, 51, 61);

        let paste = worker.live[&key].paste.as_ref().expect("active paste");
        assert_eq!(paste.operation_id, 60);
        assert_eq!(paste.next_index, 1);
        assert_eq!(paste.data, b"abc");
        assert_unmatched_paste_results(&input_results(&worker, &key), 51, 61);
        assert_no_output(&worker, &key);

        worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteCommit { operation_id: 60 },
                Vec::new(),
            )
            .expect("active paste commits");
        let staged = worker
            .take_one_terminal_input(&key)
            .expect("active paste stages");
        assert_eq!(staged.operation_id, 60);
        assert_eq!(staged.kind, WorkerInputKind::Paste);
        assert_eq!(staged.body, b"\0abc");
    }

    #[test]
    fn overflow_on_a_full_route_preserves_the_terminal_frame() {
        let (mut worker, key) = bound_route();
        fill_route(&mut worker, &key);

        let teardowns = worker.push_session_process_exit(&key.session_id, Some(0));

        assert!(teardowns.is_empty());
        let owner = &worker.live[&key];
        assert!(owner.terminal_enqueued);
        assert_eq!(kinds(&worker, &key), vec!["terminal"]);
        assert_eq!(owner.stream_epoch, 0, "a terminal frame needs no new epoch");
        assert!(worker.take_resync_requests().is_empty());
    }

    #[test]
    fn second_overflow_keeps_the_unsent_transition_from_the_receiver_epoch() {
        let (mut worker, key) = bound_route();
        fill_route(&mut worker, &key);
        assert!(worker
            .push_session_output(&key.session_id, b"overflow")
            .is_empty());
        assert_eq!(kinds(&worker, &key), vec!["resync"]);
        assert_eq!(worker.live[&key].stream_epoch, 1);
        // The route awaits its capture, so live output is suppressed; a
        // route-personal visual frame still queues and can overflow again.
        for _ in 0..MAX_ROUTE_EGRESS_FRAMES {
            let _ = worker.push_route_frame(
                &key.session_id,
                &key.subscription_id,
                encode_modes(ModesBody::default()).expect("modes"),
            );
        }

        let queued = kinds(&worker, &key);
        assert_eq!(queued.first(), Some(&"resync"));
        assert_eq!(queued.iter().filter(|kind| **kind == "resync").count(), 1);
        let transition = decode_route_resync_frame(&worker.live[&key].queue[0].frame);
        assert_eq!((transition.from_epoch, transition.to_epoch), (0, 1));
        assert_eq!(worker.live[&key].stream_epoch, 1);
        let requests = worker.take_resync_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].stream_epoch, 1);
    }

    use botster_terminal_protocol::{decode_attach_state, encode_snapshot_finish};

    fn queued_frame_kinds(worker: &ClientWorker, key: &OwnerKey) -> Vec<TerminalKind> {
        worker.live[key]
            .queue
            .iter()
            .map(|queued| queued.frame.kind())
            .collect()
    }

    fn queued_attach_failed(worker: &ClientWorker, key: &OwnerKey) -> bool {
        worker.live[key].queue.iter().any(|queued| {
            queued.kind == QueuedKind::Terminal
                && queued.frame.kind() == TerminalKind::AttachState
                && decode_attach_state(&queued.frame).expect("attach state")
                    == AttachStateCode::Failed
        })
    }

    /// Open the route's capture, queue its FINISH, and hold PROCESS_EXIT.
    fn finished_capture_with_held_exit(
        worker: &mut ClientWorker,
        key: &OwnerKey,
    ) -> CaptureIdentity {
        let identity = worker
            .capture_identity(&key.session_id, &key.subscription_id)
            .expect("capture identity");
        worker.open_route_capture(&key.session_id, &key.subscription_id);
        assert!(worker
            .push_capture_frame(
                &key.session_id,
                &key.subscription_id,
                identity,
                encode_snapshot_finish().expect("finish"),
            )
            .expect("finish enqueue")
            .is_none());
        assert!(worker
            .push_session_process_exit(&key.session_id, Some(0))
            .is_empty());
        identity
    }

    /// An unserved capture whose FINISH is still queued intact releases the
    /// held exit after it.
    #[test]
    fn an_unserved_capture_with_its_finish_intact_releases_the_exit_after_it() {
        let (mut worker, key) = bound_route();
        let identity = finished_capture_with_held_exit(&mut worker, &key);

        assert!(worker
            .end_unserved_capture(&key.session_id, &key.subscription_id, Some(identity))
            .is_none());
        let kinds = queued_frame_kinds(&worker, &key);
        let finish = kinds
            .iter()
            .position(|kind| *kind == TerminalKind::SnapshotFinish);
        let exit = kinds
            .iter()
            .position(|kind| *kind == TerminalKind::ProcessExit);
        assert!(
            finish.zip(exit).is_some_and(|(finish, exit)| finish < exit),
            "the exit follows the intact FINISH: {kinds:?}"
        );
    }

    /// On a full route the released exit would overflow and drop the queued
    /// FINISH, so the route ends with the typed ATTACH_STATE failed, which
    /// the overflow preserves, and no bare exit.
    #[test]
    fn a_full_route_ends_an_unserved_capture_typed_not_with_a_bare_exit() {
        let (mut worker, key) = bound_route();
        let identity = finished_capture_with_held_exit(&mut worker, &key);
        fill_route(&mut worker, &key);

        let _ = worker.end_unserved_capture(&key.session_id, &key.subscription_id, Some(identity));
        let kinds = queued_frame_kinds(&worker, &key);
        assert!(
            queued_attach_failed(&worker, &key),
            "the route ends with ATTACH_STATE failed: {kinds:?}"
        );
        assert!(
            !kinds.contains(&TerminalKind::ProcessExit),
            "no exit without its snapshot: {kinds:?}"
        );
    }

    /// A capture superseded by an overflow resync no longer owns the route's
    /// stream: the route ends with the typed ATTACH_STATE failed.
    #[test]
    fn a_superseded_unserved_capture_ends_typed_not_with_a_bare_exit() {
        let (mut worker, key) = bound_route();
        let identity = finished_capture_with_held_exit(&mut worker, &key);
        fill_route(&mut worker, &key);
        // One more frame overflows: visual frames drop, and the route enters
        // the next epoch with a new capture fence.
        let _ = worker.push_session_output(&key.session_id, b"x");
        assert_ne!(
            worker.live[&key].capture_fence, identity.capture_fence,
            "the overflow superseded the capture"
        );

        let _ = worker.end_unserved_capture(&key.session_id, &key.subscription_id, Some(identity));
        let kinds = queued_frame_kinds(&worker, &key);
        assert!(
            queued_attach_failed(&worker, &key),
            "the route ends with ATTACH_STATE failed: {kinds:?}"
        );
        assert!(
            !kinds.contains(&TerminalKind::ProcessExit),
            "no exit without its snapshot: {kinds:?}"
        );
    }

    /// Shutdown ends an unserved capture before the exit exists. The exit
    /// that arrives later on a full route would overflow and drop the
    /// queued FINISH, so the route ends typed instead of with a bare exit.
    #[test]
    fn a_later_exit_on_a_full_route_after_an_unserved_finish_ends_typed() {
        let (mut worker, key) = bound_route();
        let identity = worker
            .capture_identity(&key.session_id, &key.subscription_id)
            .expect("capture identity");
        worker.open_route_capture(&key.session_id, &key.subscription_id);
        assert!(worker
            .push_capture_frame(
                &key.session_id,
                &key.subscription_id,
                identity,
                encode_snapshot_finish().expect("finish"),
            )
            .expect("finish enqueue")
            .is_none());
        fill_route(&mut worker, &key);
        // No exit is held yet: shutdown ends the capture first.
        assert!(worker
            .end_unserved_capture(&key.session_id, &key.subscription_id, Some(identity))
            .is_none());

        let _ = worker.push_session_process_exit(&key.session_id, Some(0));
        let kinds = queued_frame_kinds(&worker, &key);
        assert!(
            queued_attach_failed(&worker, &key),
            "the route ends with ATTACH_STATE failed: {kinds:?}"
        );
        assert!(
            !kinds.contains(&TerminalKind::ProcessExit),
            "no exit that overtook its dropped FINISH: {kinds:?}"
        );
    }

    /// The same later exit on a route with room follows the intact FINISH.
    #[test]
    fn a_later_exit_after_an_intact_unserved_finish_follows_it() {
        let (mut worker, key) = bound_route();
        let identity = worker
            .capture_identity(&key.session_id, &key.subscription_id)
            .expect("capture identity");
        worker.open_route_capture(&key.session_id, &key.subscription_id);
        assert!(worker
            .push_capture_frame(
                &key.session_id,
                &key.subscription_id,
                identity,
                encode_snapshot_finish().expect("finish"),
            )
            .expect("finish enqueue")
            .is_none());
        assert!(worker
            .end_unserved_capture(&key.session_id, &key.subscription_id, Some(identity))
            .is_none());

        assert!(worker
            .push_session_process_exit(&key.session_id, Some(0))
            .is_empty());
        let kinds = queued_frame_kinds(&worker, &key);
        let finish = kinds
            .iter()
            .position(|kind| *kind == TerminalKind::SnapshotFinish);
        let exit = kinds
            .iter()
            .position(|kind| *kind == TerminalKind::ProcessExit);
        assert!(
            finish.zip(exit).is_some_and(|(finish, exit)| finish < exit),
            "the later exit follows the intact FINISH: {kinds:?}"
        );
    }

    /// A PROCESS_EXIT held for an open capture is queued when the capture
    /// ends. On a held route filled to its ceiling, that overflow fails the
    /// route, and end_route_capture returns the teardown for the caller.
    #[test]
    fn a_released_exit_that_overflows_a_held_route_returns_its_teardown() {
        let mut worker = ClientWorker::new();
        let client = ClientId("client".into());
        let session = SessionId("session".into());
        let subscription = SubscriptionId("route".into());
        worker.expect_terminal_adapter(client.clone(), session.clone(), subscription.clone());
        let _ = worker
            .record_attach(client, session.clone(), subscription.clone())
            .expect("declared attach");
        worker.open_route_capture(&session, &subscription);
        assert!(worker
            .push_session_process_exit(&session, Some(0))
            .is_empty());
        for _ in 0..MAX_ROUTE_EGRESS_FRAMES {
            assert!(worker
                .push_route_frame(
                    &session,
                    &subscription,
                    encode_modes(ModesBody::default()).expect("modes"),
                )
                .expect("hold accepts frames")
                .is_none());
        }

        let teardown = worker.end_route_capture(&session, &subscription);
        assert!(
            teardown.is_some(),
            "the overflowing released exit fails the held route"
        );
    }

    #[test]
    fn a_declared_route_that_never_bound_fails_on_its_first_overflow() {
        let mut worker = ClientWorker::new();
        let client = ClientId("client".into());
        let session = SessionId("session".into());
        let subscription = SubscriptionId("route".into());
        worker.expect_terminal_adapter(client.clone(), session.clone(), subscription.clone());
        let (generation, _) = worker
            .record_attach(client.clone(), session.clone(), subscription.clone())
            .expect("declared attach");
        let key = OwnerKey {
            session_id: session.clone(),
            subscription_id: subscription.clone(),
        };
        assert!(worker.live[&key].hold_until_bound);
        // Route-personal frames queue on the pre-bind hold.
        for _ in 0..MAX_ROUTE_EGRESS_FRAMES {
            let teardown = worker
                .push_route_frame(
                    &session,
                    &subscription,
                    encode_modes(ModesBody::default()).expect("modes"),
                )
                .expect("hold accepts frames");
            assert!(teardown.is_none());
        }

        let teardown = worker
            .push_route_frame(
                &session,
                &subscription,
                encode_modes(ModesBody::default()).expect("modes"),
            )
            .expect("overflowing frame is accepted for the decision");

        let teardown = teardown.expect("first overflow ends the unbound route");
        assert_eq!(teardown.generation, generation);
        assert_eq!(teardown.subscription_id, subscription);
        assert!(!worker.has_subscription(&session, &subscription));
        assert!(
            worker.take_resync_requests().is_empty(),
            "no resync capture may be requested for a route that cannot receive it"
        );
        assert!(
            matches!(
                worker.bind_waking_terminal_adapter(
                    &client,
                    session,
                    subscription,
                    generation,
                    TerminalCapabilitySet::empty(),
                    Box::new(StuckAdapter),
                ),
                Err(BindTerminalAdapterError::UnknownSubscription { .. })
            ),
            "a late bind is refused with the public failure signal"
        );
    }

    #[test]
    fn a_route_that_ends_takes_its_pending_resync_request_with_it() {
        let (mut worker, key) = bound_route();
        fill_route(&mut worker, &key);
        assert!(worker
            .push_session_output(&key.session_id, b"overflow")
            .is_empty());
        assert_eq!(worker.resync_requests.len(), 1);

        let teardown = worker.detach_live(&key.session_id, &key.subscription_id);

        assert!(teardown.is_some());
        assert!(
            worker.take_resync_requests().is_empty(),
            "an ended route must not ask the engine for a capture"
        );
    }

    #[test]
    fn a_page_from_a_replaced_attachment_is_refused_even_with_an_equal_fence() {
        let (mut worker, key) = bound_route();
        let old = worker
            .capture_identity(&key.session_id, &key.subscription_id)
            .expect("live identity");
        // Another client takes the same subscription: a new attachment with
        // a new generation and a fresh fence of zero.
        let (generation, _) = worker
            .record_attach(
                ClientId("other".into()),
                key.session_id.clone(),
                key.subscription_id.clone(),
            )
            .expect("replacement attach");
        assert_ne!(generation, old.generation);
        assert_eq!(
            worker.capture_identity(&key.session_id, &key.subscription_id),
            Some(CaptureIdentity {
                generation,
                capture_fence: 0
            })
        );

        let refused = worker.push_capture_frame(
            &key.session_id,
            &key.subscription_id,
            old,
            encode_snapshot_ready(b"GHOSTSNP").expect("ready"),
        );

        assert!(matches!(
            refused,
            Err(EnqueueRouteFrameError::EpochSuperseded)
        ));
        assert!(!worker.capture_identity_is_live(&key.session_id, &key.subscription_id, old));
    }

    #[test]
    fn a_page_from_a_superseded_capture_is_refused() {
        let (mut worker, key) = bound_route();
        fill_route(&mut worker, &key);
        assert!(worker
            .push_session_output(&key.session_id, b"overflow")
            .is_empty());

        let refused = worker.push_capture_frame(
            &key.session_id,
            &key.subscription_id,
            CaptureIdentity {
                generation: worker.live[&key].generation,
                capture_fence: 0,
            },
            encode_snapshot_history(b"stale page").expect("page"),
        );

        assert!(matches!(
            refused,
            Err(EnqueueRouteFrameError::EpochSuperseded)
        ));
        assert!(!kinds(&worker, &key).contains(&"visual"));
    }

    #[test]
    fn queued_rejections_are_capped_and_end_the_route_explicitly() {
        let (mut worker, key) = bound_route();
        for operation_id in 1..=MAX_QUEUED_REJECTIONS_PER_ROUTE as u64 {
            worker
                .reject(&key, operation_id, InputOutcome::RejectedProtocol, "")
                .expect("rejection fits");
        }

        let overflowed = worker.reject(&key, 99, InputOutcome::RejectedProtocol, "");

        assert!(overflowed.is_err());
        assert!(!worker.live.contains_key(&key), "route hard-stopped");
    }

    #[test]
    fn a_delivered_result_releases_its_lane_reservation() {
        let (mut worker, key) = bound_route();
        let usage = LaneUsage {
            operations: 1,
            bytes: 7,
        };
        let client = worker.live[&key].client_id.clone();
        worker.reserve_lane(&key.session_id, &client, usage);
        worker
            .live
            .get_mut(&key)
            .expect("find the bound route to reserve its lane")
            .lane = usage;
        worker
            .enqueue_result_with_reservation(
                &key,
                &InputResultBody {
                    operation_id: 1,
                    outcome: InputOutcome::Written,
                    accepted_payload_bytes: Some(7),
                    written_pty_bytes: Some(7),
                    mode_bits: 0,
                    detail: String::new(),
                },
                usage,
            )
            .expect("queued");
        assert_eq!(worker.session_in_flight_operations(&key.session_id), 1);

        // Deliver the READY frame, then the result.
        worker
            .live
            .get_mut(&key)
            .expect("find the bound route before READY completion")
            .in_flight = true;
        worker.complete_head(&key);
        worker
            .live
            .get_mut(&key)
            .expect("find the bound route before result completion")
            .in_flight = true;
        worker.complete_head(&key);

        assert_eq!(worker.session_in_flight_operations(&key.session_id), 0);
        assert_eq!(worker.live[&key].lane.operations, 0);
    }

    #[test]
    fn a_route_bound_after_its_unbound_capture_receives_live_output() {
        let mut worker = ClientWorker::new();
        let client = ClientId("client".into());
        let session = SessionId("session".into());
        let subscription = SubscriptionId("route".into());
        let (generation, _) = worker
            .record_attach(client.clone(), session.clone(), subscription.clone())
            .expect("valid route");
        // The drain path serves the capture while no adapter is bound.
        let teardown = worker
            .push_route_frame(
                &session,
                &subscription,
                encode_snapshot_ready(b"GHOSTSNP").expect("ready"),
            )
            .expect("unbound ready");
        assert!(teardown.is_none());
        worker
            .bind_waking_terminal_adapter(
                &client,
                session.clone(),
                subscription.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(StuckAdapter),
            )
            .expect("late bind");

        assert!(worker.push_session_output(&session, b"live").is_empty());

        let key = OwnerKey {
            session_id: session,
            subscription_id: subscription,
        };
        assert_eq!(kinds(&worker, &key), vec!["visual"]);
        assert_eq!(
            worker.live[&key].queue[0].frame.kind(),
            TerminalKind::Output
        );
    }

    /// Adapter that accepts one write per granted credit and records the
    /// kind of every accepted frame.
    #[derive(Default)]
    struct MeteredState {
        credits: usize,
        written: Vec<TerminalKind>,
    }

    struct MeteredAdapter(std::sync::Arc<std::sync::Mutex<MeteredState>>);

    impl TerminalAdapter for MeteredAdapter {
        fn try_write(
            &mut self,
            frame: &RoutedTerminalFrame,
        ) -> Result<(), TerminalAdapterWriteError> {
            let mut state = self.0.lock().expect("adapter state");
            if state.credits == 0 {
                return Err(TerminalAdapterWriteError::Full);
            }
            state.credits -= 1;
            state.written.push(frame.frame.kind());
            Ok(())
        }

        fn close(&mut self, _reason: TerminalRouteCloseReason) {}

        fn pressure(&self) -> TerminalAdapterPressure {
            if self.0.lock().expect("adapter state").credits > 0 {
                TerminalAdapterPressure::Ready
            } else {
                TerminalAdapterPressure::Full
            }
        }

        fn try_read(&mut self) -> TerminalIngress {
            TerminalIngress::Empty
        }
    }

    impl WakingTerminalAdapter for MeteredAdapter {
        fn set_wake_sink(&mut self, _sink: TerminalWakeSink) {}
    }

    fn metered_route() -> (
        ClientWorker,
        OwnerKey,
        std::sync::Arc<std::sync::Mutex<MeteredState>>,
    ) {
        let (mut worker, key) = bound_route();
        let state = std::sync::Arc::new(std::sync::Mutex::new(MeteredState::default()));
        let owner = worker.live.get_mut(&key).expect("route");
        owner.adapter = Some(Box::new(MeteredAdapter(state.clone())));
        (worker, key, state)
    }

    /// Pump one route until one attempt short of the write budget.
    fn exhaust_all_but_one(worker: &mut ClientWorker, key: &OwnerKey) {
        for _ in 1..WRITE_ATTEMPT_BUDGET {
            assert!(worker.pump_one(key, PumpOrigin::AdapterWake).is_none());
        }
        assert_eq!(
            worker.live[key].unsuccessful_writes,
            WRITE_ATTEMPT_BUDGET - 1
        );
    }

    struct SlowReaderRun {
        stall_resyncs: usize,
        capture_requests: usize,
        written: Vec<TerminalKind>,
    }

    /// Drive one bound route with a producer and a reader that drains a
    /// burst of frames once per period. The period is longer than the write
    /// budget, so the budget runs out between drains. The host serves every
    /// resync request with a fresh capture.
    fn run_slow_reader(output_every: usize, burst: usize, periods: usize) -> SlowReaderRun {
        let (mut worker, key, adapter) = metered_route();
        let drain_period = WRITE_ATTEMPT_BUDGET + WRITE_ATTEMPT_BUDGET / 4;
        let mut run = SlowReaderRun {
            stall_resyncs: 0,
            capture_requests: 0,
            written: Vec::new(),
        };
        for tick in 1..=drain_period * periods {
            if tick % output_every == 0 {
                assert!(
                    worker
                        .push_session_output(&key.session_id, b"y\n")
                        .is_empty(),
                    "producer output must not end the route"
                );
            }
            for request in worker.take_resync_requests() {
                run.capture_requests += 1;
                let identity = CaptureIdentity {
                    generation: request.generation,
                    capture_fence: request.capture_fence,
                };
                for frame in [
                    encode_snapshot_history(b"page").expect("page"),
                    encode_snapshot_ready(b"GHOSTSNP").expect("ready"),
                ] {
                    let teardown = worker
                        .push_capture_frame(&key.session_id, &key.subscription_id, identity, frame)
                        .expect("capture page for the live fence");
                    assert!(teardown.is_none());
                }
            }
            let drain = tick % drain_period == 0;
            if drain {
                adapter.lock().expect("adapter state").credits = burst;
            }
            let before = worker.live[&key].unsuccessful_writes;
            assert!(
                worker.pump_one(&key, PumpOrigin::AdapterWake).is_none(),
                "a reader that keeps writing must never be stopped"
            );
            if drain {
                adapter.lock().expect("adapter state").credits = 0;
                assert!(
                    !worker.live[&key].stall_resynced,
                    "a successful write clears the stall"
                );
            }
            let owner = &worker.live[&key];
            if before == WRITE_ATTEMPT_BUDGET - 1 {
                assert_eq!(
                    owner.unsuccessful_writes, 0,
                    "the stall resync resets the counter"
                );
                assert!(owner.stall_resynced);
                run.stall_resyncs += 1;
            }
        }
        assert!(worker.has_subscription(&key.session_id, &key.subscription_id));
        run.written = adapter.lock().expect("adapter state").written.clone();
        run
    }

    fn count_kind(written: &[TerminalKind], kind: TerminalKind) -> usize {
        written.iter().filter(|written| **written == kind).count()
    }

    #[test]
    fn a_slow_steady_reader_resyncs_on_each_stall_and_receives_each_snapshot() {
        // The producer stays under the frame ceiling, so every resync is a
        // stall resync, never an overflow resync.
        let periods = 8;
        let run = run_slow_reader(16, MAX_ROUTE_EGRESS_FRAMES, periods);

        assert_eq!(run.stall_resyncs, periods);
        assert_eq!(run.capture_requests, run.stall_resyncs);
        assert_eq!(count_kind(&run.written, TerminalKind::RouteResync), periods);
        assert_eq!(
            count_kind(&run.written, TerminalKind::SnapshotReady),
            periods
        );
        assert!(count_kind(&run.written, TerminalKind::Output) > 0);
    }

    #[test]
    fn a_slow_steady_reader_under_a_continuous_producer_never_stops() {
        let run = run_slow_reader(1, 8, 8);

        assert!(run.stall_resyncs >= 2, "stalls: {}", run.stall_resyncs);
        assert!(count_kind(&run.written, TerminalKind::RouteResync) > 0);
        assert!(count_kind(&run.written, TerminalKind::SnapshotReady) > 0);
    }

    #[test]
    fn a_stall_resync_drops_visual_frames_and_asks_for_a_fresh_capture() {
        let (mut worker, key) = bound_route();
        for _ in 0..3 {
            assert!(worker.push_session_output(&key.session_id, b"x").is_empty());
        }
        exhaust_all_but_one(&mut worker, &key);
        let fence = worker.live[&key].capture_fence;

        assert!(
            worker.pump_one(&key, PumpOrigin::AdapterWake).is_none(),
            "the first stall resyncs"
        );

        let owner = &worker.live[&key];
        assert_eq!(kinds(&worker, &key), vec!["resync"]);
        assert_eq!(owner.stream_epoch, 1);
        assert_eq!(owner.capture_fence, fence + 1);
        assert!(owner.awaiting_capture);
        assert_eq!(owner.unsuccessful_writes, 0);
        let requests = worker.take_resync_requests();
        assert_eq!(requests.len(), 1);
        assert_eq!(requests[0].stream_epoch, 1);
        assert_eq!(requests[0].capture_fence, fence + 1);
    }

    #[test]
    fn a_reader_with_no_write_after_a_stall_resync_is_stopped_at_the_next_budget() {
        let (mut worker, key) = bound_route();
        assert!(worker.push_session_output(&key.session_id, b"x").is_empty());
        exhaust_all_but_one(&mut worker, &key);
        assert!(
            worker.pump_one(&key, PumpOrigin::AdapterWake).is_none(),
            "the first stall resyncs"
        );
        exhaust_all_but_one(&mut worker, &key);

        let teardown = worker
            .pump_one(&key, PumpOrigin::AdapterWake)
            .expect("no write across a full budget after the resync ends the route");

        assert_eq!(teardown.subscription_id, key.subscription_id);
        assert!(!worker.has_subscription(&key.session_id, &key.subscription_id));
        assert!(worker.take_resync_requests().is_empty());
    }

    #[test]
    fn a_successful_write_resets_the_attempt_budget() {
        let (mut worker, key, adapter) = metered_route();
        for _ in 0..3 {
            assert!(worker.push_session_output(&key.session_id, b"x").is_empty());
        }
        exhaust_all_but_one(&mut worker, &key);
        adapter.lock().expect("adapter state").credits = 1;
        assert!(worker.pump_one(&key, PumpOrigin::AdapterWake).is_none());
        assert_eq!(worker.live[&key].unsuccessful_writes, 0);

        exhaust_all_but_one(&mut worker, &key);

        let owner = &worker.live[&key];
        assert!(!owner.stall_resynced);
        assert_eq!(owner.stream_epoch, 0, "no stall resync ran");
        assert!(worker.take_resync_requests().is_empty());
    }

    fn output_wake(key: &OwnerKey) -> TerminalWakeBatch {
        TerminalWakeBatch {
            adapter_routes: Vec::new(),
            ingress_sessions: vec![key.session_id.clone()],
        }
    }

    fn adapter_wake(key: &OwnerKey) -> TerminalWakeBatch {
        TerminalWakeBatch {
            adapter_routes: vec![crate::TerminalWakeRoute {
                session_id: key.session_id.clone(),
                subscription_id: key.subscription_id.clone(),
            }],
            ingress_sessions: Vec::new(),
        }
    }

    #[test]
    fn output_wakes_against_a_busy_adapter_never_count_toward_the_budget() {
        let (mut worker, key, adapter) = metered_route();
        for _ in 0..WRITE_ATTEMPT_BUDGET * 4 {
            assert!(worker
                .push_session_output(&key.session_id, b"y\n")
                .is_empty());
            assert!(
                worker.pump_woken(&output_wake(&key)).is_empty(),
                "a flooding producer must not end a live route"
            );
        }
        let owner = &worker.live[&key];
        assert_eq!(owner.unsuccessful_writes, 0);
        assert!(!owner.stall_resynced);
        assert!(adapter.lock().expect("adapter state").written.is_empty());

        // The reader's writable wake delivers what is queued.
        adapter.lock().expect("adapter state").credits = MAX_ROUTE_EGRESS_FRAMES;
        assert!(worker.pump_woken(&adapter_wake(&key)).is_empty());
        assert!(!adapter.lock().expect("adapter state").written.is_empty());
    }

    #[test]
    fn a_slow_reader_under_a_flooding_producer_stays_attached_and_receives_output() {
        let (mut worker, key, adapter) = metered_route();
        // The reader drains a burst on each writable wake, far less often than
        // the producer wakes the session.
        let drain_every = WRITE_ATTEMPT_BUDGET / 4;
        for tick in 1..=WRITE_ATTEMPT_BUDGET * 8 {
            assert!(worker
                .push_session_output(&key.session_id, b"y\n")
                .is_empty());
            for request in worker.take_resync_requests() {
                let identity = CaptureIdentity {
                    generation: request.generation,
                    capture_fence: request.capture_fence,
                };
                let ready = encode_snapshot_ready(b"GHOSTSNP").expect("ready");
                let teardown = worker
                    .push_capture_frame(&key.session_id, &key.subscription_id, identity, ready)
                    .expect("capture for the live fence");
                assert!(teardown.is_none());
            }
            assert!(worker.pump_woken(&output_wake(&key)).is_empty());
            if tick % drain_every == 0 {
                adapter.lock().expect("adapter state").credits = 8;
                assert!(
                    worker.pump_woken(&adapter_wake(&key)).is_empty(),
                    "a reader that keeps draining must never be stopped"
                );
                adapter.lock().expect("adapter state").credits = 0;
            }
        }
        assert!(worker.has_subscription(&key.session_id, &key.subscription_id));
        let written = adapter.lock().expect("adapter state").written.clone();
        assert!(written.contains(&TerminalKind::Output));
        assert!(!worker.live[&key].stall_resynced);
    }

    /// An instant `READER_PROGRESS_DEADLINE` ago: the deadline has passed.
    fn deadline_ago() -> Instant {
        Instant::now()
            .checked_sub(READER_PROGRESS_DEADLINE)
            .expect("the monotonic clock is past the deadline")
    }

    #[test]
    fn a_reader_that_accepts_nothing_is_closed_at_its_progress_deadline() {
        let (mut worker, key) = bound_route();
        assert!(worker.push_session_output(&key.session_id, b"x").is_empty());
        assert!(worker.pump_woken(&output_wake(&key)).is_empty());
        let since = worker.live[&key]
            .blocked_since
            .expect("a refused head starts the deadline");
        assert_eq!(
            worker.next_reader_deadline(),
            Some(since + READER_PROGRESS_DEADLINE)
        );

        // Before the deadline, no amount of producer output ends the route,
        // and adapter wakes below the budget do not either.
        for _ in 0..WRITE_ATTEMPT_BUDGET * 4 {
            assert!(worker.push_session_output(&key.session_id, b"x").is_empty());
            assert!(worker.pump_woken(&output_wake(&key)).is_empty());
        }
        assert!(worker.expired_reader_routes(Instant::now()).is_empty());
        assert!(worker.has_subscription(&key.session_id, &key.subscription_id));

        worker.live.get_mut(&key).expect("route").blocked_since = Some(deadline_ago());
        let expired = worker.expired_reader_routes(Instant::now());
        assert_eq!(expired.len(), 1, "the host wait names the dead route");
        let teardowns = worker.pump_woken(&adapter_wake(&key));
        assert_eq!(teardowns.len(), 1, "the dead reader is closed");
        assert_eq!(teardowns[0].reason, TerminalRouteCloseReason::Stalled);
        assert!(!worker.has_subscription(&key.session_id, &key.subscription_id));
        assert_eq!(worker.next_reader_deadline(), None);
    }

    #[test]
    fn a_completed_write_restarts_the_reader_progress_deadline() {
        let (mut worker, key, adapter) = metered_route();
        for _ in 0..3 {
            assert!(worker.push_session_output(&key.session_id, b"x").is_empty());
        }
        assert!(worker.pump_woken(&output_wake(&key)).is_empty());
        assert!(worker.live[&key].blocked_since.is_some());

        let nearly = Instant::now()
            .checked_sub(READER_PROGRESS_DEADLINE / 2)
            .expect("clock");
        worker.live.get_mut(&key).expect("route").blocked_since = Some(nearly);
        // One frame completes; the next is accepted and held.
        adapter.lock().expect("adapter state").credits = 2;
        assert!(worker.pump_woken(&adapter_wake(&key)).is_empty());

        let since = worker.live[&key]
            .blocked_since
            .expect("the held frame arms the deadline again");
        assert!(since > nearly, "the completion restarted the deadline");
    }

    #[test]
    fn a_frame_accepted_but_never_completed_is_closed_at_the_deadline() {
        let (mut worker, key, adapter) = metered_route();
        assert!(worker.live[&key].blocked_since.is_none());
        // The transport accepts the head and then holds it forever.
        adapter.lock().expect("adapter state").credits = 1;
        assert!(worker.pump_woken(&adapter_wake(&key)).is_empty());
        assert!(worker.live[&key].in_flight);
        assert!(
            worker.next_reader_deadline().is_some(),
            "an accepted, unfinished frame arms the host wait with no further traffic"
        );

        worker.live.get_mut(&key).expect("route").blocked_since = Some(deadline_ago());
        assert_eq!(worker.expired_reader_routes(Instant::now()).len(), 1);
        let teardowns = worker.pump_woken(&adapter_wake(&key));
        assert_eq!(teardowns.len(), 1, "the held frame never completed");
        assert!(!worker.has_subscription(&key.session_id, &key.subscription_id));
    }

    #[test]
    fn a_route_with_nothing_pending_is_not_blocked() {
        let (mut worker, key, adapter) = metered_route();
        adapter.lock().expect("adapter state").credits = usize::MAX;
        assert!(worker.pump_woken(&adapter_wake(&key)).is_empty());
        assert!(worker.live[&key].queue.is_empty());

        worker.live.get_mut(&key).expect("route").blocked_since = Some(deadline_ago());
        assert!(worker.pump_woken(&adapter_wake(&key)).is_empty());
        assert!(worker.has_subscription(&key.session_id, &key.subscription_id));
        assert_eq!(worker.live[&key].blocked_since, None);
        assert!(worker.expired_reader_routes(Instant::now()).is_empty());
    }

    /// Every Core route close names its reason. A new or changed call site
    /// fails here until this table lists it.
    #[test]
    fn every_route_close_site_passes_its_expected_reason() {
        use botster_core_test_support::close_sites::{route_close_sites, CloseSite};
        let expected = |sites: &[(&str, &str)]| {
            let mut sites: Vec<_> = sites
                .iter()
                .map(|(function, reason)| CloseSite::new(function, reason))
                .collect();
            sites.sort();
            sites
        };
        assert_eq!(
            route_close_sites(include_str!("client_worker.rs")),
            expected(&[
                ("bind_waking_terminal_adapter", "BindRejected"),
                ("bind_waking_terminal_adapter", "BindRejected"),
                ("bind_waking_terminal_adapter", "BindRejected"),
                ("bind_waking_terminal_adapter", "BindRejected"),
                ("close", "reason"),
                ("complete_operation", "InputFailed"),
                ("detach_generation", "Detached"),
                ("detach_live", "Detached"),
                ("end_unserved_capture", "Failed"),
                ("enqueue_owner_frame", "Overflowed"),
                ("expire_pastes_keys", "InputFailed"),
                ("fail_queued_input_for_session", "SessionEnded"),
                ("fail_unserved_route", "Failed"),
                ("fail_route", "Failed"),
                ("fail_route", "Failed"),
                ("filter_bound_terminal_frames", "SessionEnded"),
                ("hard_stop", "reason"),
                ("head_refused", "Stalled"),
                ("hard_stop_key", "reason"),
                ("hard_stop_owner", "reason"),
                ("intake_terminal_input_keys", "InputFailed"),
                ("overflow_route", "Overflowed"),
                ("overflow_route", "Overflowed"),
                ("pump_one", "AdapterClosed"),
                ("pump_one", "AdapterClosed"),
                ("pump_one", "AdapterClosed"),
                ("pump_one", "TerminalDelivered"),
                ("push_session_modes", "Failed"),
                ("push_session_output", "Failed"),
                ("push_session_process_exit", "Failed"),
                ("record_attach", "Replaced"),
                ("stall_route", "Stalled"),
                ("teardown_all", "Shutdown"),
                ("teardown_replaced_client_session", "Replaced"),
                ("teardown_session", "reason"),
            ])
        );
        assert_eq!(
            route_close_sites(include_str!("managed_session_runtime.rs")),
            expected(&[
                ("apply_woken_terminal_input", "WorkerLinkFailed"),
                ("apply_woken_terminal_input", "WorkerLinkFailed"),
                ("fail_expired_pending_resize", "WorkerLinkFailed"),
                ("forget_terminal_session", "SessionEnded"),
                ("route_runtime_outputs", "WorkerLinkFailed"),
                ("shutdown_session", "SessionEnded"),
                ("submit_staged_input", "WorkerLinkFailed"),
            ])
        );
        assert_eq!(
            route_close_sites(include_str!("botster.rs")),
            expected(&[("bind_waking_terminal_adapter", "BindRejected")])
        );
    }

    #[derive(Default)]
    struct ProbeState {
        pressure: Option<TerminalAdapterPressure>,
        ingress: VecDeque<TerminalIngress>,
        closes: Vec<TerminalRouteCloseReason>,
    }

    /// Adapter that records each reason Core passes to `close`. Its
    /// pressure is Ready unless set; a Ready write completes at once.
    #[derive(Clone, Default)]
    struct ReasonProbe(std::sync::Arc<std::sync::Mutex<ProbeState>>);

    impl ReasonProbe {
        fn closes(&self) -> Vec<TerminalRouteCloseReason> {
            self.0.lock().expect("probe").closes.clone()
        }

        fn set_pressure(&self, pressure: TerminalAdapterPressure) {
            self.0.lock().expect("probe").pressure = Some(pressure);
        }

        fn push_ingress(&self, ingress: TerminalIngress) {
            self.0.lock().expect("probe").ingress.push_back(ingress);
        }
    }

    impl TerminalAdapter for ReasonProbe {
        fn try_write(
            &mut self,
            _frame: &RoutedTerminalFrame,
        ) -> Result<(), TerminalAdapterWriteError> {
            match self.pressure() {
                TerminalAdapterPressure::Ready => Ok(()),
                TerminalAdapterPressure::WouldBlock => Err(TerminalAdapterWriteError::WouldBlock),
                TerminalAdapterPressure::Full => Err(TerminalAdapterWriteError::Full),
                TerminalAdapterPressure::Closed => Err(TerminalAdapterWriteError::Closed),
            }
        }

        fn close(&mut self, reason: TerminalRouteCloseReason) {
            self.0.lock().expect("probe").closes.push(reason);
        }

        fn pressure(&self) -> TerminalAdapterPressure {
            self.0
                .lock()
                .expect("probe")
                .pressure
                .unwrap_or(TerminalAdapterPressure::Ready)
        }

        fn try_read(&mut self) -> TerminalIngress {
            self.0
                .lock()
                .expect("probe")
                .ingress
                .pop_front()
                .unwrap_or(TerminalIngress::Empty)
        }
    }

    impl WakingTerminalAdapter for ReasonProbe {
        fn set_wake_sink(&mut self, _sink: TerminalWakeSink) {}
    }

    /// Attach and bind one probed route that has its capture.
    fn probe_route(
        worker: &mut ClientWorker,
        client: &str,
        subscription: &str,
    ) -> (OwnerKey, ReasonProbe) {
        let client = ClientId(client.into());
        let session = SessionId("session".into());
        let subscription = SubscriptionId(subscription.into());
        let (generation, _) = worker
            .record_attach(client.clone(), session.clone(), subscription.clone())
            .expect("valid route");
        let probe = ReasonProbe::default();
        worker
            .bind_waking_terminal_adapter(
                &client,
                session.clone(),
                subscription.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(probe.clone()),
            )
            .expect("bind");
        worker
            .push_route_frame(
                &session,
                &subscription,
                encode_snapshot_ready(b"GHOSTSNP").expect("ready"),
            )
            .expect("ready enqueue");
        assert!(worker
            .pump_woken(&output_wake(&OwnerKey {
                session_id: session.clone(),
                subscription_id: subscription.clone(),
            }))
            .is_empty());
        (
            OwnerKey {
                session_id: session,
                subscription_id: subscription,
            },
            probe,
        )
    }

    /// The adapter saw exactly one close, and the teardown carries the same reason.
    fn assert_closed_for(
        probe: &ReasonProbe,
        teardown: Option<ClientWorkerTeardown>,
        reason: TerminalRouteCloseReason,
    ) {
        assert_eq!(probe.closes(), vec![reason]);
        assert_eq!(teardown.expect("the route ended").reason, reason);
    }

    #[test]
    fn a_newer_attach_by_the_same_client_closes_the_older_route_as_replaced() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "old");
        let (_, mut replacements) = worker
            .record_attach(
                ClientId("client".into()),
                key.session_id.clone(),
                SubscriptionId("new".into()),
            )
            .expect("attach");
        assert_closed_for(
            &probe,
            replacements.pop(),
            TerminalRouteCloseReason::Replaced,
        );
    }

    #[test]
    fn another_client_taking_the_subscription_closes_it_as_replaced() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "first", "route");
        let (_, mut replacements) = worker
            .record_attach(
                ClientId("second".into()),
                key.session_id.clone(),
                key.subscription_id.clone(),
            )
            .expect("attach");
        assert_closed_for(
            &probe,
            replacements.pop(),
            TerminalRouteCloseReason::Replaced,
        );
    }

    #[test]
    fn a_detach_closes_the_route_as_detached() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "live");
        let teardown = worker.detach_live(&key.session_id, &key.subscription_id);
        assert_closed_for(&probe, teardown, TerminalRouteCloseReason::Detached);

        let (key, probe) = probe_route(&mut worker, "client", "generation");
        let generation = worker.live[&key].generation;
        assert!(matches!(
            worker.detach_generation(&key.session_id, &key.subscription_id, generation),
            DetachTerminalSubscriptionResult::Detached { .. }
        ));
        assert_eq!(probe.closes(), vec![TerminalRouteCloseReason::Detached]);
    }

    #[test]
    fn a_closed_adapter_ends_the_route_as_adapter_closed() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "route");
        probe.set_pressure(TerminalAdapterPressure::Closed);
        let mut teardowns = worker.pump_woken(&adapter_wake(&key));
        assert_closed_for(
            &probe,
            teardowns.pop(),
            TerminalRouteCloseReason::AdapterClosed,
        );
    }

    #[test]
    fn a_delivered_process_exit_ends_the_route_as_terminal_delivered() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "route");
        assert!(worker
            .push_session_process_exit(&key.session_id, Some(0))
            .is_empty());
        let mut teardowns = worker.pump_woken(&adapter_wake(&key));
        assert_closed_for(
            &probe,
            teardowns.pop(),
            TerminalRouteCloseReason::TerminalDelivered,
        );
    }

    #[test]
    fn a_reader_that_never_accepts_a_write_is_closed_as_stalled() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "route");
        probe.set_pressure(TerminalAdapterPressure::WouldBlock);
        assert!(worker.push_session_output(&key.session_id, b"x").is_empty());
        let teardown = (0..2 * WRITE_ATTEMPT_BUDGET)
            .find_map(|_| worker.pump_one(&key, PumpOrigin::AdapterWake));
        assert_closed_for(&probe, teardown, TerminalRouteCloseReason::Stalled);
    }

    #[test]
    fn lost_or_malformed_input_closes_the_route_as_input_failed() {
        for ingress in [TerminalIngress::Lost, TerminalIngress::Frame(vec![0xff])] {
            let mut worker = ClientWorker::new();
            let (key, probe) = probe_route(&mut worker, "client", "route");
            probe.push_ingress(ingress);
            let mut teardowns = worker.intake_woken(&adapter_wake(&key));
            assert_closed_for(
                &probe,
                teardowns.pop(),
                TerminalRouteCloseReason::InputFailed,
            );
        }
    }

    /// Fill the route's rejection allowance so the next rejection overflows.
    fn fill_rejections(worker: &mut ClientWorker, key: &OwnerKey) {
        for operation_id in 1..=MAX_QUEUED_REJECTIONS_PER_ROUTE as u64 {
            worker
                .reject(key, operation_id, InputOutcome::RejectedProtocol, "")
                .expect("rejection fits");
        }
    }

    #[test]
    fn capped_rejections_close_the_route_as_overflowed_and_return_its_teardown() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "route");
        probe.set_pressure(TerminalAdapterPressure::WouldBlock);
        fill_rejections(&mut worker, &key);
        let ended = worker
            .reject(&key, 99, InputOutcome::RejectedProtocol, "")
            .expect_err("the overflowing rejection ends the route");
        assert_closed_for(&probe, ended, TerminalRouteCloseReason::Overflowed);
    }

    #[test]
    fn an_input_rejection_that_overflows_reports_the_route_teardown() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "route");
        probe.set_pressure(TerminalAdapterPressure::WouldBlock);
        fill_rejections(&mut worker, &key);
        let ended = worker
            .intake_terminal_command(
                &key,
                TerminalInputCommand::PasteAbort { operation_id: 99 },
                Vec::new(),
            )
            .expect_err("the unmatched abort's rejection overflows");
        assert_closed_for(&probe, ended, TerminalRouteCloseReason::Overflowed);
    }

    #[test]
    fn session_and_worker_teardowns_carry_the_reason_they_were_given() {
        let mut worker = ClientWorker::new();
        let (key, probe) = probe_route(&mut worker, "client", "route");
        let mut teardowns =
            worker.teardown_session(&key.session_id, TerminalRouteCloseReason::WorkerLinkFailed);
        assert_closed_for(
            &probe,
            teardowns.pop(),
            TerminalRouteCloseReason::WorkerLinkFailed,
        );

        let (key, probe) = probe_route(&mut worker, "client", "owner");
        let teardown = worker.hard_stop_owner(&key, TerminalRouteCloseReason::WorkerLinkFailed);
        assert_closed_for(&probe, teardown, TerminalRouteCloseReason::WorkerLinkFailed);

        let (_, probe) = probe_route(&mut worker, "client", "all");
        let mut teardowns = worker.teardown_all();
        assert_closed_for(&probe, teardowns.pop(), TerminalRouteCloseReason::Shutdown);
    }

    #[test]
    fn unbound_routes_end_with_session_ended_or_failed() {
        let mut worker = ClientWorker::new();
        let client = ClientId("client".into());
        let session = SessionId("session".into());
        for (subscription, reason) in [
            ("exit", TerminalRouteCloseReason::SessionEnded),
            ("fail", TerminalRouteCloseReason::Failed),
        ] {
            let subscription = SubscriptionId(subscription.into());
            worker
                .record_attach(client.clone(), session.clone(), subscription.clone())
                .expect("attach");
            let teardown = if reason == TerminalRouteCloseReason::Failed {
                worker.fail_route(&session, &subscription)
            } else {
                let mut egress = vec![(
                    client.clone(),
                    TransportEgress::ProcessExit {
                        session_id: session.clone(),
                        subscription_id: subscription.clone(),
                        code: Some(0),
                    },
                )];
                worker.filter_bound_terminal_frames(&mut egress).pop()
            };
            assert_eq!(teardown.expect("unbound route ended").reason, reason);
        }
    }

    #[test]
    fn a_declared_route_that_overflows_before_binding_ends_as_overflowed() {
        let mut worker = ClientWorker::new();
        let client = ClientId("client".into());
        let session = SessionId("session".into());
        let subscription = SubscriptionId("route".into());
        worker.expect_terminal_adapter(client.clone(), session.clone(), subscription.clone());
        worker
            .record_attach(client, session.clone(), subscription.clone())
            .expect("declared attach");
        let teardown = (0..=MAX_ROUTE_EGRESS_FRAMES).find_map(|_| {
            worker
                .push_route_frame(
                    &session,
                    &subscription,
                    encode_modes(ModesBody::default()).expect("modes"),
                )
                .expect("hold accepts frames")
        });
        assert_eq!(
            teardown.expect("the overflow ends the route").reason,
            TerminalRouteCloseReason::Overflowed
        );
    }

    #[test]
    fn a_rejected_bind_closes_the_offered_adapter_as_bind_rejected() {
        let mut worker = ClientWorker::new();
        let (key, _) = probe_route(&mut worker, "client", "route");
        let generation = worker.live[&key].generation;
        let offered = ReasonProbe::default();
        assert!(matches!(
            worker.bind_waking_terminal_adapter(
                &ClientId("client".into()),
                key.session_id.clone(),
                key.subscription_id.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(offered.clone()),
            ),
            Err(BindTerminalAdapterError::AlreadyBound { .. })
        ));
        assert_eq!(
            offered.closes(),
            vec![TerminalRouteCloseReason::BindRejected]
        );
    }

    fn decode_route_resync_frame(
        frame: &TerminalFrame,
    ) -> botster_terminal_protocol::RouteResyncBody {
        botster_terminal_protocol::decode_route_resync(frame).expect("resync body")
    }
}
