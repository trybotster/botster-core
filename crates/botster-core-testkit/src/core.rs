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
    ExitStatus, GroupSignal, ProcessIdentity, SpawnError, StorageError,
};
use botster_core_edges::scheduler::ChoicePoint;
use botster_core_edges::{Entropy, Link, Scheduler, Wake as WakeEdge};
use botster_core_host::driver::{
    check_open, HandoffError, HostDriver, HostEdges, HostWake, WorkerSpawn,
};
use botster_core_host::{EngineConfig, LinkId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// `ENOEXEC`: the errno of a program that cannot be run. It is what an `exec` of a file that is not a program gives.
const ENOEXEC: i32 = 8;

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
}

/// The wake object of the testkit: a level flag that a waiter on any thread sees (TM-6, TH-2).
#[derive(Debug, Default)]
pub struct SimHostWake {
    flag: Mutex<bool>,
    changed: Condvar,
}

impl botster_core_contract::prelude::WakeHandle for SimHostWake {
    fn wait(&self, timeout: Duration) -> Wake {
        let flag = lock(&self.flag);
        if *flag {
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
    fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        None
    }
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
        let mut connect = || {
            let (host, worker) = crate::net::link_pair(64 * 1024);
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

    fn poll_process_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        self.spawner.as_mut().and_then(|s| s.poll_exit())
    }

    fn accept_link(&mut self) -> Option<LinkId> {
        let (link, end) = self.pending.pop_front()?;
        self.links.insert(link, end);
        Some(link)
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

    fn handoff_route(
        &mut self,
        link: LinkId,
        _route: RouteId,
        transport: StreamEndpoint,
        _options: &AttachOptions,
    ) -> Result<(), HandoffError> {
        // The descriptor travels with the link by value (plan 2.3: "descriptor objects passed by value").
        let descriptor = crate::net::Descriptor::new(transport);
        match self.links.get_mut(&link) {
            Some(end) => end.send_descriptor(descriptor).map_err(|_| HandoffError),
            None => Err(HandoffError),
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

impl Directories {
    /// Opens a `Core` over the in-memory directory `name`, as `Core::open` opens a real one (LC-1, LC-2, 9B, DP-8).
    ///
    /// # Errors
    /// `InvalidConfig`, `MissingWorkerPath` or `DataDirInUse`.
    pub fn open(
        &mut self,
        name: &str,
        config: &OpenConfig,
        seed: u64,
        start: Instant,
        features: Features,
        spawner: Option<Box<dyn Spawner>>,
    ) -> Result<(HostDriver<SimEdges>, Arc<Mutex<Faults>>), CoreError> {
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
        let scheduler = SchedulerHandle::with_seed(seed);
        let faults = Arc::new(Mutex::new(Faults::default()));
        let edges = SimEdges {
            registry,
            faults: Arc::clone(&faults),
            entropy: SeededEntropy::with_seed(seed),
            scheduler: HandleScheduler(scheduler),
            wake: Arc::new(SimHostWake::default()),
            spawner,
            pending: VecDeque::new(),
            links: BTreeMap::new(),
            next_link: 1,
        };
        let cfg = EngineConfig {
            limits: config.limits.clone(),
            features,
            host_epoch: epoch,
            worker_path,
            worker_protocol: botster_worker_core::WORKER_PROTOCOL,
            shadow_answerable: Vec::new(),
            terminal_identity: TerminalIdentity {
                term: "xterm-ghostty".to_string(),
                // PLACEHOLDER (Core TI-1, A2-8): the terminfo source of the pinned emulator comes from P2's binding
                // (`botster-terminal-ghostty`), which is not on `v1` yet. Core must not invent it. The follow-up PR wires
                // `botster_terminal_ghostty::terminal_identity()` and removes this value; the a2_8 ids stay pending until then.
                terminfo_source: String::new(),
            },
        };
        Ok((HostDriver::new(cfg, edges, start), faults))
    }
}

/// The features that the in-process Core reports (A2-6): what the host engine implements today.
pub fn core_features() -> Features {
    Features {
        names: BTreeSet::from([Feature::Silence, Feature::NotificationPolicy]),
        service_preamble_versions: vec![1],
    }
}
