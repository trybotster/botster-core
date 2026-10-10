//! The testkit's real `Core` (Core A5-1): the one `HostDriver` of `botster-core-host`, over in-memory edges.
//!
//! The driver and the engine are the code of the real `Core`; only the edges differ (plan 2.1, A5-1: "one code path"). The
//! registry is in memory and survives a drop and a reopen of the same data directory (LC-12, AD-1); a second open of one
//! directory is refused while the first lives (LC-2); the host epoch strictly increases (DP-8); random values come from the
//! seeded stream (plan 2.3a); and the wake object is a level flag with a condition variable (TM-6, TH-2).
//!
//! The `Process` edge starts no process: a worker spawner is injected (`SimEdges::with_spawner`). The in-process worker is
//! the session worker package's, and until it exists a start is refused with the `errno` of a program that cannot be run.

use crate::entropy::SeededEntropy;
use crate::net::LinkEnd;
use crate::scheduler::SchedulerHandle;
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, StorageError,
};
use botster_core_edges::scheduler::ChoicePoint;
use botster_core_edges::{Entropy, Link, RouteTransport, Scheduler, Wake as WakeEdge};
use botster_core_host::driver::{
    check_open, DescriptorSendError, HostDriver, HostEdges, HostWake, WorkerSpawn,
};
use botster_core_host::{EngineConfig, LinkId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::Duration;

/// `ENOEXEC`: the errno of a program that cannot be run. It is what an `exec` of a file that is not a program gives.
const ENOEXEC: i32 = 8;

/// The default queue capacity. Control frames can span multiple reads and writes.
const LINK_CAPACITY: usize = 64 * 1024;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The testkit has no panic that leaves its state half written, so a poisoned lock still holds usable state.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The registry of one data directory: rows, and the lock of LC-2.
#[derive(Debug, Default)]
pub struct Registry {
    rows: BTreeMap<String, Vec<u8>>,
    locked: bool,
}

/// The failures that a test injects at the edges (A5-3: only an edge produces an asynchronous failure).
#[derive(Debug, Default)]
pub struct Faults {
    /// The next registry write fails with this error, once.
    pub next_write: Option<StorageError>,
    /// The next spawn is refused with this `errno`, once.
    pub next_spawn: Option<i32>,
    /// The next hand-over of a stream endpoint to a worker fails, once (`fail_handoff`, Core DP-2).
    pub fail_handoff: bool,
    /// The hand-over that the edge holds until `release_handoff` (`hold_handoff`, Core OU-9).
    pub handoff_hold: HandoffHold,
}

/// The hold of a hand-over at the edge (`hold_handoff`, `release_handoff`). While a link holds one, every hand-over on that
/// link takes nothing and returns `Blocked`, as a socket that takes no byte now: the driver keeps the endpoint and its frame,
/// and the frames after it on that link wait behind it. It is not a report to Core.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum HandoffHold {
    /// No hold.
    #[default]
    Off,
    /// The next hand-over, on any link, is held.
    Armed,
    /// The hand-over on this link is held.
    Held(LinkId),
}

/// The wake object of the testkit: a level flag that a waiter on any thread sees (TM-6, TH-2). A wait on a clear flag can end
/// with a spurious `Woken`: the run's scheduler decides it (A5-2 "spurious wakes"), and `no_spurious_wakes` turns it off.
#[derive(Debug)]
pub struct SimHostWake {
    flag: Mutex<bool>,
    changed: Condvar,
    scheduler: SchedulerHandle,
}

impl SimHostWake {
    /// A clear wake whose spurious wakes the run's `scheduler` draws.
    pub fn new(scheduler: SchedulerHandle) -> SimHostWake {
        SimHostWake {
            flag: Mutex::new(false),
            changed: Condvar::new(),
            scheduler,
        }
    }
}

impl botster_core_contract::prelude::WakeHandle for SimHostWake {
    fn wait(&self, timeout: Duration) -> Wake {
        let flag = lock(&self.flag);
        if *flag {
            return Wake::Woken;
        }
        // TH-2 allows a `Woken` with no work. The flag stays clear: no work is behind it.
        if self
            .scheduler
            .with(|s| s.pick(ChoicePoint::SpuriousWake, 2))
            == 1
        {
            return Wake::Woken;
        }
        let (flag, _) = self
            .changed
            .wait_timeout_while(flag, timeout, |set| !*set)
            .unwrap_or_else(PoisonError::into_inner);
        if *flag {
            Wake::Woken
        } else {
            Wake::TimedOut
        }
    }

    /// The testkit has no descriptor. The in-memory loop of a test waits on the handle itself (A5-1: no host callbacks).
    fn fd(&self) -> std::os::fd::RawFd {
        -1
    }
}

impl WakeEdge for SimHostWake {
    fn signal(&self) {
        *lock(&self.flag) = true;
        self.changed.notify_all();
    }

    fn drain(&self) {
        *lock(&self.flag) = false;
    }
}

/// Starts a worker for the testkit. The session worker package provides the in-process one.
pub trait Spawner: Send {
    /// Starts the worker of `spec` and returns its identity. `connect` makes the host side of a new control link and returns
    /// the worker's side.
    fn spawn(
        &mut self,
        spec: &WorkerSpawn,
        connect: &mut dyn FnMut() -> LinkEnd,
    ) -> Result<ProcessIdentity, SpawnError>;
    fn signal_group(&mut self, _identity: ProcessIdentity, _signal: GroupSignal) {}
    /// What `identity` matches now (AD-6).
    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState;
    fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        None
    }
    /// Connects to the endpoint of the worker of `instance` (DESIGN.md part 6): the worker takes `end`, its side of the new
    /// link. False when no worker listens there (none was spawned, it ended, or its endpoint was removed).
    fn connect_worker(&mut self, instance: &InstanceId, end: LinkEnd) -> bool;
    /// Removes the endpoint of the worker of `instance` (DESIGN.md part 1, `Remove` step 4). A missing endpoint is no
    /// failure.
    fn remove_endpoint(&mut self, instance: &InstanceId);
}

/// The scheduler of the testkit as a `Scheduler`: every choice draws from the one seeded stream.
#[derive(Debug, Clone)]
struct HandleScheduler(SchedulerHandle);

impl Scheduler for HandleScheduler {
    fn pick(&mut self, point: ChoicePoint, candidates: usize) -> usize {
        self.0.with(|s| s.pick(point, candidates))
    }

    fn bound(&mut self, point: ChoicePoint, max: usize) -> usize {
        self.0.with(|s| s.bound(point, max))
    }
}

/// The in-memory edges of one host.
pub struct SimEdges {
    registry: Arc<Mutex<Registry>>,
    faults: Arc<Mutex<Faults>>,
    entropy: SeededEntropy,
    scheduler: HandleScheduler,
    wake: Arc<SimHostWake>,
    spawner: Option<Box<dyn Spawner>>,
    pending: VecDeque<(LinkId, LinkEnd)>,
    links: BTreeMap<LinkId, LinkEnd>,
    next_link: u64,
    link_capacity: usize,
}

impl SimEdges {
    /// Replaces the spawner: the in-process worker of the session worker package.
    pub fn with_spawner(mut self, spawner: Box<dyn Spawner>) -> SimEdges {
        self.spawner = Some(spawner);
        self
    }

    pub fn faults(&self) -> Arc<Mutex<Faults>> {
        Arc::clone(&self.faults)
    }
}

impl Drop for SimEdges {
    fn drop(&mut self) {
        // LC-2: the lock ends when the handle is dropped. LC-12: no worker ends with it.
        lock(&self.registry).locked = false;
    }
}

impl HostEdges for SimEdges {
    fn fill_random(&mut self, buf: &mut [u8]) {
        self.entropy.fill(buf);
    }

    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError> {
        if let Some(error) = lock(&self.faults).next_write.take() {
            return Err(error);
        }
        lock(&self.registry)
            .rows
            .insert(key.to_string(), bytes.to_vec());
        Ok(())
    }

    fn delete_row(&mut self, key: &str) -> Result<(), StorageError> {
        if let Some(error) = lock(&self.faults).next_write.take() {
            return Err(error);
        }
        lock(&self.registry).rows.remove(key);
        Ok(())
    }

    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError> {
        Ok(lock(&self.registry)
            .rows
            .iter()
            .filter(|(k, _)| k.starts_with(prefix))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect())
    }

    fn spawn_worker(&mut self, spec: &WorkerSpawn) -> Result<ProcessIdentity, SpawnError> {
        if let Some(errno) = lock(&self.faults).next_spawn.take() {
            return Err(SpawnError { errno });
        }
        let Some(spawner) = self.spawner.as_mut() else {
            return Err(SpawnError { errno: ENOEXEC });
        };
        let (pending, next_link) = (&mut self.pending, &mut self.next_link);
        let link_capacity = self.link_capacity;
        assert!(link_capacity > 0, "a link needs a positive queue capacity");
        let mut connect = || {
            let (host, worker) = crate::net::link_pair(link_capacity);
            let link = LinkId(*next_link);
            *next_link += 1;
            pending.push_back((link, host));
            worker
        };
        spawner.spawn(spec, &mut connect)
    }

    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        if let Some(spawner) = self.spawner.as_mut() {
            spawner.signal_group(identity, signal);
        }
    }

    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState {
        self.spawner
            .as_ref()
            .map_or(IdentityState::Absent, |s| s.identity_state(identity))
    }

    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        self.spawner.as_mut().and_then(|s| s.poll_exit())
    }

    fn accept_link(&mut self) -> Option<LinkId> {
        let (link, end) = self.pending.pop_front()?;
        self.links.insert(link, end);
        Some(link)
    }

    /// AD-6, DESIGN.md part 6: an in-memory connection to the worker endpoint. The link is the host's at once, as a
    /// completed connect is.
    fn connect_worker(&mut self, instance: &InstanceId) -> Option<LinkId> {
        let spawner = self.spawner.as_mut()?;
        let (host, worker) = crate::net::link_pair(self.link_capacity);
        if !spawner.connect_worker(instance, worker) {
            return None;
        }
        let link = LinkId(self.next_link);
        self.next_link += 1;
        self.links.insert(link, host);
        Some(link)
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        if let Some(spawner) = self.spawner.as_mut() {
            spawner.remove_endpoint(instance);
        }
    }

    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize> {
        match self.links.get_mut(&link) {
            Some(end) => end.recv(buf),
            None => Ok(0),
        }
    }

    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize> {
        match self.links.get_mut(&link) {
            Some(end) => end.send(bytes),
            None => Err(io::ErrorKind::BrokenPipe.into()),
        }
    }

    fn link_close(&mut self, link: LinkId) {
        if let Some(mut end) = self.links.remove(&link) {
            end.close();
        }
    }

    fn set_write_interest(&mut self, link: LinkId, on: bool) {
        if let Some(end) = self.links.get_mut(&link) {
            let mut interest = end.end().interest();
            interest.write = on;
            end.end().set_interest(interest);
        }
    }

    fn set_read_interest(&mut self, link: LinkId, on: bool) {
        if let Some(end) = self.links.get_mut(&link) {
            let mut interest = end.end().interest();
            interest.read = on;
            end.end().set_interest(interest);
        }
    }

    fn link_send_descriptor(
        &mut self,
        link: LinkId,
        bytes: &[u8],
        endpoint: StreamEndpoint,
    ) -> Result<usize, (StreamEndpoint, DescriptorSendError)> {
        // The descriptor travels with the link by value, on the first byte of `bytes` (plan 2.3: "descriptor objects passed
        // by value"; `SCM_RIGHTS`).
        let Some(end) = self.links.get_mut(&link) else {
            return Err((endpoint, DescriptorSendError::Failed));
        };
        {
            let mut faults = lock(&self.faults);
            match faults.handoff_hold {
                HandoffHold::Armed => {
                    faults.handoff_hold = HandoffHold::Held(link);
                    return Err((endpoint, DescriptorSendError::Blocked));
                }
                HandoffHold::Held(held) if held == link => {
                    return Err((endpoint, DescriptorSendError::Blocked));
                }
                HandoffHold::Off | HandoffHold::Held(_) => {}
            }
            // The link's own hook fails the hand-over as `sendmsg` fails: nothing is taken, and the endpoint comes back to the
            // driver, which closes it (DP-2).
            if std::mem::take(&mut faults.fail_handoff) {
                end.end().control().fail_next_handoff();
            }
        }
        end.send_with_descriptor(bytes, crate::net::Descriptor::new(endpoint))
            .map_err(|(descriptor, error)| {
                let endpoint = descriptor
                    .downcast::<StreamEndpoint>()
                    .expect("the descriptor that was sent");
                let why = match error.kind() {
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::Interrupted => {
                        DescriptorSendError::Blocked
                    }
                    _ => DescriptorSendError::Failed,
                };
                (endpoint, why)
            })
    }

    fn close_route_stream(&mut self, endpoint: StreamEndpoint, bytes: &[u8]) {
        // A route stream of the testkit is the worker end of a duplex. One write, as a non-blocking `write(2)` takes what
        // fits; its result does not matter: the stream closes either way (R-50). Another object (a unit test's) is dropped.
        if let Ok(mut stream) = endpoint.downcast::<crate::net::StreamEnd>() {
            if !bytes.is_empty() {
                let _ = RouteTransport::write(&mut stream, bytes);
            }
            RouteTransport::close(&mut stream);
        }
    }

    fn wake(&self) -> Arc<dyn HostWake> {
        Arc::clone(&self.wake) as Arc<dyn HostWake>
    }

    fn settle_wake(&mut self) {}

    fn scheduler(&mut self) -> &mut dyn Scheduler {
        &mut self.scheduler
    }
}

/// The data directories of one harness: each is a registry that survives a drop (LC-12).
#[derive(Debug, Default)]
pub struct Directories {
    dirs: BTreeMap<String, Arc<Mutex<Registry>>>,
}

/// What one run of a harness gives every `Core` that it opens.
#[derive(Debug, Clone)]
pub struct RunInputs {
    /// Seeds the random values (plan 2.3a).
    pub seed: u64,
    /// The run's one seeded stream, shared with the in-process workers (A5-2).
    pub scheduler: SchedulerHandle,
}

/// A `Core` that `Directories::open` built: the host driver, its fault switches, and the wake object of its edges.
pub struct Opened {
    pub driver: HostDriver<SimEdges>,
    pub faults: Arc<Mutex<Faults>>,
    /// The testkit's `Core` also signals it for its in-process workers.
    pub wake: Arc<dyn HostWake>,
}

/// A reader of one stored row, held apart from the harness: it reads the row as the storage edge holds it now.
#[derive(Debug, Clone)]
pub struct RowReader {
    registry: Arc<Mutex<Registry>>,
    key: String,
}

impl RowReader {
    /// The stored bytes of the row now; `None` when it is not stored.
    pub fn read(&self) -> Option<Vec<u8>> {
        lock(&self.registry).rows.get(&self.key).cloned()
    }
}

impl Directories {
    /// The stored bytes of the row `key` in the directory `name`, as the storage edge holds them.
    pub(crate) fn row(&self, name: &str, key: &str) -> Option<Vec<u8>> {
        lock(self.dirs.get(name)?).rows.get(key).cloned()
    }

    /// A reader of the row `key` of the directory `name`, which reads its later versions too; `None` with no such directory.
    pub(crate) fn row_reader(&self, name: &str, key: &str) -> Option<RowReader> {
        Some(RowReader {
            registry: Arc::clone(self.dirs.get(name)?),
            key: key.to_string(),
        })
    }

    /// The storage edge damages the stored row `key` of the directory `name` (`corrupt_registry_row`, Core A10-2): it keeps
    /// the first half of the bytes that Core's encoder wrote. False when there is no such row.
    pub(crate) fn damage_row(&self, name: &str, key: &str) -> bool {
        let Some(registry) = self.dirs.get(name) else {
            return false;
        };
        let mut registry = lock(registry);
        let Some(bytes) = registry.rows.get_mut(key) else {
            return false;
        };
        bytes.truncate(bytes.len() / 2);
        true
    }

    /// Opens a `Core` over the in-memory directory `name`, as `Core::open` opens a real one (LC-1, LC-2, 9B, DP-8).
    ///
    /// # Errors
    /// `InvalidConfig`, `MissingWorkerPath` or `DataDirInUse`.
    pub fn open(
        &mut self,
        name: &str,
        config: &OpenConfig,
        run: RunInputs,
        features: Features,
        spawner: Option<Box<dyn Spawner>>,
    ) -> Result<Opened, CoreError> {
        let RunInputs { seed, scheduler } = run;
        let worker_path = check_open(config)?;
        let registry = Arc::clone(self.dirs.entry(name.to_string()).or_default());
        {
            let mut registry = lock(&registry);
            if registry.locked {
                return Err(CoreError::new(
                    ErrorCode::DataDirInUse,
                    format!("another host holds the data directory {name}"),
                ));
            }
            registry.locked = true;
        }
        // DP-8: the epoch strictly increases at every open, kept in the registry under the lock.
        let epoch = {
            let mut registry = lock(&registry);
            let previous = registry
                .rows
                .get("meta/host-epoch")
                .and_then(|b| std::str::from_utf8(b).ok())
                .and_then(|t| t.parse::<u64>().ok())
                .unwrap_or(0);
            let epoch = previous + 1;
            registry.rows.insert(
                "meta/host-epoch".to_string(),
                epoch.to_string().into_bytes(),
            );
            epoch
        };
        let faults = Arc::new(Mutex::new(Faults::default()));
        let edges = SimEdges {
            registry,
            faults: Arc::clone(&faults),
            entropy: SeededEntropy::with_seed(seed),
            wake: Arc::new(SimHostWake::new(scheduler.clone())),
            scheduler: HandleScheduler(scheduler),
            spawner,
            pending: VecDeque::new(),
            links: BTreeMap::new(),
            next_link: 1,
            link_capacity: LINK_CAPACITY,
        };
        let identity = botster_terminal_ghostty::terminal_identity();
        let cfg = EngineConfig {
            limits: config.limits.clone(),
            features,
            host_epoch: epoch,
            worker_path,
            worker_protocol: botster_worker_core::WORKER_PROTOCOL,
            shadow_answerable: Vec::new(),
            terminal_identity: TerminalIdentity {
                term: identity.term,
                terminfo_source: identity.terminfo_source,
            },
        };
        let wake = edges.wake();
        Ok(Opened {
            driver: HostDriver::open(cfg, edges)?,
            faults,
            wake,
        })
    }
}

/// The features that the in-process Core reports (A2-6): what the host engine implements today.
pub fn core_features() -> Features {
    Features {
        names: BTreeSet::from([Feature::Silence, Feature::NotificationPolicy]),
        service_preamble_versions: vec![1],
    }
}

#[cfg(test)]
mod tests;
