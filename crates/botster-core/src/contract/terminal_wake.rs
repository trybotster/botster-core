//! Transport-neutral waking terminal adapter contract and host wait source.
//!
//! This is an advanced host/adapter seam beside [`super::terminal_adapter`].
//! The poll-path [`super::terminal_adapter::TerminalAdapter`] bind remains for
//! one migration window. Waking binds allocate route wake state only after
//! every rejection check passes.
//!
//! Public enums in this module are exhaustive at `0.1.0`. Adding a variant is a
//! breaking change.
//!
//! [`TerminalWakeSink`] holds a [`std::sync::Weak`] to Core-owned
//! [`RouteWakeState`]. A host-retained adapter or sink clone must not pin the
//! allocation after hard-stop. Strong references exist only in the live
//! waking-adapter registry and in occupied channel nodes.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, RecvTimeoutError, SyncSender, TrySendError};
use std::sync::{Arc, Mutex, Weak};
use std::time::Duration;

use crate::actor::QueueSource;
use crate::contract::terminal_adapter::TerminalAdapter;
use crate::session::{SessionId, SubscriptionId};
use crate::terminal_subscription::TerminalSubscriptionGeneration;

/// Bounded ready-channel capacity. Matches [`QueueSource::ClientWorker`].
pub const WAKE_QUEUE_CAPACITY: usize = QueueSource::ClientWorker.default_capacity();

/// Adapter-emitted wake classification.
///
/// Kept in the public contract and conformance laws. The Core pump reads
/// authority from [`TerminalAdapter::pressure`] and `try_write`, not from this
/// discriminant. Not `#[non_exhaustive]`. Adding a variant at `0.1.0` is
/// breaking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalWakeKind {
    /// Capacity returned, or the adapter otherwise has work Core should pump.
    Writable,
    /// Local close or transport death. Idempotent.
    Closed,
}

/// Content-blind duplex adapter that can emit coalesced wakes.
///
/// Supertrait of [`TerminalAdapter`]. Implementations must not provide no-op
/// defaults that swallow wakes. Core calls [`Self::set_wake_sink`] after bind
/// rejection checks pass and before the adapter is stored.
pub trait WakingTerminalAdapter: TerminalAdapter {
    /// Install the Core-owned wake sink.
    ///
    /// Core calls this once per successful waking bind. The adapter may clone
    /// the sink onto a transport thread. Clones hold only a weak handle.
    fn set_wake_sink(&mut self, sink: TerminalWakeSink);
}

/// Per-route wake state. One allocation per successful waking bind.
///
/// `queued` is written only by the producer CAS and the consumer clear. Core
/// never writes a semantic bit into this structure besides `retired`.
pub(crate) struct RouteWakeState {
    queued: AtomicBool,
    retired: AtomicBool,
    session_id: SessionId,
    subscription_id: SubscriptionId,
    #[allow(dead_code)]
    generation: TerminalSubscriptionGeneration,
}

/// Per-session ingress wake state. One allocation per live session handle.
///
/// The session registry owns the strong reference. Handles hold a [`Weak`] so
/// a retained reader cannot resurrect a forgotten `SessionId`.
pub(crate) struct SessionWakeState {
    queued: AtomicBool,
    retired: AtomicBool,
    session_id: SessionId,
}

/// Lock-free, non-blocking wake handle held by adapters.
///
/// The strong `Arc` is intentionally a [`Weak`]. Changing this back to a strong
/// `Arc` restores a host-controlled memory bound: a retained adapter or sink
/// clone would pin [`RouteWakeState`] after hard-stop. Proof 39 is red-on-revert
/// for that defect.
#[derive(Clone)]
pub struct TerminalWakeSink {
    /// Weak handle so host-retained clones cannot pin Core memory.
    state: Weak<RouteWakeState>,
    tx: SyncSender<WakeNode>,
    overflow: Arc<AtomicBool>,
    occupancy: Arc<AtomicUsize>,
}

impl std::fmt::Debug for TerminalWakeSink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TerminalWakeSink")
            .field("strong_count", &self.state.strong_count())
            .finish()
    }
}

impl TerminalWakeSink {
    /// Schedule this route. Lock-free and non-blocking on every arm.
    ///
    /// Returns `true` when this call won the coalesce gate (including the
    /// overflow arm, which leaves `queued` set). Returns `false` when the
    /// allocation is gone, the route is retired, or another wake is already
    /// queued.
    ///
    /// `kind` is recorded in the public contract only. The pump ignores it.
    #[must_use]
    pub fn wake(&self, kind: TerminalWakeKind) -> bool {
        let _ = kind;
        let Some(state) = self.state.upgrade() else {
            return false;
        };
        if state.retired.load(Ordering::Acquire) {
            return false;
        }
        if state
            .queued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return false;
        }
        match self.tx.try_send(WakeNode::Adapter(Arc::clone(&state))) {
            Ok(()) => {
                self.occupancy.fetch_add(1, Ordering::Relaxed);
                true
            }
            Err(TrySendError::Full(_)) => {
                // Producer may only set the overflow flag and return.
                // Leave queued true so overflow reconcile still sees need.
                self.overflow.store(true, Ordering::Release);
                true
            }
            Err(TrySendError::Disconnected(_)) => {
                state.queued.store(false, Ordering::Release);
                false
            }
        }
    }

    /// Test helper: current strong count of the target allocation, if any.
    #[must_use]
    pub fn strong_count(&self) -> usize {
        self.state.strong_count()
    }
}

#[derive(Clone)]
enum WakeNode {
    Adapter(Arc<RouteWakeState>),
    Ingress(Arc<SessionWakeState>),
}

/// Per-session ingress coalescing owned by the live-session registry.
///
/// Reader threads clone this handle. Every live handle for one `SessionId`
/// shares the same [`SessionWakeState`]. Notify is a CAS plus `try_send` and
/// takes no lock on any arm, including overflow.
#[derive(Clone)]
pub struct SessionWakeHandle {
    state: Weak<SessionWakeState>,
    tx: SyncSender<WakeNode>,
    overflow: Arc<AtomicBool>,
    occupancy: Arc<AtomicUsize>,
}

impl SessionWakeHandle {
    /// Notify that this session has ingress work.
    ///
    /// Lock-free and non-blocking on every arm. A forgotten or dropped session
    /// returns without sending.
    pub fn notify(&self) {
        let Some(state) = self.state.upgrade() else {
            return;
        };
        if state.retired.load(Ordering::Acquire) {
            return;
        }
        if state
            .queued
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        match self.tx.try_send(WakeNode::Ingress(Arc::clone(&state))) {
            Ok(()) => {
                self.occupancy.fetch_add(1, Ordering::Relaxed);
            }
            Err(TrySendError::Full(_)) => {
                // Leave queued true so overflow reconcile still sees need.
                self.overflow.store(true, Ordering::Release);
            }
            Err(TrySendError::Disconnected(_)) => {
                state.queued.store(false, Ordering::Release);
            }
        }
    }
}

struct WakeInner {
    tx: SyncSender<WakeNode>,
    rx: Mutex<mpsc::Receiver<WakeNode>>,
    overflow: Arc<AtomicBool>,
    occupancy: Arc<AtomicUsize>,
    registry: Mutex<HashMap<(SessionId, SubscriptionId), Arc<RouteWakeState>>>,
    session_registry: Mutex<HashMap<SessionId, Arc<SessionWakeState>>>,
    visit_count: AtomicUsize,
    #[cfg(test)]
    reverse_clear: AtomicBool,
    #[cfg(test)]
    race_hook: Mutex<Option<Box<dyn FnOnce() + Send>>>,
}

/// Host-facing wait source for adapter and ingress wakes.
///
/// The host blocks in [`Self::wait_wakes`]. Core creates no operating-system
/// thread for this wait. The public surface exposes no `RawFd`.
#[derive(Clone)]
pub struct TerminalWakeSource {
    inner: Arc<WakeInner>,
}

/// One drained wake batch. Adapter identities are routes; ingress is sessions.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TerminalWakeBatch {
    /// Bound waking-adapter routes that should be pumped.
    pub adapter_routes: Vec<TerminalWakeRoute>,
    /// Sessions whose worker or PTY input should be drained.
    pub ingress_sessions: Vec<SessionId>,
}

/// One bound waking-adapter route named by a wake.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TerminalWakeRoute {
    /// Session that owns the adapter.
    pub session_id: SessionId,
    /// Subscription bound to the adapter.
    pub subscription_id: SubscriptionId,
}

impl TerminalWakeSource {
    /// Build an empty wake source with a bounded ready channel.
    #[must_use]
    pub fn new() -> Self {
        let (tx, rx) = mpsc::sync_channel(WAKE_QUEUE_CAPACITY);
        Self {
            inner: Arc::new(WakeInner {
                tx,
                rx: Mutex::new(rx),
                overflow: Arc::new(AtomicBool::new(false)),
                occupancy: Arc::new(AtomicUsize::new(0)),
                registry: Mutex::new(HashMap::new()),
                session_registry: Mutex::new(HashMap::new()),
                visit_count: AtomicUsize::new(0),
                #[cfg(test)]
                reverse_clear: AtomicBool::new(false),
                #[cfg(test)]
                race_hook: Mutex::new(None),
            }),
        }
    }

    /// Block until a wake arrives or `timeout` elapses.
    ///
    /// Drains the channel, then if the overflow flag was set, runs one bounded
    /// reconciliation over the live waking-adapter registry and the live-session
    /// registry. The overflow flag is cleared before that walk. An already-set
    /// overflow flag does not wait on an empty channel.
    #[must_use]
    pub fn wait_wakes(&self, timeout: Duration) -> TerminalWakeBatch {
        let nodes = self.recv_nodes(timeout);
        self.assemble_batch(nodes)
    }

    /// Build a session-owned ingress handle. Call [`Self::forget_session`] after
    /// teardown commits.
    ///
    /// Repeated calls for a live `SessionId` share one coalesce state.
    #[must_use]
    pub fn session_handle(&self, session_id: SessionId) -> SessionWakeHandle {
        let state = {
            let mut registry = self
                .inner
                .session_registry
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if let Some(existing) = registry.get(&session_id) {
                if !existing.retired.load(Ordering::Acquire) {
                    Arc::clone(existing)
                } else {
                    let fresh = Arc::new(SessionWakeState {
                        queued: AtomicBool::new(false),
                        retired: AtomicBool::new(false),
                        session_id: session_id.clone(),
                    });
                    registry.insert(session_id, Arc::clone(&fresh));
                    fresh
                }
            } else {
                let fresh = Arc::new(SessionWakeState {
                    queued: AtomicBool::new(false),
                    retired: AtomicBool::new(false),
                    session_id: session_id.clone(),
                });
                registry.insert(session_id, Arc::clone(&fresh));
                fresh
            }
        };
        SessionWakeHandle {
            state: Arc::downgrade(&state),
            tx: self.inner.tx.clone(),
            overflow: Arc::clone(&self.inner.overflow),
            occupancy: Arc::clone(&self.inner.occupancy),
        }
    }

    /// Retire the session wake state and drop it from the live-session registry.
    ///
    /// Call this only after teardown commits. A retained handle then notifies
    /// without sending or re-inserting.
    pub fn forget_session(&self, session_id: &SessionId) {
        let mut registry = self
            .inner
            .session_registry
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(state) = registry.remove(session_id) {
            state.retired.store(true, Ordering::Release);
        }
    }

    /// Notify that a live session has ingress work.
    ///
    /// This path uses the live-session registry. It does not allocate a new
    /// coalesce state, and it does not resurrect a forgotten `SessionId`.
    pub fn notify_session(&self, session_id: &SessionId) {
        let state = {
            let registry = self
                .inner
                .session_registry
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            match registry.get(session_id) {
                Some(state) if !state.retired.load(Ordering::Acquire) => Arc::clone(state),
                _ => return,
            }
        };
        SessionWakeHandle {
            state: Arc::downgrade(&state),
            tx: self.inner.tx.clone(),
            overflow: Arc::clone(&self.inner.overflow),
            occupancy: Arc::clone(&self.inner.occupancy),
        }
        .notify();
    }

    /// Install a route in the waking-adapter registry and return its sink.
    ///
    /// Core calls this only after waking-bind rejection checks pass. The
    /// published conformance harness uses the same entry so adapter laws bind
    /// to a real sink.
    pub fn bind_route(
        &self,
        session_id: SessionId,
        subscription_id: SubscriptionId,
        generation: TerminalSubscriptionGeneration,
    ) -> TerminalWakeSink {
        let state = Arc::new(RouteWakeState {
            queued: AtomicBool::new(false),
            retired: AtomicBool::new(false),
            session_id: session_id.clone(),
            subscription_id: subscription_id.clone(),
            generation,
        });
        {
            let mut registry = self
                .inner
                .registry
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            registry.insert((session_id, subscription_id), Arc::clone(&state));
        }
        TerminalWakeSink {
            state: Arc::downgrade(&state),
            tx: self.inner.tx.clone(),
            overflow: Arc::clone(&self.inner.overflow),
            occupancy: Arc::clone(&self.inner.occupancy),
        }
    }

    /// Mark the route retired and drop it from the waking-adapter registry.
    pub fn retire_route(&self, session_id: &SessionId, subscription_id: &SubscriptionId) {
        let mut registry = self
            .inner
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(state) = registry.remove(&(session_id.clone(), subscription_id.clone())) {
            state.retired.store(true, Ordering::Release);
        }
    }

    /// Live waking-adapter registry size.
    #[must_use]
    pub fn registry_len(&self) -> usize {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }

    /// Whether the registry contains this bound route.
    #[must_use]
    pub fn registry_contains(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> bool {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .contains_key(&(session_id.clone(), subscription_id.clone()))
    }

    /// Current channel occupancy, counting adapter and ingress nodes.
    #[must_use]
    pub fn occupancy(&self) -> usize {
        self.inner.occupancy.load(Ordering::Relaxed)
    }

    /// Registry visits performed by overflow reconciliation since construction.
    #[must_use]
    pub fn visit_count(&self) -> usize {
        self.inner.visit_count.load(Ordering::Relaxed)
    }

    /// Strong-count census of live registry allocations plus channel occupancy.
    ///
    /// This is the Core-owned bound: live `RouteWakeState` allocations are at
    /// most registry size plus occupancy, independent of host-retained sink
    /// clones.
    #[must_use]
    pub fn live_allocation_bound(&self) -> usize {
        self.registry_len().saturating_add(self.occupancy())
    }

    /// Strong count for one registry entry, if present.
    #[must_use]
    pub fn registry_strong_count(
        &self,
        session_id: &SessionId,
        subscription_id: &SubscriptionId,
    ) -> Option<usize> {
        self.inner
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .get(&(session_id.clone(), subscription_id.clone()))
            .map(Arc::strong_count)
    }

    fn recv_nodes(&self, timeout: Duration) -> Vec<WakeNode> {
        let rx = self
            .inner
            .rx
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut nodes = Vec::new();
        while let Ok(node) = rx.try_recv() {
            self.inner.occupancy.fetch_sub(1, Ordering::Relaxed);
            nodes.push(node);
        }
        if nodes.is_empty() && !timeout.is_zero() && !self.inner.overflow.load(Ordering::Acquire) {
            match rx.recv_timeout(timeout) {
                Ok(node) => {
                    self.inner.occupancy.fetch_sub(1, Ordering::Relaxed);
                    nodes.push(node);
                    while let Ok(node) = rx.try_recv() {
                        self.inner.occupancy.fetch_sub(1, Ordering::Relaxed);
                        nodes.push(node);
                    }
                }
                Err(RecvTimeoutError::Timeout | RecvTimeoutError::Disconnected) => {}
            }
        }
        nodes
    }

    fn assemble_batch(&self, nodes: Vec<WakeNode>) -> TerminalWakeBatch {
        let reverse = {
            #[cfg(test)]
            {
                self.inner.reverse_clear.load(Ordering::Acquire)
            }
            #[cfg(not(test))]
            {
                false
            }
        };
        let (overflowed, overflowed_sessions) = if reverse {
            let walk = self.reconcile_registry();
            let sessions = self.reconcile_sessions();
            self.take_race_hook();
            let flag = self.inner.overflow.swap(false, Ordering::AcqRel);
            if flag {
                (walk, sessions)
            } else {
                (Vec::new(), Vec::new())
            }
        } else {
            // Clear-before-reconcile: a producer that stored queued=true
            // (release) before overflow (release) is visible to this acquire
            // swap, and a producer that races after the swap leaves the flag
            // set for the next drain.
            let flag = self.inner.overflow.swap(false, Ordering::AcqRel);
            self.take_race_hook();
            if flag {
                (self.reconcile_registry(), self.reconcile_sessions())
            } else {
                (Vec::new(), Vec::new())
            }
        };

        let mut adapter_states = Vec::new();
        let mut ingress = Vec::new();
        for node in nodes {
            match node {
                WakeNode::Adapter(state) => adapter_states.push(state),
                WakeNode::Ingress(state) => {
                    if state.retired.load(Ordering::Acquire) {
                        state.queued.store(false, Ordering::Release);
                        continue;
                    }
                    state.queued.store(false, Ordering::Release);
                    ingress.push(state.session_id.clone());
                }
            }
        }
        for state in overflowed {
            adapter_states.push(state);
        }
        for state in overflowed_sessions {
            if state.retired.load(Ordering::Acquire) {
                state.queued.store(false, Ordering::Release);
                continue;
            }
            state.queued.store(false, Ordering::Release);
            ingress.push(state.session_id.clone());
        }

        let mut seen_ptr = HashSet::new();
        let mut adapter_routes = Vec::new();
        for state in adapter_states {
            let ptr = Arc::as_ptr(&state) as usize;
            if !seen_ptr.insert(ptr) {
                continue;
            }
            if state.retired.load(Ordering::Acquire) {
                state.queued.store(false, Ordering::Release);
                continue;
            }
            state.queued.store(false, Ordering::Release);
            adapter_routes.push(TerminalWakeRoute {
                session_id: state.session_id.clone(),
                subscription_id: state.subscription_id.clone(),
            });
        }

        let mut seen_session = HashSet::new();
        let mut ingress_sessions = Vec::new();
        for session_id in ingress {
            if seen_session.insert(session_id.clone()) {
                ingress_sessions.push(session_id);
            }
        }

        TerminalWakeBatch {
            adapter_routes,
            ingress_sessions,
        }
    }

    fn reconcile_registry(&self) -> Vec<Arc<RouteWakeState>> {
        let registry = self
            .inner
            .registry
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut out = Vec::new();
        for state in registry.values() {
            self.inner.visit_count.fetch_add(1, Ordering::Relaxed);
            if state.retired.load(Ordering::Acquire) {
                continue;
            }
            if state.queued.load(Ordering::Acquire) {
                out.push(Arc::clone(state));
            }
        }
        out
    }

    fn reconcile_sessions(&self) -> Vec<Arc<SessionWakeState>> {
        let registry = self
            .inner
            .session_registry
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let mut out = Vec::new();
        for state in registry.values() {
            if state.retired.load(Ordering::Acquire) {
                continue;
            }
            if state.queued.load(Ordering::Acquire) {
                out.push(Arc::clone(state));
            }
        }
        out
    }

    fn take_race_hook(&self) {
        #[cfg(test)]
        {
            if let Some(hook) = self
                .inner
                .race_hook
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .take()
            {
                hook();
            }
        }
    }

    /// Live overflowed ingress sessions waiting for the next drain.
    ///
    /// Counts live session-registry entries whose queued gate is set while the
    /// overflow flag is raised. Successful in-channel notifies do not count.
    #[must_use]
    pub fn ingress_overflow_len(&self) -> usize {
        if !self.inner.overflow.load(Ordering::Acquire) {
            return 0;
        }
        self.inner
            .session_registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .values()
            .filter(|state| {
                !state.retired.load(Ordering::Acquire) && state.queued.load(Ordering::Acquire)
            })
            .count()
    }

    /// Live-session wake registry size.
    #[must_use]
    pub fn session_registry_len(&self) -> usize {
        self.inner
            .session_registry
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .len()
    }
}

impl Default for TerminalWakeSource {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
impl TerminalWakeSource {
    /// Invert clear-before-reconcile. Red-on-revert oracle only.
    pub fn set_reverse_clear_for_test(&self, reverse: bool) {
        self.inner.reverse_clear.store(reverse, Ordering::Release);
    }

    /// Run `hook` between overflow swap and registry reconcile (or the reverse).
    pub fn set_overflow_race_hook_for_test(&self, hook: impl FnOnce() + Send + 'static) {
        *self
            .inner
            .race_hook
            .lock()
            .unwrap_or_else(|error| error.into_inner()) = Some(Box::new(hook));
    }

    /// Set queued and overflow without enqueueing a channel node.
    pub fn arm_queued_overflow_for_test(&self, session_id: &SessionId) {
        let registry = self
            .inner
            .session_registry
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if let Some(state) = registry.get(session_id) {
            state.queued.store(true, Ordering::Release);
            self.inner.overflow.store(true, Ordering::Release);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    fn ids(n: u32) -> (SessionId, SubscriptionId) {
        (
            SessionId(format!("s{n}")),
            SubscriptionId(format!("sub{n}")),
        )
    }

    #[test]
    fn wake_coalesces_to_one_node() {
        let source = TerminalWakeSource::new();
        let (session, sub) = ids(1);
        let sink = source.bind_route(session, sub, TerminalSubscriptionGeneration(1));
        assert!(sink.wake(TerminalWakeKind::Writable));
        assert!(!sink.wake(TerminalWakeKind::Writable));
        assert!(!sink.wake(TerminalWakeKind::Closed));
        assert_eq!(source.occupancy(), 1);
        let batch = source.wait_wakes(Duration::from_millis(0));
        assert_eq!(batch.adapter_routes.len(), 1);
        assert_eq!(source.occupancy(), 0);
    }

    #[test]
    fn failed_upgrade_does_not_block_or_enqueue() {
        let source = TerminalWakeSource::new();
        let (session, sub) = ids(2);
        let sink = source.bind_route(
            session.clone(),
            sub.clone(),
            TerminalSubscriptionGeneration(1),
        );
        source.retire_route(&session, &sub);
        assert_eq!(sink.strong_count(), 0);
        assert!(!sink.wake(TerminalWakeKind::Writable));
        assert_eq!(source.occupancy(), 0);
        assert_eq!(source.registry_len(), 0);
    }

    #[test]
    fn overflow_sets_flag_and_leaves_queued() {
        let source = TerminalWakeSource::new();
        let mut sinks = Vec::new();
        for n in 0..=WAKE_QUEUE_CAPACITY {
            let (session, sub) = ids(n as u32);
            let sink = source.bind_route(session, sub, TerminalSubscriptionGeneration(1));
            assert!(sink.wake(TerminalWakeKind::Writable));
            sinks.push(sink);
        }
        assert!(source.occupancy() <= WAKE_QUEUE_CAPACITY);
        let batch = source.wait_wakes(Duration::from_millis(0));
        assert!(!batch.adapter_routes.is_empty());
        drop(sinks);
    }

    #[test]
    fn unique_pair_churn_without_drain_stays_bounded() {
        let source = TerminalWakeSource::new();
        let mut retained = Vec::new();
        for n in 0..1_024u32 {
            let (session, sub) = ids(n);
            let sink = source.bind_route(
                session.clone(),
                sub.clone(),
                TerminalSubscriptionGeneration(1),
            );
            let _ = sink.wake(TerminalWakeKind::Writable);
            source.retire_route(&session, &sub);
            retained.push(sink);
        }
        assert_eq!(source.registry_len(), 0);
        assert!(source.occupancy() <= WAKE_QUEUE_CAPACITY);
        assert!(source.live_allocation_bound() <= WAKE_QUEUE_CAPACITY);
        for sink in &retained {
            assert!(sink.strong_count() <= 1);
        }
    }

    #[test]
    fn reverse_clear_loses_a_race_that_production_keeps() {
        fn fill(source: &TerminalWakeSource) -> Vec<TerminalWakeSink> {
            let mut sinks = Vec::new();
            for n in 0..=WAKE_QUEUE_CAPACITY {
                let sink = source.bind_route(
                    SessionId(format!("fill{n}")),
                    SubscriptionId(format!("sub{n}")),
                    TerminalSubscriptionGeneration(1),
                );
                assert!(sink.wake(TerminalWakeKind::Writable));
                sinks.push(sink);
            }
            sinks
        }

        let production = TerminalWakeSource::new();
        let _filled = fill(&production);
        let late_session = SessionId("late".into());
        let late_sub = SubscriptionId("late-sub".into());
        let production_for_hook = production.clone();
        let late_for_hook = (late_session.clone(), late_sub.clone());
        production.set_overflow_race_hook_for_test(move || {
            let sink = production_for_hook.bind_route(
                late_for_hook.0.clone(),
                late_for_hook.1.clone(),
                TerminalSubscriptionGeneration(1),
            );
            let _ = sink.wake(TerminalWakeKind::Writable);
            std::mem::forget(sink);
        });
        let kept = production.wait_wakes(Duration::from_millis(0));
        assert!(
            kept.adapter_routes
                .iter()
                .any(|route| route.session_id == late_session),
            "clear-before-reconcile must recover a wake that races the drain"
        );

        let reversed = TerminalWakeSource::new();
        reversed.set_reverse_clear_for_test(true);
        let _filled = fill(&reversed);
        let reversed_for_hook = reversed.clone();
        reversed.set_overflow_race_hook_for_test(move || {
            let sink = reversed_for_hook.bind_route(
                SessionId("late".into()),
                SubscriptionId("late-sub".into()),
                TerminalSubscriptionGeneration(1),
            );
            let _ = sink.wake(TerminalWakeKind::Writable);
            std::mem::forget(sink);
        });
        let lost = reversed.wait_wakes(Duration::from_millis(0));
        assert!(
            !lost
                .adapter_routes
                .iter()
                .any(|route| route.session_id.0 == "late"),
            "reconcile-then-clear must lose the racy wake; that is the red-on-revert control"
        );
    }

    #[test]
    fn overflow_recovers_ingress_only_session() {
        let source = TerminalWakeSource::new();
        let mut sinks = Vec::new();
        for n in 0..WAKE_QUEUE_CAPACITY {
            let sink = source.bind_route(
                SessionId(format!("a{n}")),
                SubscriptionId(format!("s{n}")),
                TerminalSubscriptionGeneration(1),
            );
            assert!(sink.wake(TerminalWakeKind::Writable));
            sinks.push(sink);
        }
        let ingress = SessionId("ingress-only".into());
        let handle = source.session_handle(ingress.clone());
        handle.notify();
        assert_eq!(source.ingress_overflow_len(), 1);
        let batch = source.wait_wakes(Duration::from_millis(0));
        assert!(
            batch.ingress_sessions.contains(&ingress),
            "overflow must recover a session with no waking-adapter registry entry"
        );
        assert_eq!(source.ingress_overflow_len(), 0);
        drop(sinks);
    }

    #[test]
    fn ingress_overflow_does_not_fabricate_idle_adapter_route() {
        let source = TerminalWakeSource::new();
        let idle_session = SessionId("idle-route".into());
        let idle_sub = SubscriptionId("idle-sub".into());
        let idle = source.bind_route(
            idle_session.clone(),
            idle_sub.clone(),
            TerminalSubscriptionGeneration(1),
        );
        let mut handles = Vec::new();
        for n in 0..=WAKE_QUEUE_CAPACITY {
            let session = SessionId(format!("ingress{n}"));
            let handle = source.session_handle(session);
            handle.notify();
            handles.push(handle);
        }
        let batch = source.wait_wakes(Duration::from_millis(0));
        assert!(
            !batch
                .adapter_routes
                .iter()
                .any(|route| route.session_id == idle_session && route.subscription_id == idle_sub),
            "ingress-only overflow must not name an idle adapter route"
        );
        assert_eq!(batch.ingress_sessions.len(), WAKE_QUEUE_CAPACITY + 1);
        drop(idle);
        drop(handles);
    }

    #[test]
    fn forget_session_clears_overflow_residue() {
        let source = TerminalWakeSource::new();
        let mut sinks = Vec::new();
        for n in 0..WAKE_QUEUE_CAPACITY {
            let sink = source.bind_route(
                SessionId(format!("a{n}")),
                SubscriptionId(format!("s{n}")),
                TerminalSubscriptionGeneration(1),
            );
            assert!(sink.wake(TerminalWakeKind::Writable));
            sinks.push(sink);
        }
        let session = SessionId("doomed".into());
        source.session_handle(session.clone()).notify();
        assert_eq!(source.ingress_overflow_len(), 1);
        source.forget_session(&session);
        assert_eq!(source.ingress_overflow_len(), 0);
        drop(sinks);
    }

    #[test]
    fn session_wakes_coalesce_across_handles_and_notify_session() {
        let source = TerminalWakeSource::new();
        let session = SessionId("coalesce".into());
        let first = source.session_handle(session.clone());
        let second = source.session_handle(session.clone());
        first.notify();
        second.notify();
        source.notify_session(&session);
        source.notify_session(&session);
        assert_eq!(source.occupancy(), 1);
        assert_eq!(source.session_registry_len(), 1);
        let batch = source.wait_wakes(Duration::from_millis(0));
        assert_eq!(batch.ingress_sessions, vec![session]);
        assert_eq!(source.occupancy(), 0);
    }

    #[test]
    fn forget_session_retires_retained_handle() {
        let source = TerminalWakeSource::new();
        let mut sinks = Vec::new();
        for n in 0..WAKE_QUEUE_CAPACITY {
            let sink = source.bind_route(
                SessionId(format!("a{n}")),
                SubscriptionId(format!("s{n}")),
                TerminalSubscriptionGeneration(1),
            );
            assert!(sink.wake(TerminalWakeKind::Writable));
            sinks.push(sink);
        }
        let session = SessionId("dead".into());
        let handle = source.session_handle(session.clone());
        handle.notify();
        assert_eq!(source.ingress_overflow_len(), 1);
        source.forget_session(&session);
        handle.notify();
        source.notify_session(&session);
        assert_eq!(source.ingress_overflow_len(), 0);
        assert_eq!(source.session_registry_len(), 0);
        let batch = source.wait_wakes(Duration::from_millis(0));
        assert!(
            !batch.ingress_sessions.contains(&session),
            "a retained handle must not resurrect a forgotten session"
        );
        drop(sinks);
    }

    #[test]
    fn overflow_wait_does_not_use_timeout_as_progress() {
        let source = TerminalWakeSource::new();
        let session = SessionId("timer-free".into());
        let _handle = source.session_handle(session.clone());
        source.arm_queued_overflow_for_test(&session);
        assert_eq!(source.ingress_overflow_len(), 1);
        let started = Instant::now();
        let batch = source.wait_wakes(Duration::from_secs(5));
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "overflow must wake wait_wakes without the timeout"
        );
        assert!(
            batch.ingress_sessions.contains(&session),
            "overflow reconcile must recover the armed session"
        );
    }

    #[test]
    fn overflow_race_hook_recovers_queued_session() {
        let source = TerminalWakeSource::new();
        let mut sinks = Vec::new();
        for n in 0..=WAKE_QUEUE_CAPACITY {
            let sink = source.bind_route(
                SessionId(format!("fill{n}")),
                SubscriptionId(format!("sub{n}")),
                TerminalSubscriptionGeneration(1),
            );
            assert!(sink.wake(TerminalWakeKind::Writable));
            sinks.push(sink);
        }
        let session = SessionId("racy-ingress".into());
        let _handle = source.session_handle(session.clone());
        let hooked = source.clone();
        let hooked_session = session.clone();
        source.set_overflow_race_hook_for_test(move || {
            hooked.arm_queued_overflow_for_test(&hooked_session);
        });
        let batch = source.wait_wakes(Duration::from_millis(0));
        assert!(
            batch.ingress_sessions.contains(&session),
            "clear-before-reconcile must observe a session queued during the overflow race window"
        );
        drop(sinks);
    }
}
