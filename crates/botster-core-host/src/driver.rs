//! The driver of the `HostEngine` (plan 2.1, 2.5): it connects the machine to the edges and implements `CoreApi`.
//!
//! There is one driver. The real `Core` and the testkit's core both use it, each with its own [`HostEdges`], so the logic
//! that moves bytes between the links and the engine, orders the work of a `pump` with the scheduler, and keeps the wake
//! flag exists once (plan 2.1, Core A5-1: "one code path").
//!
//! The driver holds no clock and draws no random number: the host passes `now` to `pump` (TM-1), and the edges draw (2.3a).
//! It calls no operating system function: every effect is a call of an edge.

use crate::engine::{EngineConfig, HostEngine};
use crate::io::{Action, Input, LinkId, Work};
use crate::run::registry_failed;
use crate::session::ROW_PREFIX;
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    GroupSignal, IdentityState, ProcessIdentity, SpawnError, StorageError, Wake as WakeEdge,
};
use botster_core_edges::scheduler::{ChoicePoint, Scheduler};
use botster_core_edges::Machine;
use botster_core_link::frame::{encode_frame, FrameDecoder, FrameType, HEADER_LEN};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{HostMsg, WorkerMsg};
use botster_core_link::proof::TOKEN_LEN;
use botster_route_codec::prelude::QueryKind;
use std::collections::{BTreeMap, BTreeSet};
use std::io;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// What a worker needs to find the host and prove itself (AD-6, DP-8). The edge turns it into a command line and an
/// environment; the token never goes on a command line that other users can read.
#[derive(Debug, Clone)]
pub struct WorkerSpawn {
    pub program: PathBuf,
    pub instance: InstanceId,
    pub token: [u8; TOKEN_LEN],
    pub host_epoch: u64,
    /// `CoreLimits.startup`: the worker's bound of a candidate's hello and of its life with no payload and no host (AD-7).
    pub startup: std::time::Duration,
}

/// Why [`HostEdges::link_send_descriptor`] took nothing (DESIGN.md "The handoff"; worker-core DESIGN.md P4a).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DescriptorSendError {
    /// `WouldBlock` or `Interrupted`: the link takes no byte now. The writer keeps the endpoint and tries again at the next
    /// write readiness.
    Blocked,
    /// The handoff failed: the route closes `HandoffFailed`, and the driver closes the endpoint (DP-2).
    Failed,
}

/// The wake object: the `WakeHandle` that the host waits on, and the "runnable work exists" flag that the driver sets and
/// clears (TM-6, TH-2).
pub trait HostWake: WakeHandle + WakeEdge {}

impl<T: WakeHandle + WakeEdge> HostWake for T {}

/// Every edge that the host driver uses (plan 2.3), as one trait with a real implementation in `botster-core` and a testkit
/// implementation in `botster-core-testkit`. A call that cannot proceed returns `io::ErrorKind::WouldBlock`.
pub trait HostEdges: Send {
    /// The registry (`Storage`) and the random values (`Entropy`).
    fn fill_random(&mut self, buf: &mut [u8]);
    fn write_row(&mut self, key: &str, bytes: &[u8]) -> Result<(), StorageError>;
    fn delete_row(&mut self, key: &str) -> Result<(), StorageError>;
    /// Every row whose key starts with `prefix`, in key order.
    fn read_rows(&mut self, prefix: &str) -> Result<Vec<(String, Vec<u8>)>, StorageError>;

    /// Workers as processes (`Process`).
    fn spawn_worker(&mut self, spec: &WorkerSpawn) -> Result<ProcessIdentity, SpawnError>;
    fn signal_group(&mut self, identity: ProcessIdentity, signal: GroupSignal);
    /// What `identity` matches now (AD-6).
    fn identity_state(&self, identity: ProcessIdentity) -> IdentityState;
    fn poll_process_exit(&mut self) -> Option<ProcessIdentity>;

    /// The control links (`Link`). A worker connects to the host; `accept_link` returns the next new link, or `None`.
    fn accept_link(&mut self) -> Option<LinkId>;
    /// Connects to the endpoint of the worker of `instance` for an adoption (DESIGN.md "Adoption (P5)" 3.1, 6), and returns
    /// the new link, or `None` when no worker answers there. The edge owns the endpoint's path. The default has no worker
    /// endpoint: until an edge binds one, every adoption of a live worker ends `Lost(WorkerUnreachable)` (AD-2), which
    /// `Adopt(id)` may retry.
    fn connect_worker(&mut self, _instance: &InstanceId) -> Option<LinkId> {
        None
    }
    /// Removes the endpoint of the worker of `instance` if it is there (DESIGN.md "Adoption (P5)" part 1). Edges with no
    /// worker endpoints have nothing to remove. A failure is the edges' to record in their diagnostics.
    fn remove_endpoint(&mut self, _instance: &InstanceId) {}
    /// `Ok(0)` means that the peer closed the link.
    fn link_recv(&mut self, link: LinkId, buf: &mut [u8]) -> io::Result<usize>;
    fn link_send(&mut self, link: LinkId, bytes: &[u8]) -> io::Result<usize>;
    /// The bytes that the link's control stream may still take, beyond the driver's own count (IN-6, steward ruling R-91).
    /// The production edge sets no bound of its own: the driver's bound decides. The testkit's `control_queue` answers 0.
    fn link_room(&self, _link: LinkId) -> usize {
        usize::MAX
    }
    fn link_close(&mut self, link: LinkId);
    /// Write interest follows the outbound buffer (plan 2.5): on while bytes wait, off when none do.
    fn set_write_interest(&mut self, link: LinkId, on: bool);
    /// Read interest follows what the engine can take (plan 2.5 rule 7): off while a frame is held, on again when the engine
    /// can take it.
    fn set_read_interest(&mut self, link: LinkId, on: bool);
    /// Writes `bytes` (the start of a route's `AttachRoute` frame) with the route's stream riding on their first byte, as
    /// `sendmsg` with `SCM_RIGHTS` does (DP-2). `Ok((n, held))`, `n >= 1`: the link took the endpoint and the first `n`
    /// bytes. `held` is the host's own copy when the transport duplicates the descriptor (`SCM_RIGHTS`: the message holds
    /// another), and `None` when the endpoint itself travels. An error took nothing and gives the endpoint back.
    fn link_send_descriptor(
        &mut self,
        link: LinkId,
        bytes: &[u8],
        endpoint: StreamEndpoint,
    ) -> Result<(usize, Option<StreamEndpoint>), (StreamEndpoint, DescriptorSendError)>;
    /// Closes a route stream that the host still holds (OU-2, steward ruling R-50): writes `bytes` once, without waiting,
    /// then closes the stream. Best effort: a write that takes fewer bytes, or fails, still closes the stream. It never
    /// blocks the pump.
    fn close_route_stream(&mut self, endpoint: StreamEndpoint, bytes: &[u8]);

    /// The wake object (`Wake`). The driver signals and drains it, and hands it out as the `WakeHandle`.
    fn wake(&self) -> Arc<dyn HostWake>;
    /// Consumes the readiness that the wake object holds. The driver calls it when a `pump` found nothing to do, and reads
    /// every link once more after it, so that a readiness that arrives after the read is never consumed unread (plan 2.5).
    fn settle_wake(&mut self);
    /// The scheduling policy (`Scheduler`): the production policy, or the seeded policy of the testkit.
    fn scheduler(&mut self) -> &mut dyn Scheduler;
    /// What the edges know that the engine cannot: for example accept failures and foreign registry files (LC-10, one opaque value). None by default.
    fn diagnostics(&self) -> serde_json::Value {
        serde_json::Value::Null
    }
}

/// The errors of `open` that do not need the data directory: the limits (9B, LC-1) and the worker path (LC-1).
///
/// Clause: Core LC-1, Core 9B.
pub fn check_open(config: &OpenConfig) -> Result<PathBuf, CoreError> {
    config.limits.validate().map_err(CoreError::from)?;
    config.worker_path.clone().ok_or_else(|| {
        CoreError::new(
            ErrorCode::MissingWorkerPath,
            "OpenConfig.worker_path is not set",
        )
    })
}

/// The first byte count that a link read asks for (16 KiB).
const READ_CHUNK: usize = 16_384;

#[derive(Debug)]
struct LinkState {
    decoder: FrameDecoder,
    /// Bytes that the link did not take yet (plan 2.5: write interest while this is not empty).
    out: Vec<u8>,
    /// The route streams that ride with the first byte of their `AttachRoute` frame in `out`, in offset order (DP-2). The
    /// mark owns its endpoint until the link takes it.
    marks: std::collections::VecDeque<Mark>,
    /// The first frame of a link is the hello; after it, messages.
    hello_seen: bool,
    /// Bytes that were read and not decoded yet: at most one read chunk.
    pending: Vec<u8>,
    /// A decoded input that the engine cannot take now (EV-5b): the link is not read until it can.
    held: Option<Input>,
    /// The host's copies of the route streams that left on this link, until the worker has its own (DP-2, steward ruling
    /// R-65): its `RouteReceived`, or a `RouteClosed` of the route, closes the copy. A copy in flight is not the worker's
    /// own: a socket in flight with no other reference can lose its receive side (macOS).
    sent: BTreeMap<RouteId, StreamEndpoint>,
    /// A send failed: nothing more is written, and the link is read to its end of file (R-78 item 4). The worker's frames
    /// that wait in the socket are still handled.
    write_closed: bool,
    /// The worker process ended (R-78 item 4, `Action::LinkPeerEnded`): the link is read without waiting, and the first
    /// would-block closes it. No end of file is awaited.
    peer_ended: bool,
}

/// A route stream that waits in a link's outbound bytes: `at` is the offset of its frame in `out`, `len` the frame's length.
#[derive(Debug)]
struct Mark {
    at: usize,
    len: usize,
    route: RouteId,
    endpoint: StreamEndpoint,
}

/// The host driver. It is `Send`, and `Core` is `Send` and not `Sync` (TH-1) because the owner thread alone calls it.
pub struct HostDriver<E: HostEdges> {
    engine: HostEngine,
    edges: E,
    links: BTreeMap<LinkId, LinkState>,
    wake: Arc<dyn HostWake>,
    frame_bound: u32,
    /// Routes whose handoff failed, or whose held stream closed with its link (`HandoffLost`): each one is fed to the engine
    /// in a step of its own, within the budget of a pump (9B).
    failed_handoffs: std::collections::VecDeque<Input>,
    /// An exit that the process edge reaped when the pump had no budget left: the next pump feeds it first (9B). It is how
    /// the driver knows that an exit waits, so that `PumpReport.more` is true only then (TM-6).
    next_exit: Option<ProcessIdentity>,
    /// The end of a link (end of stream or a read error) that a read met when the pump had no budget left: the next pump
    /// closes the link for it, after the frames that came before it (9B, TM-6).
    link_ends: BTreeMap<LinkId, String>,
    /// The bound of each link's control stream, its unsent bytes (IN-6, steward ruling R-91; DESIGN.md "The control
    /// queue").
    pub(crate) control_bound: usize,
    /// One control message: the size of the largest `Cancel` frame (SV-3: the refused cost).
    pub(crate) control_message: usize,
}

/// The largest frame that a link takes: its header and the link's frame bound (`HostEngine::link_frame_bound`). It is the
/// control queue's term for one host message with no input payload (steward rulings R-91 and R-100; DESIGN.md "The control
/// queue"). It is a true bound: `encode_frame` refuses a larger payload, so no host message is queued larger. Some of those
/// messages carry strings that no `CoreLimits` value bounds (R-100), so the frame bound is the only bound for them.
pub(crate) fn largest_frame(frame_bound: u32) -> usize {
    HEADER_LEN + frame_bound as usize
}

/// The bound of a link's control stream (DESIGN.md "The control queue"): the input that one session may retain (IN-5), one
/// largest frame for each op that may be pending and each input op of the session (AM-3, IN-5), and room for four cancels.
/// With the default limits it is far above what a link holds in practice: the link edge's room (`HostEdges::link_room`) is
/// the limit that binds.
pub(crate) fn control_bound(
    limits: &CoreLimits,
    frame_bound: u32,
    control_message: usize,
) -> usize {
    let ops = limits.pending_ops as usize + limits.input_ops_per_session as usize;
    usize::try_from(limits.input_retained_bytes)
        .unwrap_or(usize::MAX)
        .saturating_add(ops.saturating_mul(largest_frame(frame_bound)))
        .saturating_add(4 * control_message)
}

/// The room of a link's control stream: the bound less its unsent bytes, and never more than the link edge allows.
fn control_room<E: HostEdges>(
    links: &BTreeMap<LinkId, LinkState>,
    edges: &E,
    bound: usize,
    link: LinkId,
) -> usize {
    let unsent = links.get(&link).map_or(0, |s| s.out.len());
    bound.saturating_sub(unsent).min(edges.link_room(link))
}

/// The next cursor ordinal of the data directory (steward ruling R-101 Q6): one above the registry's counter, or 1 when no
/// `Start` reserved one yet. A counter that is not a number is a corrupt registry.
fn next_ordinal<E: HostEdges>(edges: &mut E) -> Result<u64, CoreError> {
    let rows = edges
        .read_rows(crate::session::ORDINAL_KEY)
        .map_err(registry_failed)?;
    let counter = rows
        .iter()
        .find(|(key, _)| key == crate::session::ORDINAL_KEY)
        .map(|(_, bytes)| bytes.as_slice());
    crate::session::next_ordinal(counter).ok_or_else(|| {
        CoreError::new(
            ErrorCode::RegistryFailed { uncertain: false },
            "the cursor ordinal row is not a number",
        )
    })
}

impl<E: HostEdges> HostDriver<E> {
    /// Builds the driver over `edges`. The ids of the rows that the registry holds are read here, so that `Create` refuses an
    /// id that a durable row holds before `AdoptAll` runs (ID-1, LC-3, AD-2). The driver reads no clock: the first time it
    /// knows is the `now` of the first `pump` (TM-1).
    ///
    /// # Errors
    /// `RegistryFailed` when the registry cannot be read.
    pub fn open(cfg: EngineConfig, mut edges: E) -> Result<HostDriver<E>, CoreError> {
        let rows = edges.read_rows(ROW_PREFIX).map_err(registry_failed)?;
        let registry_ids = rows
            .into_iter()
            .filter_map(|(key, _)| key.strip_prefix(ROW_PREFIX).map(|id| SessionId(id.into())))
            .collect();
        let mut engine = HostEngine::new(cfg, registry_ids);
        engine.next_ordinal = next_ordinal(&mut edges)?;
        let frame_bound = engine.link_frame_bound();
        let wake = edges.wake();
        let mut payload = Vec::new();
        HostMsg::Cancel { req: u64::MAX }.encode(&mut payload);
        let mut frame = Vec::new();
        encode_frame(FrameType::HOST_MSG, &payload, frame_bound, &mut frame)
            .expect("a cancel frame is within every frame bound");
        let control_message = frame.len();
        let control_bound = control_bound(&engine.limits(), frame_bound, control_message);
        Ok(HostDriver {
            engine,
            edges,
            links: BTreeMap::new(),
            wake,
            frame_bound,
            failed_handoffs: std::collections::VecDeque::new(),
            next_exit: None,
            link_ends: BTreeMap::new(),
            control_bound,
            control_message,
        })
    }

    pub fn engine(&self) -> &HostEngine {
        &self.engine
    }

    pub fn edges(&mut self) -> &mut E {
        &mut self.edges
    }

    /// TM-6: a call that leaves work for `pump` signals the wake before it returns. Work is a runnable step, or an action that
    /// the call queued and only `pump` performs (an `attach` queues its handoff, a `cancel` its message). Parked work is
    /// neither, so it does not signal (EV-5b).
    fn sync_wake(&mut self) {
        if self.runnable() || self.engine.has_actions() {
            self.wake.signal();
        }
    }

    /// Whether the engine has ready work that the scheduler does not hold (TM-6: `more` means that runnable work remains). A
    /// held step is not runnable: it waits for the scheduler to let it go, which signals the wake (`Scheduler::held`, steward
    /// ruling R-88 group E).
    fn runnable(&mut self) -> bool {
        let ready = self.engine.ready();
        ready.iter().any(|work| !self.held(work))
    }

    /// TM-6: `PumpReport.more`: runnable work remains, or a link has input that the bound of this pump left unread.
    fn more(&mut self, budget: &Budget) -> bool {
        self.runnable() || budget.more_input
    }

    /// Whether the scheduler holds this step (Core A5-2, `Scheduler::held`). A deadline, a due step (a `Silent` or a route's
    /// stall step, R-90), and a step whose timing a clause fixes (R-20) are never held.
    fn held(&mut self, work: &Work) -> bool {
        !matches!(work, Work::Deadline | Work::DueStep)
            && !self.engine.never_deferred(work)
            && self.edges.scheduler().held(&self.engine.ops_of(work))
    }

    // ---- actions ----

    fn perform(&mut self) {
        while let Some(action) = self.engine.poll_action() {
            self.act(action);
        }
    }

    /// Performs the engine's actions, and counts the events that their results post against the pump (9B).
    fn perform_counted(&mut self, budget: &mut Budget) {
        self.perform();
        budget.account(&mut self.engine);
        // A failed handoff posts `RouteClosed`: one input per route, and only while the pump has budget (9B `pump_events`).
        while !budget.exhausted() {
            let Some(input) = self.failed_handoffs.pop_front() else {
                break;
            };
            self.feed(input);
            budget.account(&mut self.engine);
            self.perform();
        }
        if !self.failed_handoffs.is_empty() {
            budget.more_input = true;
        }
    }

    /// The production policy visits sessions round-robin (plan 2.4): when the chosen work is a session step, the scheduler
    /// picks among the session steps at the `Session` choice point, so one busy session cannot starve another.
    fn pick_session(&mut self, ready: &[Work], at: usize) -> Work {
        if !matches!(ready[at], Work::Session(_)) {
            return ready[at].clone();
        }
        let sessions: Vec<&Work> = ready
            .iter()
            .filter(|w| matches!(w, Work::Session(_)))
            .collect();
        let i = self
            .edges
            .scheduler()
            .pick(ChoicePoint::Session, sessions.len());
        sessions[i].clone()
    }

    /// A poll freed mandatory room: a link whose held frame can be taken now gets its read interest back, and the wake is
    /// signalled so that the host pumps (EV-5d, TM-6). The frame itself is delivered in the next `pump` (OR-1).
    fn unpark_links(&mut self) {
        let ids: Vec<LinkId> = self.links.keys().copied().collect();
        for link in ids {
            let ready = self
                .links
                .get(&link)
                .and_then(|s| s.held.as_ref())
                .is_some_and(|input| self.engine.can_accept(input));
            if ready {
                self.edges.set_read_interest(link, true);
                self.wake.signal();
            }
        }
    }

    /// Gives the engine an input at the time of the last `pump` (TM-1: only `pump` moves the clock).
    fn feed(&mut self, input: Input) {
        if let Input::LinkMsg {
            link,
            msg: WorkerMsg::RouteReceived { route } | WorkerMsg::RouteClosed { route, .. },
        } = &input
        {
            // The worker holds its own copy, or the route is closed: the host's copy closes as it drops (R-65).
            if let Some(state) = self.links.get_mut(link) {
                drop(state.sent.remove(route));
            }
        }
        self.engine.input(input);
    }

    fn act(&mut self, action: Action) {
        match action {
            Action::Random { ticket, len } => {
                let mut bytes = vec![0u8; len];
                self.edges.fill_random(&mut bytes);
                self.feed(Input::Random { ticket, bytes });
            }
            Action::WriteRow { ticket, key, bytes } => {
                let result = self.edges.write_row(&key, &bytes);
                self.feed(Input::RowWritten { ticket, result });
            }
            Action::DeleteRow { ticket, key } => {
                let result = self.edges.delete_row(&key);
                self.feed(Input::RowDeleted { ticket, result });
            }
            Action::ReadRows { ticket, prefix } => {
                let result = self.edges.read_rows(&prefix);
                self.feed(Input::Rows { ticket, result });
            }
            Action::SpawnWorker {
                ticket,
                program,
                instance,
                token,
                host_epoch,
                startup,
            } => {
                let result = self.edges.spawn_worker(&WorkerSpawn {
                    program,
                    instance,
                    token,
                    host_epoch,
                    startup,
                });
                self.feed(Input::Spawned { ticket, result });
            }
            Action::RemoveEndpoint { instance } => self.edges.remove_endpoint(&instance),
            Action::SendHello { link, hello } => {
                let mut payload = Vec::new();
                if hello.encode(&mut payload).is_ok() {
                    self.send_frame(link, FrameType::HELLO, &payload);
                }
            }
            Action::SendMsg { link, msg } => {
                let mut payload = Vec::new();
                msg.encode(&mut payload);
                self.send_frame(link, FrameType::HOST_MSG, &payload);
            }
            Action::CloseLink { link } => self.close_link(link),
            Action::LinkPeerEnded { link } => {
                if let Some(state) = self.links.get_mut(&link) {
                    state.peer_ended = true;
                }
            }
            Action::ConnectWorker { ticket, instance } => {
                let link = self.edges.connect_worker(&instance);
                if let Some(link) = link {
                    self.add_link(link);
                }
                self.feed(Input::WorkerConnected { ticket, link });
            }
            Action::ProbeIdentity { identity } => {
                let state = self.edges.identity_state(identity);
                self.feed(Input::IdentityState { identity, state });
            }
            Action::SignalGroup { identity, signal } => self.edges.signal_group(identity, signal),
            Action::HandoffRoute {
                link,
                route,
                transport,
                options,
                limits,
                client,
            } => self.send_handoff(link, route, transport, options, limits, client),
            Action::CloseRoute {
                route: _,
                transport,
                frame,
            } => self
                .edges
                .close_route_stream(transport, frame.as_deref().unwrap_or_default()),
        }
    }

    // ---- links ----

    /// Queues the route's `AttachRoute` frame like any frame, with its stream marked at the frame's first byte (DP-2). A
    /// link that is gone, or a frame over the bound, fails the handoff; the endpoint is dropped, which closes it.
    fn send_handoff(
        &mut self,
        link: LinkId,
        route: RouteId,
        endpoint: StreamEndpoint,
        options: AttachOptions,
        limits: AppliedRouteLimits,
        client: ClientId,
    ) {
        let mut payload = Vec::new();
        HostMsg::AttachRoute {
            route,
            options,
            limits,
            client,
        }
        .encode(&mut payload);
        let Some(state) = self.links.get_mut(&link).filter(|s| !s.write_closed) else {
            self.failed_handoffs
                .push_back(Input::HandoffFailed { route });
            return;
        };
        let at = state.out.len();
        if encode_frame(
            FrameType::HOST_MSG,
            &payload,
            self.frame_bound,
            &mut state.out,
        )
        .is_err()
        {
            state.out.truncate(at);
            self.failed_handoffs
                .push_back(Input::HandoffFailed { route });
            return;
        }
        let len = state.out.len() - at;
        state.marks.push_back(Mark {
            at,
            len,
            route,
            endpoint,
        });
        self.flush(link);
    }

    fn send_frame(&mut self, link: LinkId, kind: FrameType, payload: &[u8]) {
        // A link whose write side failed queues nothing (R-78 item 4).
        let Some(state) = self.links.get_mut(&link).filter(|s| !s.write_closed) else {
            return;
        };
        if let Err(error) = encode_frame(kind, payload, self.frame_bound, &mut state.out) {
            // A frame over the bound can never be sent: the link is broken.
            self.close_link_because(link, &error);
            return;
        }
        self.flush(link);
    }

    /// Writes the outbound bytes in order. The bytes before a mark go by `link_send`; at a mark, the frame's first bytes go by
    /// `link_send_descriptor` with the route's stream, so a partly written earlier frame is always complete first (DP-2).
    fn flush(&mut self, link: LinkId) {
        let Some(state) = self.links.get_mut(&link) else {
            return;
        };
        while !state.out.is_empty() {
            let next_mark = state.marks.front().map(|m| m.at);
            let sent = if next_mark == Some(0) {
                let mark = state.marks.pop_front().expect("a mark at 0");
                match self
                    .edges
                    .link_send_descriptor(link, &state.out[..mark.len], mark.endpoint)
                {
                    Ok((n, held)) => {
                        if let Some(held) = held {
                            state.sent.insert(mark.route, held);
                        }
                        n
                    }
                    Err((endpoint, DescriptorSendError::Blocked)) => {
                        state.marks.push_front(Mark { endpoint, ..mark });
                        break;
                    }
                    Err((endpoint, DescriptorSendError::Failed)) => {
                        // Nothing of the frame was taken: it is dropped whole, so the framing stays intact. The endpoint
                        // closes when it drops.
                        drop(endpoint);
                        self.failed_handoffs
                            .push_back(Input::HandoffFailed { route: mark.route });
                        mark.len
                    }
                }
            } else {
                let until = next_mark.unwrap_or(state.out.len());
                match self.edges.link_send(link, &state.out[..until]) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(e) if e.kind() == io::ErrorKind::WouldBlock => break,
                    Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) => {
                        self.close_write(link, &error);
                        return;
                    }
                }
            };
            state.out.drain(..sent);
            for mark in &mut state.marks {
                mark.at -= sent;
            }
        }
        let wanted = !state.out.is_empty();
        self.edges.set_write_interest(link, wanted);
    }

    /// Closes a link that the driver gives up on, and records why (LC-10 diagnostics). A link whose write side failed keeps
    /// that first reason.
    fn close_link_because(&mut self, link: LinkId, why: &dyn std::fmt::Display) {
        if self.links.get(&link).is_some_and(|s| !s.write_closed) {
            self.engine.record_link_close(link, why);
        }
        self.close_link(link);
    }

    /// Closes the write side of a link whose send failed, and records why (R-78 item 4). The link is read on, so the frames
    /// that the worker wrote before it ended are still handled; its end of file or a read error closes it (`close_link`).
    /// The streams in the outbound bytes never left: their routes close `HandoffLost` now. The streams that left stay with
    /// the link until their `RouteReceived` or its close.
    fn close_write(&mut self, link: LinkId, why: &dyn std::fmt::Display) {
        let Some(state) = self.links.get_mut(&link) else {
            return;
        };
        state.write_closed = true;
        state.out.clear();
        let marks = std::mem::take(&mut state.marks);
        self.engine.record_link_close(link, why);
        for mark in marks {
            self.failed_handoffs
                .push_back(Input::HandoffLost { route: mark.route });
        }
        self.edges.set_write_interest(link, false);
        self.feed(Input::LinkWriteFailed { link });
    }

    fn close_link(&mut self, link: LinkId) {
        self.link_ends.remove(&link);
        if let Some(state) = self.links.remove(&link) {
            // The streams that did not leave close with their marks. Nothing of them was sent: the engine decides each
            // route's close (`HandoffLost`), unless the link's loss closed it first (the first reason wins, OU-2).
            for mark in state.marks {
                self.failed_handoffs
                    .push_back(Input::HandoffLost { route: mark.route });
            }
            // The streams that left with no `RouteReceived`: Core cannot confirm the handoff, so the host's copy closes and
            // the engine decides each route's close as for a stream that never left (R-65; the steward's lost-ack ruling).
            for (route, held) in state.sent {
                drop(held);
                self.failed_handoffs.push_back(Input::HandoffLost { route });
            }
            self.edges.link_close(link);
            self.feed(Input::LinkClosed { link });
        }
    }

    /// A new link, accepted or made: its first frame in is a hello (AD-6).
    fn add_link(&mut self, link: LinkId) {
        self.links.insert(
            link,
            LinkState {
                decoder: FrameDecoder::new(self.frame_bound),
                out: Vec::new(),
                marks: std::collections::VecDeque::new(),
                hello_seen: false,
                pending: Vec::new(),
                held: None,
                sent: BTreeMap::new(),
                write_closed: false,
                peer_ended: false,
            },
        );
    }

    /// Reads every link that has input, and gives the engine what it decoded (plan 2.5). Control comes before data: this runs
    /// before the work of a `pump`. A link is read only as far as the engine can take what it decoded and as far as the
    /// budget of the pump allows: the rest stays unread (kernel buffer, or the one frame in `held`).
    fn service_links(&mut self, budget: &mut Budget) {
        while let Some(link) = self.edges.accept_link() {
            self.add_link(link);
        }
        let ids: Vec<LinkId> = self.links.keys().copied().collect();
        for link in ids {
            self.flush(link);
            self.control_room(link, budget);
            self.retry_held(link, budget);
            self.read_link(link, budget);
        }
    }

    /// After a flush: a control stream with room for one control message lets a session whose `cancel` was refused `Full`
    /// post `SessionWritable` (IN-6, SV-3). The event counts against the pump (9B); with no budget left it waits for the
    /// next pump, which the pump reports as `more`. A link with no refused cancel has no waiting work, so its room is never
    /// `more` (TM-6).
    fn control_room(&mut self, link: LinkId, budget: &mut Budget) {
        if !self.engine.waits_for_room(link)
            || control_room(&self.links, &self.edges, self.control_bound, link)
                < self.control_message
        {
            return;
        }
        if budget.exhausted() {
            budget.more_input = true;
            return;
        }
        self.engine.control_room(link);
        budget.account(&mut self.engine);
    }

    /// An open link of an ended worker with no held frame is runnable work (R-78 item 4, TM-6: `more` only for input that is
    /// known to wait). Its drain has not read since the end, or it stopped at the pump's budget; every other read either
    /// closes the link or holds a frame. A held frame waits for mandatory room and is not runnable work: the poll that frees
    /// room restores the link (EV-5d).
    fn draining(&self) -> bool {
        self.links
            .values()
            .any(|s| s.peer_ended && s.held.is_none())
    }

    /// Gives the engine the exits that the process edge reaped, while the pump has budget (9B `pump_events`).
    ///
    /// An exit that the pump has no budget for is taken from the edge and kept for the next pump, which feeds it first. So the
    /// pump reports `more` only when an exit really waits (TM-6: `more` means runnable work remains), never because the
    /// budget ran out before the edge was asked.
    fn take_exits(&mut self, budget: &mut Budget) {
        loop {
            let Some(identity) = self
                .next_exit
                .take()
                .or_else(|| self.edges.poll_process_exit())
            else {
                return;
            };
            if budget.exhausted() {
                self.next_exit = Some(identity);
                budget.more_input = true;
                return;
            }
            self.feed(Input::ProcessExited { identity });
            budget.account(&mut self.engine);
        }
    }

    /// Delivers the frame that the engine could not take before, as soon as it can (EV-5d): it runs whenever the link is
    /// serviced, so a frame held behind a step (the end of a start) goes in after that step, before any later frame.
    fn retry_held(&mut self, link: LinkId, budget: &mut Budget) {
        let Some(input) = self.links.get_mut(&link).and_then(|s| s.held.take()) else {
            return;
        };
        // A frame that waits for mandatory-queue room is parked: it is not runnable, so it never makes `more` true (TM-6,
        // EV-5b). A poll that frees room signals the wake.
        if !self.engine.can_accept(&input) {
            // A frame that waited for budget can find the queue full now: its link stops reading, as in `deliver`.
            self.edges.set_read_interest(link, false);
            self.links.get_mut(&link).expect("kept").held = Some(input);
            return;
        }
        // A held frame posts events like any other input: it waits for a pump with budget left (9B `pump_events`). It is
        // runnable, so the pump reports `more`.
        if budget.exhausted() {
            self.links.get_mut(&link).expect("kept").held = Some(input);
            budget.more_input = true;
            return;
        }
        self.feed(input);
        budget.account(&mut self.engine);
        self.edges.set_read_interest(link, true);
    }

    fn read_link(&mut self, link: LinkId, budget: &mut Budget) {
        let mut buf = vec![0u8; READ_CHUNK];
        loop {
            let Some(state) = self.links.get_mut(&link) else {
                return;
            };
            if state.held.is_some() {
                return;
            }
            // 1. The end of the link that this read met in a pump with no budget left. It came after every frame that the
            // link carried before it, so it waits for nothing else.
            if let Some(why) = self.link_ends.remove(&link) {
                self.end_link(link, why, budget);
                return;
            }
            // 2. The bytes that were read earlier and not yet decoded, or a frame that the decoder completed: one frame at a
            // time. `deliver` decides whether the frame goes to the engine now, waits for room, or waits for budget.
            if !state.pending.is_empty() || state.decoder.ready() {
                let took = state.decoder.push(&state.pending);
                state.pending.drain(..took);
                match state.decoder.next_frame() {
                    Ok(Some(frame)) => {
                        let first = !state.hello_seen;
                        state.hello_seen = true;
                        match self.deliver(link, first, frame.kind, &frame.payload, budget) {
                            Delivery::Done => {}
                            Delivery::Held => return,
                            Delivery::Bad(why) => {
                                self.end_link(link, why, budget);
                                return;
                            }
                        }
                        if !self.links.contains_key(&link) {
                            return;
                        }
                        continue;
                    }
                    // The decoder took every pending byte and waits for the rest of a frame: the read below comes next.
                    // Going on to the read, not back to the top, keeps each turn of this loop on a delivered frame or a
                    // read, so the loop ends by its own structure (plan section 8).
                    Ok(None) => {}
                    Err(error) => {
                        // A frame over the bound ends the link (plan section 3).
                        self.end_link(link, error.to_string(), budget);
                        return;
                    }
                }
            }
            // 3. More bytes, within the bytes that one pump may read from this link (9B `pump_bytes`). A read never takes
            // more than what is left of that allowance. A read posts no event, so it runs with no event budget left too:
            // that is how the pump learns whether a whole frame waits (TM-6).
            let slice = budget.bytes_left(link).min(READ_CHUNK);
            if slice == 0 {
                // The pump read every byte that it may read from this link. Whether more bytes wait cannot be known without
                // a read past that bound, so the pump reports `more`.
                budget.more_input = true;
                return;
            }
            let n = match self.edges.link_recv(link, &mut buf[..slice]) {
                Ok(0) => {
                    self.end_link(link, "the peer closed it".to_string(), budget);
                    return;
                }
                Ok(n) => n,
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                    // The worker process ended, so nothing more comes: the drain ends here, and a partial frame is a
                    // truncated link (R-78 item 4, steward follow-up: never wait for the end of file). Closing it posts
                    // events, so a pump with no budget left keeps the end for the next pump (9B, TM-6: `end_link`).
                    if self.links.get(&link).is_some_and(|s| s.peer_ended) {
                        self.end_link(
                            link,
                            "the worker process ended: its link is drained".to_string(),
                            budget,
                        );
                    }
                    return;
                }
                Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(error) => {
                    self.end_link(link, error.to_string(), budget);
                    return;
                }
            };
            budget.read(link, n);
            if let Some(state) = self.links.get_mut(&link) {
                state.pending.extend_from_slice(&buf[..n]);
            }
        }
    }

    /// Ends a link for `why`. Closing it posts events (the loss of its session), so a pump with no budget left keeps the end
    /// for the next pump, which closes the link first (9B), and reports `more` (TM-6).
    fn end_link(&mut self, link: LinkId, why: String, budget: &mut Budget) {
        if budget.exhausted() {
            self.link_ends.insert(link, why);
            budget.more_input = true;
        } else {
            self.close_link_because(link, &why);
        }
    }

    /// Turns one frame into an input. `Held` when the engine cannot take it now (EV-5b: the link's read interest goes off), or
    /// when the pump has no budget left for it (9B).
    fn deliver(
        &mut self,
        link: LinkId,
        first: bool,
        kind: FrameType,
        payload: &[u8],
        budget: &mut Budget,
    ) -> Delivery {
        let input = if first {
            if kind != FrameType::HELLO {
                return Delivery::Bad(format!(
                    "the first frame is of type {}, not a hello",
                    kind.0
                ));
            }
            match Hello::decode(payload) {
                Ok(hello) => Input::LinkHello { link, hello },
                Err(error) => return Delivery::Bad(error.to_string()),
            }
        } else {
            if kind != FrameType::WORKER_MSG {
                return Delivery::Bad(format!(
                    "a frame of type {} is not a worker message",
                    kind.0
                ));
            }
            match WorkerMsg::decode(payload) {
                Ok(msg) => Input::LinkMsg { link, msg },
                Err(error) => return Delivery::Bad(error.to_string()),
            }
        };
        if !self.engine.can_accept(&input) {
            self.edges.set_read_interest(link, false);
            if let Some(state) = self.links.get_mut(&link) {
                state.held = Some(input);
            }
            return Delivery::Held;
        }
        // The engine can take it, but this pump has no budget left (9B `pump_events`): it waits for the next pump, which
        // takes it first (`retry_held`). It is runnable, so the pump reports `more` (TM-6).
        if budget.exhausted() {
            if let Some(state) = self.links.get_mut(&link) {
                state.held = Some(input);
            }
            budget.more_input = true;
            return Delivery::Held;
        }
        self.feed(input);
        budget.account(&mut self.engine);
        Delivery::Done
    }
}

enum Delivery {
    Done,
    Held,
    /// The frame is not what the link carries here: the link closes, for this reason.
    Bad(String),
}

/// The bounds of one `pump` (9B `pump_events`, `pump_bytes`, plan 2.4).
struct Budget {
    /// The events that this pump may post (the scheduler's bound).
    bound: usize,
    posted: usize,
    /// The bytes that one link may deliver in this pump.
    per_link_bytes: usize,
    read: BTreeMap<LinkId, usize>,
    /// Input remains unread because of a bound: the host must pump again.
    more_input: bool,
}

impl Budget {
    fn exhausted(&self) -> bool {
        self.posted >= self.bound
    }

    fn account(&mut self, engine: &mut HostEngine) {
        self.posted += engine.take_posted() as usize;
    }

    fn bytes_left(&self, link: LinkId) -> usize {
        self.per_link_bytes
            .saturating_sub(self.read.get(&link).copied().unwrap_or(0))
    }

    fn read(&mut self, link: LinkId, n: usize) {
        *self.read.entry(link).or_insert(0) += n;
    }
}

impl<E: HostEdges> CoreApi for HostDriver<E> {
    fn begin(&mut self, op: Op) -> Result<OpId, CoreError> {
        let id = self.engine.begin(op)?;
        let fixed_timing = self.engine.fixed_timing(id);
        self.edges.scheduler().admitted(id.0, fixed_timing);
        self.sync_wake();
        Ok(id)
    }

    /// The only call that makes progress (OR-1, TM-2): the order of the work is the scheduler's (plan 2.4), and every kind of
    /// work (link input, process exits, steps) counts against the bounds of this pump (9B).
    fn pump(&mut self, now: Now) -> PumpReport {
        // Events that an earlier call posted (a `cancel`) are not this pump's.
        let _ = self.engine.take_posted();
        let limits = self.engine.limits();
        let bound = self
            .edges
            .scheduler()
            .bound(ChoicePoint::PumpBound, limits.pump_events as usize);
        let mut budget = Budget {
            bound,
            posted: 0,
            per_link_bytes: usize::try_from(limits.pump_bytes).unwrap_or(usize::MAX),
            read: BTreeMap::new(),
            more_input: false,
        };
        // The one place where time enters Core (TM-1): every later input of this pump is at this time.
        self.engine.handle(now.monotonic, Input::Clock(now.unix));
        // A due atomic step (a `Silent`, a route's stall step) is an older step than any input that arrives now: it runs
        // first, in due order, with the budget it needs (E3-1 items 3 and 5). The loop runs once for each step that is due,
        // so this pump ends whatever a step does (9B).
        for _ in 0..self.engine.due_steps() {
            if budget.exhausted() {
                break;
            }
            self.feed(Input::Run(Work::DueStep));
            budget.account(&mut self.engine);
        }
        self.take_exits(&mut budget);
        // An exit's actions run before the links are read: a link of an ended worker is marked for its drain
        // (`LinkPeerEnded`, R-78 item 4), so this pump's read ends the drain and the work loop below classifies the session.
        self.perform_counted(&mut budget);
        // Each link's held frame goes first, before the link's later frames (`service_links`).
        self.service_links(&mut budget);
        self.perform_counted(&mut budget);
        let mut deferred: BTreeSet<Work> = BTreeSet::new();
        loop {
            // At the event bound only an effect without an event may run; every step with an event is carried (E3-1).
            let ready: Vec<Work> = self
                .engine
                .ready()
                .into_iter()
                .filter(|w| !deferred.contains(w))
                .filter(|w| !budget.exhausted() || *w == Work::Deadline)
                .collect();
            // A held step is not runnable work: it does not run, and no other choice point is asked about it.
            let ready: Vec<Work> = ready.into_iter().filter(|w| !self.held(w)).collect();
            if ready.is_empty() {
                break;
            }
            let at = self
                .edges
                .scheduler()
                .pick(ChoicePoint::ReadyWork, ready.len());
            let work = self.pick_session(&ready, at);
            // A5-2: the scheduler may defer the progress of an operation to a later pump. A deadline is never deferred.
            if !matches!(work, Work::Deadline | Work::DueStep)
                && !self.engine.never_deferred(&work)
                && self
                    .edges
                    .scheduler()
                    .pick(ChoicePoint::OperationDeferral, 2)
                    == 1
            {
                deferred.insert(work);
                continue;
            }
            self.feed(Input::Run(work));
            budget.account(&mut self.engine);
            self.perform_counted(&mut budget);
            self.service_links(&mut budget);
            self.perform_counted(&mut budget);
        }
        // An ended link that waits for its drain needs no term here: the settle below reads every link.
        let mut more = self.more(&budget);
        if !more {
            self.edges.settle_wake();
            // A reaper may have queued an exit after the exits were taken above, and the settle consumed its wake: the exits
            // are taken again after the settle, so that none waits in the edge without a wake (TM-6).
            self.take_exits(&mut budget);
            self.service_links(&mut budget);
            self.perform_counted(&mut budget);
            more = self.more(&budget) || self.draining();
        }
        if more {
            // Work or input remains: the wake stays set, so a host that waits on it pumps again (TM-6).
            self.wake.signal();
        } else {
            self.wake.drain();
        }
        PumpReport {
            more,
            events_posted: u32::try_from(budget.posted).unwrap_or(u32::MAX),
        }
    }

    /// A5-2: the scheduler chooses the size of the non-empty part of the queue that a poll returns; the production policy
    /// returns `max`.
    fn poll_events(&mut self, max: usize) -> Vec<Event> {
        let batch = self.edges.scheduler().bound(ChoicePoint::PollBatch, max);
        let events = self.engine.poll_events(batch);
        self.unpark_links();
        self.sync_wake();
        events
    }

    fn wake_handle(&self) -> Arc<dyn WakeHandle> {
        let wake: Arc<dyn HostWake> = Arc::clone(&self.wake);
        wake
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.engine.next_deadline()
    }

    /// The cancel's message to the worker is queued, and the next `pump` sends it (OR-1, TM-6, IN-6: `Admitted` is "admitted to
    /// the worker stream"). A control stream with no room for one control message refuses it `Full` (IN-6, R-91).
    fn cancel(&mut self, op: OpId) -> CancelResult {
        let (links, edges, bound, need) = (
            &self.links,
            &self.edges,
            self.control_bound,
            self.control_message,
        );
        let result = self
            .engine
            .cancel_with_room(op, |link| control_room(links, edges, bound, link) >= need);
        self.sync_wake();
        result
    }

    fn get(&self, id: &SessionId) -> Result<SessionRecord, CoreError> {
        self.engine.get(id)
    }

    fn list(&self) -> Vec<SessionRecord> {
        self.engine.list()
    }

    fn status(&self) -> Status {
        self.engine.status()
    }

    fn diagnostics(&self) -> serde_json::Value {
        let mut diagnostics = self.engine.diagnostics();
        diagnostics["edges"] = self.edges.diagnostics();
        // The sessions whose work waits only for mandatory-queue room: an engine step, or a frame that a link holds (EV-5b,
        // TM-6). The test control `parked_work` reads it (steward ruling R-87).
        diagnostics["parked_work"] = serde_json::json!(self
            .engine
            .parked_on_room(self.links.values().filter_map(|l| l.held.as_ref())));
        diagnostics
    }

    fn terminal_state(&self, id: &SessionId) -> Result<TerminalState, CoreError> {
        self.engine.terminal_state(id)
    }

    fn read_page(&self, capture: CaptureId, page: u32) -> Result<Page, CoreError> {
        self.engine.read_page(capture, page)
    }

    fn release(&mut self, capture: CaptureId) {
        self.engine.release(capture);
    }

    fn release_owner(&mut self, client: &ClientId) {
        self.engine.release_owner(client);
    }

    fn snapshot_formats(&self, session: &SessionId) -> Result<Vec<SnapshotFormat>, CoreError> {
        self.engine.snapshot_formats(session)
    }

    fn shadow_answerable_kinds(&self) -> Vec<QueryKind> {
        self.engine.shadow_answerable_kinds()
    }

    fn attach(
        &mut self,
        client: ClientId,
        session: SessionId,
        transport: RouteTransport,
        options: AttachOptions,
    ) -> Result<AttachResult, AttachRefused> {
        let result = self.engine.attach(client, session, transport, options)?;
        self.sync_wake();
        Ok(result)
    }

    fn tap_read(&mut self, session: &SessionId, _max: usize) -> Result<TapChunk, CoreError> {
        self.engine.tap_read(session)
    }

    fn set_silence_threshold(
        &mut self,
        session: &SessionId,
        threshold: Option<Duration>,
    ) -> Result<(), CoreError> {
        let result = self.engine.set_silence_threshold(session, threshold);
        self.sync_wake();
        result
    }

    fn service_send(
        &mut self,
        _id: &ServiceId,
        _lane: u8,
        _frame: &OutboundFrame,
    ) -> Result<(), SendError> {
        Err(SendError::UnknownService)
    }

    fn service_recv(&mut self, _id: &ServiceId, _lane: u8) -> Result<Option<Frame>, RecvError> {
        Err(RecvError::UnknownService)
    }

    fn service_report(&self, id: &ServiceId) -> Result<SpawnReport, CoreError> {
        Err(CoreError::new(
            ErrorCode::UnknownService,
            format!("no service {id}"),
        ))
    }

    fn service_log_tail(&self, id: &ServiceId, _max: usize) -> Result<Vec<u8>, CoreError> {
        Err(CoreError::new(
            ErrorCode::UnknownService,
            format!("no service {id}"),
        ))
    }

    fn features(&self) -> Features {
        self.engine.features()
    }

    fn limits(&self) -> CoreLimits {
        self.engine.limits()
    }

    fn worker_protocol(&self) -> u8 {
        self.engine.worker_protocol()
    }

    fn adoptable_worker_protocols(&self) -> BTreeSet<u8> {
        self.engine.adoptable_worker_protocols()
    }

    fn worker_protocol_compatibility(&self, protocol: Option<u8>) -> WorkerCompatibility {
        self.engine.worker_protocol_compatibility(protocol)
    }

    fn terminal_identity(&self) -> TerminalIdentity {
        self.engine.terminal_identity()
    }
}
