//! Unit tests of the engine, each citing the clause it proves. They drive the machine the way a driver does: the `World`
//! performs the actions on a map for the registry and records the rest, and a scripted worker answers on the link. Nothing
//! here is a test branch of the engine: the engine is built from its public config like any other.

use crate::io::{Action, Input, LinkId, Work};
use crate::{EngineConfig, HostEngine};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, IdentityState, ProcessIdentity, SpawnError, StorageError,
};
use botster_core_edges::Machine;
use botster_core_link::hello::Hello;
use botster_core_link::msg::{AdoptReport, AdoptedPayload, HostMsg, Observation, WorkerMsg};
use botster_core_link::proof::{token_proof, TOKEN_LEN};
use std::collections::{BTreeMap, BTreeSet};
use std::time::{Duration, Instant};

/// The real instant that a test gives its injected clock as the start. It is the one call of the crate's tests that reads the
/// real clock, so its allowance covers that one call (the machine crates read no clock: plan 2.3c).
#[allow(clippy::disallowed_methods)]
pub(crate) fn real_now() -> std::time::Instant {
    std::time::Instant::now()
}

pub(crate) fn size() -> Size {
    Size {
        rows: 24,
        cols: 80,
        cell_px: None,
    }
}

pub(crate) fn request() -> SpawnRequest {
    SpawnRequest {
        argv: vec!["/bin/hold".into()],
        env: BTreeMap::new(),
        cwd: "/".into(),
        size: size(),
        labels: BTreeMap::new(),
        color_profile: None,
        notification_policy: None,
        size_policy: None,
    }
}

pub(crate) fn sid(name: &str) -> SessionId {
    SessionId(name.to_string())
}

pub(crate) fn create(name: &str) -> Op {
    Op::Create {
        session: sid(name),
        request: request(),
    }
}

pub(crate) fn features() -> Features {
    Features {
        names: BTreeSet::from([Feature::Silence, Feature::NotificationPolicy]),
        service_preamble_versions: vec![1],
    }
}

pub(crate) fn config(limits: CoreLimits) -> EngineConfig {
    EngineConfig {
        limits,
        features: features(),
        host_epoch: 7,
        worker_path: "/bin/worker".into(),
        worker_protocol: 1,
        shadow_answerable: Vec::new(),
        terminal_identity: TerminalIdentity {
            term: "xterm-ghostty".into(),
            terminfo_source: "x".into(),
        },
    }
}

pub(crate) fn limits(change: impl FnOnce(&mut CoreLimits)) -> CoreLimits {
    let mut limits = CoreLimits::default();
    change(&mut limits);
    limits
}

/// The worker protocol number in the hello of the scripted worker.
pub(crate) const HELLO_PROTOCOL: u8 = 1;

/// A scripted worker at its endpoint (DESIGN.md "Adoption (P5)" 3.1): what it answers when a host connects.
#[derive(Debug, Clone)]
pub(crate) struct Endpoint {
    pub token: [u8; TOKEN_LEN],
    /// The protocol in the worker's hello.
    pub protocol: u8,
    /// The payload state of the report that follows the worker's hello. `None`: the worker answers no hello.
    pub payload: Option<AdoptedPayload>,
}

/// What a scripted worker does when the engine speaks to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Autopilot {
    /// Connects, launches, and ends when asked, at once.
    Full,
    /// Spawns, and then does nothing: the test speaks for the worker.
    Silent,
}

pub(crate) struct World {
    pub engine: HostEngine,
    pub now: Instant,
    pub unix: UnixSeconds,
    pub rows: BTreeMap<String, Vec<u8>>,
    pub row_writes: Vec<String>,
    pub fail_row: Option<StorageError>,
    /// The next row write takes effect and is then reported with this error (AD-7: an uncertain write that happened).
    pub fail_row_after_write: Option<StorageError>,
    pub refuse_spawn: Option<i32>,
    pub autopilot: Autopilot,
    pub sent: Vec<(LinkId, HostMsg)>,
    pub hellos: Vec<(LinkId, Hello)>,
    pub signals: Vec<(ProcessIdentity, botster_core_edges::edges::GroupSignal)>,
    pub closed: Vec<LinkId>,
    pub trace: Vec<String>,
    spawned: BTreeMap<InstanceId, (ProcessIdentity, [u8; TOKEN_LEN], LinkId)>,
    /// The processes that the operating system runs: a worker is in it from its spawn until it ends. It outlives a host
    /// (LC-12), so a handle opened `over` another one sees its workers.
    pub alive: BTreeSet<ProcessIdentity>,
    identities: BTreeMap<LinkId, ProcessIdentity>,
    next_pid: u32,
    pub(crate) next_link: u64,
    random: u8,
    inject: Vec<Input>,
    /// The workers that a host can connect to, by instance. A worker that is not alive answers no connect.
    pub endpoints: BTreeMap<InstanceId, Endpoint>,
    /// The instances that the host connected to, in order.
    pub connects: Vec<InstanceId>,
    adopt_links: BTreeMap<LinkId, InstanceId>,
}

impl World {
    pub fn new(limits: CoreLimits) -> World {
        World::open(config(limits), BTreeMap::new(), BTreeSet::new(), 100)
    }

    /// A handle whose platform offers `feature` too (A2-6).
    pub fn offering(feature: Feature) -> World {
        let mut cfg = config(CoreLimits::default());
        cfg.features.names.insert(feature);
        World::configured(cfg)
    }

    /// A handle opened with `cfg`: the features, limits and lists that `open` gives the engine (A2-6, 9B, EV-8).
    pub fn configured(cfg: EngineConfig) -> World {
        World::open(cfg, BTreeMap::new(), BTreeSet::new(), 100)
    }

    /// A new handle over the registry and the processes of `earlier` (a host that was dropped, LC-12). The handle reads the
    /// ids of the rows when it opens (ID-1), as `HostDriver::open` does. Its host epoch is above `earlier`'s (DP-8: every
    /// open raises it), and its spawns take pids after `earlier`'s, as the operating system gives no live process's pid to a
    /// new one.
    pub fn over(earlier: &World) -> World {
        let mut cfg = earlier.engine.cfg.clone();
        cfg.host_epoch += 1;
        let mut world = World::open(
            cfg,
            earlier.rows.clone(),
            earlier.alive.clone(),
            earlier.next_pid,
        );
        // The workers of `earlier` keep their endpoints (LC-12). A worker answers a connect only when a test scripts its
        // endpoint (`tests::adoption`); otherwise the connect fails, as with the driver's default edge.
        world.endpoints = earlier.endpoints.clone();
        world.spawned = earlier.spawned.clone();
        world.next_link = earlier.next_link;
        world
    }

    fn open(
        cfg: EngineConfig,
        rows: BTreeMap<String, Vec<u8>>,
        alive: BTreeSet<ProcessIdentity>,
        next_pid: u32,
    ) -> World {
        let start = real_now();
        let registry_ids = rows
            .keys()
            .filter_map(|key| key.strip_prefix(crate::session::ROW_PREFIX))
            .map(sid)
            .collect();
        World {
            engine: HostEngine::new(cfg, registry_ids),
            now: start,
            unix: 1_000_000,
            rows,
            row_writes: Vec::new(),
            fail_row: None,
            fail_row_after_write: None,
            refuse_spawn: None,
            autopilot: Autopilot::Full,
            sent: Vec::new(),
            hellos: Vec::new(),
            signals: Vec::new(),
            closed: Vec::new(),
            trace: Vec::new(),
            spawned: BTreeMap::new(),
            alive,
            identities: BTreeMap::new(),
            next_pid,
            next_link: 1,
            random: 1,
            inject: Vec::new(),
            endpoints: BTreeMap::new(),
            connects: Vec::new(),
            adopt_links: BTreeMap::new(),
        }
    }

    pub fn default() -> World {
        World::new(CoreLimits::default())
    }

    pub fn advance(&mut self, by: Duration) {
        self.now += by;
        self.unix += by.as_secs();
    }

    pub fn feed(&mut self, input: Input) {
        self.end_exited(&input);
        self.engine.handle(self.now, input);
        self.perform();
    }

    /// A process whose exit the engine is told of is no longer running.
    fn end_exited(&mut self, input: &Input) {
        if let Input::ProcessExited { identity, .. } = input {
            self.alive.remove(identity);
        }
    }

    /// Performs every action that the engine queued, and feeds the answers back (plan 2.1).
    fn perform(&mut self) {
        loop {
            while let Some(action) = self.engine.poll_action() {
                self.act(action);
            }
            if self.inject.is_empty() {
                break;
            }
            let inputs = std::mem::take(&mut self.inject);
            for input in inputs {
                self.end_exited(&input);
                self.engine.handle(self.now, input);
            }
        }
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Random { ticket, len } => {
                self.random = self.random.wrapping_add(1);
                self.inject.push(Input::Random {
                    ticket,
                    bytes: vec![self.random; len],
                });
            }
            Action::WriteRow { ticket, key, bytes } => {
                self.trace.push(format!("write {key}"));
                let result = match self.fail_row.take() {
                    Some(error) => Err(error),
                    None => {
                        self.rows.insert(key.clone(), bytes);
                        self.row_writes.push(key);
                        self.fail_row_after_write.take().map_or(Ok(()), Err)
                    }
                };
                self.inject.push(Input::RowWritten { ticket, result });
            }
            Action::DeleteRow { ticket, key } => {
                self.trace.push(format!("delete {key}"));
                let result = match self.fail_row.take() {
                    Some(error) => Err(error),
                    None => {
                        self.rows.remove(&key);
                        Ok(())
                    }
                };
                self.inject.push(Input::RowDeleted { ticket, result });
            }
            Action::ReadRows { ticket, prefix } => {
                let rows = self
                    .rows
                    .iter()
                    .filter(|(k, _)| k.starts_with(&prefix))
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect();
                self.inject.push(Input::Rows {
                    ticket,
                    result: Ok(rows),
                });
            }
            Action::SpawnWorker {
                ticket,
                instance,
                token,
                ..
            } => {
                self.trace.push("spawn".into());
                if let Some(errno) = self.refuse_spawn.take() {
                    self.inject.push(Input::Spawned {
                        ticket,
                        result: Err(SpawnError { errno }),
                    });
                    return;
                }
                self.next_pid += 1;
                let identity = ProcessIdentity {
                    pid: self.next_pid,
                    start_time: 5,
                };
                let link = LinkId(self.next_link);
                self.next_link += 1;
                self.identities.insert(link, identity);
                self.alive.insert(identity);
                self.spawned
                    .insert(instance.clone(), (identity, token, link));
                self.inject.push(Input::Spawned {
                    ticket,
                    result: Ok(identity),
                });
                if self.autopilot == Autopilot::Full {
                    self.inject.push(self.hello_input(
                        &instance,
                        HELLO_PROTOCOL,
                        token,
                        self.engine.cfg.host_epoch,
                        link,
                    ));
                }
            }
            Action::ConnectWorker { ticket, instance } => {
                self.trace.push("connect".into());
                self.connects.push(instance.clone());
                let identity = self
                    .spawned
                    .get(&instance)
                    .map(|(identity, _, _)| *identity);
                let answers = self.endpoints.contains_key(&instance)
                    && identity.is_some_and(|i| self.alive.contains(&i));
                let link = answers.then(|| {
                    let link = LinkId(self.next_link);
                    self.next_link += 1;
                    self.adopt_links.insert(link, instance.clone());
                    self.identities
                        .insert(link, identity.expect("an answering worker was spawned"));
                    link
                });
                self.inject.push(Input::WorkerConnected { ticket, link });
            }
            Action::SendHello { link, hello } => {
                if let Some(instance) = self.adopt_links.get(&link).cloned() {
                    self.endpoint_answers(link, &instance, &hello);
                }
                self.hellos.push((link, hello));
            }
            Action::SendMsg { link, msg } => {
                self.trace.push(format!("send {}", msg_name(&msg)));
                if self.autopilot == Autopilot::Full {
                    if let Some(reply) = self.reply(link, &msg) {
                        for r in reply {
                            self.inject.push(Input::LinkMsg { link, msg: r });
                        }
                    }
                    // The worker ends after its teardown (LC-7 step 3).
                    if matches!(msg, HostMsg::Remove) {
                        if let Some(identity) = self.identities.get(&link).copied() {
                            self.inject.push(Input::ProcessExited {
                                identity,
                                status: ExitStatus::Code(0),
                            });
                        }
                    }
                }
                self.sent.push((link, msg));
            }
            Action::CloseLink { link } => self.closed.push(link),
            Action::ProbeIdentity { identity } => {
                let state = if self.alive.contains(&identity) {
                    IdentityState::Matches
                } else if self.alive.iter().any(|p| p.pid == identity.pid) {
                    IdentityState::Reused
                } else {
                    IdentityState::Absent
                };
                self.inject.push(Input::IdentityState { identity, state });
            }
            Action::SignalGroup { identity, signal } => {
                // The operating system ends a process that a kill reaches (AD-6: only a matching one is signalled).
                if signal == botster_core_edges::edges::GroupSignal::Kill {
                    self.alive.remove(&identity);
                }
                self.signals.push((identity, signal));
            }
            Action::HandoffRoute { .. } => self.trace.push("handoff".into()),
        }
    }

    /// The scripted worker at an endpoint answers the host's hello with its own, then with its report (DESIGN.md 3.4, 4).
    fn endpoint_answers(&mut self, link: LinkId, instance: &InstanceId, hello: &Hello) {
        let endpoint = self.endpoints[instance].clone();
        let Some(payload) = endpoint.payload else {
            return;
        };
        self.inject.push(self.hello_input(
            instance,
            endpoint.protocol,
            endpoint.token,
            hello.host_epoch,
            link,
        ));
        let launched = matches!(
            payload,
            AdoptedPayload::Running { .. } | AdoptedPayload::Exited { .. }
        );
        self.inject.push(Input::LinkMsg {
            link,
            msg: WorkerMsg::Adopted {
                report: Box::new(AdoptReport {
                    payload,
                    features: BTreeSet::from([Feature::FocusReport]),
                    terminal: launched.then(terminal_state),
                    formats: Vec::new(),
                }),
            },
        });
    }

    fn hello_input(
        &self,
        instance: &InstanceId,
        protocol: u8,
        token: [u8; TOKEN_LEN],
        epoch: u64,
        link: LinkId,
    ) -> Input {
        Input::LinkHello {
            link,
            hello: Hello {
                protocol,
                instance: instance.clone(),
                proof: token_proof(&token, instance, epoch),
                host_epoch: epoch,
            },
        }
    }

    /// The scripted worker's answers.
    fn reply(&mut self, _link: LinkId, msg: &HostMsg) -> Option<Vec<WorkerMsg>> {
        match msg {
            HostMsg::Launch(spec) => Some(vec![if spec.cwd.contains("no-such") {
                WorkerMsg::LaunchFailed {
                    reason: StartFailReason::CwdMissing,
                }
            } else {
                WorkerMsg::Launched {
                    features: BTreeSet::from([Feature::FocusReport]),
                    terminal: terminal_state(),
                    formats: vec![SnapshotFormat {
                        name: "GHOSTSNP".into(),
                        version: 1,
                    }],
                    payload: botster_core_link::msg::PayloadId {
                        pid: 900,
                        start_time: 3,
                    },
                }
            }]),
            HostMsg::Stop => Some(vec![WorkerMsg::Exited {
                code: None,
                signal: Some(15),
            }]),
            HostMsg::Kill => Some(vec![WorkerMsg::Exited {
                code: None,
                signal: Some(9),
            }]),
            HostMsg::Remove => Some(vec![WorkerMsg::RemoveResult {
                uploads: UploadsOutcome::Deleted,
            }]),
            HostMsg::Op { req, op } => Some(vec![WorkerMsg::Done {
                req: *req,
                result: match op {
                    Op::ReadModeFlags { .. } => OpResult::Ok(OpOutput::Modes(Modes {
                        flags: terminal_state().modes,
                        model_rev: ModelRev(1),
                    })),
                    _ => OpResult::Ok(OpOutput::Unit),
                },
            }]),
            _ => None,
        }
    }

    /// Runs the ready work that `choose` picks, one step at a time, until it picks none. An engine whose steps make no
    /// progress fails the test at the step bound, instead of hanging it.
    pub fn settle(&mut self, mut choose: impl FnMut(&[Work]) -> Option<Work>) {
        let mut guard = 0;
        while let Some(work) = choose(&self.engine.ready()) {
            self.feed(Input::Run(work));
            guard += 1;
            assert!(guard < 10_000, "the engine does not settle");
        }
    }

    /// One `pump` with no budget and no deferral: the clock, then the first ready work until none is left. Budgets, deferral and
    /// the order of a seeded scheduler are the driver's, and the driver's own tests prove them (`tests::driver`).
    pub fn pump(&mut self) -> PumpReport {
        self.feed(Input::Clock(self.unix));
        self.settle(|ready| ready.first().cloned());
        PumpReport {
            more: self.engine.runnable(),
            events_posted: self.engine.take_posted(),
        }
    }

    /// One `pump` whose scheduler always picks the last ready work: an order that the contract leaves open (OR-3, A5-2), so
    /// an order that a clause fixes must hold under it too.
    pub fn pump_last_first(&mut self) {
        self.feed(Input::Clock(self.unix));
        self.settle(|ready| ready.last().cloned());
    }

    /// Pumps and polls until `event` shows up, and returns the events up to it.
    pub fn until(&mut self, mut wanted: impl FnMut(&Event) -> bool) -> Vec<Event> {
        let mut seen = Vec::new();
        for _ in 0..50 {
            self.pump();
            for event in self.engine.poll_events(64) {
                let hit = wanted(&event);
                seen.push(event);
                if hit {
                    return seen;
                }
            }
        }
        panic!("the event never came: {seen:?}");
    }

    /// Pumps and polls until every op of `ops` completed, and returns their results.
    pub fn complete_all(&mut self, ops: &[OpId]) -> BTreeMap<OpId, OpResult> {
        let mut results = BTreeMap::new();
        for _ in 0..50 {
            self.pump();
            for event in self.engine.poll_events(64) {
                if let Event::Completed { op, result } = event {
                    if ops.contains(&op) {
                        results.insert(op, result);
                    }
                }
            }
            if results.len() == ops.len() {
                return results;
            }
        }
        panic!("not every op completed: {results:?} of {ops:?}");
    }

    pub fn complete(&mut self, op: OpId) -> OpResult {
        let events = self.until(|e| matches!(e, Event::Completed { op: o, .. } if *o == op));
        match events.last() {
            Some(Event::Completed { result, .. }) => result.clone(),
            _ => unreachable!(),
        }
    }

    pub fn run(&mut self, op: Op) -> OpResult {
        let id = self.engine.begin(op).expect("the op is admitted");
        self.complete(id)
    }

    pub fn ok(&mut self, op: Op) -> OpOutput {
        match self.run(op) {
            OpResult::Ok(out) => out,
            other => panic!("the op failed: {other:?}"),
        }
    }

    /// A session that runs: create and start with the scripted worker.
    pub fn running(&mut self, name: &str) {
        self.ok(create(name));
        if self.autopilot == Autopilot::Full {
            self.ok(Op::Start { id: sid(name) });
            return;
        }
        // A silent worker: the test says hello and reports the launch for it.
        let start = self.engine.begin(Op::Start { id: sid(name) }).unwrap();
        self.pump();
        let link = self.link_of_after_hello(name);
        self.feed(Input::LinkMsg {
            link,
            msg: WorkerMsg::Launched {
                features: BTreeSet::from([Feature::FocusReport]),
                terminal: terminal_state(),
                formats: vec![],
                payload: botster_core_link::msg::PayloadId {
                    pid: 900,
                    start_time: 3,
                },
            },
        });
        self.complete(start);
    }

    pub fn worker_says(&mut self, session: &str, msg: WorkerMsg) {
        let link = self.engine.sessions[&sid(session)]
            .worker
            .link
            .expect("the session has a link");
        self.feed(Input::LinkMsg { link, msg });
    }

    pub fn link_of(&self, session: &str) -> LinkId {
        self.engine.sessions[&sid(session)]
            .worker
            .link
            .expect("the session has a link")
    }

    /// The request number of the last op that the host sent to the worker of `session`, as the worker read it.
    pub fn last_request(&self, session: &str) -> u64 {
        let link = self.link_of(session);
        self.sent
            .iter()
            .rev()
            .find_map(|(l, m)| match m {
                HostMsg::Op { req, .. } if *l == link => Some(*req),
                _ => None,
            })
            .expect("an op was sent to the worker")
    }

    pub fn identity_of(&self, session: &str) -> ProcessIdentity {
        self.engine.sessions[&sid(session)]
            .worker
            .identity
            .expect("the session has a worker")
    }

    pub fn instance_of(&self, session: &str) -> InstanceId {
        self.engine.sessions[&sid(session)].instance.clone()
    }

    pub fn token_of(&self, session: &str) -> [u8; TOKEN_LEN] {
        self.engine.sessions[&sid(session)]
            .token
            .expect("the session has a token")
    }

    pub fn exited(&mut self, session: &str) {
        let identity = self.identity_of(session);
        self.feed(Input::ProcessExited {
            identity,
            status: ExitStatus::Code(0),
        });
    }
}

fn msg_name(msg: &HostMsg) -> &'static str {
    match msg {
        HostMsg::Launch(_) => "launch",
        HostMsg::Stop => "stop",
        HostMsg::Kill => "kill",
        HostMsg::Op { .. } => "op",
        HostMsg::Cancel { .. } => "cancel",
        HostMsg::Remove => "remove",
        _ => "other",
    }
}

pub(crate) fn terminal_state() -> TerminalState {
    TerminalState {
        size: size(),
        modes: serde_json::from_value(serde_json::json!({})).unwrap_or_else(|_| default_modes()),
        title: None,
        cwd: None,
        last_output_at: None,
        focused: Some(false),
        model_rev: ModelRev(1),
        input_rev: InputRevs {
            client: InputRev(0),
            host: InputRev(0),
        },
    }
}

fn default_modes() -> ModeFlags {
    ModeFlags::default()
}

pub(crate) fn states(events: &[Event]) -> Vec<(String, SessionState)> {
    events
        .iter()
        .filter_map(|e| match e {
            Event::SessionState { id, state, .. } => Some((id.0.clone(), *state)),
            _ => None,
        })
        .collect()
}

pub(crate) fn observation(msg: Observation) -> WorkerMsg {
    WorkerMsg::Observed { observation: msg }
}

mod admission;
mod adoption;
mod boundaries;
mod driver;
mod flow_edges;
mod lifecycle;
mod losses;
mod queue_pressure;
mod ready;
mod registry;
mod remove;
mod worker_link;
