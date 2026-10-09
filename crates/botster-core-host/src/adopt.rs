//! The adoption of a row whose worker may live (AD-1, AD-2, AD-6; DESIGN.md "Adoption (P5)", parts 3 to 5).
//!
//! The host probes the recorded worker identity, connects to the worker endpoint, speaks first with its own proof and epoch,
//! checks the worker's answer, and reads the worker's report. For `AdoptAll`, the row's recorded state (the intent) and the
//! report's payload state (the facts) decide the row's one `SessionState` (LC-11; steward ruling R-35, contracts `main`
//! `f969f5e`, with the correction `c3ed727`). For `Adopt(id)` of a `Lost` session, the report alone decides it, and nothing
//! is launched (steward ruling R-36, contracts `main` `c62085f`, with the follow-up `d18b6de`).

use crate::engine::{HostEngine, Owner};
use crate::flow::*;
use crate::io::{Action, LinkId};
use crate::session::Admit;
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{IdentityState, ProcessIdentity};
use botster_core_link::hello::Hello;
use botster_core_link::msg::{AdoptReport, AdoptedPayload};
use botster_core_link::proof::{host_proof, token_proof};

/// The exit of a payload that never ran: no code and no signal.
fn never_ran(cause: ExitCause) -> Exit {
    Exit {
        code: None,
        signal: None,
        cause,
    }
}

impl HostEngine {
    fn adopt_flow(&mut self, id: &SessionId) -> Option<&mut AdoptFlow> {
        match &mut self.sessions.get_mut(id)?.flow {
            Flow::Adopt(f) => Some(f),
            _ => None,
        }
    }

    fn set_adopt_phase(&mut self, id: &SessionId, phase: AdoptPhase) {
        if let Some(f) = self.adopt_flow(id) {
            f.phase = phase;
        }
    }

    /// Starts the adoption of `id` for `op`, with the row's recorded state `recorded` (`None` for `Adopt(id)`). No operation
    /// is admitted on the session until the adoption posts its state.
    pub(crate) fn begin_adoption(
        &mut self,
        op: OpId,
        id: &SessionId,
        recorded: Option<SessionState>,
    ) {
        let s = self
            .sessions
            .get_mut(id)
            .expect("an adoption has a session");
        if recorded.is_none() {
            // R-36: the `Stop` that ended in `Lost` completed (LC-5), so a retry keeps nothing that the host asked for. The
            // exit cause follows the report alone, as for a new handle that reads the same row.
            s.host_ended = false;
            s.killed = false;
        }
        // LC-9: the protocol is what this host reads in the worker's hello. Until it reads one, the session has none, also
        // when the row recorded one (a worker whose hello was never read is never guessed). The row keeps its own (R-36).
        if let Some(recorded) = s.worker_protocol.take() {
            s.row_protocol = Some(recorded);
        }
        // AD-6: each adoption proves the worker again, so a refusal of an earlier one is not kept.
        s.worker.gone = false;
        s.admit = Admit::Adopting;
        s.adopting = Some(op);
        s.flow = Flow::Adopt(AdoptFlow {
            op,
            phase: AdoptPhase::Probe,
            recorded,
            deadline: None,
            post: None,
        });
    }

    /// Whether a row that `op` adopts has not posted its state yet.
    pub(crate) fn adopting_rows(&self, op: OpId) -> bool {
        self.sessions.values().any(|s| s.adopting == Some(op))
    }

    pub(crate) fn run_adopt(&mut self, id: &SessionId, f: AdoptFlow) {
        match f.phase {
            AdoptPhase::Probe => {
                let Some(identity) = self.identity_of(id) else {
                    // `Adopt(id)` of a `Lost` session whose start never recorded its worker (a hello may end the start
                    // before the spawn answers): no worker can be found, so nothing is probed or connected (AD-1, AD-2;
                    // steward ruling R-36, follow-up `d18b6de`; review P5-F26).
                    self.adopt_end(id, End::Lost(LostReason::StartInterrupted), "");
                    return;
                };
                // One reachability deadline, `startup`, from the start of the adoption (DESIGN.md 3.7).
                let deadline = self.mono().map(|now| now + self.cfg.limits.startup);
                if let Some(f) = self.adopt_flow(id) {
                    f.deadline = deadline;
                    f.phase = AdoptPhase::AwaitProbe;
                }
                self.act(Action::ProbeIdentity { identity });
            }
            AdoptPhase::Connect => {
                let ticket = self.ticket(Owner::Session(id.clone()));
                let instance = self.sessions[id].instance.clone();
                self.act(Action::ConnectWorker { ticket, instance });
                let s = self.sessions.get_mut(id).expect("a flow has a session");
                s.ticket = Some(ticket);
                if let Flow::Adopt(f) = &mut s.flow {
                    f.phase = AdoptPhase::AwaitConnect;
                }
            }
            AdoptPhase::Post => self.post_adoption(id, f),
            AdoptPhase::AwaitProbe
            | AdoptPhase::AwaitConnect
            | AdoptPhase::AwaitHello
            | AdoptPhase::AwaitReport => {}
        }
    }

    /// The answer of an identity probe of an adoption. False when no adoption waits for it.
    pub(crate) fn adopt_probed(&mut self, identity: ProcessIdentity, state: IdentityState) -> bool {
        let found = self
            .sessions
            .iter()
            .find(|(_, s)| {
                s.worker.identity == Some(identity)
                    && matches!(&s.flow, Flow::Adopt(f) if f.phase == AdoptPhase::AwaitProbe)
            })
            .map(|(id, _)| id.clone());
        let Some(id) = found else {
            return false;
        };
        match state {
            IdentityState::Matches => self.set_adopt_phase(&id, AdoptPhase::Connect),
            // AD-2 `WorkerGone`. A reused pid is never signalled (AD-6).
            IdentityState::Absent | IdentityState::Reused => {
                self.adopt_end(&id, End::Lost(LostReason::WorkerGone), "")
            }
        }
        true
    }

    /// The answer of the connect (DESIGN.md 3.1, 3.2): the host speaks first, with its own proof and epoch.
    pub(crate) fn adopt_connected(&mut self, id: &SessionId, link: Option<LinkId>) {
        let waiting = matches!(
            self.sessions.get(id).map(|s| &s.flow),
            Some(Flow::Adopt(f)) if f.phase == AdoptPhase::AwaitConnect
        );
        if !waiting {
            if let Some(link) = link {
                self.close_link(link, "the adoption that made the link ended (AD-6)");
            }
            return;
        }
        let Some(link) = link else {
            // No worker answers at the endpoint: AD-2 `WorkerUnreachable`; Core never binds or spawns in its place.
            self.adopt_end(id, End::Lost(LostReason::WorkerUnreachable), "");
            return;
        };
        let s = self.sessions.get_mut(id).expect("checked above");
        s.worker.link = Some(link);
        s.worker.link_failed = false;
        let token = s.token.expect("an adopted row has its token");
        let instance = s.instance.clone();
        self.links.insert(link, id.clone());
        self.act(Action::SendHello {
            link,
            hello: Hello {
                protocol: self.cfg.worker_protocol,
                proof: host_proof(&token, &instance, self.cfg.host_epoch),
                instance,
                host_epoch: self.cfg.host_epoch,
            },
        });
        self.set_adopt_phase(id, AdoptPhase::AwaitHello);
    }

    /// The worker's answer to the host's hello (DESIGN.md 3.5, 3.6).
    pub(crate) fn adopt_hello(&mut self, link: LinkId, id: &SessionId, hello: Hello) {
        let waiting = matches!(
            self.sessions.get(id).map(|s| &s.flow),
            Some(Flow::Adopt(f)) if f.phase == AdoptPhase::AwaitHello
        );
        if !waiting {
            self.links.remove(&link);
            self.close_link(link, "a second hello on an adopted link (AD-6)");
            return;
        }
        let s = &self.sessions[id];
        let token = s.token.expect("an adopted row has its token");
        let proved = hello.instance == s.instance
            && hello.host_epoch == self.cfg.host_epoch
            && hello.proof == token_proof(&token, &s.instance, self.cfg.host_epoch);
        if !proved {
            // A10-1, A11-1, AD-6: the process that answered is not the session's worker. "A process that does not match is
            // never signalled", and its row is `WorkerGone` (A10-1: "a non-matching process at the recorded pid is
            // `WorkerGone`"). Core decodes no later frame of the link. The worker is gone for this host, so `Remove` never
            // probes or signals the recorded identity either.
            self.sessions
                .get_mut(id)
                .expect("checked above")
                .worker
                .gone = true;
            self.adopt_end(
                id,
                End::Lost(LostReason::WorkerGone),
                "the worker's hello does not prove the token, the instance or the host epoch (AD-6)",
            );
            return;
        }
        self.sessions
            .get_mut(id)
            .expect("checked above")
            .worker_protocol = Some(hello.protocol);
        if !self.adoptable_worker_protocols().contains(&hello.protocol) {
            // AD-4, A6-2: the protocol is recorded (LC-9), and the worker is not signalled.
            let why = format!(
                "the worker protocol {} is not adoptable (AD-4)",
                hello.protocol
            );
            self.adopt_end(id, End::Lost(LostReason::WorkerVersion), &why);
            return;
        }
        self.set_adopt_phase(id, AdoptPhase::AwaitReport);
    }

    /// The worker's report decides the row's state, with the row's intent for `AdoptAll` (R-35) and without it for
    /// `Adopt(id)` (R-36; DESIGN.md 5).
    pub(crate) fn adopt_report(&mut self, id: &SessionId, report: AdoptReport) {
        let Some(f) = self.adopt_flow(id).cloned() else {
            return;
        };
        if f.phase != AdoptPhase::AwaitReport {
            return;
        }
        let s = self.sessions.get_mut(id).expect("a flow has a session");
        s.worker_features = Some(report.features);
        s.formats = report.formats;
        if report.terminal.is_some() {
            s.terminal = report.terminal;
        }
        if let AdoptedPayload::Running { payload } = report.payload {
            s.payload = Some(ProcessIdentity {
                pid: payload.pid,
                start_time: payload.start_time,
            });
        }
        use AdoptedPayload as P;
        use SessionState as R;
        let Some(recorded) = f.recorded else {
            self.adopt_retry(id, f, report.payload);
            return;
        };
        match (recorded, report.payload) {
            // R-35 (a): Core does AD-7 step 4 with the `Launch` built from the row. The worker accepts at most one `Launch`
            // in its life, so a `Launch` that it accepted earlier is reported as one of the other states, never this one.
            (R::Starting, P::NotLaunched) => self.adopt_start(id, f, StartPhase::SendLaunch),
            // R-35 (b): no second `Launch`; the spawn's answer is applied as in (a).
            (R::Starting, P::Spawning) => self.adopt_start(id, f, StartPhase::AwaitLaunched),
            // R-35 (a): the same outcome as a failed launch of an ordinary `Start` (LC-4).
            (R::Starting, P::LaunchFailed { reason }) => {
                self.adopt_start(id, f, StartPhase::AwaitLaunched);
                self.fail_start(id, reason, End::Exited(never_ran(ExitCause::Other)));
            }
            // AD-1: `Running`, or `Exited` when the payload ended meanwhile.
            (R::Starting | R::Running, P::Running { .. }) => {
                self.adopt_end_with(id, SessionState::Running, "");
            }
            (R::Starting | R::Running, P::Exited { code, signal }) => {
                let exit = self.exit_of(id, code, signal);
                self.adopt_end(id, End::Exited(exit), "");
            }
            // AD-1: a `Stopping` row is adopted `Stopping`, and the stop is sent again with `stop_grace` from the adoption.
            (R::Stopping, P::Spawning | P::Running { .. }) => self.adopt_stop(id),
            // The payload ended: the stop needs nothing more. The host asked for the end (`HostStop`).
            (R::Stopping, P::Exited { code, signal }) => {
                self.sessions.get_mut(id).expect("kept").host_ended = true;
                let exit = self.exit_of(id, code, signal);
                self.adopt_end(id, End::Exited(exit), "");
            }
            // R-35 (c): nothing runs, and the stop needs nothing. No `Launch` is sent.
            (R::Stopping, P::NotLaunched | P::LaunchFailed { .. }) => {
                self.adopt_end(id, End::Exited(never_ran(ExitCause::HostStop)), "");
            }
            // AD-1: an `Exited` row whose final-model worker is alive is adopted `Exited`, with the exit that the row
            // recorded.
            (R::Exited(exit), P::Exited { .. } | P::LaunchFailed { .. }) => {
                self.adopt_end(id, End::Exited(exit), "");
            }
            // The other pairs. R-35 (d): `Running` or `Exited` with `NotLaunched` or `Spawning`. The worker is
            // authenticated (AD-6), so the durable row is the wrong record: a corrupt registry record although its bytes
            // decode. `Running` with `LaunchFailed`, and `Exited` with `Running`, contradict the row in the same way: an
            // `Exited` row is written only after the end that it records, and a `Running` row only after `Launched`. A
            // row of any other state never reaches the handshake (`session_of_row`).
            _ => self.adopt_end(id, End::Lost(LostReason::RegistryCorrupt), ""),
        }
    }

    /// R-36, with the follow-up `d18b6de`: `Adopt(id)` of a `Lost` session re-reads the worker (AD-3), and no `Launch` is
    /// sent (AD-2). There is no `Stopping` outcome: a host that still wants the payload ended calls `Stop` after the
    /// adoption.
    fn adopt_retry(&mut self, id: &SessionId, f: AdoptFlow, payload: AdoptedPayload) {
        match payload {
            AdoptedPayload::Running { .. } => self.adopt_end_with(id, SessionState::Running, ""),
            AdoptedPayload::Exited { code, signal } => {
                let exit = self.exit_of(id, code, signal);
                self.adopt_end(id, End::Exited(exit), "");
            }
            // AD-2 `StartInterrupted`: the start never reached its payload, and a `Lost` row gets no `Launch`.
            AdoptedPayload::NotLaunched => {
                self.adopt_end(id, End::Lost(LostReason::StartInterrupted), "")
            }
            // As R-35 (b): the spawn's answer, bounded by the `startup` of the adoption (TM-3), with no `Launch` sent.
            AdoptedPayload::Spawning => self.adopt_start(id, f, StartPhase::AwaitLaunched),
            // The same outcome as a failed launch of an ordinary `Start` (LC-4).
            AdoptedPayload::LaunchFailed { reason } => {
                self.adopt_start(id, f, StartPhase::AwaitLaunched);
                self.fail_start(id, reason, End::Exited(never_ran(ExitCause::Other)));
            }
        }
    }

    /// R-35 (a), (b): the start's own path (AD-7 step 4 and LC-4) posts the row's one state.
    fn adopt_start(&mut self, id: &SessionId, f: AdoptFlow, phase: StartPhase) {
        self.sessions.get_mut(id).expect("kept").flow = Flow::Start(StartFlow {
            op: f.op,
            phase,
            failure: None,
            deadline: f.deadline,
            error: None,
            hello_seen: true,
            adopted: true,
        });
    }

    /// AD-1: the stop's own path posts `Stopping`, the row's one state, and resends the stop.
    fn adopt_stop(&mut self, id: &SessionId) {
        let s = self.sessions.get_mut(id).expect("kept");
        s.host_ended = true;
        s.flow = Flow::Stop(StopFlow {
            phase: StopPhase::SendStop,
            deadline: None,
            end: None,
        });
    }

    /// The adoption ends in `end`. An end with a worker (`Running`, `Exited`) keeps the authenticated link: an `Exited`
    /// session stays readable from its final-model worker until `Remove` (AD-1, ST-5; review P5-F23). A `Lost` end
    /// closes the link that the adoption made, and records `why`.
    pub(crate) fn adopt_end(&mut self, id: &SessionId, end: End, why: &str) {
        self.adopt_end_with(id, end.state(), why);
    }

    fn adopt_end_with(&mut self, id: &SessionId, state: SessionState, why: &str) {
        if matches!(state, SessionState::Lost(_)) {
            let why = if why.is_empty() {
                "the adoption ended without the worker (AD-2)"
            } else {
                why
            };
            self.close_worker_link(id, why);
        }
        if let Some(f) = self.adopt_flow(id) {
            f.deadline = None;
            f.post = Some(state);
            f.phase = AdoptPhase::Post;
        }
    }

    /// The deadline of the connect, the hello and the report passed (DESIGN.md 3.7). No signal: a worker may live there.
    pub(crate) fn adopt_expired(&mut self, id: &SessionId) {
        self.adopt_end(
            id,
            End::Lost(LostReason::WorkerUnreachable),
            "the adoption passed its startup deadline (DESIGN.md 3.7)",
        );
    }

    /// The link of an adoption closed before the row's state was posted.
    pub(crate) fn adopt_link_closed(&mut self, id: &SessionId) {
        self.adopt_end(id, End::Lost(LostReason::WorkerUnreachable), "");
    }

    /// Posts the row's one state (LC-11, EV-5b: only with room, so this step posts it).
    fn post_adoption(&mut self, id: &SessionId, f: AdoptFlow) {
        let state = f.post.expect("Post has its state");
        // The state that the row records: the row's own for `AdoptAll`, the shown `Lost` for `Adopt(id)`.
        let row = f.recorded.or(self.sessions[id].shown);
        // A retry that ends in the state that the session shows posts no second event for it (A2-1 `Adopt`: the record
        // tells the outcome).
        let shown = self.sessions[id].shown == Some(state);
        if !shown && !self.post_state(id, state) {
            return;
        }
        let s = self.sessions.get_mut(id).expect("a flow has a session");
        s.shown = Some(state);
        match state {
            SessionState::Running => s.admit = Admit::Running,
            SessionState::Exited(exit) => {
                s.exit = Some(exit);
                s.admit = Admit::Exited;
            }
            SessionState::Lost(_) => s.admit = Admit::Lost,
            _ => unreachable!("an adoption posts Running, Exited or Lost"),
        }
        // A state that the row does not record yet is written. A `Lost` row keeps the worker's identity, so `Adopt(id)`
        // may retry it (R-36).
        if row != Some(state) {
            self.write_final_row(id, state);
        }
        self.flow_done(id);
        self.adoption_posted(id);
        // An end that the worker reported while the state waited (review P5-F25) applies after `Running`, in contract
        // order (OR-2). Another adoption end already is the session's end.
        let pending = self.sessions.get_mut(id).and_then(|s| s.pending_end.take());
        if let (SessionState::Running, Some(end)) = (state, pending) {
            self.begin_end_flow(id, end);
        }
    }

    /// The row's state is posted: an `Adopt` op completes with the record, and an `AdoptAll` op counts the row as done.
    pub(crate) fn adoption_posted(&mut self, id: &SessionId) {
        let Some(op) = self.sessions.get_mut(id).and_then(|s| s.adopting.take()) else {
            return;
        };
        if matches!(self.ops.get(&op).map(|p| &p.op), Some(Op::Adopt { .. })) {
            let record = self.sessions[id].record().expect("the state was posted");
            self.complete(op, OpResult::Ok(OpOutput::Record(record)));
        }
    }
}
