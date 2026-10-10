//! The in-process session worker (plan 4.1, Core A5-1): the real `Worker` machine of `botster-worker-core`, stepped by the
//! [`Sim`] on the test thread, with the scripted program as its `Program` edge and an in-memory control link.
//!
//! - [`WorkerSpawner`] is the `Process` edge of the testkit's `Core` for workers: a spawn puts a `Worker` and its edges into
//!   the `Sim`, and a signal or an end of the worker process is a script point of that edge.
//! - [`TestkitCore`] is the `Core` of the testkit (plan 4.1): `pump(now)` lets the `Sim` run the ready work of every worker,
//!   then runs the host's own pump. Workers progress during the host's `pump`, as real workers progress during real time.
//!
//! The worker logic is the machine's and nothing here decides a worker behavior: the edges below only move bytes, start the
//! scripted program, deliver signals and report exits (plan 2.1: a difference between the two runs is a bug in an edge).

use crate::core::{SimEdges, Spawner};
use crate::net::{EndControl, Interest, LinkEnd, StreamEnd};
use crate::program::{ProgramControl, ScriptedProgram};
use crate::resume_controls::{CaptureLog, CaptureRecord, ModelLog};
use crate::scheduler::SchedulerHandle;
use crate::sim::{Binding, MachineNode, Sim};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, WindowSize,
};
use botster_core_edges::{Link, Machine, Program, RouteTransport as _};
use botster_core_host::driver::{HostDriver, HostWake, WorkerSpawn};
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType, DEFAULT_MAX_PAYLOAD};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{Observation, PayloadId, WorkerMsg};
use botster_core_link::proof::{token_proof, TOKEN_LEN};
use botster_route_codec::prelude::QueryKind;
use botster_worker_core::{
    Action, CandidateId, DescriptorId, Drain, Input, PayloadSpec, SpawnFailure, Worker,
    WorkerConfig,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::{Duration, Instant};

/// `ENOEXEC`: the errno of a program that the in-process edge cannot run (a script that is not valid, or a step that needs a
/// real process). It is what an `exec` of a file that is not a program gives.
const ENOEXEC: i32 = 8;

/// `EIO`: the errno of a write to a PTY that is gone or that fails.
const EIO: i32 = rustix::io::Errno::IO.raw_os_error();

/// The most inputs that one `Sim` run handles before it reports a livelock: far above what a transcript's workers do between
/// two host pumps.
const SIM_STEP_LIMIT: usize = 100_000;

/// The bytes of one read of the control link or the PTY.
const READ_CHUNK: usize = 64 * 1024;

/// The wake of the host that holds the control link of the worker of `cell` now. The cell's guard ends before the host's
/// table is locked: `Processes::end` locks a table, then a cell.
fn control_wake(cell: &Mutex<ProcessCell>) -> Option<Arc<dyn HostWake>> {
    let control = lock(cell).control.as_ref().and_then(Weak::upgrade)?;
    let wake = lock(&control).wake.clone();
    wake
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // The testkit has no panic that leaves its state half written, so a poisoned lock still holds usable state.
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// The OS facts of one in-process worker process that the `Process` edge controls.
#[derive(Debug, Default)]
struct ProcessCell {
    /// The worker-control signal (`GroupSignal::EndPayload`) was delivered and the worker has not taken it yet.
    end_payload: bool,
    /// `SIGTERM` was delivered and the worker has not taken it yet (the worker's handler ends its payload, then itself).
    terminate: bool,
    /// The worker process ended: by its own `Exit`, or by a signal of the `Process` edge.
    ended: bool,
    /// The worker's end of its control link breaks at the next turn of the worker (`break_control`): the host reads the end of
    /// the link, the worker takes `LinkClosed`, and the process keeps running.
    break_link: bool,
    /// The worker's end of its control link, for reading what it holds for the host (`edges_quiet`). It outlives the end.
    link: Option<EndControl>,
    /// The worker's payload runs: true from its spawn until its sim process ends (its exit is queued for the worker), its
    /// reap, or the end of the worker (`payload_alive`).
    payload_alive: bool,
    /// The controls of the payload's program edge, from the spawn until the reap or the end of the worker (`pty_output`,
    /// `pty_blocked`): the PTY stays readable after the payload's exit until the worker reaps it.
    program: Option<ProgramControl>,
    /// The table of the host that holds the worker's control link now: the spawning host, then each host that adopts the
    /// worker (the fence, `Action::AdoptLink`). A control's wake goes to that host. Weak: a dropped host's table ends.
    control: Option<Weak<Mutex<Processes>>>,
    /// What reached the worker's terminal model since the payload's spawn (`oracle_resume`). It outlives the end.
    model_log: ModelLog,
    /// The worker machine itself, for reading its live model between pumps (`oracle_resume`).
    worker: Option<SharedWorker>,
}

/// The worker machine, shared with its process cell. The sim is its only writer; a control reads it between pumps.
#[derive(Clone, Debug)]
struct SharedWorker(Arc<Mutex<Worker>>);

impl Machine for SharedWorker {
    type Input = Input;
    type Action = Action;

    fn handle(&mut self, now: Instant, input: Input) {
        lock(&self.0).handle(now, input);
    }

    fn poll_action(&mut self) -> Option<Action> {
        lock(&self.0).poll_action()
    }

    fn next_deadline(&self) -> Option<Instant> {
        lock(&self.0).next_deadline()
    }
}

/// The process table of the workers that one host spawned: identities and the exits that the host has not polled.
#[derive(Default)]
struct Processes {
    /// The workers that this host spawned: it reaps them (their exits come to it only).
    cells: BTreeMap<ProcessIdentity, Arc<Mutex<ProcessCell>>>,
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
    /// The workers whose control link this host holds: the ones that it spawned and did not lose to an adoption, and the
    /// ones that it adopted (`edges_quiet` reads their links).
    links: BTreeMap<ProcessIdentity, Arc<Mutex<ProcessCell>>>,
    /// The worker's end of every connection that this host made to a worker endpoint, from its connect on: its bytes, or
    /// the end of file of a refused candidate, are a report for this host until it reads them (`edges_quiet`).
    connections: Vec<EndControl>,
    /// The wake object of the host that owns the table. An edge event that a control causes between two pumps wakes that
    /// host, as the real event wakes a real host (TM-6).
    wake: Option<Arc<dyn HostWake>>,
}

/// The process table of one host, kept by the harness once the spawner belongs to the host's edges.
pub(crate) struct ProcessTable(Arc<Mutex<Processes>>);

impl ProcessTable {
    /// The wake object of the host that owns the table.
    pub(crate) fn set_wake(&self, wake: Arc<dyn HostWake>) {
        lock(&self.0).wake = Some(wake);
    }

    /// True while the process edge holds a report that the host has not consumed: an exit that it has not polled, or the
    /// control link of a worker that it holds (spawned or adopted) with bytes or an end of file that the host has not read. A new worker's link holds its hello,
    /// so a link that the host has not accepted yet counts too. It reads the state and changes nothing (`edges_quiet`).
    pub(crate) fn holds_reports(&self) -> bool {
        let processes = lock(&self.0);
        !processes.exits.is_empty()
            || processes.connections.iter().any(EndControl::holds_for_peer)
            || processes.links.values().any(|cell| {
                lock(cell)
                    .link
                    .as_ref()
                    .is_some_and(EndControl::holds_for_peer)
            })
    }
}

impl Processes {
    /// Ends the process `id` once and queues its exit for the host.
    fn end(&mut self, id: ProcessIdentity, status: ExitStatus) {
        if let Some(cell) = self.cells.get(&id) {
            let mut cell = lock(cell);
            if !cell.ended {
                cell.ended = true;
                self.exits.push_back((id, status));
            }
        }
    }
}

/// Every worker process of a run, by identity, with the table of the handle that spawned it: one operating system for every
/// handle of the run. A handle's identity probe and signals reach a worker of an earlier handle, as the real ones do (AD-6,
/// LC-12), and the exit of a worker goes to the handle that spawned it only, as a real reaper's does.
type RunProcesses = BTreeMap<ProcessIdentity, (Arc<Mutex<ProcessCell>>, Arc<Mutex<Processes>>)>;

/// The numbers of the in-process processes: workers and payloads share one counter, so every identity is distinct.
#[derive(Debug)]
struct Pids {
    next: u32,
}

impl Pids {
    fn next(&mut self) -> u32 {
        self.next += 1;
        self.next
    }
}

/// A session instance of the run: the data directory of its host and its `InstanceId`. It keys a held start
/// (`hold_start_at`) and a worker endpoint (`<data_dir>/w/<InstanceId>`, DESIGN.md part 1). An `InstanceId` names one
/// incarnation in its registry (Core ID-1), and each new directory starts its host epoch at 1 (DP-8), so two directories of
/// one run can have the same instance: the directory is part of the key.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct InstanceKey {
    pub(crate) dir: String,
    pub(crate) instance: InstanceId,
}

/// A connection to a worker endpoint: the worker's end, and the table of the host that connected (`None` for a process that
/// is not a host). An adoption over it moves the worker's control link to that host.
type Connection = (LinkEnd, Option<Arc<Mutex<Processes>>>);

/// The connections that wait on one worker endpoint, oldest first (DESIGN.md part 6).
type Endpoint = Arc<Mutex<VecDeque<Connection>>>;

/// The endpoint of each worker of the run (DESIGN.md parts 1, 6): bound at the spawn, before the first hello. A worker that
/// exits removes its own. A killed worker cannot: its endpoint stays as `None`, a socket file that no worker listens on,
/// until the host's `Remove` or a cleaner removes it. Every handle of the run reaches it, as a path in one file system.
type Endpoints = Arc<Mutex<BTreeMap<InstanceKey, Option<Endpoint>>>>;

/// The field of the handshake that an impostor gets wrong (`impostor_worker`, Core A10-1), or the protocol that a stand-in
/// for the worker announces (`announce_protocol`, Core A6-2, AD-4).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImpostorField {
    /// The proof is made with another token.
    Token,
    /// The hello names another `InstanceId`.
    Instance,
    /// The hello proves the recorded token and instance, and it announces this worker protocol number. The in-process worker
    /// has one protocol, and no worker of an invented protocol is built (the control's row): only Core's version check is
    /// under test.
    Protocol(u8),
}

/// A worker-shaped frame that an impostor sends behind its hello, so that Core meets it after its check rejected the
/// handshake (Core A11-1, steward ruling R-42).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ImpostorFrame {
    /// A cleanup result of a `Remove` that claims every upload deleted.
    CleanupDeleted,
    /// A state report that claims the payload ended.
    StateExited,
    /// A notification.
    Notification,
}

/// What `impostor_worker` sets for one session: the process that answers its endpoint at the next adoption instead of its
/// worker. The real worker keeps running and is never connected.
pub(crate) struct ImpostorPlan {
    /// The session, so that `signals_received` finds the impostor after `Remove` deleted the row.
    pub(crate) session: SessionId,
    pub(crate) field: ImpostorField,
    pub(crate) script: Vec<ImpostorFrame>,
    /// The session's recorded token, so that a wrong instance is the only fault of an `Instance` impostor.
    pub(crate) token: [u8; TOKEN_LEN],
    /// The worker identity that the row records: the process that Core's AD-6 check refuses is at it.
    pub(crate) worker: ProcessIdentity,
}

/// The state of one impostor of the run.
struct Impostor {
    plan: ImpostorPlan,
    /// The connection that it answered, once a host connected.
    link: Option<ImpostorLink>,
    /// The group signals that Core sent to the recorded identity after the impostor answered (`signals_received`).
    signals: Vec<GroupSignal>,
}

/// An impostor's end of a host's connection.
struct ImpostorLink {
    end: LinkEnd,
    decoder: FrameDecoder,
    answered: bool,
    done: bool,
}

impl Impostor {
    /// The impostor's connection has bytes or an end of file that it has not read.
    fn has_ready(&self) -> bool {
        self.link
            .as_ref()
            .is_some_and(|l| !l.done && l.end.is_ready())
    }

    /// Reads what the host sent. On the host's first hello it answers with the wrong hello of its plan (Core A10-1) and its
    /// scripted frames, in one write (Core A11-1, steward ruling R-42: "after the rejection" is the order in which Core meets
    /// the frames). At the end of file, Core has closed the link: the impostor closes its end.
    ///
    /// # Panics
    /// When the link does not take the whole write: R-42 makes a short or failed write a setup failure, never a pass.
    fn step(&mut self) {
        let plan = &self.plan;
        let Some(link) = self.link.as_mut().filter(|l| !l.done) else {
            return;
        };
        let mut buf = [0u8; 4096];
        loop {
            match link.end.recv(&mut buf) {
                Ok(0) => {
                    link.done = true;
                    link.end.close();
                    return;
                }
                Ok(n) => {
                    let mut rest = &buf[..n];
                    while !rest.is_empty() {
                        let took = link.decoder.push(rest);
                        rest = &rest[took..];
                        while let Ok(Some(frame)) = link.decoder.next_frame() {
                            if frame.kind != FrameType::HELLO || link.answered {
                                continue;
                            }
                            if let Ok(hello) = Hello::decode(&frame.payload) {
                                let mut answer = impostor_hello(plan, &hello);
                                for scripted in &plan.script {
                                    answer.extend(script_frame(*scripted));
                                }
                                let sent = link.end.send(&answer).ok();
                                assert_eq!(
                                    sent,
                                    Some(answer.len()),
                                    "R-42: the impostor's hello and its script are one whole write"
                                );
                                link.answered = true;
                            }
                        }
                        if took == 0 {
                            break;
                        }
                    }
                }
                Err(_) => return,
            }
        }
    }
}

/// The impostor's hello to the host's hello `host`: the instance of the endpoint and a proof from another token, or another
/// instance with a proof from the recorded token (Core A10-1). Either one fails Core's AD-6 check.
fn impostor_hello(plan: &ImpostorPlan, host: &Hello) -> Vec<u8> {
    let protocol = botster_worker_core::WORKER_PROTOCOL;
    let (instance, token, protocol) = match plan.field {
        ImpostorField::Token => (host.instance.clone(), plan.token.map(|b| !b), protocol),
        ImpostorField::Instance => (
            InstanceId(format!("{}-impostor", host.instance.0)),
            plan.token,
            protocol,
        ),
        ImpostorField::Protocol(announced) => (host.instance.clone(), plan.token, announced),
    };
    let hello = Hello {
        protocol,
        proof: token_proof(&token, &instance, host.host_epoch),
        instance,
        host_epoch: host.host_epoch,
    };
    let mut payload = Vec::new();
    hello.encode(&mut payload).expect("a hello encodes");
    framed(FrameType::HELLO, &payload)
}

/// A scripted worker-shaped frame of Core A11-1.
fn script_frame(frame: ImpostorFrame) -> Vec<u8> {
    let msg = match frame {
        ImpostorFrame::CleanupDeleted => WorkerMsg::RemoveResult {
            uploads: UploadsOutcome::Deleted,
        },
        ImpostorFrame::StateExited => WorkerMsg::Exited {
            code: Some(0),
            signal: None,
        },
        ImpostorFrame::Notification => WorkerMsg::Observed {
            observation: Observation::Notification {
                source: NotificationSource::Osc9,
                title: None,
                body: "impostor".into(),
                truncated: false,
            },
        },
    };
    let mut payload = Vec::new();
    msg.encode(&mut payload);
    framed(FrameType::WORKER_MSG, &payload)
}

fn framed(kind: FrameType, payload: &[u8]) -> Vec<u8> {
    let mut out = Vec::new();
    encode_frame(kind, payload, DEFAULT_MAX_PAYLOAD, &mut out).expect("a small frame encodes");
    out
}

/// The `Sim` of one harness: every in-process worker of every handle (plan 4.1: "a `Sim` owns ... every `Worker`").
#[derive(Clone)]
pub struct Workers {
    sim: Arc<Mutex<Sim>>,
    pids: Arc<Mutex<Pids>>,
    run_processes: Arc<Mutex<RunProcesses>>,
    /// The starts that are held before the payload's launch (`hold_start_at`, AD-7 step 4).
    held_starts: Arc<Mutex<BTreeSet<InstanceKey>>>,
    endpoints: Endpoints,
    /// `withhold_control_link`: the sessions whose live worker does not take a host's connection. The connections wait in
    /// `withheld_links`, never read, so the host's deadline ends the adoption (Core A6-1, AD-2).
    withheld: Arc<Mutex<BTreeSet<InstanceKey>>>,
    withheld_links: Arc<Mutex<Vec<LinkEnd>>>,
    /// `impostor_worker`: the session endpoints that an impostor answers at the next adoption (Core A10-1, A11-1).
    impostors: Arc<Mutex<BTreeMap<InstanceKey, Impostor>>>,
    scheduler: SchedulerHandle,
    read_chunk: usize,
}

impl std::fmt::Debug for Workers {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Workers").finish_non_exhaustive()
    }
}

impl Workers {
    /// The scheduler of the run: the one seeded stream that the host's edges share (A5-2).
    pub fn scheduler(&self) -> SchedulerHandle {
        self.scheduler.clone()
    }

    /// The workers of a run whose seeded stream is `scheduler` (A5-2) and whose virtual clock starts at `start`.
    pub fn new(scheduler: SchedulerHandle, start: Instant) -> Workers {
        Self::with_read_chunk(scheduler, start, READ_CHUNK)
    }

    /// One positive read bound applies to control bytes and program bytes.
    pub(crate) fn with_read_chunk(
        scheduler: SchedulerHandle,
        start: Instant,
        read_chunk: usize,
    ) -> Workers {
        assert!(read_chunk > 0, "a worker needs a positive read bound");
        Workers {
            sim: Arc::new(Mutex::new(Sim::with_scheduler(scheduler.clone(), start))),
            pids: Arc::new(Mutex::new(Pids { next: 1000 })),
            run_processes: Arc::default(),
            held_starts: Arc::default(),
            endpoints: Arc::default(),
            withheld: Arc::default(),
            withheld_links: Arc::default(),
            impostors: Arc::default(),
            scheduler,
            read_chunk,
        }
    }

    /// Runs every ready input of every worker at `now`, until none is ready.
    ///
    /// # Panics
    /// When the workers still have ready work after [`SIM_STEP_LIMIT`] inputs: a worker that makes work for itself without
    /// end is a defect, never a result.
    pub fn run(&self, now: Instant) {
        // A pump is a step of the program edge: the input cap of `pty_chunk` is available again.
        for program in self.programs() {
            program.new_step();
        }
        {
            let mut sim = lock(&self.sim);
            sim.advance_to(now);
            if let Err(livelock) = sim.run_until_idle(SIM_STEP_LIMIT) {
                panic!("the in-process workers did not settle: {livelock:?}");
            }
        }
        for impostor in lock(&self.impostors).values_mut() {
            impostor.step();
        }
    }

    /// Runs the workers' ready work at the virtual clock, with no step of the program edge and no host pump: the route data
    /// plane is the worker's, so a route progresses with no pump (Core TH-3, OU-9).
    pub fn run_ready(&self) {
        if let Err(livelock) = lock(&self.sim).run_until_idle(SIM_STEP_LIMIT) {
            panic!("the in-process workers did not settle: {livelock:?}");
        }
    }

    /// True when a worker has ready work at the virtual clock, a PTY write waits for the next step of `pty_chunk`, or an
    /// impostor has bytes or an end of file to read.
    pub fn has_ready(&self) -> bool {
        lock(&self.impostors).values().any(Impostor::has_ready)
            || lock(&self.sim).has_ready()
            || self
                .programs()
                .iter()
                .any(ProgramControl::waits_for_next_step)
    }

    /// `withhold_control_link`: the live worker of `key` takes no host's connection from now on (Core A6-1).
    ///
    /// # Errors
    /// Its connections are already withheld.
    pub(crate) fn withhold(&self, key: InstanceKey) -> Result<(), String> {
        let text = format!("{} in {}", key.instance.0, key.dir);
        if lock(&self.withheld).insert(key) {
            Ok(())
        } else {
            Err(format!("the control link of {text} is already withheld"))
        }
    }

    /// `impostor_worker`: an impostor answers the endpoint of `key` at the next adoption (Core A10-1).
    ///
    /// # Errors
    /// An impostor is already set for `key`.
    pub(crate) fn impostor(&self, key: InstanceKey, plan: ImpostorPlan) -> Result<(), String> {
        let text = format!("{} in {}", key.instance.0, key.dir);
        let mut impostors = lock(&self.impostors);
        if impostors.contains_key(&key) {
            return Err(format!("an impostor already answers for {text}"));
        }
        impostors.insert(
            key,
            Impostor {
                plan,
                link: None,
                signals: Vec::new(),
            },
        );
        Ok(())
    }

    /// `signals_received`: the group signals that Core sent to the process that the AD-6 check refused, the impostor of the
    /// session `session` of the data directory `dir`, in order. `None` when no impostor was set for that session.
    pub(crate) fn impostor_signals(
        &self,
        dir: &str,
        session: &SessionId,
    ) -> Option<Vec<GroupSignal>> {
        let impostors = lock(&self.impostors);
        let mut found = impostors
            .iter()
            .filter(|(key, i)| key.dir == dir && i.plan.session == *session)
            .peekable();
        found.peek()?;
        Some(found.flat_map(|(_, i)| i.signals.iter().copied()).collect())
    }

    /// The program edges of every payload of the run that is not reaped.
    fn programs(&self) -> Vec<ProgramControl> {
        lock(&self.run_processes)
            .values()
            .filter_map(|(cell, _)| lock(cell).program.clone())
            .collect()
    }

    /// The earliest deadline of a worker.
    pub fn next_deadline(&self) -> Option<Instant> {
        lock(&self.sim).next_deadline()
    }

    /// The worker's end of the control link of the process `identity` breaks (`break_control`). The break is an edge event:
    /// the worker's edges deliver it at the worker's next turn, and the host that holds its control link now is woken.
    ///
    /// # Errors
    /// The process is not a worker of this run, or it has ended.
    pub(crate) fn break_link(&self, identity: ProcessIdentity) -> Result<(), String> {
        let (cell, _) = lock(&self.run_processes)
            .get(&identity)
            .cloned()
            .ok_or_else(|| format!("no worker process {identity:?} in this run"))?;
        {
            let mut cell = lock(&cell);
            if cell.ended {
                return Err(format!("the worker process {identity:?} has ended"));
            }
            cell.break_link = true;
        }
        if let Some(wake) = control_wake(&cell) {
            wake.signal();
        }
        Ok(())
    }

    /// `lose_worker`: the process edge ends the worker process `identity`, as a kill from outside Core does (Core AD-2, IN-7).
    /// Its exit goes to the host that spawned it, and its link, its endpoint and its payload end with it
    /// (`WorkerEdges::ended`). The host that spawned it and the host that holds its control link are woken (TM-6).
    ///
    /// # Errors
    /// The process is not a worker of this run, or it has ended.
    pub(crate) fn lose_worker(&self, identity: ProcessIdentity) -> Result<(), String> {
        let (cell, owner) = lock(&self.run_processes)
            .get(&identity)
            .cloned()
            .ok_or_else(|| format!("no worker process {identity:?} in this run"))?;
        if lock(&cell).ended {
            return Err(format!("the worker process {identity:?} has ended"));
        }
        // The cell guard ends before a table is locked: `Processes::end` locks a table, then the cell.
        let control = control_wake(&cell);
        let spawner = {
            let mut owner = lock(&owner);
            owner.end(identity, ExitStatus::Signal(9));
            owner.wake.clone()
        };
        for wake in [spawner, control].into_iter().flatten() {
            wake.signal();
        }
        Ok(())
    }

    /// Holds the start `key` before the payload's launch (`hold_start_at`, AD-7 step 4): its worker takes the host's
    /// `Launch`, and no payload is spawned until [`Workers::release_start`].
    ///
    /// # Errors
    /// The start of `key` is already held.
    pub(crate) fn hold_start(&self, key: InstanceKey) -> Result<(), String> {
        let text = format!("{} in {}", key.instance.0, key.dir);
        if lock(&self.held_starts).insert(key) {
            Ok(())
        } else {
            Err(format!("the start of {text} is already held"))
        }
    }

    /// Ends the hold of [`Workers::hold_start`]. A worker that kept a spawn, the process `worker`, spawns its payload at its
    /// next turn, and the host that holds its control link is woken.
    ///
    /// # Errors
    /// No hold of `key` remains: none was set, it was released, or its worker ended.
    pub(crate) fn release_start(
        &self,
        key: &InstanceKey,
        worker: Option<ProcessIdentity>,
    ) -> Result<(), String> {
        if !lock(&self.held_starts).remove(key) {
            return Err(format!(
                "no hold of the start of {} in {}",
                key.instance.0, key.dir
            ));
        }
        let cell = worker.and_then(|id| lock(&self.run_processes).get(&id).cloned());
        if let Some(wake) = cell.and_then(|(cell, _)| control_wake(&cell)) {
            wake.signal();
        }
        Ok(())
    }

    /// What reached the terminal model of the worker process `identity` (`oracle_resume`).
    ///
    /// # Errors
    /// The process is not a worker of this run.
    pub(crate) fn model_log(&self, identity: ProcessIdentity) -> Result<ModelLog, String> {
        lock(&self.run_processes)
            .get(&identity)
            .map(|(cell, _)| lock(cell).model_log.clone())
            .ok_or_else(|| format!("no worker process {identity:?} in this run"))
    }

    /// A snapshot of the live model of the worker process `identity` (`Worker::model_snapshot`, `oracle_resume`): `None`
    /// before its launch made the model.
    ///
    /// # Errors
    /// The process is not a worker of this run, it has ended, or its model could not make the snapshot.
    pub(crate) fn model_snapshot(
        &self,
        identity: ProcessIdentity,
    ) -> Result<Option<Vec<u8>>, String> {
        let worker = lock(&self.run_processes)
            .get(&identity)
            .and_then(|(cell, _)| lock(cell).worker.clone())
            .ok_or_else(|| format!("no worker machine of the process {identity:?} in this run"))?;
        let snapshot = lock(&worker.0).model_snapshot();
        snapshot
            .transpose()
            .map_err(|e| format!("the model of {identity:?} made no snapshot: {e:?}"))
    }

    /// The output of the worker process `identity` reaches its log but not its model: a worker that stopped stepping
    /// (the negative proof of `oracle_resume`).
    #[cfg(test)]
    pub(crate) fn log_unapplied_output(&self, identity: ProcessIdentity, bytes: &[u8]) {
        if let Some((cell, _)) = lock(&self.run_processes).get(&identity) {
            lock(cell).model_log.read(bytes);
        }
    }

    /// The worker machine of `identity` steps output that its log never got: its live model diverges from the capture and
    /// the suffix (the negative proof of `oracle_resume`).
    #[cfg(test)]
    pub(crate) fn apply_unlogged_output(
        &self,
        identity: ProcessIdentity,
        bytes: &[u8],
        now: Instant,
    ) {
        let worker = lock(&self.run_processes)
            .get(&identity)
            .and_then(|(cell, _)| lock(cell).worker.clone())
            .expect("a worker machine");
        lock(&worker.0).handle(now, Input::PtyOutput(bytes.to_vec()));
    }

    /// True while the payload of the worker process `identity` runs (`payload_alive`). The payload is in the worker's process
    /// group, so it ends with the worker; a process that is not a worker of this run has no payload.
    pub(crate) fn payload_alive(&self, identity: ProcessIdentity) -> bool {
        lock(&self.run_processes)
            .get(&identity)
            .is_some_and(|(cell, _)| {
                let cell = lock(cell);
                cell.payload_alive && !cell.ended
            })
    }

    /// The program edge of the payload of the worker process `identity` (`pty_output`, `pty_blocked`), and the wake of the
    /// host that holds its control link now (the spawning host, or the host that adopted it). A control that makes the edge ready signals that wake, as the real edge event wakes a
    /// real host (TM-6).
    ///
    /// # Errors
    /// The process is not a worker of this run, it has ended, or it has no payload.
    pub(crate) fn program_edge(
        &self,
        identity: ProcessIdentity,
    ) -> Result<(ProgramControl, Option<Arc<dyn HostWake>>), String> {
        self.program_edge_between(identity, || {})
    }

    /// [`Workers::program_edge`]: `between` runs after the cell is read and before the control host's table is locked, so a
    /// test can put the end of the process exactly there (F63).
    fn program_edge_between(
        &self,
        identity: ProcessIdentity,
        between: impl FnOnce(),
    ) -> Result<(ProgramControl, Option<Arc<dyn HostWake>>), String> {
        let (cell, _) = lock(&self.run_processes)
            .get(&identity)
            .cloned()
            .ok_or_else(|| format!("no worker process {identity:?} in this run"))?;
        // The cell guard ends before a table is locked: `Processes::end` locks a table, then the cell.
        let program = {
            let cell = lock(&cell);
            if cell.ended {
                return Err(format!("the worker process {identity:?} has ended"));
            }
            cell.program
                .clone()
                .ok_or_else(|| format!("the worker process {identity:?} has no payload"))?
        };
        between();
        Ok((program, control_wake(&cell)))
    }

    /// A cleaner removes the endpoint of the worker of `key` (DESIGN.md part 1): a live worker keeps running, and a new
    /// host's connect fails. False when there is no endpoint (it was never bound, or it was removed).
    // Only the tests of the endpoint use it: the adoption controls act at the spawner's connect.
    #[cfg(test)]
    pub(crate) fn unlink_endpoint(&self, key: &InstanceKey) -> bool {
        lock(&self.endpoints).remove(key).is_some()
    }

    /// A process of this user that is not a host connects to the endpoint of the live worker of `key` (AD-6: the endpoint
    /// is closed to other users only). The caller holds the connection's other end. `None` when no worker listens there.
    // Only the tests of the endpoint use it: the adoption controls act at the spawner's connect.
    #[cfg(test)]
    pub(crate) fn connect_endpoint(&self, key: &InstanceKey) -> Option<LinkEnd> {
        let endpoint = lock(&self.endpoints).get(key).cloned().flatten()?;
        let (stranger, worker) = crate::net::link_pair(self.read_chunk);
        lock(&endpoint).push_back((worker, None));
        Some(stranger)
    }

    /// True when no edge toward the host of `table` holds a report that it has not consumed: no worker has ready work (input
    /// to read, bytes to write, a payload's output or exit, a signal), and the host's process table holds no exit and no
    /// unread link report (`edges_quiet`). The fence of `await_quiet` (Core A5-2).
    pub(crate) fn edges_quiet(&self, table: &ProcessTable) -> bool {
        !self.has_ready() && !table.holds_reports()
    }

    /// The `Process` edge of the host of the data directory `dir` for its workers.
    pub fn spawner(&self, dir: &str) -> WorkerSpawner {
        WorkerSpawner {
            workers: self.clone(),
            dir: dir.to_string(),
            processes: Arc::default(),
        }
    }
}

/// The `Process` edge of the testkit's `Core` for workers (P1's [`Spawner`]).
pub struct WorkerSpawner {
    workers: Workers,
    /// The data directory of the host: with an instance, it names a start (`InstanceKey`).
    dir: String,
    processes: Arc<Mutex<Processes>>,
}

impl WorkerSpawner {
    /// The process table of this host, for the harness.
    pub(crate) fn table(&self) -> ProcessTable {
        ProcessTable(Arc::clone(&self.processes))
    }
}

impl WorkerSpawner {
    fn key(&self, instance: &InstanceId) -> InstanceKey {
        InstanceKey {
            dir: self.dir.clone(),
            instance: instance.clone(),
        }
    }
}

impl Spawner for WorkerSpawner {
    /// AD-7 step 2: the worker exists, binds its endpoint and connects to the host; its payload waits for the host's
    /// `Launch`.
    fn spawn(
        &mut self,
        spec: &WorkerSpawn,
        connect: &mut dyn FnMut() -> LinkEnd,
    ) -> Result<ProcessIdentity, SpawnError> {
        let id = ProcessIdentity {
            pid: lock(&self.workers.pids).next(),
            start_time: 1,
        };
        let cell = Arc::new(Mutex::new(ProcessCell::default()));
        lock(&cell).control = Some(Arc::downgrade(&self.processes));
        {
            let mut processes = lock(&self.processes);
            processes.cells.insert(id, Arc::clone(&cell));
            processes.links.insert(id, Arc::clone(&cell));
        }
        lock(&self.workers.run_processes)
            .insert(id, (Arc::clone(&cell), Arc::clone(&self.processes)));
        let key = self.key(&spec.instance);
        let endpoint = Endpoint::default();
        lock(&self.workers.endpoints).insert(key.clone(), Some(Arc::clone(&endpoint)));
        let mut link = connect();
        lock(&cell).link = Some(link.end().control());
        let worker = Worker::new(WorkerConfig {
            startup: spec.startup,
            ..WorkerConfig::new(spec.instance.clone(), spec.token, spec.host_epoch)
        });
        let mut edges = WorkerEdges {
            id,
            cell,
            processes: Arc::clone(&self.processes),
            pids: Arc::clone(&self.workers.pids),
            key,
            held_starts: Arc::clone(&self.workers.held_starts),
            endpoint,
            endpoints: Arc::clone(&self.workers.endpoints),
            candidates: BTreeMap::new(),
            next_candidate: 0,
            held_spawn: None,
            scheduler: self.workers.scheduler.clone(),
            link,
            link_open: true,
            outbound: VecDeque::new(),
            written: 0,
            payload: None,
            spawned: None,
            exit: None,
            drain: None,
            pty_write: None,
            wait_writable: false,
            ready: Vec::new(),
            read_chunk: self.workers.read_chunk,
            descriptors: BTreeMap::new(),
            next_descriptor: 0,
            routes: BTreeMap::new(),
            pty_budget: None,
        };
        let mut worker = SharedWorker(Arc::new(Mutex::new(worker)));
        lock(&edges.cell).worker = Some(worker.clone());
        let mut sim = lock(&self.workers.sim);
        // The worker's first actions (its hello) come from its construction, before any input: they are performed here, as
        // the real driver performs them before its first poll.
        let now = sim.now();
        while let Some(action) = worker.poll_action() {
            edges.perform(now, action);
        }
        sim.add(Box::new(MachineNode::new(worker, edges)));
        Ok(id)
    }

    /// `EndPayload` and `Term` reach the worker's handlers: the worker takes them as inputs. A `Kill` ends the worker
    /// process, and its exit is reported to the handle that spawned it. A worker of an earlier handle of the run is reached
    /// too: the signal goes to a process, not to a handle.
    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal) {
        // `signals_received`: a signal to the identity that an impostor answered for reaches that process too.
        for impostor in lock(&self.workers.impostors).values_mut() {
            if impostor.link.is_some() && impostor.plan.worker == identity {
                impostor.signals.push(signal);
            }
        }
        let Some((cell, owner)) = lock(&self.workers.run_processes).get(&identity).cloned() else {
            return;
        };
        match signal {
            GroupSignal::EndPayload => {
                let mut cell = lock(&cell);
                if !cell.ended {
                    cell.end_payload = true;
                }
            }
            GroupSignal::Term => {
                let mut cell = lock(&cell);
                if !cell.ended {
                    cell.terminate = true;
                }
            }
            GroupSignal::Kill => lock(&owner).end(identity, ExitStatus::Signal(9)),
        }
    }

    /// A worker of any handle of the run matches until it ended.
    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState {
        match lock(&self.workers.run_processes).get(&identity) {
            Some((cell, _)) if !lock(cell).ended => IdentityState::Matches,
            _ => IdentityState::Absent,
        }
    }

    fn poll_exit(&mut self) -> Option<(ProcessIdentity, ExitStatus)> {
        lock(&self.processes).exits.pop_front()
    }

    /// A live worker of this data directory listens on its endpoint: it takes `end` as a candidate.
    fn connect_worker(&mut self, instance: &InstanceId, mut end: LinkEnd) -> bool {
        let Some(endpoint) = lock(&self.workers.endpoints)
            .get(&self.key(instance))
            .cloned()
            .flatten()
        else {
            return false;
        };
        lock(&self.processes).connections.push(end.end().control());
        let key = self.key(instance);
        if lock(&self.workers.withheld).contains(&key) {
            // The worker never reads the connection: the host's hello waits until the host's deadline.
            lock(&self.workers.withheld_links).push(end);
            return true;
        }
        if let Some(impostor) = lock(&self.workers.impostors)
            .get_mut(&key)
            .filter(|i| i.link.is_none())
        {
            end.end().set_interest(Interest {
                read: true,
                write: false,
            });
            impostor.link = Some(ImpostorLink {
                end,
                decoder: FrameDecoder::new(DEFAULT_MAX_PAYLOAD),
                answered: false,
                done: false,
            });
            return true;
        }
        lock(&endpoint).push_back((end, Some(Arc::clone(&self.processes))));
        true
    }

    fn remove_endpoint(&mut self, instance: &InstanceId) {
        lock(&self.workers.endpoints).remove(&self.key(instance));
    }
}

/// One ready input of a worker, in the order of plan 2.4: control first, then the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ready {
    EndPayload,
    Terminate,
    /// The link has bytes, or its peer closed it.
    Link,
    /// `break_control` broke the worker's end of the link.
    LinkBroken,
    /// A connection waits on the endpoint.
    Accept,
    /// A candidate has bytes, or its peer closed it.
    Candidate(CandidateId),
    /// The link takes bytes and some of `outbound` waits for it.
    Flush,
    Spawned,
    /// The hold of the start ended, and the worker kept a spawn: the spawn runs now, and its answer is the input.
    HeldSpawn,
    /// A `PtyWrite` waits to be offered to the program.
    PtyWrite,
    /// The program takes input again after a write that it did not take.
    PtyWritable,
    PtyRead,
    PtyDrained,
    Exited,
    /// A `RouteWrite` waits, and the route's stream would take bytes now.
    RouteWrite(RouteId),
    /// The route's stream takes bytes again after a write that it did not take.
    RouteWritable(RouteId),
    /// The route's stream has client bytes, or the client closed it.
    RouteRead(RouteId),
}

/// The most bytes that one read of a route's stream takes (a socket read's buffer).
const ROUTE_READ_BYTES: usize = 64 * 1024;

/// The worker's end of a route's stream (DP-2), and its one outstanding write.
#[derive(Debug)]
struct RouteEdge {
    end: StreamEnd,
    write: Option<Vec<u8>>,
    wait_writable: bool,
    /// The client's end closed, or a read failed: the machine was told once, and the stream is not read again.
    ended: bool,
}

/// The edges of one in-process worker: the control link, the scripted program on its PTY, and its process cell.
///
/// `ready` reads no edge and writes none: it only reads flags, so the order of every effect is the scheduler's (plan 2.5
/// rule 8). Each effect is one input: a read of the link or the PTY, a write of queued link bytes (`LinkWritten`), a PTY
/// write, a spawn's answer, an exit.
struct WorkerEdges {
    id: ProcessIdentity,
    cell: Arc<Mutex<ProcessCell>>,
    processes: Arc<Mutex<Processes>>,
    pids: Arc<Mutex<Pids>>,
    /// The session instance that the worker serves.
    key: InstanceKey,
    /// The run's held starts (`hold_start_at`).
    held_starts: Arc<Mutex<BTreeSet<InstanceKey>>>,
    /// The payload spawn that the worker asked for while its start was held (AD-7 step 4).
    held_spawn: Option<PayloadSpec>,
    /// This worker's endpoint, and the run's table of endpoints (the worker's exit removes its own).
    endpoint: Endpoint,
    endpoints: Endpoints,
    /// The accepted connections whose hello has not passed.
    candidates: BTreeMap<CandidateId, Connection>,
    next_candidate: u64,
    scheduler: SchedulerHandle,
    link: LinkEnd,
    link_open: bool,
    /// Bytes of `LinkSend` that the link has not taken yet, in order.
    outbound: VecDeque<u8>,
    /// The bytes of `LinkSend` written so far (`Input::LinkWritten`).
    written: u64,
    payload: Option<ScriptedProgram>,
    /// The answer to `SpawnPayload`, until the worker takes it.
    spawned: Option<Result<PayloadId, SpawnFailure>>,
    /// The exit of the payload that the program edge reported and the worker has not taken yet.
    exit: Option<ExitStatus>,
    /// The drain that a `DrainPty` asked for (`Drain`); `None` when no drain is asked.
    drain: Option<Drain>,
    /// The bytes of a `PtyWrite` that the program has not been offered yet.
    pty_write: Option<Vec<u8>>,
    /// The program took no byte at the last write: `PtyWritable` follows its write readiness.
    wait_writable: bool,
    /// The inputs counted by the last `ready`.
    ready: Vec<Ready>,
    read_chunk: usize,
    /// The route streams that the link delivered and no `AttachRoute` bound yet, by the id the machine has.
    descriptors: BTreeMap<DescriptorId, StreamEnd>,
    next_descriptor: u64,
    /// The bound route streams.
    routes: BTreeMap<RouteId, RouteEdge>,
    /// The PTY bytes that the worker may still read (`PtyReadBudget`, OU-3d); `None`: no limit.
    pty_budget: Option<usize>,
}

impl WorkerEdges {
    /// The end of the worker process: the OS closes its descriptors, so its link, its endpoint and every connection on it
    /// close, and its payload's PTY is gone. A worker that exits (`exited`) removes its endpoint; a killed one leaves it,
    /// and no worker listens on it. A spawn that a hold kept is dropped with its hold: no payload runs for the session later.
    fn ended(&mut self, exited: bool) {
        if self.link_open {
            self.link_open = false;
            self.link.close();
        }
        let mut endpoints = lock(&self.endpoints);
        if exited {
            endpoints.remove(&self.key);
        } else if let Some(endpoint) = endpoints.get_mut(&self.key) {
            *endpoint = None;
        }
        drop(endpoints);
        for (mut end, _) in std::mem::take(&mut *lock(&self.endpoint)) {
            end.close();
        }
        for (_, (mut end, _)) in std::mem::take(&mut self.candidates) {
            end.close();
        }
        // The process's route streams close with it.
        for (_, mut end) in std::mem::take(&mut self.descriptors) {
            end.close();
        }
        for (_, mut route) in std::mem::take(&mut self.routes) {
            route.end.close();
        }
        self.outbound.clear();
        self.payload = None;
        let mut cell = lock(&self.cell);
        cell.payload_alive = false;
        cell.program = None;
        drop(cell);
        self.held_spawn = None;
        lock(&self.held_starts).remove(&self.key);
    }

    fn start_held(&self) -> bool {
        lock(&self.held_starts).contains(&self.key)
    }

    /// Queues the payload's exit for the worker once its process ended. From then on, the payload is not alive.
    fn poll_payload_exit(&mut self) {
        let Some(program) = self.payload.as_mut() else {
            return;
        };
        if self.exit.is_none() {
            self.exit = program.poll_exit();
            if self.exit.is_some() {
                lock(&self.cell).payload_alive = false;
            }
        }
    }

    fn spawn_payload(&mut self, spec: &PayloadSpec) -> Result<PayloadId, SpawnFailure> {
        // LC-4: the working directory is checked in the test's real file system, as the real edge checks it.
        if !std::path::Path::new(&spec.cwd).is_dir() {
            return Err(SpawnFailure::CwdMissing);
        }
        let mut program = ScriptedProgram::from_argv(&spec.argv, &self.scheduler)
            .map_err(|_| SpawnFailure::Exec { errno: ENOEXEC })?;
        let window = WindowSize {
            cols: u16::try_from(spec.size.cols).unwrap_or(u16::MAX),
            rows: u16::try_from(spec.size.rows).unwrap_or(u16::MAX),
            width_px: 0,
            height_px: 0,
        };
        program
            .resize(window)
            .map_err(|_| SpawnFailure::Exec { errno: ENOEXEC })?;
        let mut cell = lock(&self.cell);
        cell.program = Some(program.control());
        cell.model_log = ModelLog::new(spec.size);
        drop(cell);
        self.payload = Some(program);
        lock(&self.cell).payload_alive = true;
        Ok(PayloadId {
            pid: lock(&self.pids).next(),
            start_time: 1,
        })
    }

    /// Writes what the link takes now and reports the bytes written in all (`LinkWritten`). A link whose peer is gone ends:
    /// its bytes are lost (`LinkClosed`); a full link is written later.
    fn flush(&mut self) -> Input {
        while !self.outbound.is_empty() {
            let (head, _) = self.outbound.as_slices();
            match self.link.send(head) {
                Ok(0) => break,
                Ok(n) => {
                    self.outbound.drain(..n);
                    self.written += n as u64;
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(_) => {
                    self.outbound.clear();
                    self.link_open = false;
                    self.link.close();
                    return Input::LinkClosed;
                }
            }
        }
        Input::LinkWritten {
            total: self.written,
        }
    }
}

impl Binding<SharedWorker> for WorkerEdges {
    fn ready(&mut self, _now: Instant, machine: &SharedWorker) -> usize {
        self.ready.clear();
        if lock(&self.cell).ended {
            self.ended(false);
            return 0;
        }
        // The revision after the last input, at the output read so far (`oracle_resume`).
        let rev = lock(&machine.0).model_rev();
        lock(&self.cell).model_log.rev(rev);
        if lock(&self.cell).end_payload {
            self.ready.push(Ready::EndPayload);
        }
        if lock(&self.cell).terminate {
            self.ready.push(Ready::Terminate);
        }
        if self.link_open && lock(&self.cell).break_link {
            self.ready.push(Ready::LinkBroken);
        }
        if self.link_open {
            // Plan 2.5: read interest always; write interest while bytes wait.
            self.link.end().set_interest(Interest {
                read: true,
                write: !self.outbound.is_empty(),
            });
            let flags = self.link.end().readiness();
            if flags.readable {
                self.ready.push(Ready::Link);
            }
            if flags.writable && !self.outbound.is_empty() {
                self.ready.push(Ready::Flush);
            }
        }
        if !lock(&self.endpoint).is_empty() {
            self.ready.push(Ready::Accept);
        }
        for (id, (end, _)) in &mut self.candidates {
            end.end().set_interest(Interest {
                read: true,
                write: false,
            });
            if end.end().readiness().readable {
                self.ready.push(Ready::Candidate(*id));
            }
        }
        if self.pty_write.is_some() && self.spawned.is_none() {
            self.ready.push(Ready::PtyWrite);
        }
        if self.spawned.is_some() {
            // The payload's inputs depend on its spawn: none is offered before the spawn's answer is taken.
            self.ready.push(Ready::Spawned);
        } else if self.held_spawn.is_some() {
            if !self.start_held() {
                self.ready.push(Ready::HeldSpawn);
            }
        } else if self.payload.is_some() {
            if self.wait_writable
                && self
                    .payload
                    .as_mut()
                    .is_some_and(ScriptedProgram::is_writable)
            {
                self.ready.push(Ready::PtyWritable);
            }
            self.poll_payload_exit();
            let unread = self.payload.as_mut().map_or(0, ScriptedProgram::unread);
            // A complete drain gives `PtyDrained`, and a drain whose next read would find nothing is complete; output is read
            // while it waits.
            match (self.drain, unread) {
                (Some(Drain::Done), _) | (Some(_), 0) => self.ready.push(Ready::PtyDrained),
                (None, 0) => {}
                // OU-3d, OU-7: a budget of zero stops every read, a drain's too, until the routes take bytes.
                _ if self.pty_budget == Some(0) => {}
                _ => self.ready.push(Ready::PtyRead),
            }
            if self.exit.is_some() {
                self.ready.push(Ready::Exited);
            }
        }
        for (id, route) in &mut self.routes {
            route.end.end().set_interest(Interest {
                read: !route.ended,
                write: route.write.is_some() || route.wait_writable,
            });
            // Plan 2.5 rule 8: read only with the read interest that the route registered (none once it ended).
            if route.end.end().readiness().readable && route.end.end().interest().read {
                self.ready.push(Ready::RouteRead(*id));
            }
            if route.end.end().readiness().writable {
                if route.write.is_some() {
                    self.ready.push(Ready::RouteWrite(*id));
                } else if route.wait_writable {
                    self.ready.push(Ready::RouteWritable(*id));
                }
            }
        }
        self.ready.len()
    }

    fn take(&mut self, _now: Instant, _machine: &SharedWorker, index: usize) -> Input {
        match self.ready[index] {
            Ready::EndPayload => {
                lock(&self.cell).end_payload = false;
                Input::EndPayload
            }
            Ready::Terminate => {
                lock(&self.cell).terminate = false;
                Input::Terminate
            }
            Ready::Link => {
                // A descriptor rides on its byte, so it comes before the bytes of its frame (DP-2, `SCM_RIGHTS`).
                if let Some(descriptor) = self.link.recv_descriptor() {
                    let end = descriptor
                        .downcast::<StreamEndpoint>()
                        .and_then(|endpoint| {
                            endpoint
                                .downcast::<StreamEnd>()
                                .map_err(crate::net::Descriptor::new)
                        })
                        .expect("the host hands over a testkit stream end");
                    self.next_descriptor += 1;
                    let id = DescriptorId(self.next_descriptor);
                    self.descriptors.insert(id, end);
                    return Input::Descriptor(id);
                }
                let mut buf = vec![0u8; self.read_chunk];
                match self.link.recv(&mut buf) {
                    Ok(n) if n > 0 => {
                        buf.truncate(n);
                        Input::LinkBytes(buf)
                    }
                    _ => {
                        self.outbound.clear();
                        self.link_open = false;
                        Input::LinkClosed
                    }
                }
            }
            // The break loses the unwritten bytes, as a broken socket does.
            Ready::LinkBroken => {
                lock(&self.cell).break_link = false;
                self.outbound.clear();
                self.link_open = false;
                self.link.close();
                Input::LinkClosed
            }
            Ready::Accept => {
                let connection = lock(&self.endpoint).pop_front().expect("counted as ready");
                self.next_candidate += 1;
                let id = CandidateId(self.next_candidate);
                self.candidates.insert(id, connection);
                Input::Candidate(id)
            }
            Ready::Candidate(id) => {
                let (end, _) = self.candidates.get_mut(&id).expect("counted as ready");
                let mut buf = vec![0u8; self.read_chunk];
                match end.recv(&mut buf) {
                    Ok(n) if n > 0 => {
                        buf.truncate(n);
                        Input::CandidateBytes(id, buf)
                    }
                    _ => {
                        if let Some((mut end, _)) = self.candidates.remove(&id) {
                            end.close();
                        }
                        Input::CandidateClosed(id)
                    }
                }
            }
            Ready::Flush => self.flush(),
            Ready::Spawned => Input::Spawned(self.spawned.take().expect("counted as ready")),
            Ready::HeldSpawn => {
                let spec = self.held_spawn.take().expect("counted as ready");
                Input::Spawned(self.spawn_payload(&spec))
            }
            Ready::PtyWrite => {
                let bytes = self.pty_write.take().expect("counted as ready");
                let Some(program) = self.payload.as_mut() else {
                    // No PTY any more (the leader was reaped): the write fails as a write to a closed PTY does.
                    return Input::PtyWritten(Err(EIO));
                };
                Input::PtyWritten(match program.write(&bytes) {
                    Ok(n) => Ok(n),
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                        self.wait_writable = true;
                        Ok(0)
                    }
                    Err(e) => Err(e.raw_os_error().unwrap_or(EIO)),
                })
            }
            Ready::PtyWritable => {
                self.wait_writable = false;
                Input::PtyWritable
            }
            Ready::PtyRead => {
                let program = self.payload.as_mut().expect("counted as ready");
                let chunk = self
                    .pty_budget
                    .map_or(self.read_chunk, |budget| budget.min(self.read_chunk));
                let want = self.drain.map_or(chunk, |drain| drain.want(chunk));
                let mut buf = vec![0u8; want];
                let n = program
                    .read(&mut buf)
                    .expect("a program with unread output reads some");
                buf.truncate(n);
                if let Some(budget) = self.pty_budget.as_mut() {
                    *budget -= n;
                }
                lock(&self.cell).model_log.read(&buf);
                if let Some(drain) = self.drain {
                    let Ok(next) =
                        drain.after_read(n, || Ok::<_, std::convert::Infallible>(program.unread()));
                    self.drain = Some(next);
                }
                Input::PtyOutput(buf)
            }
            Ready::PtyDrained => {
                self.drain = None;
                Input::PtyDrained
            }
            Ready::Exited => Input::PayloadExited(self.exit.take().expect("counted as ready")),
            Ready::RouteWrite(id) => {
                let route = self.routes.get_mut(&id).expect("counted as ready");
                let bytes = route.write.take().expect("counted as ready");
                // As the real driver: `Interrupted` writes again, `WouldBlock` is `Ok(0)` and waits for writable, and only
                // another error is terminal (`Input::RouteWritten`).
                let result = loop {
                    match route.end.write(&bytes) {
                        Ok(n) => break Ok(n),
                        Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                            route.wait_writable = true;
                            break Ok(0);
                        }
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                        Err(e) => break Err(e.raw_os_error().unwrap_or(EIO)),
                    }
                };
                Input::RouteWritten { route: id, result }
            }
            Ready::RouteWritable(id) => {
                self.routes
                    .get_mut(&id)
                    .expect("counted as ready")
                    .wait_writable = false;
                Input::RouteWritable { route: id }
            }
            // As the real driver: a read of what the stream holds; the end of the stream, or an error other than
            // `Interrupted`, ends the route's reads (OU-5).
            Ready::RouteRead(id) => {
                let route = self.routes.get_mut(&id).expect("counted as ready");
                let mut buf = vec![0u8; ROUTE_READ_BYTES];
                let read = loop {
                    match route.end.read(&mut buf) {
                        Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                        other => break other,
                    }
                };
                match read {
                    Ok(n) if n > 0 => {
                        buf.truncate(n);
                        Input::RouteRead {
                            route: id,
                            bytes: buf,
                        }
                    }
                    // Readable with nothing to take: an empty read, which the machine ignores.
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => Input::RouteRead {
                        route: id,
                        bytes: Vec::new(),
                    },
                    _ => {
                        route.ended = true;
                        Input::RouteEnded { route: id }
                    }
                }
            }
        }
    }

    fn timer(&mut self, _now: Instant) -> Input {
        Input::Timer
    }

    fn perform(&mut self, _now: Instant, action: Action) {
        match action {
            // Queued; the write is its own input (`Ready::Flush`), as the real driver writes when the socket takes bytes.
            Action::LinkSend(bytes) => {
                if self.link_open {
                    self.outbound.extend(bytes);
                }
            }
            // The machine closes only when everything it sent is written.
            Action::LinkClose => {
                if self.link_open {
                    self.link_open = false;
                    self.link.close();
                }
            }
            Action::SpawnPayload(spec) if self.start_held() => self.held_spawn = Some(spec),
            Action::SpawnPayload(spec) => self.spawned = Some(self.spawn_payload(&spec)),
            Action::DrainPty => {
                self.drain = Some(Drain::asked(
                    self.payload.as_mut().map_or(0, ScriptedProgram::unread),
                ));
            }
            // The write is its own input (`Ready::PtyWrite`), so the scheduler orders it among the other ready work.
            Action::PtyWrite(bytes) => self.pty_write = Some(bytes),
            Action::SignalPayload(signal) => {
                if let Some(program) = self.payload.as_mut() {
                    program.signal(signal);
                }
                self.poll_payload_exit();
            }
            Action::ReapPayload => {
                self.payload = None;
                let mut cell = lock(&self.cell);
                cell.payload_alive = false;
                cell.program = None;
            }
            Action::Exit => {
                lock(&self.processes).end(self.id, ExitStatus::Code(0));
                self.ended(true);
            }
            Action::CandidateClose(id) => {
                if let Some((mut end, _)) = self.candidates.remove(&id) {
                    end.close();
                }
            }
            Action::BindRoute { descriptor, route } => {
                let mut end = self
                    .descriptors
                    .remove(&descriptor)
                    .expect("the machine binds a descriptor that the link delivered");
                end.end().control().owned();
                self.routes.insert(
                    route,
                    RouteEdge {
                        end,
                        write: None,
                        wait_writable: false,
                        ended: false,
                    },
                );
            }
            Action::CloseDescriptor(id) => {
                if let Some(mut end) = self.descriptors.remove(&id) {
                    end.close();
                }
            }
            // The write is its own input (`Ready::RouteWrite`), as the real driver writes when the socket takes bytes.
            Action::RouteWrite { route, bytes } => {
                if let Some(edge) = self.routes.get_mut(&route) {
                    edge.write = Some(bytes);
                }
            }
            Action::RouteClose { route } => {
                if let Some(mut edge) = self.routes.remove(&route) {
                    edge.end.close();
                }
            }
            Action::PtyReadBudget(budget) => self.pty_budget = budget,
            // The fence (DP-8): the old link closes with its unwritten bytes, and the candidate is the link from now on. A
            // break of the old link (`break_control`) ends with it.
            Action::AdoptLink(id) => {
                if self.link_open {
                    self.link.close();
                }
                self.outbound.clear();
                self.written = 0;
                // The machine adopts a candidate in the input that gave its hello, so the candidate is still here.
                let (link, adopter) = self
                    .candidates
                    .remove(&id)
                    .expect("the machine adopts only a candidate that it holds");
                self.link = link;
                self.link_open = true;
                let earlier = {
                    let mut cell = lock(&self.cell);
                    cell.break_link = false;
                    cell.link = Some(self.link.end().control());
                    match &adopter {
                        Some(adopter) => cell.control.replace(Arc::downgrade(adopter)),
                        None => None,
                    }
                };
                // The control link moves to the adopting host: its `edges_quiet` reads the link, and a control wakes it.
                // The spawning host still reaps the worker (`cells`). The cell's guard ends before a table is locked.
                if let Some(adopter) = adopter {
                    if let Some(earlier) = earlier.as_ref().and_then(Weak::upgrade) {
                        lock(&earlier).links.remove(&self.id);
                    }
                    lock(&adopter).links.insert(self.id, Arc::clone(&self.cell));
                }
            }
        }
    }
}

/// The `Core` of the testkit (plan 4.1): the real host driver, and the in-process workers that progress during its pump.
pub struct TestkitCore {
    driver: HostDriver<SimEdges>,
    workers: Workers,
    wake: Arc<dyn HostWake>,
    captures: CaptureLog,
    route_ends: crate::route_client::RouteEnds,
}

impl TestkitCore {
    /// `wake` is the wake object of the driver's edges.
    pub fn new(
        driver: HostDriver<SimEdges>,
        wake: Arc<dyn HostWake>,
        workers: Workers,
    ) -> TestkitCore {
        TestkitCore {
            driver,
            workers,
            wake,
            captures: CaptureLog::default(),
            route_ends: Default::default(),
        }
    }

    /// The route-ended causes of the routes that Core closed (A2-3), for the client ends of this handle's routes.
    pub(crate) fn route_ends(&self) -> crate::route_client::RouteEnds {
        self.route_ends.clone()
    }

    /// The captures that the host completed, with their pages (`oracle_resume`).
    pub(crate) fn captures(&self) -> CaptureLog {
        self.captures.clone()
    }

    /// The host driver, for the controls that act on the host's edges.
    pub fn driver_mut(&mut self) -> &mut HostDriver<SimEdges> {
        &mut self.driver
    }
}

impl CoreApi for TestkitCore {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        self.driver.begin(op)
    }

    /// TM-1, plan 4.1: the clock moves to `now`, the workers run their ready work, then the host pumps. A worker that still
    /// has ready work keeps `more` true and the wake set (TM-6), as a real worker's pending bytes would.
    fn pump(&mut self, now: Now) -> PumpReport {
        self.workers.run(now.monotonic);
        let mut report = self.driver.pump(now);
        if self.workers.has_ready() {
            report.more = true;
            self.wake.signal();
        }
        report
    }

    /// A completed capture is recorded with every page that `read_page` gives, before the caller can release it.
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let events = self.driver.poll_events(max);
        for event in &events {
            if let Event::RouteClosed { route, reason, .. } = event {
                self.route_ends.closed(*route, *reason);
            }
            if let Event::Completed {
                result: OpResult::Ok(OpOutput::Capture(capture)),
                ..
            } = event
            {
                let mut bytes = Vec::new();
                for page in 0..capture.page_count {
                    if let Ok(page) = self.driver.read_page(capture.capture, page) {
                        bytes.extend_from_slice(&page.bytes.0);
                    }
                }
                self.captures.insert(
                    capture.capture,
                    CaptureRecord {
                        model_rev: capture.model_rev,
                        bytes,
                    },
                );
            }
        }
        events
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        self.driver.wake_handle()
    }

    /// The earliest deadline of the host or of a worker: a worker's deadline (the grace of `EndPayload`) is reached only
    /// when the host pumps at that time.
    fn next_deadline(&self) -> Option<Instant> {
        match (self.driver.next_deadline(), self.workers.next_deadline()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    fn cancel(&mut self, op: OpId) -> CancelResult {
        self.driver.cancel(op)
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.driver.get(id)
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.driver.list()
    }

    fn status(&self) -> Status {
        self.driver.status()
    }

    fn diagnostics(&self) -> serde_json::Value {
        self.driver.diagnostics()
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.driver.terminal_state(id)
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.driver.read_page(capture, page)
    }

    fn release(&mut self, capture: CaptureId) {
        self.driver.release(capture)
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.driver.release_owner(client)
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.driver.snapshot_formats(session)
    }

    fn shadow_answerable_kinds(&self) -> Vec<QueryKind> {
        self.driver.shadow_answerable_kinds()
    }

    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        self.driver.attach(client, session, transport, options)
    }

    fn tap_read(&mut self, session: &SessionId, max: usize) -> Result<TapChunk, CoreError> {
        self.driver.tap_read(session, max)
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        self.driver.set_silence_threshold(session, threshold)
    }

    fn service_send(
        &mut self,
        id: &ServiceId,
        lane: u8,
        frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        self.driver.service_send(id, lane, frame)
    }

    fn service_recv(&mut self, id: &ServiceId, lane: u8) -> Result<Option<Frame>, RecvError> {
        self.driver.service_recv(id, lane)
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        self.driver.service_report(id)
    }

    fn service_log_tail(&self, id: &ServiceId, max: usize) -> Result<Vec<u8>, CoreError> {
        self.driver.service_log_tail(id, max)
    }

    fn features(&self) -> Features {
        self.driver.features()
    }

    fn limits(&self) -> CoreLimits {
        self.driver.limits()
    }

    fn worker_protocol(&self) -> u8 {
        self.driver.worker_protocol()
    }

    fn adoptable_worker_protocols(&self) -> std::collections::BTreeSet<u8> {
        self.driver.adoptable_worker_protocols()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.driver.worker_protocol_compatibility(protocol)
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        self.driver.terminal_identity()
    }
}

#[cfg(test)]
mod tests;
