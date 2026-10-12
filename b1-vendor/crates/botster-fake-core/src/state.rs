//! The state of one `FakeCore` handle, and the data directory that outlives it.

use crate::choice::Chooser;
use crate::queue::Queue;
use crate::script::Script;
use botster_core_contract::prelude::*;
use botster_fake_route::FakeWorker;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Arc, Mutex};
use std::time::Instant;

/// The state that admission sees, updated at `begin` in `begin` order (Core AM-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Adm {
    Created,
    Starting,
    Running,
    Stopping,
    Exited,
    Lost,
    /// `Remove` was admitted: the id stays taken until its completion (Core LC-7).
    Removing,
}

/// What the payload process is doing (Core LC-5, EV-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payload {
    None,
    Alive,
    /// A signal was sent: the payload ends after `pumps` more pumps with this exit (Core LC-5, LC-6).
    Dying {
        pumps: u32,
        exit: botster_core_contract::prelude::Exit,
    },
    /// The payload ended; `Exited` is posted once the output tail is queued (Core OU-7).
    Dead(botster_core_contract::prelude::Exit),
}

/// One session: the registry row, the payload and the worker (Core 3, LC-9).
pub struct Sess {
    pub id: SessionId,
    pub instance: InstanceId,
    pub state: SessionState,
    pub request: SpawnRequest,
    pub labels: BTreeMap<String, String>,
    pub size: Size,
    pub exit: Option<botster_core_contract::prelude::Exit>,
    pub protocol: Option<u8>,
    pub script: Script,
    pub worker: Option<FakeWorker>,
    pub payload: Payload,
    /// The pumps until the program ends by itself (Core 11, scripted exits).
    pub program_exit_in: Option<u32>,
    pub program_exit_started: bool,
    /// The control link of the session is broken (Core LC-5, A2-1).
    pub control_broken: bool,
    /// Child processes that the program forked into the payload's group (Core LC-6).
    pub children: usize,
    /// The next step of the program, and how much of the input stream it has read (Core 11, scripted output and exits).
    pub pc: usize,
    pub input_read: usize,
    /// The modes that the host last saw, to post `ModesChanged` (Core EV-7).
    pub seen_modes: botster_route_codec::prelude::ModeFlags,
    /// Route closes that wait for the `Exited` event of the session (Core OU-7).
    pub held: Vec<botster_fake_route::RouteEvent>,
    pub kill_at: Option<Instant>,
    /// Routes of this session: contract route id to the worker's route id (Core ID-1).
    pub routes: BTreeMap<RouteId, u32>,
    /// What the host last learned of the revisions (Core IN-10, ST-4): the values as last reported.
    pub reported: (u64, u64, u64),
}

/// A service (Core 10). Only what the listed clauses need.
pub struct Svc {
    pub state: Option<ServiceState>,
    pub report: SpawnReport,
}

/// A row of the admission table (Core AM-1).
pub struct AdmRow {
    pub instance: InstanceId,
    pub state: Adm,
    /// The host writes in flight and their payload bytes (Core IN-5).
    pub io_ops: usize,
    pub io_bytes: u64,
}

/// The data directory: the registry rows and the workers that outlive a handle (Core LC-2, LC-12, AD-1).
#[derive(Default)]
pub struct StoreData {
    pub locked: bool,
    pub epoch: u64,
    pub rows: BTreeMap<SessionId, Sess>,
    pub services: BTreeMap<ServiceId, Svc>,
    pub next_instance: u64,
    /// The handle that holds each adopted session now (Core AD-1, DP-8).
    pub holders: BTreeMap<SessionId, std::sync::Weak<Mutex<Inner>>>,
}

/// A data directory that two handles can contend for.
#[derive(Clone, Default)]
pub struct FakeStore(pub(crate) Arc<Mutex<StoreData>>);

impl FakeStore {
    pub fn new() -> Self {
        Self::default()
    }
}

/// Where an operation runs (Core AM-2, OR-3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Lane {
    /// The lifecycle of one session, in `begin` order.
    Life(SessionId),
    /// The host writes of one session, in `begin` order.
    Io(SessionId),
    /// Anything else: concurrent, in any order.
    Free,
}

/// An operation that `begin` admitted and whose completion is not yet posted.
pub struct Pending {
    pub id: OpId,
    pub op: Op,
    pub lane: Lane,
    pub instance: Option<InstanceId>,
    /// Pumps left before the next step (Core OR-1).
    pub delay: u32,
    pub stage: u8,
    /// Done waiting for something that the worker or the program must do.
    pub waiting: bool,
    /// An internal operation has no slot and no completion (a `StopAll` target).
    pub silent: bool,
    /// The payload bytes that a host write holds against `input_retained_bytes` (Core IN-5).
    pub size: u64,
    /// The sessions and services that a `StopAll` targets (Core LC-12).
    pub targets: Vec<SessionId>,
    pub service_targets: Vec<ServiceId>,
}

pub struct StoredCapture {
    pub owner: ClientId,
    pub instance: InstanceId,
    pub taken: Instant,
    pub pages: Vec<Vec<u8>>,
    pub model_rev: ModelRev,
}

pub struct RouteRef {
    pub session: SessionId,
    pub instance: InstanceId,
    pub worker_route: u32,
    /// The baseline is released to the client at the first `pump` after the attach (Core OU-9, OR-1).
    pub ready: bool,
    pub closed: bool,
    pub tag: Option<String>,
}

pub struct WakeState {
    pub signaled: bool,
    pub spurious: bool,
}

/// The state of one handle.
pub struct Inner {
    pub seed: u64,
    pub limits: CoreLimits,
    pub store: Arc<Mutex<StoreData>>,
    pub ch: Chooser,
    pub sessions: BTreeMap<SessionId, Sess>,
    /// The admission table: the state that `begin` sees (Core AM-1).
    pub adm: BTreeMap<SessionId, AdmRow>,
    pub posted: u32,
    pub services: BTreeMap<ServiceId, Svc>,
    pub ops: Vec<Pending>,
    /// Every op id this handle minted, and whether it is a `WriteInput` (Core IN-6, ID-1).
    pub minted: BTreeMap<OpId, (bool, Option<InstanceId>)>,
    pub completed: BTreeSet<OpId>,
    pub next_op: u64,
    pub next_route: u64,
    pub next_capture: u64,
    pub routes: BTreeMap<RouteId, RouteRef>,
    pub captures: BTreeMap<CaptureId, StoredCapture>,
    pub queue: Queue,
    /// Slots held by ops whose `Completed` the host has not polled (Core EV-5, AM-4).
    pub slots: usize,
    pub now: Option<Now>,
    pub deferred: Vec<Event>,
    pub wake: Arc<Mutex<WakeState>>,
    pub withheld: BTreeSet<Feature>,
    pub unavailable: BTreeSet<Bound>,
    pub injected: Vec<(&'static str, CoreError)>,
    pub adopted: bool,
    /// The targets that `begin(StopAll)` just captured (Core LC-12).
    pub stop_targets: (Vec<SessionId>, Vec<ServiceId>),
    /// The input capacity that a completed write still holds until its `Completed` is polled (Core AM-4, IN-5).
    pub charged: BTreeMap<OpId, (SessionId, InstanceId, u64)>,
    /// This handle, for the route ends that follow a session across an adoption (Core DP-8).
    pub me: std::sync::Weak<Mutex<Inner>>,
    pub cancels: BTreeSet<OpId>,
    pub refused_full: BTreeSet<SessionId>,
    /// Sessions whose control queue has no room (Core IN-6).
    pub full: BTreeSet<SessionId>,
}

impl Drop for Inner {
    /// Dropping the handle never ends a worker (Core LC-12): the rows and workers stay in the data directory, and the lock is released (Core LC-2).
    fn drop(&mut self) {
        if let Ok(mut store) = self.store.lock() {
            store.locked = false;
            for id in self.sessions.keys() {
                store.holders.remove(id);
            }
            store.rows.append(&mut self.sessions);
            store.services.append(&mut self.services);
        }
    }
}
