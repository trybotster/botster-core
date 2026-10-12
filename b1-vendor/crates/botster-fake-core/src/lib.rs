//! FakeCore, the in-memory fake of the Core contract (Core v1.17 with A2, A3 and A4; ledger prefix `core`).
//!
//! Written from the clauses, not from real Core (Core 11). It is deterministic: its only randomness is a ChaCha8 generator from the
//! seed, its only clock is the `Now` that the host passes to `pump`, and it has no thread and no timer. It models no terminal and
//! encodes no input (R-5): screen contents, modes and the bytes that a semantic input writes are script data (see [`script`]).
//!
//! Routes run through `botster-fake-route`, the shared worker endpoint, which also holds the one admission point of a session
//! (Core AM-2). Every order that Core section 11 leaves open is a named choice point (see [`choice`]).

pub mod choice;
mod control;
mod input;
mod ops;
mod pump;
pub mod queue;
mod reads;
mod routes;
pub mod script;
mod services;
mod state;

pub use control::CONTROLS;
pub use routes::FakeRouteEnd;
pub use state::{FakeStore, Inner};

use botster_core_contract::prelude::*;
use choice::Choice;
use state::{Sess, WakeState};
use std::collections::{BTreeMap, BTreeSet};
use std::os::fd::RawFd;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

/// The worker protocol number T of the fake (Core AD-4).
pub const WORKER_PROTOCOL: u8 = 2;

/// The closure that a host write gives the worker (Core IN-8, IN-10).
pub(crate) type Decider = botster_fake_route::Decide;

/// One handle on a data directory. Clone it to reach the same handle from a test harness; dropping the last clone drops the handle.
#[derive(Clone)]
pub struct FakeCore {
    pub(crate) inner: Arc<Mutex<Inner>>,
    wake: Arc<Mutex<WakeState>>,
    /// Core TH-1: `Core` is `Send` and not `Sync`.
    _one_thread: std::marker::PhantomData<std::cell::Cell<()>>,
}

struct FakeWake(Arc<Mutex<WakeState>>);

impl WakeHandle for FakeWake {
    /// Core TH-2: a wait returns `Woken` while runnable work exists, `TimedOut` otherwise, and may wake with no work. The fake has no
    /// clock, so a wait never blocks: time passes only when the host advances its own clock.
    fn wait(&self, _timeout: Duration) -> Wake {
        let mut w = self.0.lock().expect("wake lock");
        let woken = w.signaled || std::mem::take(&mut w.spurious);
        if woken {
            Wake::Woken
        } else {
            Wake::TimedOut
        }
    }

    fn fd(&self) -> RawFd {
        -1
    }
}

impl FakeCore {
    /// Core LC-1, LC-2, 9B: opens the data directory. `seed` fixes every open order.
    pub fn open(store: &FakeStore, config: OpenConfig, seed: u64) -> Result<FakeCore, CoreError> {
        // Core LC-1: a missing worker path.
        if config.worker_path.is_none() {
            return Err(CoreError::new(
                ErrorCode::MissingWorkerPath,
                "no worker_path",
            ));
        }
        // Core 9B: a limit that is zero or out of range.
        config.limits.validate()?;
        let mut data = store.0.lock().expect("store lock");
        // Core LC-2: the lock on the data directory is exclusive.
        if data.locked {
            return Err(CoreError::new(
                ErrorCode::DataDirInUse,
                "another handle holds the directory",
            ));
        }
        data.locked = true;
        // Core DP-8: the host epoch strictly increases at every open.
        data.epoch += 1;
        drop(data);
        let wake = Arc::new(Mutex::new(WakeState {
            signaled: false,
            spurious: false,
        }));
        let inner = Inner {
            seed,
            limits: config.limits,
            store: store.0.clone(),
            ch: choice::Chooser::new(seed),
            sessions: BTreeMap::new(),
            adm: BTreeMap::new(),
            posted: 0,
            services: BTreeMap::new(),
            ops: vec![],
            minted: BTreeMap::new(),
            completed: BTreeSet::new(),
            next_op: 1,
            next_route: 1,
            next_capture: 1,
            routes: BTreeMap::new(),
            captures: BTreeMap::new(),
            queue: queue::Queue::default(),
            slots: 0,
            now: None,
            deferred: vec![],
            wake: wake.clone(),
            withheld: BTreeSet::new(),
            unavailable: BTreeSet::new(),
            injected: vec![],
            adopted: false,
            stop_targets: Default::default(),
            charged: BTreeMap::new(),
            me: std::sync::Weak::new(),
            cancels: BTreeSet::new(),
            refused_full: BTreeSet::new(),
            full: BTreeSet::new(),
        };
        let inner = Arc::new(Mutex::new(inner));
        inner.lock().expect("core lock").me = Arc::downgrade(&inner);
        Ok(FakeCore {
            inner,
            wake,
            _one_thread: std::marker::PhantomData,
        })
    }

    pub(crate) fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("core lock")
    }

    /// The outcomes that each choice point took so far (design 7.2 rule 3).
    pub fn choices_taken(&self) -> BTreeMap<Choice, BTreeSet<usize>> {
        self.lock().ch.seen().clone()
    }
}

impl Inner {
    /// Core TM-6: runnable work signals the wake handle before the call returns.
    pub(crate) fn signal(&self) {
        self.wake.lock().expect("wake lock").signaled = true;
    }

    pub(crate) fn err<T>(code: ErrorCode, detail: &str) -> Result<T, CoreError> {
        Err(CoreError::new(code, detail))
    }

    pub(crate) fn sess(&self, id: &SessionId) -> Result<&Sess, CoreError> {
        self.sessions
            .get(id)
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownSession, "no such session"))
    }

    /// The features that this Core offers (Core A2-6, A3-3): every mandatory name and the optional ones that are not withheld.
    pub(crate) fn feature_set(&self) -> BTreeSet<Feature> {
        use Feature::*;
        [
            RouteTransportStream,
            ServicePreambleVersions,
            Silence,
            SizePolicyOther,
            PayloadDiesWithGuardian,
            BoundCpuSeconds,
            BoundOpenFiles,
            BoundProcesses,
            BoundAddressSpaceBytes,
            OutputDropped,
            NotificationPolicy,
        ]
        .into_iter()
        .filter(|f| !self.withheld.contains(f))
        .collect()
    }
}

impl CoreApi for FakeCore {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        self.lock().begin(op)
    }

    fn pump(&mut self, now: Now) -> PumpReport {
        self.lock().pump(now)
    }

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.lock().poll(max)
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        Arc::new(FakeWake(self.wake.clone()))
    }

    fn next_deadline(&self) -> Option<std::time::Instant> {
        self.lock().next_deadline()
    }

    fn cancel(&mut self, op: OpId) -> CancelResult {
        self.lock().cancel(op)
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.lock().sess(id).map(|s| s.record())
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.lock().sessions.values().map(Sess::record).collect()
    }

    fn status(&self) -> Status {
        // Core LC-10: sessions and states only.
        Status {
            sessions: self
                .lock()
                .sessions
                .values()
                .map(|s| SessionStatus {
                    id: s.id.clone(),
                    state: s.state,
                })
                .collect(),
        }
    }

    fn diagnostics(&self) -> serde_json::Value {
        // Core LC-10: one opaque value with no compatibility promise.
        serde_json::Value::Null
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        let inner = self.lock();
        inner.sess(id).map(|s| s.terminal_state())
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.lock().read_page(capture, page)
    }

    fn release(&mut self, capture: CaptureId) {
        self.lock().captures.remove(&capture);
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.lock().captures.retain(|_, c| &c.owner != client);
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.lock().sess(session)?;
        Ok(vec![SnapshotFormat {
            name: "GHOSTSNP".into(),
            version: 1,
        }])
    }

    fn shadow_answerable_kinds(&self) -> Vec<botster_route_codec::prelude::QueryKind> {
        vec![]
    }

    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        // Core DP-2: the transport stays with the caller on a sync refusal (steward ruling R-19).
        self.lock()
            .attach(client, session, &transport, options)
            .map_err(|error| AttachRefused::new(error, transport))
    }

    fn tap_read(&mut self, session: &SessionId, _max: usize) -> Result<TapChunk, CoreError> {
        let inner = self.lock();
        let s = inner.sess(session)?;
        Ok(TapChunk {
            instance: s.instance.clone(),
            bytes: Default::default(),
            dropped_before: 0,
        })
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        _threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        let inner = self.lock();
        inner.sess(session)?;
        // Core A2-7: no `silence` feature is `Unsupported`.
        if inner.withheld.contains(&Feature::Silence) {
            return Inner::err(ErrorCode::Unsupported { what: None }, "no silence feature");
        }
        Ok(())
    }

    fn service_send(
        &mut self,
        id: &ServiceId,
        _lane: u8,
        _frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        let inner = self.lock();
        if inner.services.contains_key(id) {
            Err(SendError::NotConnected)
        } else {
            Err(SendError::UnknownService)
        }
    }

    fn service_recv(&mut self, id: &ServiceId, _lane: u8) -> Result<Option<Frame>, RecvError> {
        let inner = self.lock();
        if inner.services.contains_key(id) {
            Ok(None)
        } else {
            Err(RecvError::UnknownService)
        }
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        let inner = self.lock();
        inner
            .services
            .get(id)
            .map(|s| s.report.clone())
            .ok_or_else(|| CoreError::new(ErrorCode::UnknownService, "no such service"))
    }

    fn service_log_tail(&self, id: &ServiceId, _max: usize) -> Result<Vec<u8>, CoreError> {
        let inner = self.lock();
        if inner.services.contains_key(id) {
            Ok(vec![])
        } else {
            Inner::err(ErrorCode::UnknownService, "no such service")
        }
    }

    fn features(&self) -> Features {
        Features {
            names: self.lock().feature_set(),
            service_preamble_versions: vec![1],
        }
    }

    fn limits(&self) -> CoreLimits {
        self.lock().limits.clone()
    }

    fn worker_protocol(&self) -> u8 {
        WORKER_PROTOCOL
    }

    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        // Core AD-4: {T, T - 1}.
        [WORKER_PROTOCOL, WORKER_PROTOCOL - 1].into()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        match protocol {
            // Core AD-4: an absent protocol is `Unknown`, never compatible.
            None => WorkerCompatibility::Unknown,
            Some(p) if p == WORKER_PROTOCOL || p == WORKER_PROTOCOL - 1 => {
                WorkerCompatibility::Compatible
            }
            Some(_) => WorkerCompatibility::Incompatible,
        }
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        // Core TI-1: `term` is `xterm-ghostty`. The terminfo text is script data, which the fake does not hold.
        TerminalIdentity {
            term: "xterm-ghostty".into(),
            terminfo_source: String::new(),
        }
    }
}
