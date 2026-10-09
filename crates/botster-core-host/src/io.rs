//! The inputs and the actions of the `HostEngine` (plan 2.1): what a driver feeds in, and what it must perform.
//!
//! The engine never calls an edge. When it needs a registry write, a random value or a process, it emits an action with a
//! [`Ticket`], and the driver answers with the input that carries the same ticket. A message of a worker link is an input too.
//! The engine decodes no bytes: the driver owns framing and gives it messages (plan section 3).

use botster_core_contract::prelude::*;
use botster_core_edges::edges::{
    ExitStatus, GroupSignal, IdentityState, ProcessIdentity, SpawnError, StorageError,
};
use botster_core_link::hello::{Hello, TokenProof};
use botster_core_link::msg::{HostMsg, WorkerMsg};
use botster_core_link::proof::TOKEN_LEN;
use std::path::PathBuf;

/// The number that ties an action to its answer. Tickets are counters, never random (plan 2.3a).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ticket(pub u64);

/// A control link, as the driver numbers it. The engine learns which session it serves from the hello.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LinkId(pub u64);

/// A piece of ready work that the driver may run (plan 2.4: the choice points of A5-2).
///
/// A driver lists the work in the order that the production policy runs it, and a seeded policy picks another index.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Work {
    /// One step of an operation.
    Op(OpId),
    /// One step of the flow of a session (create, start, stop or remove).
    Session(SessionId),
    /// The earliest due deadline whose effect posts no event: capture expiry, the kill of `stop_grace`, the startup and
    /// remove grace. It runs in its due pump whatever the event budget (TM-3, erratum 3 E3-1 item 1).
    Deadline,
    /// The earliest due `Silent`: one atomic step with its event. It needs event budget, and otherwise it is carried to the
    /// next pump (TM-4, E3-1 items 2, 3 and 6).
    Silent,
    /// A route event that waited for queue room.
    Parked,
}

/// What the engine asks its driver to do.
#[derive(Debug)]
#[non_exhaustive]
pub enum Action {
    /// `len` random bytes from the `Entropy` edge; the answer is [`Input::Random`] (AD-6).
    Random {
        ticket: Ticket,
        len: usize,
    },
    /// One durable registry write (AD-7); the answer is [`Input::RowWritten`].
    WriteRow {
        ticket: Ticket,
        key: String,
        bytes: Vec<u8>,
    },
    /// Reads every row whose key starts with `prefix`, in key order (AD-1); the answer is [`Input::Rows`].
    ReadRows {
        ticket: Ticket,
        prefix: String,
    },
    /// Deletes a registry row (LC-7 step 4); the answer is [`Input::RowDeleted`].
    DeleteRow {
        ticket: Ticket,
        key: String,
    },
    /// Starts a worker (AD-7 step 2); the answer is [`Input::Spawned`].
    SpawnWorker {
        ticket: Ticket,
        program: PathBuf,
        instance: InstanceId,
        token: [u8; TOKEN_LEN],
        host_epoch: u64,
        /// `CoreLimits.startup`, for the worker (DESIGN.md "Adoption (P5)", parts 1 and 7).
        startup: std::time::Duration,
    },
    /// Removes the endpoint of the worker of `instance`, if it is still there (DESIGN.md "Adoption (P5)" part 1; SV-9): the
    /// worker's end is verified (LC-7 step 3), and a worker that was killed could not remove it. No answer: a failure is
    /// recorded in the edges' diagnostics and does not fail the `Remove`.
    RemoveEndpoint {
        instance: InstanceId,
    },
    /// Connects to the endpoint of the worker of `instance` (DESIGN.md "Adoption (P5)" 3.1); the answer is
    /// [`Input::WorkerConnected`]. The host speaks first on the new link: the worker cannot prove the epoch of a host that
    /// it has not heard (DP-8).
    ConnectWorker {
        ticket: Ticket,
        instance: InstanceId,
    },
    /// The host's side of the hello, on a link that sent a valid one (AD-6).
    SendHello {
        link: LinkId,
        hello: Hello,
    },
    SendMsg {
        link: LinkId,
        msg: HostMsg,
    },
    /// Closes a link: a hello that failed AD-6 or AD-4, or a session that ended.
    CloseLink {
        link: LinkId,
    },
    /// Asks what the identity of a worker matches now (AD-6); the answer is [`Input::IdentityState`]. A worker that this handle
    /// did not spawn has no exit watch, so this is how the engine learns that it is gone (LC-7 step 3, AD-2 `WorkerGone`).
    ProbeIdentity {
        identity: ProcessIdentity,
    },
    /// Signals the process group of a worker, only when its identity still matches (AD-6).
    SignalGroup {
        identity: ProcessIdentity,
        signal: GroupSignal,
    },
    /// Hands a route's connected stream to the worker over its link (DP-2). Core owns the endpoint from here.
    HandoffRoute {
        link: LinkId,
        route: RouteId,
        transport: StreamEndpoint,
        options: AttachOptions,
    },
}

/// What a driver tells the engine.
#[derive(Debug)]
#[non_exhaustive]
pub enum Input {
    /// The unix time of this `pump` (TM-1). It only stamps `at` fields.
    Clock(UnixSeconds),
    /// Runs one piece of ready work.
    Run(Work),
    Random {
        ticket: Ticket,
        bytes: Vec<u8>,
    },
    RowWritten {
        ticket: Ticket,
        result: Result<(), StorageError>,
    },
    RowDeleted {
        ticket: Ticket,
        result: Result<(), StorageError>,
    },
    Rows {
        ticket: Ticket,
        result: Result<Vec<(String, Vec<u8>)>, StorageError>,
    },
    Spawned {
        ticket: Ticket,
        result: Result<ProcessIdentity, SpawnError>,
    },
    /// The answer to [`Action::ConnectWorker`]: the new link, or `None` when no worker answers at the endpoint.
    WorkerConnected {
        ticket: Ticket,
        link: Option<LinkId>,
    },
    /// The first frame of a link, a valid-framed hello: of a worker that connected, or the worker's answer on a link that
    /// the host made (`Action::ConnectWorker`).
    LinkHello {
        link: LinkId,
        hello: Hello,
    },
    LinkMsg {
        link: LinkId,
        msg: WorkerMsg,
    },
    /// The link ended: the peer closed it, or the driver broke it.
    LinkClosed {
        link: LinkId,
    },
    /// The handoff of a route's stream to its worker failed: the route closes `HandoffFailed` (DP-2, OU-2).
    HandoffFailed {
        route: RouteId,
    },
    /// A process that the engine spawned ended (the exit watch of the `Process` edge).
    ProcessExited {
        identity: ProcessIdentity,
        status: ExitStatus,
    },
    IdentityState {
        identity: ProcessIdentity,
        state: IdentityState,
    },
}

/// The proof of a hello as the engine needs it: the engine recomputes it from the token (AD-6).
pub type Proof = TokenProof;
