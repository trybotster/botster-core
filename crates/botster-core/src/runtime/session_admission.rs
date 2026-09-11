//! Session identity ownership before and after process launch.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicI32, AtomicU64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

use crate::SessionId;

static NEXT_SCOPE: AtomicU64 = AtomicU64::new(1);

const RESERVED: u8 = 0;
const LAUNCHING: u8 = 1;
const RUNTIME: u8 = 2;
const ENDED: u8 = 3;
const RELEASED: u8 = 4;
const CLEANUP_UNCONFIRMED: u8 = 5;
const CREATION_POSSIBLE: u8 = 6;

/// A refusal before Core grants session identity ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionReservationRefusal {
    /// Another reservation or session owns the identity.
    Occupied,
    /// Another caller currently accesses the admission table.
    Busy,
    /// The runtime does not implement session reservations.
    Unsupported,
    /// Core cannot issue another unique identity.
    IdentityExhausted,
    /// The admission table cannot safely accept another operation.
    Unavailable,
    /// The token does not identify the current reservation.
    InvalidToken,
    /// The existing pending-spawn allowance is full.
    Capacity,
}

/// Current execution ownership for a reservation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionReservationState {
    /// Core has not started the launch.
    Reserved,
    /// Core has started the launch and cannot yet release ownership.
    Launching,
    /// Core lost authoritative evidence of PTY cleanup after launch admission.
    CleanupUnconfirmed,
    /// A runtime or engine session still owns the identity.
    Session,
    /// Execution ended, but the reservation still excludes other producers.
    Ended,
    /// Core issued a definitive release.
    Released,
}

/// The result of an explicit release request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionReservationRelease {
    /// Core released this exact reservation generation.
    Released,
    /// Core must retain ownership while launch work remains unresolved.
    RetainedPending,
    /// Worker exit alone did not prove that the PTY session was cleaned up.
    RetainedUnconfirmed,
    /// Core must retain ownership while a runtime or engine session exists.
    RetainedSession,
}

#[derive(Debug)]
struct ReservationEntry {
    scope: u64,
    generation: u64,
    owner: SessionAdmissionOwner,
    request_id: Option<u64>,
    pending_counted: AtomicBool,
    pending_owned: AtomicBool,
    runtime_owned: AtomicBool,
    process_group: AtomicI32,
    process_exit_observed: AtomicBool,
    explicit: bool,
    session_id: SessionId,
    phase: AtomicU8,
    engine_owners: AtomicUsize,
}

/// A Core-issued reference to retained session identity ownership.
///
/// Cloning or dropping this reference does not release the reservation.
#[derive(Debug, Clone)]
pub struct SessionReservation(Arc<ReservationEntry>);

impl PartialEq for SessionReservation {
    fn eq(&self, other: &Self) -> bool {
        self.0.scope == other.0.scope && self.0.generation == other.0.generation
    }
}

impl Eq for SessionReservation {}

impl SessionReservation {
    /// Return the exact reserved session identity.
    #[must_use]
    pub fn session_id(&self) -> &SessionId {
        &self.0.session_id
    }

    pub(crate) fn belongs_to(&self, owner: &SessionAdmissionOwner) -> bool {
        owner.0.scope == self.0.owner.0.scope
    }

    /// Return the caller's reserve operation number, when supplied.
    #[must_use]
    pub fn request_id(&self) -> Option<u64> {
        self.0.request_id
    }

    fn transfer_pending_charge(&self) {
        if self.0.pending_counted.swap(false, Ordering::AcqRel) {
            self.0.owner.0.pending.fetch_sub(1, Ordering::AcqRel);
        }
    }

    /// Read an advisory view without waiting for the admission table.
    ///
    /// Only a release receipt authorizes reuse. Concurrent transitions can
    /// change this view before its caller uses it.
    #[must_use]
    pub fn state(&self) -> SessionReservationState {
        if self.0.pending_owned.load(Ordering::Acquire) {
            return SessionReservationState::Launching;
        }
        if self.0.runtime_owned.load(Ordering::Acquire)
            || self.0.engine_owners.load(Ordering::Acquire) != 0
        {
            return SessionReservationState::Session;
        }
        self.execution_state()
    }

    pub(crate) fn execution_state(&self) -> SessionReservationState {
        match self.0.phase.load(Ordering::Acquire) {
            RESERVED => SessionReservationState::Reserved,
            LAUNCHING | CREATION_POSSIBLE => SessionReservationState::Launching,
            CLEANUP_UNCONFIRMED => SessionReservationState::CleanupUnconfirmed,
            RUNTIME => SessionReservationState::Session,
            ENDED => SessionReservationState::Ended,
            RELEASED => SessionReservationState::Released,
            _ => unreachable!("invalid reservation phase"),
        }
    }

    pub(crate) fn pending_started(&self) {
        self.0.pending_owned.store(true, Ordering::Release);
    }

    pub(crate) fn pending_collected(&self) {
        self.0.pending_owned.store(false, Ordering::Release);
        self.refresh_cleanup();
    }

    pub(crate) fn runtime_installing(&self) {
        self.0.runtime_owned.store(true, Ordering::Release);
    }

    pub(crate) fn runtime_removed(&self) {
        self.0.runtime_owned.store(false, Ordering::Release);
    }

    pub(crate) fn capture_process_group(&self, process_group: Option<i32>) {
        if let Some(group) = process_group.filter(|group| *group > 0) {
            let _ = self.0.process_group.compare_exchange(
                0,
                group,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    pub(crate) fn observe_process_exit(&self) {
        self.0.process_exit_observed.store(true, Ordering::Release);
        if self.0.pending_owned.load(Ordering::Acquire) {
            self.cleanup_unconfirmed();
        }
        self.refresh_cleanup();
    }

    pub(crate) fn refresh_cleanup(&self) {
        if !self.0.process_exit_observed.load(Ordering::Acquire) {
            return;
        }
        if self.0.pending_owned.load(Ordering::Acquire) || self.0.runtime_owned.load(Ordering::Acquire) {
            return;
        }
        if matches!(self.0.phase.load(Ordering::Acquire), ENDED | RELEASED) {
            return;
        }
        if process_group_absent(self.0.process_group.load(Ordering::Acquire)) {
            self.runtime_ended();
        } else {
            self.cleanup_unconfirmed();
        }
    }

    pub(crate) fn runtime_installed(&self) {
        let _ = self
            .0
            .phase
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |phase| {
                matches!(phase, LAUNCHING | CREATION_POSSIBLE).then_some(RUNTIME)
            });
    }

    pub(crate) fn runtime_ended(&self) {
        let _ = self
            .0
            .phase
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |phase| {
                matches!(
                    phase,
                    LAUNCHING | CREATION_POSSIBLE | RUNTIME | CLEANUP_UNCONFIRMED
                )
                .then_some(ENDED)
            });
    }

    pub(crate) fn creation_possible(&self) {
        let _ = self.0.phase.compare_exchange(
            LAUNCHING,
            CREATION_POSSIBLE,
            Ordering::AcqRel,
            Ordering::Acquire,
        );
    }

    pub(crate) fn launch_failed(&self) {
        let _ =
            self.0
                .phase
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |phase| match phase {
                    LAUNCHING => Some(ENDED),
                    CREATION_POSSIBLE => Some(CLEANUP_UNCONFIRMED),
                    _ => None,
                });
    }

    pub(crate) fn cleanup_unconfirmed(&self) {
        let _ = self
            .0
            .phase
            .fetch_update(Ordering::AcqRel, Ordering::Acquire, |phase| {
                matches!(phase, LAUNCHING | CREATION_POSSIBLE | RUNTIME)
                    .then_some(CLEANUP_UNCONFIRMED)
            });
    }

    fn engine_owner(&self) -> EngineSessionAdmission {
        self.0.engine_owners.fetch_add(1, Ordering::AcqRel);
        EngineSessionAdmission(self.clone())
    }
}

/// One engine entry retains ownership even after the runtime session ends.
#[derive(Debug)]
pub(crate) struct EngineSessionAdmission(SessionReservation);

impl EngineSessionAdmission {
    pub(crate) fn reservation(&self) -> SessionReservation {
        self.0.clone()
    }
}

/// Identity of one engine that consumes reservations.
#[derive(Debug, Clone)]
pub(crate) struct SessionAdmissionOwner(Arc<AdmissionOwnerState>);

#[derive(Debug)]
struct AdmissionOwnerState {
    scope: Option<u64>,
    pending: AtomicUsize,
}

impl Default for SessionAdmissionOwner {
    fn default() -> Self {
        Self(Arc::new(AdmissionOwnerState {
            scope: next_scope(),
            pending: AtomicUsize::new(0),
        }))
    }
}

impl SessionAdmissionOwner {
    fn raw() -> Self {
        Self(Arc::new(AdmissionOwnerState {
            scope: Some(0),
            pending: AtomicUsize::new(0),
        }))
    }

    pub(crate) fn pending_count(&self) -> usize {
        self.0.pending.load(Ordering::Acquire)
    }
}

fn next_scope() -> Option<u64> {
    NEXT_SCOPE
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .ok()
}

impl Clone for EngineSessionAdmission {
    fn clone(&self) -> Self {
        self.0.engine_owner()
    }
}

impl Drop for EngineSessionAdmission {
    fn drop(&mut self) {
        self.0 .0.engine_owners.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug)]
struct AdmissionTable {
    scope: Option<u64>,
    next_generation: u64,
    entries: HashMap<SessionId, SessionReservation>,
}

/// The admission table shared by a built-in runtime and its engine.
#[derive(Debug)]
pub struct SessionAdmission {
    table: Arc<Mutex<AdmissionTable>>,
    synchronous: bool,
}

impl Default for SessionAdmission {
    fn default() -> Self {
        let scope = next_scope();
        Self {
            table: Arc::new(Mutex::new(AdmissionTable {
                scope,
                next_generation: 1,
                entries: HashMap::new(),
            })),
            synchronous: false,
        }
    }
}

impl SessionAdmission {
    pub(crate) fn synchronous() -> Self {
        Self {
            synchronous: true,
            ..Self::default()
        }
    }

    pub(crate) fn shared(&self) -> Self {
        Self {
            table: Arc::clone(&self.table),
            synchronous: self.synchronous,
        }
    }

    fn table(&self) -> Result<MutexGuard<'_, AdmissionTable>, SessionReservationRefusal> {
        self.table.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => SessionReservationRefusal::Busy,
            TryLockError::Poisoned(_) => SessionReservationRefusal::Unavailable,
        })
    }

    fn ordinary_table(&self) -> Result<MutexGuard<'_, AdmissionTable>, SessionReservationRefusal> {
        if self.synchronous {
            self.table
                .lock()
                .map_err(|_| SessionReservationRefusal::Unavailable)
        } else {
            self.table()
        }
    }

    /// Reserve one identity before a runtime starts any process.
    pub fn reserve(
        &self,
        session_id: SessionId,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        self.reserve_for(session_id, &SessionAdmissionOwner::raw(), true, None, None)
    }

    pub(crate) fn reserve_for(
        &self,
        session_id: SessionId,
        owner: &SessionAdmissionOwner,
        explicit: bool,
        request_id: Option<u64>,
        limit: Option<usize>,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        let mut table = if explicit {
            self.table()?
        } else {
            self.ordinary_table()?
        };
        Self::insert(&mut table, session_id, owner, explicit, request_id, limit)
    }

    fn insert(
        table: &mut AdmissionTable,
        session_id: SessionId,
        owner: &SessionAdmissionOwner,
        explicit: bool,
        request_id: Option<u64>,
        limit: Option<usize>,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        if table.entries.contains_key(&session_id) {
            return Err(SessionReservationRefusal::Occupied);
        }
        if owner.0.scope.is_none() {
            return Err(SessionReservationRefusal::IdentityExhausted);
        }
        if limit.is_some_and(|limit| owner.pending_count() >= limit) {
            return Err(SessionReservationRefusal::Capacity);
        }
        let scope = table
            .scope
            .ok_or(SessionReservationRefusal::IdentityExhausted)?;
        let generation = table.next_generation;
        table.next_generation = generation
            .checked_add(1)
            .ok_or(SessionReservationRefusal::IdentityExhausted)?;
        let reservation = SessionReservation(Arc::new(ReservationEntry {
            scope,
            generation,
            owner: owner.clone(),
            request_id,
            pending_counted: AtomicBool::new(true),
            pending_owned: AtomicBool::new(false),
            runtime_owned: AtomicBool::new(false),
            process_group: AtomicI32::new(0),
            process_exit_observed: AtomicBool::new(false),
            explicit,
            session_id: session_id.clone(),
            phase: AtomicU8::new(RESERVED),
            engine_owners: AtomicUsize::new(0),
        }));
        owner.0.pending.fetch_add(1, Ordering::AcqRel);
        table.entries.insert(session_id, reservation.clone());
        Ok(reservation)
    }

    /// Reserve from an explicitly synchronous runtime adapter.
    pub(crate) fn reserve_synchronous(
        &self,
        session_id: SessionId,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        let mut table = self
            .table
            .lock()
            .map_err(|_| SessionReservationRefusal::Unavailable)?;
        Self::insert(
            &mut table,
            session_id,
            &SessionAdmissionOwner::raw(),
            false,
            None,
            None,
        )
    }

    pub(crate) fn current(
        &self,
        session_id: &SessionId,
    ) -> Result<Option<SessionReservation>, SessionReservationRefusal> {
        Ok(self.ordinary_table()?.entries.get(session_id).cloned())
    }

    pub(crate) fn reserve_implicit(
        &self,
        session_id: SessionId,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        self.reserve_for(session_id, &SessionAdmissionOwner::raw(), false, None, None)
    }

    pub(crate) fn validate(
        &self,
        reservation: &SessionReservation,
    ) -> Result<(), SessionReservationRefusal> {
        let table = self.table()?;
        if table.entries.get(reservation.session_id()) != Some(reservation) {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        Ok(())
    }

    pub(crate) fn attach_engine(
        &self,
        reservation: &SessionReservation,
        owner: &SessionAdmissionOwner,
        implicit_handoff: bool,
    ) -> Result<EngineSessionAdmission, SessionReservationRefusal> {
        let table = if reservation.0.explicit {
            self.table()?
        } else {
            self.ordinary_table()?
        };
        let raw_handoff =
            implicit_handoff && !reservation.0.explicit && reservation.0.owner.0.scope == Some(0);
        if table.entries.get(reservation.session_id()) != Some(reservation)
            || (!reservation.belongs_to(owner) && !raw_handoff)
            || reservation.0.phase.load(Ordering::Acquire) != RUNTIME
            || reservation.0.engine_owners.load(Ordering::Acquire) != 0
        {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        reservation.transfer_pending_charge();
        Ok(reservation.engine_owner())
    }

    pub(crate) fn retire_implicit(
        &self,
        reservation: &SessionReservation,
    ) -> Result<(), SessionReservationRefusal> {
        if !reservation.0.explicit && reservation.state() == SessionReservationState::Ended {
            let _ = self.release(reservation)?;
        }
        Ok(())
    }

    /// Start only the launch that owns this exact reservation.
    pub(crate) fn begin_launch(
        &self,
        reservation: &SessionReservation,
        session_id: &SessionId,
    ) -> Result<(), SessionReservationRefusal> {
        let table = if reservation.0.explicit {
            self.table()?
        } else {
            self.ordinary_table()?
        };
        if reservation.session_id() != session_id
            || table.entries.get(session_id) != Some(reservation)
        {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        reservation
            .0
            .phase
            .compare_exchange(RESERVED, LAUNCHING, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| SessionReservationRefusal::InvalidToken)?;
        Ok(())
    }

    /// Release only an unused reservation or an ended execution.
    pub fn release(
        &self,
        reservation: &SessionReservation,
    ) -> Result<SessionReservationRelease, SessionReservationRefusal> {
        reservation.refresh_cleanup();
        let mut table = if reservation.0.explicit {
            self.table()?
        } else {
            self.ordinary_table()?
        };
        if reservation.0.scope != table.scope.unwrap_or(0) {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        if reservation.state() == SessionReservationState::Released {
            return Ok(SessionReservationRelease::Released);
        }
        if table.entries.get(reservation.session_id()) != Some(reservation) {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        match reservation.state() {
            SessionReservationState::Launching => Ok(SessionReservationRelease::RetainedPending),
            SessionReservationState::CleanupUnconfirmed => {
                Ok(SessionReservationRelease::RetainedUnconfirmed)
            }
            SessionReservationState::Session => Ok(SessionReservationRelease::RetainedSession),
            SessionReservationState::Reserved | SessionReservationState::Ended => {
                reservation.0.phase.store(RELEASED, Ordering::Release);
                reservation.transfer_pending_charge();
                table.entries.remove(reservation.session_id());
                Ok(SessionReservationRelease::Released)
            }
            SessionReservationState::Released => unreachable!("release holds the admission table"),
        }
    }
}

fn process_group_absent(process_group: i32) -> bool {
    if process_group <= 0 {
        return false;
    }
    #[cfg(all(unix, feature = "local-runtime"))]
    {
        unsafe extern "C" {
            fn kill(pid: i32, signal: i32) -> i32;
        }
        // SAFETY: Signal zero only observes a positive, captured process group.
        let result = unsafe { kill(-process_group, 0) };
        result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH)
    }
    #[cfg(not(all(unix, feature = "local-runtime")))]
    {
        false
    }
}
