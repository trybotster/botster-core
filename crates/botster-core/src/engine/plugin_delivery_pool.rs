//! Host-call delivery pools (plan `plugin-process-host.md` section 5.1).
//!
//! A plugin's host call is answered by one result invocation. So that the
//! answer can never be refused for capacity (a suspended handler would wait
//! forever), the Hub reserves a pool at load. The pool pre-funds, for the
//! plugin's current worker generation, Background queue slots and request
//! bytes, and completion-store entries of a fixed payload allowance. One
//! accepted host call holds one unit: one slot, its declared result size,
//! and one completion entry. The unit returns exactly once: when the
//! drain hands out its result's completion, or on `release_call`. The pool
//! dies with the generation. The same reservation also sets aside the
//! plugin's ordinary completion share, all or nothing.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};

use crate::actor::{PluginAdmissionResult, PluginInvocationRequest, PluginKey};

use super::completion_store::UnitTag;
use super::EngineShared;

/// What one plugin generation reserves at load. Every value is Hub policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginDeliveryQuota {
    /// Host calls that may be in flight at once, each holding one unit.
    pub call_result_slots: usize,
    /// Sum of the declared result request sizes of the units in flight.
    pub call_result_request_bytes: usize,
    /// Completion payload allowance of one result invocation.
    pub call_result_completion_bytes: usize,
    /// Completion entries set aside for the plugin's ordinary invocations.
    pub ordinary_completion_entries: usize,
    /// Completion payload bytes set aside for the plugin's ordinary
    /// invocations, across all its entries.
    pub ordinary_completion_bytes: usize,
}

/// Identity of one host call, unique within the plugin's generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CallId(pub u64);

/// Why a delivery reservation was refused. Nothing was reserved.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DeliveryRefusal {
    /// The engine-wide completion pool cannot fund the quota now, or the
    /// plugin's queue is too full to set the pool's room aside.
    Backpressured(String),
    /// The plugin is not loaded, or its worker is stopping.
    WorkerStopped(String),
    /// The quota can never fit this engine's configuration, or this
    /// generation already reserved.
    RejectedBudget(String),
}

/// Why `accept_call` refused a host call. On the process host this means the
/// child ignored its credits, which is a protocol violation.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PoolOverdraw {
    /// Every unit is in use.
    NoSlot,
    /// The declared result size does not fit the free request bytes.
    NoBytes,
    /// The call id is already in use in this generation.
    DuplicateCall,
    /// The pool's generation retired.
    Closed,
}

impl fmt::Display for PoolOverdraw {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let reason = match self {
            Self::NoSlot => "every delivery unit is in use",
            Self::NoBytes => "the declared result size exceeds the free request bytes",
            Self::DuplicateCall => "the call id is already in use",
            Self::Closed => "the delivery pool's generation retired",
        };
        f.write_str(reason)
    }
}

impl std::error::Error for PoolOverdraw {}

/// Called once per returned unit, outside every engine lock.
pub type UnitReturnedNotifier = Arc<dyn Fn(CallId) + Send + Sync + 'static>;

/// One plugin generation's host-call delivery pool.
#[derive(Clone)]
pub struct DeliveryPool {
    pub(super) inner: Arc<PoolInner>,
}

impl fmt::Debug for DeliveryPool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeliveryPool")
            .field("plugin_key", &self.inner.plugin_key)
            .field("generation", &self.inner.generation)
            .finish_non_exhaustive()
    }
}

pub(super) struct PoolInner {
    pub(super) id: u64,
    pub(super) plugin_key: PluginKey,
    pub(super) generation: u64,
    pub(super) slots: usize,
    pub(super) request_bytes: usize,
    pub(super) completion_bytes: usize,
    pub(super) fund: u64,
    pub(super) shared: Weak<EngineShared>,
    state: Mutex<PoolState>,
    notifier: Mutex<Option<UnitReturnedNotifier>>,
}

#[derive(Default)]
struct PoolState {
    units: HashMap<CallId, Unit>,
    used_bytes: usize,
    closed: bool,
}

/// Why a result could not start its admission.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum AdmitRefusal {
    /// The pool's generation retired.
    Closed,
    /// No accepted host call has this id.
    UnknownCall,
    /// This call's result was already admitted.
    AlreadyAdmitted,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Phase {
    /// Accepted; no result admitted yet.
    Accepted,
    /// A result admission is in progress.
    Admitting,
    /// The result job holds the unit until its completion drains.
    Admitted,
}

struct Unit {
    declared_bytes: usize,
    phase: Phase,
}

/// The delivery reservation of one worker generation, kept in its admission
/// state.
pub(super) struct GenerationDelivery {
    pub(super) pool: Arc<PoolInner>,
    pub(super) pool_slots: usize,
    pub(super) pool_request_bytes: usize,
    pub(super) share_fund: u64,
}

impl PoolInner {
    pub(super) fn new(
        id: u64,
        plugin_key: PluginKey,
        generation: u64,
        quota: &PluginDeliveryQuota,
        fund: u64,
        shared: Weak<EngineShared>,
    ) -> Self {
        Self {
            id,
            plugin_key,
            generation,
            slots: quota.call_result_slots,
            request_bytes: quota.call_result_request_bytes,
            completion_bytes: quota.call_result_completion_bytes,
            fund,
            shared,
            state: Mutex::new(PoolState::default()),
            notifier: Mutex::new(None),
        }
    }

    fn lock(&self) -> MutexGuard<'_, PoolState> {
        // Only this module's bookkeeping runs under the lock.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Mark `call`'s unit as admitting. Returns its declared result size.
    pub(super) fn begin_admit(&self, call: CallId) -> Result<usize, AdmitRefusal> {
        let mut state = self.lock();
        if state.closed {
            return Err(AdmitRefusal::Closed);
        }
        let unit = state
            .units
            .get_mut(&call)
            .ok_or(AdmitRefusal::UnknownCall)?;
        if unit.phase != Phase::Accepted {
            return Err(AdmitRefusal::AlreadyAdmitted);
        }
        unit.phase = Phase::Admitting;
        Ok(unit.declared_bytes)
    }

    /// Finish an admission: `admitted` keeps the unit until its completion
    /// drains; otherwise the unit is accepted again.
    pub(super) fn end_admit(&self, call: CallId, admitted: bool) {
        if let Some(unit) = self.lock().units.get_mut(&call) {
            unit.phase = if admitted {
                Phase::Admitted
            } else {
                Phase::Accepted
            };
        }
    }

    /// Return `call`'s unit if it is in `phase`, checking and removing under
    /// one lock, then tell the host outside it. Returns whether it returned.
    fn return_unit(&self, call: CallId, phase: Phase) -> bool {
        let returned = {
            let mut state = self.lock();
            match state.units.get(&call) {
                Some(unit) if unit.phase == phase => {
                    let declared = unit.declared_bytes;
                    state.units.remove(&call);
                    state.used_bytes -= declared;
                    true
                }
                _ => false,
            }
        };
        if returned {
            let notifier = self
                .notifier
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .clone();
            if let Some(notifier) = notifier {
                notifier(call);
            }
        }
        returned
    }

    /// The generation retired: every unit dies with the pool.
    pub(super) fn close(&self) {
        let mut state = self.lock();
        state.closed = true;
        state.units.clear();
        state.used_bytes = 0;
    }
}

impl DeliveryPool {
    /// Record an accepted host call that declares a result request of at
    /// most `max_result_bytes`. It holds one unit until its terminal.
    pub fn accept_call(&self, call: CallId, max_result_bytes: usize) -> Result<(), PoolOverdraw> {
        let mut state = self.inner.lock();
        if state.closed {
            return Err(PoolOverdraw::Closed);
        }
        if state.units.contains_key(&call) {
            return Err(PoolOverdraw::DuplicateCall);
        }
        if state.units.len() >= self.inner.slots {
            return Err(PoolOverdraw::NoSlot);
        }
        if state
            .used_bytes
            .checked_add(max_result_bytes)
            .is_none_or(|used| used > self.inner.request_bytes)
        {
            return Err(PoolOverdraw::NoBytes);
        }
        state.used_bytes += max_result_bytes;
        state.units.insert(
            call,
            Unit {
                declared_bytes: max_result_bytes,
                phase: Phase::Accepted,
            },
        );
        Ok(())
    }

    /// Admit the call's single result invocation. It never refuses for
    /// capacity: the unit already holds the queue room and the completion
    /// entry. `WorkerStopped` means the generation retired;
    /// `RejectedBudget` means a caller bug (no such call, a second result,
    /// an unregistered handler, or a request over the declared size), and
    /// the unit stays accepted.
    pub fn admit_result(
        &self,
        call: CallId,
        request: PluginInvocationRequest,
    ) -> PluginAdmissionResult {
        let Some(shared) = self.inner.shared.upgrade() else {
            return PluginAdmissionResult::WorkerStopped {
                request_id: request.request_id,
                class: crate::actor::PluginInvocationClass::Background,
                reason: "the plugin worker engine is gone".to_string(),
            };
        };
        super::admit_pool_result(&shared, &self.inner, call, request)
    }

    /// Terminal for a call that will never receive a result. Returns false
    /// if the call is unknown or its result was already admitted (its unit
    /// then returns when that result's completion drains).
    pub fn release_call(&self, call: CallId) -> bool {
        self.inner.return_unit(call, Phase::Accepted)
    }

    /// Run `notifier` once per returned unit, outside every engine lock.
    pub fn install_unit_returned(&self, notifier: UnitReturnedNotifier) {
        *self
            .inner
            .notifier
            .lock()
            .unwrap_or_else(PoisonError::into_inner) = Some(notifier);
    }

    /// Units and request bytes currently free.
    #[must_use]
    pub fn free(&self) -> (usize, usize) {
        let state = self.inner.lock();
        if state.closed {
            return (0, 0);
        }
        (
            self.inner.slots - state.units.len(),
            self.inner.request_bytes - state.used_bytes,
        )
    }
}

/// Return the units whose result completions were just drained.
pub(super) fn return_units(shared: &EngineShared, units: Vec<UnitTag>) {
    if units.is_empty() {
        return;
    }
    let pools: Vec<_> = {
        let registry = shared.pools.lock().unwrap_or_else(PoisonError::into_inner);
        units
            .into_iter()
            .filter_map(|unit| {
                registry
                    .get(&unit.pool)
                    .and_then(Weak::upgrade)
                    .map(|pool| (pool, CallId(unit.call)))
            })
            .collect()
    };
    for (pool, call) in pools {
        pool.return_unit(call, Phase::Admitted);
    }
}
