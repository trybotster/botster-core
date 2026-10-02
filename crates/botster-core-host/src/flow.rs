//! The flows of a session (plan 2.2): create, start, stop and remove are machines of the session, not of one operation.
//!
//! A `Stop` joins a stop that is already running, a `StopAll` stops a session that no operation names, and a `Stop` of a
//! `Starting` session waits for the start to end (LC-12). So the flow is the session's, and an operation is a waiter on it.
//! Every step of a flow changes the state of the session and posts at most one event, so that "a state change and its event
//! are one atomic step" (EV-5b) and `pump_events` is a bound that a step cannot overshoot (9B).

use botster_core_contract::prelude::*;
use std::time::Instant;

/// What the session is working on. At most one flow runs at a time: the admission table (AM-1) allows no other.
#[derive(Debug, Clone, Default)]
pub enum Flow {
    #[default]
    Idle,
    Create(CreateFlow),
    Start(StartFlow),
    Stop(StopFlow),
    Remove(RemoveFlow),
}

/// `begin(Create)`: the row is written, then `SessionState{Created}` is posted, then `Completed` (OR-2).
#[derive(Debug, Clone)]
pub struct CreateFlow {
    pub op: OpId,
    pub phase: CreatePhase,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CreatePhase {
    WriteRow,
    PostCreated,
    Finish,
}

/// Why a start failed, and the state that the session reaches (LC-4: `Exited` or `Lost`, never `Starting`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartFailure {
    pub reason: StartFailReason,
    pub state: SessionState,
}

/// `begin(Start)`: AD-7 in order. The row is written, the worker is spawned, its identity is written, and only then does the
/// worker launch the payload.
#[derive(Debug, Clone)]
pub struct StartFlow {
    pub op: OpId,
    pub phase: StartPhase,
    pub failure: Option<StartFailure>,
    /// The `startup` deadline, from the spawn until the payload runs (LC-4, A2-5).
    pub deadline: Option<Instant>,
    /// The error that completes the op when it is not a `StartFailed` (a registry failure).
    pub error: Option<CoreError>,
    /// The hello arrived before the identity row was durable.
    pub hello_seen: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartPhase {
    /// Draws the token (AD-6).
    Token,
    /// AD-7 step 1: the row `Starting` with the instance and the token.
    RowStarting,
    PostStarting,
    /// AD-7 step 2.
    Spawn,
    /// AD-7 step 3.
    RowIdentity,
    /// Waits for the worker's hello (AD-6, AD-4).
    AwaitHello,
    /// AD-7 step 4: the launch request.
    SendLaunch,
    AwaitLaunched,
    PostRunning,
    /// Posts the state that a failed start reaches.
    PostFailed,
    Finish,
}

/// What a stop is waiting for. The request to the payload goes out before the state event is posted, so a full queue never
/// delays the effect of a stop (EV-5c).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopPhase {
    RowWrite,
    /// The graceful request, and the `stop_grace` deadline.
    SendStop,
    /// `SessionState{Stopping}`. The payload may end while this event waits for room: `end` is then set.
    PostStopping,
    AwaitExit,
    PostEnd,
    /// Completes one waiter per step.
    Finish,
}

/// A stop of the payload (LC-5): the graceful request, then after `stop_grace` the kill of the group.
#[derive(Debug, Clone)]
pub struct StopFlow {
    pub phase: StopPhase,
    pub deadline: Option<Instant>,
    /// How the session ended, once it did.
    pub end: Option<SessionEnd>,
}

/// `begin(Remove)`: LC-7 in order.
#[derive(Debug, Clone)]
pub struct RemoveFlow {
    pub op: OpId,
    pub phase: RemovePhase,
    /// The authenticated worker's cleanup result, or `OutcomeUnknown` when none came (A6-3).
    pub uploads: Option<UploadsOutcome>,
    /// The worker process ended (LC-7 step 3: the worker ends before the row is deleted).
    pub worker_gone: bool,
    /// The grace after which a worker that did not end is killed.
    pub deadline: Option<Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemovePhase {
    /// Step 2 and the request of step 3. The effects come first (EV-5c); the events follow as room allows.
    SendRemove,
    /// Step 1: closes the routes that are still bound, one event per step.
    CloseRoutes,
    /// Step 3: the worker's complete result, and the end of the worker process.
    AwaitTeardown,
    /// Step 4.
    DeleteRow,
    /// Step 5 and `SessionState{Released}`.
    PostReleased,
    Finish,
}
