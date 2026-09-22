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
    startup_created: AtomicBool,
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

/// An opaque identity for one reservation in this process.
///
/// This value retains no reservation storage and grants no launch or release
/// authority. Equality remains valid after the reservation ends. It does not
/// establish current ownership or successful cleanup. Do not persist this value
/// or use it to identify reservations across processes.
///
/// Release requires the original reservation:
///
/// ```compile_fail
/// use botster_core::{SessionAdmission, SessionId};
/// let admission = SessionAdmission::default();
/// let reservation = admission.reserve(SessionId("example".into())).unwrap();
/// admission.release(&reservation.identity());
/// ```
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SessionReservationIdentity {
    scope: u64,
    generation: u64,
}

impl std::fmt::Debug for SessionReservationIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("SessionReservationIdentity { .. }")
    }
}

impl PartialEq for SessionReservation {
    fn eq(&self, other: &Self) -> bool {
        self.0.scope == other.0.scope && self.0.generation == other.0.generation
    }
}

impl Eq for SessionReservation {}

impl SessionReservation {
    /// Copy this reservation's identity without retaining its storage.
    ///
    /// Keep the original reservation while launch or release work needs it.
    #[must_use]
    pub fn identity(&self) -> SessionReservationIdentity {
        SessionReservationIdentity {
            scope: self.0.scope,
            generation: self.0.generation,
        }
    }

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
    /// Callers that do not hold the admission table lock must treat this as
    /// advisory. Only a release receipt authorizes reuse. Concurrent
    /// transitions can change this view before its caller uses it.
    #[must_use]
    pub fn state(&self) -> SessionReservationState {
        self.owned_state()
    }

    fn owned_state(&self) -> SessionReservationState {
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

    /// Whether launch work created a session child.
    ///
    /// This follows the reservation phase, not error text. `false` means the
    /// launch was still pre-child. `true` means a child may have existed.
    #[must_use]
    pub fn startup_created_child(&self) -> bool {
        !matches!(
            self.execution_state(),
            SessionReservationState::Reserved
                | SessionReservationState::Launching
                | SessionReservationState::Ended
        )
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
        if let Some(group) = process_group.filter(|group| *group > 1) {
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

    pub(crate) fn mark_startup_created(&self) {
        self.0.startup_created.store(true, Ordering::Release);
    }

    pub(crate) fn refresh_cleanup(&self) {
        self.refresh_cleanup_inner(false);
    }

    fn refresh_cleanup_allowing_startup_created(&self) {
        self.refresh_cleanup_inner(true);
    }

    fn refresh_cleanup_inner(&self, allow_startup_created: bool) {
        let observed = self.0.process_exit_observed.load(Ordering::Acquire)
            || (allow_startup_created && self.0.startup_created.load(Ordering::Acquire));
        if !observed {
            return;
        }
        if self.0.pending_owned.load(Ordering::Acquire)
            || self.0.runtime_owned.load(Ordering::Acquire)
        {
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
        compare_exchange_from(&self.0.phase, &[LAUNCHING, CREATION_POSSIBLE], RUNTIME);
    }

    pub(crate) fn runtime_ended(&self) {
        compare_exchange_from(
            &self.0.phase,
            &[LAUNCHING, CREATION_POSSIBLE, RUNTIME, CLEANUP_UNCONFIRMED],
            ENDED,
        );
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
        if self
            .0
            .phase
            .compare_exchange(LAUNCHING, ENDED, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            let _ = self.0.phase.compare_exchange(
                CREATION_POSSIBLE,
                CLEANUP_UNCONFIRMED,
                Ordering::AcqRel,
                Ordering::Acquire,
            );
        }
    }

    pub(crate) fn cleanup_unconfirmed(&self) {
        compare_exchange_from(
            &self.0.phase,
            &[LAUNCHING, CREATION_POSSIBLE, RUNTIME],
            CLEANUP_UNCONFIRMED,
        );
    }

    fn attach_engine_owner(&self, admission: SessionAdmission) -> EngineSessionAdmission {
        self.0.engine_owners.fetch_add(1, Ordering::AcqRel);
        EngineSessionAdmission {
            reservation: self.clone(),
            admission,
        }
    }

    fn implicit_entry_is_reusable(&self) -> bool {
        !self.0.explicit
            && self.0.engine_owners.load(Ordering::Acquire) == 0
            && !self.0.pending_owned.load(Ordering::Acquire)
            && !self.0.runtime_owned.load(Ordering::Acquire)
            && matches!(self.0.phase.load(Ordering::Acquire), ENDED | RELEASED)
    }
}

/// One engine entry retains ownership even after the runtime session ends.
#[derive(Debug)]
pub(crate) struct EngineSessionAdmission {
    reservation: SessionReservation,
    admission: SessionAdmission,
}

impl EngineSessionAdmission {
    pub(crate) fn reservation(&self) -> SessionReservation {
        self.reservation.clone()
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
    /// Owner 0 is a raw runtime caller with no engine identity.
    ///
    /// Engines must not consume or release owner-0 tokens except through the
    /// implicit handoff that adopts a live runtime session.
    fn raw() -> Self {
        Self(Arc::new(AdmissionOwnerState {
            scope: Some(0),
            pending: AtomicUsize::new(0),
        }))
    }

    fn is_raw(&self) -> bool {
        self.0.scope == Some(0)
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
        self.reservation
            .0
            .engine_owners
            .fetch_add(1, Ordering::AcqRel);
        Self {
            reservation: self.reservation.clone(),
            admission: self.admission.clone(),
        }
    }
}

impl Drop for EngineSessionAdmission {
    fn drop(&mut self) {
        let previous = self
            .reservation
            .0
            .engine_owners
            .fetch_sub(1, Ordering::AcqRel);
        if previous == 1 {
            self.admission.needs_sweep.store(true, Ordering::Release);
        }
    }
}

#[derive(Debug)]
struct AdmissionTable {
    scope: Option<u64>,
    next_generation: u64,
    entries: HashMap<SessionId, SessionReservation>,
}

/// The admission table shared by a built-in runtime and its engine.
///
/// A table constructed without [`SessionAdmission::synchronous`] must be
/// driven by one owner thread. Ordinary Spawn and Adopt use `try_lock` and
/// return [`SessionReservationRefusal::Busy`] if another thread holds the
/// table. Launch threads and engine-owner drop never take this lock.
#[derive(Debug)]
pub struct SessionAdmission {
    table: Arc<Mutex<AdmissionTable>>,
    needs_sweep: Arc<AtomicBool>,
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
            needs_sweep: Arc::new(AtomicBool::new(false)),
            synchronous: false,
        }
    }
}

impl Clone for SessionAdmission {
    fn clone(&self) -> Self {
        self.shared()
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
            needs_sweep: Arc::clone(&self.needs_sweep),
            synchronous: self.synchronous,
        }
    }

    /// Table lock sites: `table`, `ordinary_table`, and `reserve_synchronous`.
    ///
    /// Those run on the runtime or engine owner that issued the operation.
    /// Launch threads and `EngineSessionAdmission` drop only store atomics
    /// and `needs_sweep`. They never take this lock. A second engine that
    /// shares the table is a second owner and may contend.
    fn table(&self) -> Result<MutexGuard<'_, AdmissionTable>, SessionReservationRefusal> {
        let mut guard = self.table.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => SessionReservationRefusal::Busy,
            TryLockError::Poisoned(_) => SessionReservationRefusal::Unavailable,
        })?;
        self.sweep_if_needed(&mut guard);
        Ok(guard)
    }

    fn ordinary_table(&self) -> Result<MutexGuard<'_, AdmissionTable>, SessionReservationRefusal> {
        if self.synchronous {
            let mut guard = self
                .table
                .lock()
                .map_err(|_| SessionReservationRefusal::Unavailable)?;
            self.sweep_if_needed(&mut guard);
            return Ok(guard);
        }
        let mut guard = self.table.try_lock().map_err(|error| match error {
            TryLockError::WouldBlock => SessionReservationRefusal::Busy,
            TryLockError::Poisoned(_) => SessionReservationRefusal::Unavailable,
        })?;
        self.sweep_if_needed(&mut guard);
        Ok(guard)
    }

    fn sweep_if_needed(&self, table: &mut AdmissionTable) {
        if !self.needs_sweep.swap(false, Ordering::AcqRel) {
            return;
        }
        table.entries.retain(|_, reservation| {
            if reservation.implicit_entry_is_reusable() {
                reservation.0.phase.store(RELEASED, Ordering::Release);
                reservation.transfer_pending_charge();
                false
            } else {
                true
            }
        });
    }

    fn owner_may_use(
        reservation: &SessionReservation,
        owner: Option<&SessionAdmissionOwner>,
    ) -> bool {
        match owner {
            Some(owner) => reservation.belongs_to(owner),
            None => reservation.0.owner.is_raw(),
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
        self.insert(&mut table, session_id, owner, explicit, request_id, limit)
    }

    fn insert(
        &self,
        table: &mut AdmissionTable,
        session_id: SessionId,
        owner: &SessionAdmissionOwner,
        explicit: bool,
        request_id: Option<u64>,
        limit: Option<usize>,
    ) -> Result<SessionReservation, SessionReservationRefusal> {
        if let Some(existing) = table.entries.get(&session_id).cloned() {
            existing.refresh_cleanup_allowing_startup_created();
            self.retire_implicit_locked(table, &existing);
            if table.entries.contains_key(&session_id) {
                return Err(SessionReservationRefusal::Occupied);
            }
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
            startup_created: AtomicBool::new(false),
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
        self.sweep_if_needed(&mut table);
        self.insert(
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
        owner: &SessionAdmissionOwner,
    ) -> Result<(), SessionReservationRefusal> {
        let table = if reservation.0.explicit {
            self.table()?
        } else {
            self.ordinary_table()?
        };
        self.authenticate_locked(&table, reservation, Some(owner))
    }

    fn authenticate_locked(
        &self,
        table: &AdmissionTable,
        reservation: &SessionReservation,
        owner: Option<&SessionAdmissionOwner>,
    ) -> Result<(), SessionReservationRefusal> {
        if table.entries.get(reservation.session_id()) != Some(reservation)
            || !Self::owner_may_use(reservation, owner)
        {
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
        let phase = reservation.0.phase.load(Ordering::Acquire);
        let raw_handoff =
            implicit_handoff && !reservation.0.explicit && reservation.0.owner.is_raw();
        if table.entries.get(reservation.session_id()) != Some(reservation)
            || (!reservation.belongs_to(owner) && !raw_handoff)
            || !matches!(phase, LAUNCHING | RUNTIME)
            || reservation.0.engine_owners.load(Ordering::Acquire) != 0
        {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        reservation.transfer_pending_charge();
        Ok(reservation.attach_engine_owner(self.shared()))
    }

    pub(crate) fn retire_implicit(
        &self,
        _reservation: &SessionReservation,
    ) -> Result<(), SessionReservationRefusal> {
        self.needs_sweep.store(true, Ordering::Release);
        Ok(())
    }

    fn retire_implicit_locked(&self, table: &mut AdmissionTable, reservation: &SessionReservation) {
        if table.entries.get(reservation.session_id()) != Some(reservation)
            || !reservation.implicit_entry_is_reusable()
        {
            return;
        }
        reservation.0.phase.store(RELEASED, Ordering::Release);
        reservation.transfer_pending_charge();
        table.entries.remove(reservation.session_id());
    }

    /// Start only the launch that owns this exact reservation.
    pub(crate) fn begin_launch(
        &self,
        reservation: &SessionReservation,
        session_id: &SessionId,
        owner: Option<&SessionAdmissionOwner>,
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
        if let Some(owner) = owner {
            if !Self::owner_may_use(reservation, Some(owner)) {
                return Err(SessionReservationRefusal::InvalidToken);
            }
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
        self.release_with_owner(reservation, None)
    }

    pub(crate) fn release_for(
        &self,
        reservation: &SessionReservation,
        owner: &SessionAdmissionOwner,
    ) -> Result<SessionReservationRelease, SessionReservationRefusal> {
        self.release_with_owner(reservation, Some(owner))
    }

    fn release_with_owner(
        &self,
        reservation: &SessionReservation,
        owner: Option<&SessionAdmissionOwner>,
    ) -> Result<SessionReservationRelease, SessionReservationRefusal> {
        reservation.refresh_cleanup_allowing_startup_created();
        let mut table = if reservation.0.explicit {
            self.table()?
        } else {
            self.ordinary_table()?
        };
        if reservation.0.scope != table.scope.unwrap_or(0) {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        if !Self::owner_may_use(reservation, owner) {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        if reservation.owned_state() == SessionReservationState::Released {
            return Ok(SessionReservationRelease::Released);
        }
        if table.entries.get(reservation.session_id()) != Some(reservation) {
            return Err(SessionReservationRefusal::InvalidToken);
        }
        match reservation.owned_state() {
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

fn compare_exchange_from(phase: &AtomicU8, from: &[u8], to: u8) {
    let mut current = phase.load(Ordering::Acquire);
    while from.contains(&current) {
        match phase.compare_exchange(current, to, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => return,
            Err(actual) => current = actual,
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

#[cfg(test)]
#[test]
fn async_table_reports_busy_when_another_thread_holds_the_lock() {
    let admission = SessionAdmission::default();
    let held = admission.table.lock().expect("owner lock");
    let other = admission.clone();
    let join =
        std::thread::spawn(move || other.reserve_implicit(SessionId("second-thread".into())));
    assert_eq!(
        join.join().expect("second thread"),
        Err(SessionReservationRefusal::Busy)
    );
    drop(held);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn session(name: &str) -> SessionId {
        SessionId(name.to_string())
    }

    #[test]
    fn explicit_release_frees_identity_for_reuse() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let first = admission
            .reserve_for(session("reuse"), &owner, true, Some(1), Some(4))
            .expect("reserve");
        assert_eq!(
            admission
                .reserve_for(session("reuse"), &owner, true, Some(2), Some(4))
                .expect_err("occupied"),
            SessionReservationRefusal::Occupied
        );
        assert_eq!(
            admission.release_for(&first, &owner).expect("release"),
            SessionReservationRelease::Released
        );
        let second = admission
            .reserve_for(session("reuse"), &owner, true, Some(3), Some(4))
            .expect("reuse after release");
        assert_ne!(first, second);
        assert_eq!(first.identity(), first.clone().identity());
        assert_ne!(first.identity(), second.identity());
        assert_eq!(second.request_id(), Some(3));
    }

    #[test]
    fn detached_identity_survives_storage_without_reusing_table_scope() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let reservation = admission
            .reserve_for(session("detached"), &owner, true, None, None)
            .expect("reserve");
        let entry = Arc::downgrade(&reservation.0);
        let owner_state = Arc::downgrade(&owner.0);
        let identity = reservation.identity();
        admission
            .release_for(&reservation, &owner)
            .expect("release");
        drop(reservation);
        drop(owner);
        drop(admission);
        assert!(entry.upgrade().is_none());
        assert!(owner_state.upgrade().is_none());

        let replacement = SessionAdmission::synchronous();
        let next = replacement.reserve(session("detached")).expect("reserve");
        assert_ne!(identity, next.identity());
        replacement.release(&next).expect("release");
    }

    #[test]
    fn detached_identity_does_not_change_ownership() {
        fn copy_identity(identity: SessionReservationIdentity) -> SessionReservationIdentity {
            identity
        }

        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let reservation = admission
            .reserve_for(session("identity-only"), &owner, true, None, None)
            .expect("reserve");
        let entry_count = Arc::strong_count(&reservation.0);
        let owner_count = Arc::strong_count(&owner.0);
        let identity = reservation.identity();
        assert_eq!(copy_identity(identity), reservation.identity());
        assert_eq!(copy_identity(identity), identity);
        assert_eq!(Arc::strong_count(&reservation.0), entry_count);
        assert_eq!(Arc::strong_count(&owner.0), owner_count);
        assert_eq!(owner.pending_count(), 1);
        assert_eq!(reservation.state(), SessionReservationState::Reserved);
        assert_eq!(
            admission
                .reserve(session("identity-only"))
                .expect_err("occupied"),
            SessionReservationRefusal::Occupied
        );
        assert_eq!(
            admission
                .release_for(&reservation, &owner)
                .expect("release"),
            SessionReservationRelease::Released
        );
        assert_eq!(owner.pending_count(), 0);
        assert_eq!(identity, reservation.identity());
    }

    #[test]
    fn generation_exhaustion_does_not_reuse_detached_identity() {
        let admission = SessionAdmission::synchronous();
        admission.table.lock().expect("table").next_generation = u64::MAX - 1;
        let reservation = admission
            .reserve(session("last-generation"))
            .expect("reserve");
        let identity = reservation.identity();
        admission.release(&reservation).expect("release");
        for _ in 0..2 {
            assert_eq!(
                admission
                    .reserve(session("last-generation"))
                    .expect_err("exhausted"),
                SessionReservationRefusal::IdentityExhausted
            );
        }
        assert_eq!(
            admission.table.lock().expect("table").next_generation,
            u64::MAX
        );
        assert_eq!(identity, reservation.identity());
    }

    #[test]
    fn pending_spawn_capacity_uses_owner_limit() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let first = admission
            .reserve_for(session("cap-a"), &owner, true, Some(1), Some(1))
            .expect("first");
        assert_eq!(
            admission
                .reserve_for(session("cap-b"), &owner, true, Some(2), Some(1))
                .expect_err("capacity"),
            SessionReservationRefusal::Capacity
        );
        admission.release_for(&first, &owner).expect("release");
        admission
            .reserve_for(session("cap-b"), &owner, true, Some(2), Some(1))
            .expect("capacity restored");
    }

    #[test]
    fn engine_owner_cannot_release_another_engine_token() {
        let admission = SessionAdmission::synchronous();
        let owner_a = SessionAdmissionOwner::default();
        let owner_b = SessionAdmissionOwner::default();
        let reserved = admission
            .reserve_for(session("auth"), &owner_a, true, None, None)
            .expect("reserve");
        assert_eq!(
            admission
                .release_for(&reserved, &owner_b)
                .expect_err("auth"),
            SessionReservationRefusal::InvalidToken
        );
        assert_eq!(reserved.state(), SessionReservationState::Reserved);
        assert_eq!(
            admission.release_for(&reserved, &owner_a).expect("owner a"),
            SessionReservationRelease::Released
        );
    }

    #[test]
    fn public_release_rejects_engine_owned_tokens() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let reserved = admission
            .reserve_for(session("raw-release"), &owner, true, None, None)
            .expect("reserve");
        assert_eq!(
            admission.release(&reserved).expect_err("engine token"),
            SessionReservationRefusal::InvalidToken
        );
        admission.release_for(&reserved, &owner).expect("owned");
    }

    #[test]
    fn implicit_ended_entry_retires_without_owner_wait() {
        let admission = SessionAdmission::synchronous();
        let reserved = admission
            .reserve_implicit(session("implicit"))
            .expect("implicit");
        admission
            .begin_launch(&reserved, reserved.session_id(), None)
            .expect("launch");
        reserved.runtime_installed();
        reserved.runtime_installing();
        reserved.runtime_removed();
        reserved.runtime_ended();
        admission.retire_implicit(&reserved).expect("retire");
        admission
            .reserve_implicit(session("implicit"))
            .expect("reuse after implicit retirement");
    }

    #[test]
    fn last_engine_owner_drop_retires_ended_implicit_entry() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let reserved = admission
            .reserve_implicit(session("drop-retire"))
            .expect("implicit");
        admission
            .begin_launch(&reserved, reserved.session_id(), None)
            .expect("launch");
        reserved.runtime_installed();
        let engine = admission
            .attach_engine(&reserved, &owner, true)
            .expect("attach launching or runtime");
        reserved.runtime_removed();
        reserved.runtime_ended();
        assert_eq!(
            admission
                .reserve_implicit(session("drop-retire"))
                .expect_err("engine fence"),
            SessionReservationRefusal::Occupied
        );
        drop(engine);
        admission
            .reserve_implicit(session("drop-retire"))
            .expect("retired after last engine owner");
    }

    #[test]
    fn attach_engine_rejects_released_and_reserved_tokens() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let reserved = admission
            .reserve_for(session("attach"), &owner, true, None, None)
            .expect("reserve");
        assert_eq!(
            admission
                .attach_engine(&reserved, &owner, false)
                .expect_err("reserved"),
            SessionReservationRefusal::InvalidToken
        );
        admission
            .begin_launch(&reserved, reserved.session_id(), Some(&owner))
            .expect("launch");
        let attached = admission
            .attach_engine(&reserved, &owner, false)
            .expect("launching");
        drop(attached);
        reserved.runtime_ended();
        admission.release_for(&reserved, &owner).expect("release");
        assert_eq!(
            admission
                .attach_engine(&reserved, &owner, false)
                .expect_err("released"),
            SessionReservationRefusal::InvalidToken
        );
    }

    #[test]
    fn process_exit_without_group_stays_unconfirmed() {
        let admission = SessionAdmission::synchronous();
        let reserved = admission
            .reserve_implicit(session("unknown-group"))
            .expect("implicit");
        admission
            .begin_launch(&reserved, reserved.session_id(), None)
            .expect("launch");
        reserved.runtime_installed();
        reserved.runtime_removed();
        reserved.observe_process_exit();
        assert_eq!(
            reserved.execution_state(),
            SessionReservationState::CleanupUnconfirmed
        );
        assert_eq!(
            admission.release(&reserved).expect("retain"),
            SessionReservationRelease::RetainedUnconfirmed
        );
    }

    #[test]
    fn late_phase_store_cannot_overwrite_released() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let reserved = admission
            .reserve_for(session("cas"), &owner, true, None, None)
            .expect("reserve");
        admission.release_for(&reserved, &owner).expect("release");
        reserved.runtime_installed();
        reserved.runtime_ended();
        reserved.cleanup_unconfirmed();
        assert_eq!(
            reserved.execution_state(),
            SessionReservationState::Released
        );
    }

    #[test]
    fn last_engine_drop_does_not_lock_or_busy_ordinary_reserve() {
        let admission = SessionAdmission::default();
        let owner = SessionAdmissionOwner::default();
        let reserved = admission
            .reserve_implicit(session("drop-no-lock"))
            .expect("implicit");
        admission
            .begin_launch(&reserved, reserved.session_id(), None)
            .expect("launch");
        reserved.runtime_installed();
        let engine = admission
            .attach_engine(&reserved, &owner, true)
            .expect("attach");
        reserved.runtime_removed();
        reserved.runtime_ended();
        drop(engine);
        admission
            .reserve_implicit(session("drop-no-lock-peer"))
            .expect("ordinary reserve after drop");
        admission
            .reserve_implicit(session("drop-no-lock"))
            .expect("sweep retires ended implicit id");
    }

    #[test]
    fn engine_owner_cannot_consume_a_raw_token_except_implicit_handoff() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let raw = admission
            .reserve_implicit(session("raw-token"))
            .expect("raw");
        assert_eq!(
            admission
                .release_for(&raw, &owner)
                .expect_err("raw release"),
            SessionReservationRefusal::InvalidToken
        );
        admission
            .begin_launch(&raw, raw.session_id(), Some(&owner))
            .expect_err("raw begin_launch");
        admission.release(&raw).expect("raw public release");
    }

    #[cfg(all(unix, feature = "local-runtime"))]
    #[test]
    fn startup_created_release_confirms_absent_group() {
        let admission = SessionAdmission::synchronous();
        let reserved = admission
            .reserve_implicit(session("spf1-absent"))
            .expect("implicit");
        admission
            .begin_launch(&reserved, reserved.session_id(), None)
            .expect("launch");
        reserved.capture_process_group(Some(unused_process_group()));
        reserved.mark_startup_created();
        reserved.cleanup_unconfirmed();
        assert_eq!(
            admission.release(&reserved).expect("confirm"),
            SessionReservationRelease::Released
        );
    }

    #[cfg(all(unix, feature = "local-runtime"))]
    #[test]
    fn startup_created_keeps_present_and_unknown_groups() {
        let admission = SessionAdmission::synchronous();
        let present = admission
            .reserve_implicit(session("spf1-present"))
            .expect("present");
        admission
            .begin_launch(&present, present.session_id(), None)
            .expect("launch");
        let live = unsafe { libc::getpgrp() };
        present.capture_process_group(Some(live));
        present.mark_startup_created();
        present.cleanup_unconfirmed();
        assert_eq!(
            admission.release(&present).expect("present unconfirmed"),
            SessionReservationRelease::RetainedUnconfirmed
        );
        assert_eq!(
            admission
                .reserve_implicit(session("spf1-present"))
                .expect_err("exclusion"),
            SessionReservationRefusal::Occupied
        );

        let unknown = admission
            .reserve_implicit(session("spf1-unknown"))
            .expect("unknown");
        admission
            .begin_launch(&unknown, unknown.session_id(), None)
            .expect("launch");
        unknown.mark_startup_created();
        unknown.cleanup_unconfirmed();
        assert_eq!(
            admission.release(&unknown).expect("unknown unconfirmed"),
            SessionReservationRelease::RetainedUnconfirmed
        );
    }

    #[cfg(all(unix, feature = "local-runtime"))]
    fn unused_process_group() -> i32 {
        let mut group = 1_000_000;
        while group < 1_000_200 {
            // SAFETY: signal 0 only observes whether the process group exists.
            let result = unsafe { libc::kill(-group, 0) };
            if result == -1 && std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH) {
                return group;
            }
            group += 1;
        }
        panic!("could not find an unused process group");
    }

    #[cfg(all(unix, feature = "local-runtime"))]
    #[test]
    fn process_exit_confirms_absent_group_and_rejects_stale_identity() {
        let admission = SessionAdmission::synchronous();
        let owner = SessionAdmissionOwner::default();
        let first = admission
            .reserve_for(session("pgid"), &owner, true, None, None)
            .expect("first");
        admission
            .begin_launch(&first, first.session_id(), Some(&owner))
            .expect("launch");
        first.runtime_installed();
        first.capture_process_group(Some(unused_process_group()));
        first.runtime_removed();
        first.observe_process_exit();
        assert_eq!(first.execution_state(), SessionReservationState::Ended);
        admission
            .release_for(&first, &owner)
            .expect("release ended");

        let second = admission
            .reserve_for(session("pgid"), &owner, true, None, None)
            .expect("new generation");
        first.observe_process_exit();
        first.cleanup_unconfirmed();
        first.runtime_ended();
        assert_eq!(second.state(), SessionReservationState::Reserved);
        assert_eq!(first.execution_state(), SessionReservationState::Released);
    }

    #[cfg(all(unix, feature = "local-runtime"))]
    #[test]
    fn process_exit_keeps_exclusion_when_group_is_present() {
        let admission = SessionAdmission::synchronous();
        let reserved = admission
            .reserve_implicit(session("live-group"))
            .expect("implicit");
        admission
            .begin_launch(&reserved, reserved.session_id(), None)
            .expect("launch");
        reserved.runtime_installed();
        let live = unsafe { libc::getpgrp() };
        assert!(live > 0);
        reserved.capture_process_group(Some(live));
        reserved.runtime_removed();
        reserved.observe_process_exit();
        assert_eq!(
            reserved.execution_state(),
            SessionReservationState::CleanupUnconfirmed
        );
        assert_eq!(
            admission
                .reserve_implicit(session("live-group"))
                .expect_err("exclusion"),
            SessionReservationRefusal::Occupied
        );
    }
}
