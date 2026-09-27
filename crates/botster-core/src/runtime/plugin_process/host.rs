//! Parent side of one plugin process: startup through `Loaded`, the invoke
//! path, stop, kill, and the exit watch that is the only reaper.

use std::collections::VecDeque;
use std::io::{self, Read};
use std::net::Shutdown;
use std::os::fd::{AsFd, AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::ExitStatusExt;
use std::process::{Child, ChildStderr, ExitStatus};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use crate::actor::{
    PluginInvocationFailure, PluginInvocationFailureKind, PluginInvocationRequest,
    PluginInvocationResult, PluginKey,
};
use crate::contract::session_protocol::{Frame, FrameDecoder, MAX_FRAME_LEN};
use crate::engine::{CallId, DeliveryPool};
use crate::runtime::process_exit::ExitWatch;
use crate::runtime::{PluginCancellationToken, PluginRuntime};
use crate::session::RequestId;

use super::ingress::{CallAdmission, Ingress, PluginIngress, PluginIngressNotifier};
use super::invocations::{Admission, GraceExpiry, Invocation, Invocations, Outcome};
use super::launch::{launch, Launched};
use super::outbound::{Lane, LaneBounds, Next, Outbound, Refused};
use super::protocol::{
    decode_json, encode_bounded, encode_json_bounded, send_all, BootstrapFrame, CancelFrame,
    CreditFrame, CreditGrants, FailedFrame, HostCallFrame, LoadEnvelope, LoadFrame, LoadedFrame,
    LogFrame, PluginRegistration, ReadyFrame, CAUSE_MEMORY_CAP, CAUSE_PANIC, FATAL_MESSAGE_BYTES,
    FRAME_BOOTSTRAP, FRAME_BOOTSTRAP_FAILED, FRAME_CANCEL, FRAME_CREDIT, FRAME_HOST_CALL,
    FRAME_INVOCATION_RESULT, FRAME_INVOKE, FRAME_LOAD, FRAME_LOADED, FRAME_LOAD_FAILED, FRAME_LOG,
    FRAME_READY, FRAME_SHUTDOWN, PROTOCOL_MAGIC, PROTOCOL_VERSION,
};
use super::supervisor::{Expiry, KillState, ProcessKiller, Supervisor};
use super::{
    PluginExitCause, PluginKillReason, PluginProcessConfig, PluginProcessError, PluginProcessExited,
};

/// Callback run once, outside every lock, after the process is reaped.
pub type PluginExitNotifier = Arc<dyn Fn() + Send + Sync + 'static>;

/// A loaded plugin process. Dropping it stops the process; the exit watch
/// reaps it even after the handle is gone.
pub struct PluginProcess {
    shared: Arc<Shared>,
}

struct Shared {
    pid: u32,
    killer: Arc<ProcessKiller>,
    supervisor: Supervisor,
    ipc: UnixStream,
    outbound: Outbound,
    max_frame_bytes: usize,
    shutdown_deadline: Duration,
    cancel_grace: Duration,
    shutdown_sent: AtomicBool,
    state: Mutex<State>,
    changed: Condvar,
    inbound: Mutex<Inbound>,
    invocations: Invocations,
    ingress: Ingress,
    stderr: Arc<Mutex<StderrTail>>,
    #[cfg(test)]
    order: Option<Arc<OrderSeam>>,
    #[cfg(test)]
    seams: Option<Arc<TestSeams>>,
}

struct State {
    startup: Startup,
    exit: Option<PluginProcessExited>,
    notifier: Option<PluginExitNotifier>,
}

enum Startup {
    AwaitReady,
    AwaitLoaded,
    Loaded(Option<PluginRegistration>),
    BootstrapFailed(String),
    LoadFailed(String),
}

impl std::fmt::Debug for PluginProcess {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PluginProcess")
            .field("pid", &self.shared.pid)
            .finish_non_exhaustive()
    }
}

impl PluginProcess {
    /// Start a worker, bootstrap it, and load the plugin.
    ///
    /// Returns the handle and the worker's registration once the worker
    /// reports `Loaded`. The startup deadline bounds this call: on expiry,
    /// and on every other failure after the process started, the group is
    /// killed and reaped before the error returns.
    pub fn spawn(
        config: &PluginProcessConfig,
        load: &LoadFrame,
    ) -> Result<(Self, PluginRegistration), PluginProcessError> {
        if config.max_frame_bytes == 0 || config.max_frame_bytes > MAX_FRAME_LEN {
            // The codec cannot represent a larger frame; refuse rather than
            // silently advertise a bound that would not hold.
            return Err(PluginProcessError::InvalidConfig(format!(
                "max_frame_bytes must be between 1 and {MAX_FRAME_LEN}"
            )));
        }
        if config.max_in_flight_invokes == 0 {
            return Err(PluginProcessError::InvalidConfig(
                "max_in_flight_invokes must be positive".to_string(),
            ));
        }
        if config.reply_credits.bytes > config.max_frame_bytes {
            // A full-size reply could not be framed (plan section 5.1).
            return Err(PluginProcessError::InvalidConfig(
                "max_frame_bytes must be at least reply_credits.bytes".to_string(),
            ));
        }
        if CreditFrame::widest().iter().any(|credit| {
            encode_json_bounded(FRAME_CREDIT, credit, config.max_frame_bytes).is_err()
        }) {
            // Credit must always be returnable, whatever its values.
            return Err(PluginProcessError::InvalidConfig(
                "max_frame_bytes cannot carry a credit frame".to_string(),
            ));
        }
        // Encode before starting anything, so an oversize frame starts no process.
        let bootstrap = encode_json_bounded(
            FRAME_BOOTSTRAP,
            &BootstrapFrame {
                magic: PROTOCOL_MAGIC.to_string(),
                version: PROTOCOL_VERSION,
                sandbox: config.sandbox.clone(),
                memory_cap_bytes: config.memory_cap_bytes,
            },
            config.max_frame_bytes,
        )
        .map_err(PluginProcessError::Encode)?;
        let load = encode_json_bounded(
            FRAME_LOAD,
            &LoadEnvelope {
                load,
                grants: grants(config),
            },
            config.max_frame_bytes,
        )
        .map_err(PluginProcessError::Encode)?;

        let shared = start(config)?;
        let startup = shared.supervisor.arm(
            Instant::now() + config.startup_deadline,
            PluginKillReason::StartupDeadline,
        );
        shared.send(Lane::Startup, bootstrap);
        let phase = shared.wait_startup(|startup| !matches!(startup, Startup::AwaitReady));
        let phase = match phase {
            Phase::Advanced => {
                shared.send(Lane::Startup, load);
                shared.wait_startup(|startup| !matches!(startup, Startup::AwaitLoaded))
            }
            other => other,
        };
        match phase {
            Phase::Advanced => {}
            Phase::Exited(exit) => return Err(PluginProcessError::Exited(exit)),
        }
        let mut state = shared.lock_state();
        let result = match std::mem::replace(&mut state.startup, Startup::Loaded(None)) {
            Startup::Loaded(Some(registration)) => Ok(registration),
            Startup::BootstrapFailed(reason) => Err(Failure::Bootstrap(reason)),
            Startup::LoadFailed(reason) => Err(Failure::Load(reason)),
            Startup::AwaitReady | Startup::AwaitLoaded | Startup::Loaded(None) => {
                unreachable!("wait_startup returns only after the startup phase advanced")
            }
        };
        drop(state);
        match result {
            Ok(registration) => {
                shared.supervisor.disarm(startup);
                Ok((Self { shared }, registration))
            }
            Err(failure) => {
                shared.killer.kill(PluginKillReason::StartupFailed);
                let exit = shared.wait_exit();
                Err(match failure {
                    Failure::Bootstrap(reason) => {
                        PluginProcessError::BootstrapFailed { reason, exit }
                    }
                    Failure::Load(reason) => PluginProcessError::LoadFailed { reason, exit },
                })
            }
        }
    }

    /// Process id, which is also the process group id.
    #[must_use]
    pub fn pid(&self) -> u32 {
        self.shared.pid
    }

    /// Ask the worker to exit, bounded by the shutdown deadline. Later calls
    /// do nothing.
    pub fn stop(&self) {
        self.shared.stop();
    }

    /// Kill the process group now.
    pub fn kill(&self) {
        self.shared.killer.kill(PluginKillReason::Requested);
    }

    /// The reaped exit, once the exit watch has recorded it.
    #[must_use]
    pub fn exit(&self) -> Option<PluginProcessExited> {
        self.shared.lock_state().exit.clone()
    }

    /// Attach the delivery pool of the plugin's engine generation (plan
    /// section 5.1) and grant its free units to the child. Until this runs,
    /// the child has no delivery credit and refuses its host calls locally.
    /// Each returned unit is sent back to the child as credit.
    ///
    /// # Errors
    ///
    /// `InvalidConfig` when a pool is already attached.
    pub fn attach_delivery(&self, pool: DeliveryPool) -> Result<(), PluginProcessError> {
        let (slots, request_bytes) = self
            .shared
            .ingress
            .attach(pool.clone())
            .map_err(PluginProcessError::InvalidConfig)?;
        let shared = Arc::downgrade(&self.shared);
        pool.install_unit_returned(Arc::new(move |call: CallId| {
            if let Some(shared) = shared.upgrade() {
                shared
                    .outbound
                    .push_credit(CreditFrame::Delivery { call_id: call.0 });
            }
        }));
        self.shared.outbound.push_credit(CreditFrame::DeliveryPool {
            slots,
            request_bytes,
        });
        Ok(())
    }

    /// Take at most `max_items` host calls and log lines, in the order the
    /// child sent them, whose encoded frames sum to at most `max_bytes`. The
    /// drained ingress and log credit returns to the child. A drained reply
    /// keeps its credit until [`Self::release_reply`].
    pub fn drain_ingress(&self, max_items: usize, max_bytes: usize) -> Vec<PluginIngress> {
        let (items, returned) = self.shared.ingress.drain(max_items, max_bytes);
        if returned.ingress_bytes > 0 {
            self.shared.outbound.push_credit(CreditFrame::IngressBytes {
                bytes: returned.ingress_bytes,
            });
        }
        if returned.log_count > 0 {
            self.shared.outbound.push_credit(CreditFrame::Log {
                count: returned.log_count,
                bytes: returned.log_bytes,
            });
        }
        items
    }

    /// The terminal of a reply: return its credit to the child. Returns true
    /// once per reply; false for an id that is not an open reply.
    pub fn release_reply(&self, call_id: CallId) -> bool {
        if !self.shared.ingress.release_reply(call_id) {
            return false;
        }
        self.shared
            .outbound
            .push_credit(CreditFrame::Reply { call_id: call_id.0 });
        true
    }

    /// Run `notifier` after each host call or log line enters the ingress
    /// queue. It runs outside every lock and must return promptly.
    pub fn install_ingress_notifier(&self, notifier: PluginIngressNotifier) {
        self.shared.ingress.install_notifier(notifier);
    }

    /// Run `notifier` once after the process is reaped (at once when it
    /// already is). It runs outside every lock and must return promptly.
    pub fn install_exit_notifier(&self, notifier: PluginExitNotifier) {
        let mut state = self.shared.lock_state();
        if state.exit.is_some() {
            drop(state);
            notifier();
        } else {
            state.notifier = Some(notifier);
        }
    }
}

impl Drop for PluginProcess {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

/// The process host is a [`PluginRuntime`]: `PluginWorkerEngine` keeps its
/// admission, deadlines, and completions, and each executor's invocation is
/// forwarded to the child.
impl PluginRuntime for PluginProcess {
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: PluginCancellationToken,
    ) -> PluginInvocationResult {
        self.shared.invoke(request, &cancellation)
    }

    fn stop(&self, _plugin_key: &PluginKey) {
        self.shared.stop();
    }
}

/// The failure an exit gives to every invocation still in flight.
fn exit_failure(exit: &PluginProcessExited) -> (PluginInvocationFailureKind, String) {
    match &exit.cause {
        PluginExitCause::Stopped => (
            PluginInvocationFailureKind::WorkerStopped,
            "the plugin process stopped".to_string(),
        ),
        PluginExitCause::Killed(reason) => (
            PluginInvocationFailureKind::WorkerKilled,
            format!("the plugin process was killed: {reason:?}"),
        ),
        PluginExitCause::MemoryCap => (
            PluginInvocationFailureKind::WorkerKilled,
            "the plugin process exceeded its memory cap".to_string(),
        ),
        PluginExitCause::Panic => (
            PluginInvocationFailureKind::WorkerCrashed,
            format!("the plugin process panicked: {}", exit.fatal_message),
        ),
        PluginExitCause::Crashed { signal, code } => (
            PluginInvocationFailureKind::WorkerCrashed,
            format!("the plugin process crashed (signal {signal:?}, exit code {code:?})"),
        ),
    }
}

/// Send queued frames in order. A failed send kills the group: the process
/// cannot be driven any further. The exit closes the queue, which ends this
/// thread.
fn run_writer(shared: &Shared, ipc: &UnixStream) {
    while let Some(next) = shared.outbound.next() {
        let queued = match next {
            Next::Frame(queued) => queued,
            Next::Credit(credit) => {
                // From here the child may receive and spend this credit, so
                // the account restores it before the write, not after: a
                // compliant child can spend it before `send_all` returns.
                shared.ingress.credit_taken(&credit);
                // spawn checked that every credit fits the frame bound.
                let sent = encode_json_bounded(FRAME_CREDIT, &credit, shared.max_frame_bytes)
                    .map_err(|error| io::Error::other(error.to_string()))
                    .and_then(|frame| send_all(ipc.as_fd(), &frame));
                if sent.is_err() {
                    shared.killer.kill(PluginKillReason::TransportClosed);
                    shared.outbound.close();
                    return;
                }
                continue;
            }
        };
        let sent = send_all(ipc.as_fd(), &queued.frame);
        #[cfg(test)]
        if let Some(seams) = &shared.seams {
            seams.after_send(queued.lane);
        }
        shared.outbound.written(queued.lane, queued.frame.len());
        if let Some(owner) = &queued.owner {
            let sink = |lane, frame, owner| shared.send_owned(lane, frame, owner);
            shared.invocations.frame_written(owner, &sink);
        }
        if sent.is_err() {
            shared.killer.kill(PluginKillReason::TransportClosed);
            shared.outbound.close();
            return;
        }
    }
}

/// The child's initial credits, from the Hub's configuration.
fn grants(config: &PluginProcessConfig) -> CreditGrants {
    CreditGrants {
        ingress_bytes: config.ingress_bytes,
        reply_count: config.reply_credits.count,
        reply_bytes: config.reply_credits.bytes,
        log_count: config.log_credits.count,
        log_bytes: config.log_credits.bytes,
        max_in_flight_invokes: config.max_in_flight_invokes,
    }
}

enum Failure {
    Bootstrap(String),
    Load(String),
}

enum Phase {
    Advanced,
    Exited(PluginProcessExited),
}

impl Shared {
    fn lock_state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Lock order: `inbound` before `state`.
    fn lock_inbound(&self) -> MutexGuard<'_, Inbound> {
        self.inbound.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Wait for the startup phase to advance or for the exit. There is no
    /// timer here: the supervisor's startup deadline kills the group, and the
    /// exit watch then records the exit.
    fn wait_startup(&self, advanced: impl Fn(&Startup) -> bool) -> Phase {
        let mut state = self.lock_state();
        loop {
            if advanced(&state.startup) {
                return Phase::Advanced;
            }
            if let Some(exit) = &state.exit {
                return Phase::Exited(exit.clone());
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Wait for the exit watch to record the reaped exit. Callers kill the
    /// group first, so the exit follows as an event.
    fn wait_exit(&self) -> PluginProcessExited {
        let mut state = self.lock_state();
        loop {
            if let Some(exit) = &state.exit {
                return exit.clone();
            }
            state = self
                .changed
                .wait(state)
                .unwrap_or_else(PoisonError::into_inner);
        }
    }

    /// Queue a startup or shutdown frame for the writer. Never blocks. A
    /// closed queue means the process is gone or going; the exit reports
    /// the rest. These lanes hold one frame each, so they cannot be full.
    fn send(&self, lane: Lane, frame: Vec<u8>) {
        let _ = self.outbound.push(lane, frame, None);
    }

    /// Queue an invocation's frame. The table admits at most
    /// `max_in_flight_invokes` records, each with at most one `Invoke` and
    /// one `Cancel` frame, so these lanes cannot be full either.
    fn send_owned(&self, lane: Lane, frame: Vec<u8>, owner: RequestId) -> bool {
        match self.outbound.push(lane, frame, Some(owner)) {
            Ok(()) => true,
            Err(Refused::Closed) => false,
            Err(Refused::Full) => {
                debug_assert!(false, "the {lane:?} lane exceeded its derived bound");
                false
            }
        }
    }

    fn stop(&self) {
        if self.shutdown_sent.swap(true, Ordering::SeqCst) {
            return;
        }
        // Fail every caller at once, so the engine's executor join returns
        // promptly; late results of admitted invocations are dropped.
        self.invocations.stop();
        if self.lock_state().exit.is_some() {
            return;
        }
        self.supervisor.arm(
            Instant::now() + self.shutdown_deadline,
            PluginKillReason::ShutdownDeadline,
        );
        match encode_bounded(FRAME_SHUTDOWN, &[], self.max_frame_bytes) {
            Ok(frame) => self.send(Lane::Shutdown, frame),
            Err(_) => self.killer.kill(PluginKillReason::ShutdownDeadline),
        }
    }

    /// Forward one invocation and wait for its outcome as an event: the
    /// child's result, the exit, or `stop`. Admission waits for room, also as
    /// an event (a retired record), and ends early on cancel. A cancel of an
    /// admitted invocation queues `Cancel` and arms the cancel grace, owned
    /// by the invocation: the result path disarms it, and its expiry kills
    /// only an invocation that is still unsettled.
    fn invoke(
        &self,
        request: PluginInvocationRequest,
        cancellation: &PluginCancellationToken,
    ) -> PluginInvocationResult {
        let failed = |kind, reason: String| {
            PluginInvocationResult::Failed(PluginInvocationFailure {
                request_id: request.request_id.clone(),
                handler: request.handler.clone(),
                kind,
                timeout_ms: None,
                reason,
            })
        };
        let frame = match encode_json_bounded(FRAME_INVOKE, &request, self.max_frame_bytes) {
            Ok(frame) => frame,
            Err(error) => {
                return failed(
                    PluginInvocationFailureKind::HandlerFailed,
                    format!("the invocation does not fit the frame bound: {error}"),
                )
            }
        };
        let invocation = Invocation::new(&request, frame);
        let _subscription = cancellation.subscribe(invocation.clone());
        let sink = |lane, frame, owner| self.send_owned(lane, frame, owner);
        match self.invocations.admit(&invocation, &sink) {
            Admission::Admitted => {}
            Admission::Waiting =>
            {
                #[cfg(test)]
                if let Some(seams) = &self.seams {
                    seams.admission_waiting();
                }
            }
            Admission::Closed => return self.closed_failure(&failed),
            Admission::Full => {
                return failed(
                    PluginInvocationFailureKind::Backpressured,
                    "more callers than the plugin's invocation width are waiting".to_string(),
                )
            }
        }

        let mut cancel_handled = false;
        let mut state = invocation.lock();
        let outcome = loop {
            if let Some(outcome) = state.outcome.take() {
                break outcome;
            }
            if state.cancelled && !cancel_handled {
                cancel_handled = true;
                let admitted = state.admitted;
                drop(state);
                if !admitted && self.invocations.withdraw(&invocation, &sink) {
                    return failed(
                        PluginInvocationFailureKind::Cancelled,
                        "cancelled before it reached the plugin process".to_string(),
                    );
                }
                self.cancel_admitted(&invocation);
                state = invocation.lock();
                continue;
            }
            state = invocation.wait(state);
        };
        drop(state);
        #[cfg(test)]
        hold_before_consume();
        match outcome {
            Outcome::Result(result) => result,
            Outcome::Failed(kind, reason) => failed(kind, reason),
        }
    }

    /// Queue `Cancel` and arm the grace for an admitted, unsettled
    /// invocation. The grace is armed under the invocation's lock after
    /// checking that no outcome settled, so a result that arrived first
    /// leaves no deadline behind.
    fn cancel_admitted(&self, invocation: &Arc<Invocation>) {
        let cancel = CancelFrame {
            request_id: invocation.request_id().clone(),
        };
        let Ok(frame) = encode_json_bounded(FRAME_CANCEL, &cancel, self.max_frame_bytes) else {
            return;
        };
        let sink = |lane, frame, owner| self.send_owned(lane, frame, owner);
        if !self.invocations.cancel(invocation, frame, &sink) {
            return;
        }
        #[cfg(test)]
        hold_before_arm();
        let mut state = invocation.lock();
        if state.outcome.is_none() && !state.abandoned && state.grace.is_none() {
            state.grace = Some(self.supervisor.arm_expiry(
                Instant::now() + self.cancel_grace,
                Expiry::Guarded(Arc::new(GraceExpiry {
                    invocation: invocation.clone(),
                })),
            ));
        }
    }

    fn closed_failure(
        &self,
        failed: &dyn Fn(PluginInvocationFailureKind, String) -> PluginInvocationResult,
    ) -> PluginInvocationResult {
        let (kind, reason) = match self.lock_state().exit.clone() {
            Some(exit) => exit_failure(&exit),
            None => (
                PluginInvocationFailureKind::WorkerStopped,
                "the plugin process is stopping".to_string(),
            ),
        };
        failed(kind, reason)
    }

    /// Settle every caller with the exit's failure and admit nothing more.
    /// Runs once, from the exit watch.
    fn fail_invocations(&self, exit: &PluginProcessExited) {
        let (kind, reason) = exit_failure(exit);
        for grace in self.invocations.exit(&kind, &reason) {
            self.supervisor.disarm(grace);
        }
    }
}

/// Start the process and its threads. On any failure after the process
/// started, it is killed and reaped before this returns.
fn start(config: &PluginProcessConfig) -> Result<Arc<Shared>, PluginProcessError> {
    let Launched {
        child,
        ipc,
        fatal,
        stderr,
    } = launch(config).map_err(PluginProcessError::Launch)?;
    let pid = child.id();
    let Ok(pgid) = libc::pid_t::try_from(pid) else {
        return Err(rollback(child, None, io::Error::other("pid out of range")));
    };
    let abort = |child: Child, error: io::Error| rollback(child, Some(pgid), error);
    // Register while the child is unreaped, so its pid cannot be reused first.
    let registered = match start_fault(FaultPoint::RegisterWatch, pid) {
        Some(error) => Err(error),
        None => ExitWatch::register(pid),
    };
    let watch = match registered {
        Ok(watch) => watch,
        Err(error) => return Err(abort(child, error)),
    };
    let killer = Arc::new(ProcessKiller::new(pgid));
    let supervisor = match Supervisor::start(killer.clone(), format!("plugin-supervisor-{pid}")) {
        Ok(supervisor) => supervisor,
        Err(error) => return Err(abort(child, error)),
    };
    let (reader_ipc, writer_ipc) = match ipc.try_clone().and_then(|reader| {
        let writer = ipc.try_clone()?;
        Ok((reader, writer))
    }) {
        Ok(streams) => streams,
        Err(error) => {
            supervisor.stop();
            return Err(abort(child, error));
        }
    };
    let stderr_fd = stderr.as_raw_fd();
    let tail = match StderrTail::new(stderr, config.stderr_tail_bytes) {
        Ok(tail) => Arc::new(Mutex::new(tail)),
        Err(error) => {
            supervisor.stop();
            return Err(abort(child, error));
        }
    };
    let shared = Arc::new(Shared {
        pid,
        killer,
        supervisor,
        ipc,
        outbound: Outbound::new(LaneBounds::derived(config.max_in_flight_invokes)),
        max_frame_bytes: config.max_frame_bytes,
        shutdown_deadline: config.shutdown_deadline,
        cancel_grace: config.cancel_grace,
        shutdown_sent: AtomicBool::new(false),
        state: Mutex::new(State {
            startup: Startup::AwaitReady,
            exit: None,
            notifier: None,
        }),
        changed: Condvar::new(),
        inbound: Mutex::new(Inbound {
            ipc: reader_ipc,
            decoder: FrameDecoder::with_max_len(config.max_frame_bytes),
            done: false,
            ingress_arrived: false,
        }),
        invocations: Invocations::new(config.max_in_flight_invokes),
        ingress: Ingress::new(grants(config)),
        stderr: tail.clone(),
        #[cfg(test)]
        order: ORDER_SEAM.with(|slot| slot.borrow_mut().take()),
        #[cfg(test)]
        seams: TEST_SEAMS.with(|slot| slot.borrow_mut().take()),
    });

    // The child stays here until the exit watch is running and has taken it,
    // so a failed thread start still leaves an owner that can reap it.
    let (handoff, taken) = mpsc::sync_channel::<(ExitWatch, Child, OwnedFd)>(1);
    let watch_shared = shared.clone();
    let spawned = match start_fault(FaultPoint::SpawnExitWatch, pid) {
        Some(error) => Err(error),
        None => thread::Builder::new()
            .name(format!("plugin-exit-watch-{pid}"))
            .spawn(move || {
                if let Ok((watch, child, fatal)) = taken.recv() {
                    run_exit_watch(&watch_shared, watch, child, &fatal);
                }
            }),
    };
    if let Err(error) = spawned {
        shared.supervisor.stop();
        return Err(abort(child, error));
    }
    if let Err(mpsc::SendError((_, child, _))) = handoff.send((watch, child, fatal)) {
        // The exit watch ended before it took the child; reap it here.
        shared.supervisor.stop();
        return Err(abort(child, io::Error::other("the exit watch ended early")));
    }
    // From here the exit watch owns the child; failures kill and wait for it.
    let reader_shared = shared.clone();
    let writer_shared = shared.clone();
    let started = thread::Builder::new()
        .name(format!("plugin-reader-{pid}"))
        .spawn(move || run_reader(&reader_shared))
        .and_then(|_| {
            thread::Builder::new()
                .name(format!("plugin-writer-{pid}"))
                .spawn(move || run_writer(&writer_shared, &writer_ipc))
        })
        .and_then(|_| {
            thread::Builder::new()
                .name(format!("plugin-stderr-{pid}"))
                .spawn(move || run_stderr(stderr_fd, &tail))
        });
    if let Err(error) = started {
        shared.killer.kill(PluginKillReason::Requested);
        shared.wait_exit();
        return Err(PluginProcessError::Launch(error));
    }
    Ok(shared)
}

/// Roll back a start that failed before the exit watch owned the child: kill
/// the whole group while the leader is unreaped, then reap the leader. The
/// blocking wait is the exit event itself; SIGKILL guarantees it.
fn rollback(mut child: Child, pgid: Option<libc::pid_t>, error: io::Error) -> PluginProcessError {
    match pgid {
        // SAFETY: killpg only sends a signal; the leader is unreaped, so its
        // group id is still ours.
        Some(pgid) => unsafe {
            libc::killpg(pgid, libc::SIGKILL);
        },
        None => {
            let _ = child.kill();
        }
    }
    let _ = child.wait();
    PluginProcessError::Launch(error)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum FaultPoint {
    RegisterWatch,
    SpawnExitWatch,
}

/// Test seam: a fault armed on the spawning thread fails the start at
/// `point`, after running its hook with the child's pid.
#[cfg(test)]
pub(super) struct StartFault {
    pub point: FaultPoint,
    pub before: Box<dyn FnOnce(u32)>,
}

#[cfg(test)]
thread_local! {
    pub(super) static START_FAULT: std::cell::RefCell<Option<StartFault>> =
        const { std::cell::RefCell::new(None) };
}

/// Test seam: one side reports that it reached a point, then waits until
/// the test releases it.
#[cfg(test)]
pub(super) struct Gate {
    pub reached: mpsc::Sender<()>,
    pub release: mpsc::Receiver<()>,
}

#[cfg(test)]
impl Gate {
    fn hold(self) {
        let _ = self.reached.send(());
        let _ = self.release.recv();
    }
}

/// Test seams on the host's own threads.
#[cfg(test)]
#[derive(Default)]
pub(super) struct TestSeams {
    /// Hold the writer once, after it sent a frame of this lane and before
    /// it retires the frame.
    pub writer_hold: Mutex<Option<(Lane, Gate)>>,
    /// Reports each settled child result.
    pub settled: Mutex<Option<mpsc::Sender<()>>>,
    /// Reports each invocation that waits for room.
    pub waiting: Mutex<Option<mpsc::Sender<()>>>,
    /// Hold the reader once, after the socket became readable and before it
    /// takes the inbound lock, so the child's writes back up.
    pub reader_hold: Mutex<Option<Gate>>,
}

#[cfg(test)]
impl TestSeams {
    fn after_send(&self, lane: Lane) {
        let gate = {
            let mut hold = self.writer_hold.lock().expect("writer hold");
            match hold.take() {
                Some((held, gate)) if held == lane => Some(gate),
                other => {
                    *hold = other;
                    None
                }
            }
        };
        if let Some(gate) = gate {
            gate.hold();
        }
    }

    fn admission_waiting(&self) {
        if let Some(waiting) = &*self.waiting.lock().expect("waiting") {
            let _ = waiting.send(());
        }
    }

    fn result_settled(&self) {
        if let Some(settled) = &*self.settled.lock().expect("settled") {
            let _ = settled.send(());
        }
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static TEST_SEAMS: std::cell::RefCell<Option<Arc<TestSeams>>> =
        const { std::cell::RefCell::new(None) };
    /// Holds the invoking thread after its outcome settled, before it
    /// consumes it.
    pub(super) static HOLD_BEFORE_CONSUME: std::cell::RefCell<Option<Gate>> =
        const { std::cell::RefCell::new(None) };
    /// Holds the invoking thread after it queued `Cancel`, before it arms
    /// the grace.
    pub(super) static HOLD_BEFORE_ARM: std::cell::RefCell<Option<Gate>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn hold_before_consume() {
    if let Some(gate) = HOLD_BEFORE_CONSUME.with(|slot| slot.borrow_mut().take()) {
        gate.hold();
    }
}

#[cfg(test)]
fn hold_before_arm() {
    if let Some(gate) = HOLD_BEFORE_ARM.with(|slot| slot.borrow_mut().take()) {
        gate.hold();
    }
}

/// Test seam: forces one order between the reader's EOF handling and the
/// exit watch's reap (plan section 7.5 requires both orders).
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Order {
    /// The reader handles EOF (and its kill) before the exit watch reaps.
    EofFirst,
    /// The exit is published before the reader handles EOF.
    ExitFirst,
}

#[cfg(test)]
#[derive(Default)]
pub(super) struct Stage {
    pub eof_handled: bool,
    pub exit_published: bool,
}

#[cfg(test)]
pub(super) struct OrderSeam {
    pub mode: Order,
    stage: Mutex<Stage>,
    changed: Condvar,
}

#[cfg(test)]
impl OrderSeam {
    pub(super) fn new(mode: Order) -> Arc<Self> {
        Arc::new(Self {
            mode,
            stage: Mutex::new(Stage::default()),
            changed: Condvar::new(),
        })
    }

    fn mark(&self, update: impl FnOnce(&mut Stage)) {
        update(&mut self.stage.lock().expect("order seam"));
        self.changed.notify_all();
    }

    fn wait_until(&self, done: impl Fn(&Stage) -> bool) {
        let mut stage = self.stage.lock().expect("order seam");
        while !done(&stage) {
            stage = self.changed.wait(stage).expect("order seam");
        }
    }

    fn before_eof(&self, wait_for_exit: bool) {
        if wait_for_exit {
            self.wait_until(|stage| stage.exit_published);
        }
    }
}

#[cfg(test)]
thread_local! {
    pub(super) static ORDER_SEAM: std::cell::RefCell<Option<Arc<OrderSeam>>> =
        const { std::cell::RefCell::new(None) };
}

#[cfg(test)]
fn start_fault(point: FaultPoint, pid: u32) -> Option<io::Error> {
    let fault = START_FAULT.with(|slot| {
        let mut slot = slot.borrow_mut();
        if slot.as_ref().is_some_and(|fault| fault.point == point) {
            slot.take()
        } else {
            None
        }
    })?;
    (fault.before)(pid);
    Some(io::Error::other(format!(
        "injected start fault at {point:?}"
    )))
}

#[cfg(not(test))]
fn start_fault(_point: FaultPoint, _pid: u32) -> Option<io::Error> {
    None
}

/// The parent's receiving end. Frames are consumed only under this lock: the
/// reader thread takes it when the socket becomes readable, and the exit
/// watch takes it after the reap to handle every frame the child sent before
/// it died. So a typed failure frame is never lost to the exit race.
struct Inbound {
    ipc: UnixStream,
    decoder: FrameDecoder,
    done: bool,
    /// A host call or log line was queued since the last take; the caller
    /// runs the ingress notifier after it releases this lock.
    ingress_arrived: bool,
}

enum InboundEnd {
    Closed,
    Violation(String),
}

impl Inbound {
    fn take_ingress_arrived(&mut self) -> bool {
        std::mem::take(&mut self.ingress_arrived)
    }

    /// Handle every frame that is readable now, without blocking. Returns
    /// `Some` once the channel is finished.
    fn drain(&mut self, shared: &Shared) -> Option<InboundEnd> {
        let mut buf = vec![0u8; 64 * 1024];
        while !self.done {
            // SAFETY: recv into a live buffer of its stated length; the
            // socket is owned by `self`. MSG_DONTWAIT keeps the shared
            // descriptor blocking for the writer.
            let read = unsafe {
                libc::recv(
                    self.ipc.as_raw_fd(),
                    buf.as_mut_ptr().cast(),
                    buf.len(),
                    libc::MSG_DONTWAIT,
                )
            };
            let read = match read {
                0 => {
                    self.done = true;
                    return Some(InboundEnd::Closed);
                }
                read if read > 0 => usize::try_from(read).unwrap_or(0),
                _ => {
                    let error = io::Error::last_os_error();
                    match error.kind() {
                        io::ErrorKind::Interrupted => continue,
                        io::ErrorKind::WouldBlock => return None,
                        _ => {
                            self.done = true;
                            return Some(InboundEnd::Closed);
                        }
                    }
                }
            };
            let frames = match self.decoder.feed(&buf[..read]) {
                Ok(frames) => frames,
                Err(error) => {
                    self.done = true;
                    return Some(InboundEnd::Violation(error.to_string()));
                }
            };
            for frame in frames {
                match handle_frame(shared, &frame) {
                    Ok(arrived) => self.ingress_arrived |= arrived,
                    Err(violation) => {
                        self.done = true;
                        return Some(InboundEnd::Violation(violation));
                    }
                }
            }
        }
        Some(InboundEnd::Closed)
    }
}

fn run_reader(shared: &Shared) {
    let fd = shared.lock_inbound().ipc.as_raw_fd();
    loop {
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll on one valid pollfd; the socket lives in `shared`,
        // which this thread keeps alive. No timeout: readability is the event.
        if unsafe { libc::poll(&mut poll_fd, 1, -1) } < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            break;
        }
        #[cfg(test)]
        if let Some(seams) = &shared.seams {
            let gate = seams.reader_hold.lock().expect("reader hold").take();
            if let Some(gate) = gate {
                gate.hold();
            }
        }
        let mut inbound = shared.lock_inbound();
        if inbound.done {
            // The exit watch already finished the channel.
            return;
        }
        let end = inbound.drain(shared);
        let arrived = inbound.take_ingress_arrived();
        drop(inbound);
        if arrived {
            shared.ingress.notify();
        }
        match end {
            None => {}
            Some(InboundEnd::Closed) => break,
            Some(InboundEnd::Violation(violation)) => {
                shared
                    .killer
                    .kill(PluginKillReason::ProtocolViolation(violation));
                return;
            }
        }
    }
    #[cfg(test)]
    if let Some(order) = &shared.order {
        order.before_eof(order.mode == Order::ExitFirst);
    }
    // The channel is gone. That is not an exit: kill, and let the exit watch
    // record how the process ended.
    shared.killer.kill(PluginKillReason::TransportClosed);
    #[cfg(test)]
    if let Some(order) = &shared.order {
        order.mark(|stage| stage.eof_handled = true);
    }
}

/// Handle one child frame. Returns whether it queued ingress for the Hub;
/// `Err` is a protocol violation.
fn handle_frame(shared: &Shared, frame: &Frame) -> Result<bool, String> {
    let mut state = shared.lock_state();
    if matches!(state.startup, Startup::Loaded(_)) {
        let handler = match frame.frame_type {
            FRAME_INVOCATION_RESULT => Some(handle_result as fn(&Shared, &Frame) -> _),
            FRAME_HOST_CALL => Some(handle_host_call as fn(&Shared, &Frame) -> _),
            FRAME_LOG => Some(handle_log as fn(&Shared, &Frame) -> _),
            _ => None,
        };
        if let Some(handler) = handler {
            drop(state);
            return handler(shared, frame);
        }
    }
    let next = match (&state.startup, frame.frame_type) {
        (Startup::AwaitReady, FRAME_READY) => {
            let ready: ReadyFrame = decode_json(frame).map_err(|error| error.to_string())?;
            if ready.magic != PROTOCOL_MAGIC || ready.version != PROTOCOL_VERSION {
                return Err(format!(
                    "Ready carries protocol {} v{}",
                    ready.magic, ready.version
                ));
            }
            if ready.pid != shared.pid {
                return Err(format!("Ready carries pid {}", ready.pid));
            }
            Startup::AwaitLoaded
        }
        (Startup::AwaitReady, FRAME_BOOTSTRAP_FAILED) => {
            let failed: FailedFrame = decode_json(frame).map_err(|error| error.to_string())?;
            Startup::BootstrapFailed(failed.reason)
        }
        (Startup::AwaitLoaded, FRAME_LOADED) => {
            let loaded: LoadedFrame = decode_json(frame).map_err(|error| error.to_string())?;
            Startup::Loaded(Some(loaded.registration))
        }
        (Startup::AwaitLoaded, FRAME_LOAD_FAILED) => {
            let failed: FailedFrame = decode_json(frame).map_err(|error| error.to_string())?;
            Startup::LoadFailed(failed.reason)
        }
        (_, frame_type) => {
            return Err(format!(
                "frame type {frame_type:#04x} is not valid in this phase"
            ))
        }
    };
    state.startup = next;
    drop(state);
    shared.changed.notify_all();
    Ok(false)
}

/// The encoded length of a received frame (type byte plus payload): what the
/// child charged to its credits.
fn frame_bytes(frame: &Frame) -> usize {
    frame.payload.len() + 1
}

/// Queue one host call for the Hub. The call must name an invocation that is
/// in flight, and it must fit the child's credits.
fn handle_host_call(shared: &Shared, frame: &Frame) -> Result<bool, String> {
    let call: HostCallFrame = decode_json(frame).map_err(|error| error.to_string())?;
    if !shared.invocations.is_live(&call.invocation_request_id) {
        return Err(format!(
            "host call {} names request {:?}, which is not in flight",
            call.call_id, call.invocation_request_id
        ));
    }
    match shared.ingress.host_call(call, frame_bytes(frame))? {
        CallAdmission::Queued => Ok(true),
        CallAdmission::Dropped => Ok(false),
    }
}

/// Queue one log line for the Hub. It must fit the child's log credit.
fn handle_log(shared: &Shared, frame: &Frame) -> Result<bool, String> {
    let log: LogFrame = decode_json(frame).map_err(|error| error.to_string())?;
    shared.ingress.log(log, frame_bytes(frame))?;
    Ok(true)
}

/// Settle the caller of one result. Only in-flight ids of this process are
/// accepted, so stale or foreign ids need no history: an id that is not in
/// flight is a protocol violation. A late result for an invocation that
/// `stop` already failed is dropped.
fn handle_result(shared: &Shared, frame: &Frame) -> Result<bool, String> {
    let result: PluginInvocationResult = decode_json(frame).map_err(|error| error.to_string())?;
    let sink = |lane, frame, owner| shared.send_owned(lane, frame, owner);
    if let Some(grace) = shared.invocations.settle_result(result, &sink)? {
        shared.supervisor.disarm(grace);
    }
    #[cfg(test)]
    if let Some(seams) = &shared.seams {
        seams.result_settled();
    }
    Ok(false)
}

fn run_exit_watch(shared: &Shared, watch: ExitWatch, mut child: Child, fatal: &OwnedFd) {
    // The exit is an OS event; a failed wait falls back to the blocking reap.
    let _ = watch.wait(None);
    #[cfg(test)]
    if let Some(order) = &shared.order {
        if order.mode == Order::EofFirst {
            order.wait_until(|stage| stage.eof_handled);
        }
    }
    let shutdown_sent = shared.shutdown_sent.load(Ordering::SeqCst);
    let (cause, fatal_message) = shared.killer.reap_with(|kills| {
        let (fatal_byte, fatal_message) = read_fatal(fatal);
        let status = match child.try_wait() {
            Ok(Some(status)) => Some(status),
            _ => child.wait().ok(),
        };
        (
            classify(kills, fatal_byte, status, shutdown_sent),
            fatal_message,
        )
    });
    let (stderr_tail, stderr_dropped_bytes) = {
        let mut tail = shared.stderr.lock().unwrap_or_else(PoisonError::into_inner);
        tail.drain();
        tail.snapshot()
    };
    let exit = PluginProcessExited {
        pid: shared.pid,
        cause,
        fatal_message,
        stderr_tail,
        stderr_dropped_bytes,
    };
    shared.supervisor.stop();
    // Handle every frame the child sent before it died (for example a
    // LoadFailed report), before the exit becomes visible. Violations no
    // longer matter: the process is gone.
    let arrived = {
        let mut inbound = shared.lock_inbound();
        let _ = inbound.drain(shared);
        inbound.take_ingress_arrived()
    };
    if arrived {
        shared.ingress.notify();
    }
    shared.outbound.close();
    let _ = shared.ipc.shutdown(Shutdown::Both);
    let notifier = {
        let mut state = shared.lock_state();
        state.exit = Some(exit.clone());
        state.notifier.take()
    };
    shared.changed.notify_all();
    shared.fail_invocations(&exit);
    #[cfg(test)]
    if let Some(order) = &shared.order {
        order.mark(|stage| stage.exit_published = true);
    }
    if let Some(notifier) = notifier {
        notifier();
    }
}

/// Read the fatal cause byte and its message without blocking. The child
/// writes them with one write of at most `1 + FATAL_MESSAGE_BYTES` bytes; the
/// parent reads no more than that.
fn read_fatal(fatal: &OwnedFd) -> (Option<u8>, String) {
    let mut buf = [0u8; 1 + FATAL_MESSAGE_BYTES];
    // SAFETY: a bounded read into a live local from a non-blocking
    // descriptor that this process owns.
    let read = unsafe { libc::read(fatal.as_raw_fd(), buf.as_mut_ptr().cast(), buf.len()) };
    let Ok(read) = usize::try_from(read) else {
        return (None, String::new());
    };
    match buf[..read].split_first() {
        Some((cause, message)) => (Some(*cause), String::from_utf8_lossy(message).into_owned()),
        None => (None, String::new()),
    }
}

/// Classify from exit evidence (plan section 7.5). A kill reason counts only
/// when the process died of SIGKILL and a parent kill was delivered.
fn classify(
    kills: &KillState,
    fatal_byte: Option<u8>,
    status: Option<ExitStatus>,
    shutdown_sent: bool,
) -> PluginExitCause {
    match fatal_byte {
        Some(CAUSE_MEMORY_CAP) => return PluginExitCause::MemoryCap,
        Some(CAUSE_PANIC) => return PluginExitCause::Panic,
        _ => {}
    }
    let signal = status.and_then(|status| status.signal());
    let code = status.and_then(|status| status.code());
    if signal == Some(libc::SIGKILL) {
        if let Some(reason) = &kills.first_delivered {
            return PluginExitCause::Killed(reason.clone());
        }
    }
    if code == Some(0) && shutdown_sent {
        return PluginExitCause::Stopped;
    }
    PluginExitCause::Crashed { signal, code }
}

/// Drain stderr as it becomes readable, so a chatty child never blocks on a
/// full pipe. Reads happen only under the tail lock; the exit watch drains
/// the rest under the same lock after the reap, so the tail holds every byte
/// written before the exit.
fn run_stderr(fd: libc::c_int, tail: &Mutex<StderrTail>) {
    loop {
        let mut poll_fd = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: poll on one valid pollfd; the descriptor lives in `tail`,
        // which this thread keeps alive. No timeout: readability is the event.
        let ready = unsafe { libc::poll(&mut poll_fd, 1, -1) };
        if ready < 0 {
            if io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            return;
        }
        let mut tail = tail.lock().unwrap_or_else(PoisonError::into_inner);
        if tail.drain() {
            return;
        }
    }
}

/// The child's stderr and its last `capacity` bytes, plus a count of what
/// fell off.
struct StderrTail {
    stderr: ChildStderr,
    eof: bool,
    capacity: usize,
    bytes: VecDeque<u8>,
    dropped: u64,
}

impl StderrTail {
    fn new(stderr: ChildStderr, capacity: usize) -> io::Result<Self> {
        let fd = stderr.as_raw_fd();
        // SAFETY: fcntl get/set on the stderr pipe that this process owns.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
        if flags < 0 || unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) } < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Self {
            stderr,
            eof: false,
            capacity,
            bytes: VecDeque::with_capacity(capacity.min(64 * 1024)),
            dropped: 0,
        })
    }

    /// Read everything the pipe holds now. Returns true at EOF or on error.
    fn drain(&mut self) -> bool {
        let mut buf = [0u8; 4096];
        while !self.eof {
            match self.stderr.read(&mut buf) {
                Ok(0) => self.eof = true,
                Ok(read) => self.push(&buf[..read]),
                Err(error) if error.kind() == io::ErrorKind::Interrupted => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return false,
                Err(_) => self.eof = true,
            }
        }
        true
    }

    fn push(&mut self, data: &[u8]) {
        self.bytes.extend(data);
        let excess = self.bytes.len().saturating_sub(self.capacity);
        self.bytes.drain(..excess);
        self.dropped += excess as u64;
    }

    fn snapshot(&self) -> (Vec<u8>, u64) {
        (self.bytes.iter().copied().collect(), self.dropped)
    }
}

#[cfg(test)]
#[path = "host_test.rs"]
mod host_test;
