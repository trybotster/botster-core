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
use crate::net::{Interest, LinkEnd};
use crate::program::ScriptedProgram;
use crate::scheduler::SchedulerHandle;
use crate::sim::{Binding, MachineNode, Sim};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, WindowSize,
};
use botster_core_edges::{Link, Machine, Program};
use botster_core_host::driver::{HostDriver, HostWake, WorkerSpawn};
use botster_core_link::msg::PayloadId;
use botster_route_codec::prelude::QueryKind;
use botster_worker_core::{Action, Input, PayloadSpec, SpawnFailure, Worker, WorkerConfig};
use std::collections::{BTreeMap, VecDeque};
use std::io;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// `ENOEXEC`: the errno of a program that the in-process edge cannot run (a script that is not valid, or a step that needs a
/// real process). It is what an `exec` of a file that is not a program gives.
const ENOEXEC: i32 = 8;

/// The most inputs that one `Sim` run handles before it reports a livelock: far above what a transcript's workers do between
/// two host pumps.
const SIM_STEP_LIMIT: usize = 100_000;

/// The bytes of one read of the control link or the PTY.
const READ_CHUNK: usize = 64 * 1024;

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
}

/// The process table of the workers that one host spawned: identities and the exits that the host has not polled.
#[derive(Debug, Default)]
struct Processes {
    cells: BTreeMap<ProcessIdentity, Arc<Mutex<ProcessCell>>>,
    exits: VecDeque<(ProcessIdentity, ExitStatus)>,
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

/// The `Sim` of one harness: every in-process worker of every handle (plan 4.1: "a `Sim` owns ... every `Worker`").
#[derive(Clone)]
pub struct Workers {
    sim: Arc<Mutex<Sim>>,
    pids: Arc<Mutex<Pids>>,
    run_processes: Arc<Mutex<RunProcesses>>,
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
        let mut sim = lock(&self.sim);
        sim.advance_to(now);
        if let Err(livelock) = sim.run_until_idle(SIM_STEP_LIMIT) {
            panic!("the in-process workers did not settle: {livelock:?}");
        }
    }

    /// True when a worker has ready work at the virtual clock.
    pub fn has_ready(&self) -> bool {
        lock(&self.sim).has_ready()
    }

    /// The earliest deadline of a worker.
    pub fn next_deadline(&self) -> Option<Instant> {
        lock(&self.sim).next_deadline()
    }

    /// The `Process` edge of one host for its workers.
    pub fn spawner(&self) -> WorkerSpawner {
        WorkerSpawner {
            workers: self.clone(),
            processes: Arc::default(),
        }
    }
}

/// The `Process` edge of the testkit's `Core` for workers (P1's [`Spawner`]).
pub struct WorkerSpawner {
    workers: Workers,
    processes: Arc<Mutex<Processes>>,
}

impl Spawner for WorkerSpawner {
    /// AD-7 step 2: the worker exists and connects to the host; its payload waits for the host's `Launch`.
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
        lock(&self.processes).cells.insert(id, Arc::clone(&cell));
        lock(&self.workers.run_processes)
            .insert(id, (Arc::clone(&cell), Arc::clone(&self.processes)));
        let link = connect();
        let worker = Worker::new(WorkerConfig::new(
            spec.instance.clone(),
            spec.token,
            spec.host_epoch,
        ));
        let mut edges = WorkerEdges {
            id,
            cell,
            processes: Arc::clone(&self.processes),
            pids: Arc::clone(&self.workers.pids),
            scheduler: self.workers.scheduler.clone(),
            link,
            link_open: true,
            outbound: VecDeque::new(),
            written: 0,
            payload: None,
            spawned: None,
            exit: None,
            output_ended: false,
            drain: false,
            ready: Vec::new(),
            read_chunk: self.workers.read_chunk,
        };
        let mut worker = worker;
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
}

/// One ready input of a worker, in the order of plan 2.4: control first, then the payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Ready {
    EndPayload,
    Terminate,
    /// The link has bytes, or its peer closed it.
    Link,
    /// The link takes bytes and some of `outbound` waits for it.
    Flush,
    Spawned,
    PtyRead,
    PtyDrained,
    Exited,
}

/// The edges of one in-process worker: the control link, the scripted program on its PTY, and its process cell.
///
/// `ready` reads no edge and writes none: it only reads flags, so the order of every effect is the scheduler's (plan 2.5
/// rule 8). Each effect is one input: a read of the link or the PTY, a write of queued link bytes (`LinkWritten`), a spawn's
/// answer, an exit.
struct WorkerEdges {
    id: ProcessIdentity,
    cell: Arc<Mutex<ProcessCell>>,
    processes: Arc<Mutex<Processes>>,
    pids: Arc<Mutex<Pids>>,
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
    /// A read of the program found the end of its output.
    output_ended: bool,
    /// `DrainPty` asked for one `PtyDrained` once the program has no byte to read.
    drain: bool,
    /// The inputs counted by the last `ready`.
    ready: Vec<Ready>,
    read_chunk: usize,
}

impl WorkerEdges {
    /// The end of the worker process: the OS closes its descriptors, so its link closes, and its payload's PTY is gone.
    fn ended(&mut self) {
        if self.link_open {
            self.link_open = false;
            self.link.close();
        }
        self.outbound.clear();
        self.payload = None;
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
        self.payload = Some(program);
        self.output_ended = false;
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

impl Binding<Worker> for WorkerEdges {
    fn ready(&mut self, _now: Instant, _machine: &Worker) -> usize {
        self.ready.clear();
        if lock(&self.cell).ended {
            self.ended();
            return 0;
        }
        if lock(&self.cell).end_payload {
            self.ready.push(Ready::EndPayload);
        }
        if lock(&self.cell).terminate {
            self.ready.push(Ready::Terminate);
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
        if self.spawned.is_some() {
            // The payload's inputs depend on its spawn: none is offered before the spawn's answer is taken.
            self.ready.push(Ready::Spawned);
        } else if let Some(program) = self.payload.as_mut() {
            if self.exit.is_none() {
                self.exit = program.poll_exit();
            }
            if !self.output_ended && program.is_readable() {
                self.ready.push(Ready::PtyRead);
            } else if self.drain {
                self.ready.push(Ready::PtyDrained);
            }
            if self.exit.is_some() {
                self.ready.push(Ready::Exited);
            }
        }
        self.ready.len()
    }

    fn take(&mut self, _now: Instant, _machine: &Worker, index: usize) -> Input {
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
            Ready::Flush => self.flush(),
            Ready::Spawned => Input::Spawned(self.spawned.take().expect("counted as ready")),
            Ready::PtyRead => {
                let program = self.payload.as_mut().expect("counted as ready");
                let mut buf = vec![0u8; self.read_chunk];
                match program.read(&mut buf) {
                    Ok(n) if n > 0 => {
                        buf.truncate(n);
                        Input::PtyOutput(buf)
                    }
                    other => {
                        // The end of the output, or no byte now: the program has nothing more to read.
                        if matches!(other, Ok(0)) {
                            self.output_ended = true;
                        }
                        self.drain = false;
                        Input::PtyDrained
                    }
                }
            }
            Ready::PtyDrained => {
                self.drain = false;
                Input::PtyDrained
            }
            Ready::Exited => Input::PayloadExited(self.exit.take().expect("counted as ready")),
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
            Action::SpawnPayload(spec) => self.spawned = Some(self.spawn_payload(&spec)),
            Action::DrainPty => self.drain = true,
            Action::SignalPayload(signal) => {
                if let Some(program) = self.payload.as_mut() {
                    program.signal(signal);
                }
            }
            Action::ReapPayload => self.payload = None,
            Action::Exit => {
                lock(&self.processes).end(self.id, ExitStatus::Code(0));
                self.ended();
            }
        }
    }
}

/// The `Core` of the testkit (plan 4.1): the real host driver, and the in-process workers that progress during its pump.
pub struct TestkitCore {
    driver: HostDriver<SimEdges>,
    workers: Workers,
    wake: Arc<dyn HostWake>,
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
        }
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

    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        self.driver.poll_events(max)
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
