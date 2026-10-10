//! `RealCoreHarness` (plan 4.2, the real-process tier): the `CoreHarness` over the real `Core::open`, real `botster-worker`
//! processes and the real `botster-conformance-probe`, all prebuilt and verified against the candidate manifest.
//!
//! Lead ruling (2026-10-04): the harness owns the processes through public launch inputs only. `OpenConfig.worker_path` and
//! `argv[0]` of every session program are wrapper scripts of the group guard of `botster-test-process`
//! ([`botster_test_process::Guard`]), which exec the verified binaries. Core and the worker run unchanged, with no injection
//! seam and no test branch. Production starts and reaps every process; the guard's anchors end every group on every exit
//! path of the test, its death included.
//!
//! An anchor gives its group the subject's configured stop grace before its rounds of `KILL` (ruling item 9), so the harness
//! keeps one guard for each stop grace that an `open` names.
//!
//! Plan 23l: `open` composes `HostDriver<EdgeTap<RealEdges>>` over [`botster_core::open_parts`], the parts of `Core::open`
//! itself. `Core` only delegates to its `HostDriver`, so every id runs on Core's own composition; the [`EdgeTap`] passes
//! every edge call through unchanged and observes it. The controls that it serves:
//!
//! - `edges_quiet` (`{quiet}`): [`Tap::quiet`]. Quiet means that nothing arrived on an edge and nothing is unread, never
//!   that a worker finished.
//! - `break_control` (`{session}`): the inner edge's `link_close` on the session's control link ([`Tap::break_link`]).
//!   Core's end of the stream is dropped (not `shutdown`), so the worker reads EOF, and every later call of Core on that
//!   `LinkId` reaches `RealEdges`' closed-link state: `Ok(0)` on `link_recv`, `BrokenPipe` on `link_send`.
//!
//! - `corrupt_registry_row` (`{session}`, Core A10-2, AD-2): the storage edge damages the stored bytes of the session's row,
//!   which Core's own encoder wrote ([`Tap::stored_row`], [`Tap::store_row`]), in the testkit's way
//!   ([`crate::controls::damaged`]). Core's real decoder rejects them.
//! - `payload_alive` (`{session}` → `{alive}`, Core EV-5(c), AD-7): the recorded payload's identity still matches (the inner
//!   edge's AD-6 check, [`Tap::identity_state`]) and the process is live, not a zombie that its worker has yet to reap
//!   ([`botster_test_process::platform::live_members`] of its own group: the payload leads its group, `setsid`). It
//!   sends no signal.
//! - `lose_worker` (`{session, reason?}`, Core AD-2, IN-7): the recorded worker's process group gets `KILL` through the
//!   inner edge's identity-checked `signal_group` ([`Tap::kill_group`]): only while the recorded pid and start time match,
//!   so a reused pid is never signalled (AD-6). The worker's group is the worker's own (Core spawns it as a group leader),
//!   and the payload leads its own group (`setsid`), so the kill never targets the payload's group; the payload ends with
//!   its terminal. Only `worker_gone` (the default) is built; `worker_unreachable` gives `unsupported_control`, as on the
//!   testkit.
//!
//! Every other control gives `unsupported_control`, which is never a pass.

use crate::candidate::{Candidate, PROBE, WORKER};
use crate::controls::{damaged, parse};
use crate::edge_tap::{EdgeTap, Rows, Tap};
use crate::harness::{limits_of, no_route};
use crate::process_controls::LoseReason;
use botster_core::RealEdges;
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::{IdentityState, ProcessIdentity};
use botster_core_host::driver::HostDriver;
use botster_core_host::session::{row_key, Row};
use botster_test_process::{platform, Guard};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

/// The binary that a wrapper execs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    Worker,
    Probe,
}

/// The tap of one open handle. The driver owns the tap; the harness keeps a `Weak`, so the data directory's lock ends with
/// the handle (LC-2).
struct Handle {
    data_dir: PathBuf,
    tap: Weak<Mutex<Tap<RealEdges>>>,
}

/// The processes of a session, as Core's own registry rows name them (AD-6, LC-5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionProcesses {
    pub instance: InstanceId,
    /// The worker process, once Core recorded it.
    pub worker: Option<ProcessIdentity>,
    /// The payload process, once the worker reported it.
    pub payload: Option<ProcessIdentity>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EdgesQuiet {}

fn on() -> bool {
    true
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct BreakControl {
    session: SessionId,
    #[serde(default = "on")]
    on: bool,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OfSession {
    session: SessionId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LoseWorker {
    session: SessionId,
    reason: Option<LoseReason>,
}

/// The real-tier harness. A real implementation ignores the seed (plan 4.2c).
pub struct RealCoreHarness {
    /// One guard for each stop grace. Declared before `root`, so that every guard ends its groups before the root goes.
    guards: BTreeMap<Duration, Guard>,
    candidate: Candidate,
    /// The worker wrapper of each file name and stop grace.
    workers: BTreeMap<(String, Duration), PathBuf>,
    /// The program wrapper that `probe_binary` names: the one of the latest `open`, with that open's stop grace.
    probe: PathBuf,
    /// Core TH-1 for the facade's `Core` type, as the runner checked it at compile time (`with_core_type`).
    core_send_not_sync: Option<bool>,
    /// False for the plain harness of the pass-through test: `open` is `Core::open`, with no tap and no control.
    tapped: bool,
    /// The tap of each handle that `open` opened, until `drop_handle`.
    handles: BTreeMap<String, Handle>,
    /// The registry rows of each data directory, across every handle that opened it.
    rows: BTreeMap<PathBuf, Rows>,
    /// The guards' sockets, the wrappers and the data directories.
    root: tempfile::TempDir,
}

/// Locks a tap or a row table. A panic while it was held leaves plain data behind, so a poisoned lock is still usable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

impl std::fmt::Debug for RealCoreHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealCoreHarness")
            .field("candidate", &self.candidate)
            .field("root", &self.root.path())
            .finish_non_exhaustive()
    }
}

impl RealCoreHarness {
    /// A harness over the verified binaries of `candidate`. Before the first `open`, a session program gets the default
    /// stop grace of the Core limits.
    ///
    /// # Errors
    /// The root directory, the parent of the data directories, a guard's socket or the probe's wrapper could not be made.
    pub fn new(candidate: Candidate) -> io::Result<RealCoreHarness> {
        RealCoreHarness::make(candidate, true)
    }

    /// The plain harness of the pass-through test (plan 23l): `open` is the production `Core::open`, with no tap, so it
    /// serves no control. A transcript with no control passes on it exactly when it passes on [`RealCoreHarness::new`].
    ///
    /// # Errors
    /// As [`RealCoreHarness::new`].
    pub fn plain(candidate: Candidate) -> io::Result<RealCoreHarness> {
        RealCoreHarness::make(candidate, false)
    }

    fn make(candidate: Candidate, tapped: bool) -> io::Result<RealCoreHarness> {
        let mut harness = RealCoreHarness {
            guards: BTreeMap::new(),
            candidate,
            workers: BTreeMap::new(),
            probe: PathBuf::new(),
            core_send_not_sync: None,
            tapped,
            handles: BTreeMap::new(),
            rows: BTreeMap::new(),
            root: tempfile::tempdir()?,
        };
        // `Core::open` creates the data directory itself, not its parent.
        std::fs::create_dir(harness.root.path().join("d"))?;
        harness.probe = harness.wrapper(Role::Probe, PROBE, CoreLimits::default().stop_grace)?;
        Ok(harness)
    }

    /// Core TH-1: the runner names the facade's `Core` type and checks it at compile time; `type_check: send_not_sync`
    /// reads the answer.
    #[must_use]
    pub fn with_core_type(mut self, send_not_sync: bool) -> RealCoreHarness {
        self.core_send_not_sync = Some(send_not_sync);
        self
    }

    /// The directory of the harness: the guards' sockets, the wrappers and the data directories.
    pub fn root(&self) -> &Path {
        self.root.path()
    }

    /// The wrapper `<root>/w/<grace>/<role>/<file_name>` of the guard of `grace`: it execs the verified binary of `role`
    /// through that guard's anchor. The role, not the file name, selects the binary: a worker may have any file name (Core
    /// E1-1), the probe's too, and each role has its own directory, so the two never share a path.
    fn wrapper(&mut self, role: Role, file_name: &str, grace: Duration) -> io::Result<PathBuf> {
        let (program, role_dir) = match role {
            Role::Worker => (self.candidate.worker.clone(), "worker"),
            Role::Probe => (self.candidate.probe.clone(), "probe"),
        };
        let dir = self
            .root
            .path()
            .join("w")
            .join(grace.as_nanos().to_string())
            .join(role_dir);
        std::fs::create_dir_all(&dir)?;
        let path = dir.join(file_name);
        self.guard(grace)?.wrapper(&path, &program, &[])?;
        Ok(path)
    }

    /// The guard whose anchors give `grace`, made at its first use with its socket in `<root>/g/<grace>`.
    fn guard(&mut self, grace: Duration) -> io::Result<&Guard> {
        if !self.guards.contains_key(&grace) {
            let dir = self
                .root
                .path()
                .join("g")
                .join(grace.as_nanos().to_string());
            std::fs::create_dir_all(&dir)?;
            let guard = Guard::with_binary(&dir, self.candidate.anchor.clone())?.grace(grace);
            self.guards.insert(grace, guard);
        }
        Ok(&self.guards[&grace])
    }

    fn worker_wrapper(&mut self, file_name: &str, grace: Duration) -> io::Result<PathBuf> {
        let key = (file_name.to_string(), grace);
        if let Some(path) = self.workers.get(&key) {
            return Ok(path.clone());
        }
        let path = self.wrapper(Role::Worker, file_name, grace)?;
        self.workers.insert(key, path.clone());
        Ok(path)
    }

    /// The processes of `session` in `data_dir`, from every registry row that passed through a tap of that directory, of
    /// any handle. A row outlives the session's removal. `None` when no row of the session passed through.
    pub fn session_processes(
        &self,
        data_dir: &DataDirRef,
        session: &SessionId,
    ) -> Option<SessionProcesses> {
        let rows = self.rows.get(Path::new(&data_dir.0))?;
        let rows = lock(rows);
        let row = rows.get(session)?;
        Some(SessionProcesses {
            instance: row.instance.clone(),
            worker: row.worker.map(|w| w.identity()),
            payload: row.payload.map(|w| w.identity()),
        })
    }

    /// The data directory of the open handle `handle`.
    pub fn data_dir_of(&self, handle: &str) -> Option<DataDirRef> {
        self.handles
            .get(handle)
            .map(|h| DataDirRef(h.data_dir.display().to_string()))
    }

    fn tap(&self, handle: &str) -> Result<Arc<Mutex<Tap<RealEdges>>>, ControlError> {
        self.handles
            .get(handle)
            .and_then(|h| h.tap.upgrade())
            .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))
    }

    fn edges_quiet(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let EdgesQuiet {} = parse(args)?;
        let tap = self.tap(handle)?;
        let quiet = lock(&tap).quiet();
        Ok(json!({ "quiet": quiet }))
    }

    fn break_control(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let args: BreakControl = parse(args)?;
        if !args.on {
            return Err(ControlError::Bad(
                "a broken control link is not mended (`on: false`)".into(),
            ));
        }
        let tap = self.tap(handle)?;
        let dir = self
            .data_dir_of(handle)
            .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
        let processes = self.session_processes(&dir, &args.session).ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no row", args.session.0))
        })?;
        let mut tap = lock(&tap);
        let link = tap.link_of(&processes.instance).ok_or_else(|| {
            ControlError::Bad(format!(
                "the session {} has no open control link",
                args.session.0
            ))
        })?;
        tap.break_link(link);
        Ok(Value::Null)
    }

    /// The recorded processes of `session` on the host of `handle`, with that host's tap.
    fn recorded(
        &self,
        handle: &str,
        session: &SessionId,
    ) -> Result<(Arc<Mutex<Tap<RealEdges>>>, SessionProcesses), ControlError> {
        let tap = self.tap(handle)?;
        let dir = self
            .data_dir_of(handle)
            .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
        let processes = self
            .session_processes(&dir, session)
            .ok_or_else(|| ControlError::Bad(format!("no row of the session {}", session.0)))?;
        Ok((tap, processes))
    }

    /// Core A10-2, AD-2: the session's stored row becomes bytes that Core's decoder rejects.
    fn corrupt_registry_row(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let OfSession { session } = parse(args)?;
        let tap = self.tap(handle)?;
        let mut tap = lock(&tap);
        let key = row_key(&session);
        let bytes = tap
            .stored_row(&key)
            .ok_or_else(|| ControlError::Bad(format!("no row of the session {}", session.0)))?;
        let bytes = damaged(&bytes);
        if Row::decode(&session, &bytes).is_some() {
            return Err(ControlError::Bad(format!(
                "the damaged row of the session {} still decodes",
                session.0
            )));
        }
        tap.store_row(&key, &bytes).map_err(|error| {
            ControlError::Bad(format!(
                "the damaged row of the session {} is not stored: {error:?}",
                session.0
            ))
        })?;
        Ok(Value::Null)
    }

    /// Core EV-5(c), AD-7: `{alive}`, whether the session's payload runs now. A session whose row names no payload has none.
    fn payload_alive(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let OfSession { session } = parse(args)?;
        let (tap, processes) = self.recorded(handle, &session)?;
        let alive = match processes.payload {
            Some(payload) => {
                lock(&tap).identity_state(payload) == IdentityState::Matches
                    && runs(payload).map_err(|error| {
                        ControlError::Bad(format!(
                            "the payload of the session {} cannot be read: {error}",
                            session.0
                        ))
                    })?
            }
            None => false,
        };
        Ok(json!({ "alive": alive }))
    }

    /// Core AD-2, IN-7: the session's worker ends from outside Core, as a kill does. A recorded worker that no longer
    /// matches its identity (it ended, or its pid was reused) gets no signal, and the control is `Bad`.
    fn lose_worker(&self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let LoseWorker { session, reason } = parse(args)?;
        if let Some(LoseReason::WorkerUnreachable) = reason {
            return Err(ControlError::Unsupported);
        }
        let (tap, processes) = self.recorded(handle, &session)?;
        let worker = processes.worker.ok_or_else(|| {
            ControlError::Bad(format!("the session {} has no worker process", session.0))
        })?;
        match lock(&tap).kill_group(worker) {
            IdentityState::Matches => Ok(Value::Null),
            state => Err(ControlError::Bad(format!(
                "the recorded worker of the session {} is not signalled: {state:?}",
                session.0
            ))),
        }
    }

    fn harness_failed(error: io::Error) -> CoreError {
        CoreError::new(
            ErrorCode::Internal,
            format!("the real harness could not prepare a wrapper: {error}"),
        )
    }
}

impl CoreHarness for RealCoreHarness {
    /// Opens Core's own composition over a real data directory, with the guarded worker wrapper as its worker path (LC-1,
    /// LC-2, 9B): `open_parts`, then `HostDriver::open` over the tapped edges, which is `Core::open` with the tap between.
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        let limits = limits_of(spec)?;
        let grace = limits.stop_grace;
        let worker_path = match &spec.worker {
            Some(worker) => Some(
                self.worker_wrapper(worker.file_name.as_deref().unwrap_or(WORKER), grace)
                    .map_err(RealCoreHarness::harness_failed)?,
            ),
            None => None,
        };
        let data_dir = PathBuf::from(&spec.data_dir.0);
        let config = OpenConfig {
            data_dir: data_dir.clone(),
            worker_path,
            limits,
        };
        let core: Box<dyn CoreApi> = if self.tapped {
            let (cfg, edges) = botster_core::open_parts(config)?;
            let rows = Arc::clone(self.rows.entry(data_dir.clone()).or_default());
            let (edges, tap) = EdgeTap::new(edges, rows);
            let driver = HostDriver::open(cfg, edges)?;
            self.handles
                .insert(spec.handle.clone(), Handle { data_dir, tap });
            Box::new(driver)
        } else {
            Box::new(botster_core::Core::open(config)?)
        };
        self.probe = self
            .wrapper(Role::Probe, PROBE, grace)
            .map_err(RealCoreHarness::harness_failed)?;
        Ok(core)
    }

    /// Core reads no clock: `pump` takes `now` from the host (Core TM-1), so the runner's clock is the one Core sees
    /// (R-43 A). The workers and their payloads still follow real time.
    fn injects_clock(&self) -> bool {
        true
    }

    /// The workers and payloads are real processes, so the injected clock does not control every source of progress. The
    /// driver then never jumps the clock to Core's next deadline: it moves only by `advance_clock` (steward ruling R-46).
    fn progress_is_injected(&self) -> bool {
        false
    }

    /// A directory under the harness's root (`<root>/d/<name>`), kept across a drop and a reopen (Core LC-12, AD-1).
    /// `Core::open` creates it; `new` made its parent.
    fn data_dir(&mut self, name: &str) -> DataDirRef {
        DataDirRef(self.root.path().join("d").join(name).display().to_string())
    }

    /// `Current` is the prebuilt `botster-worker`. There is no `Previous` while the worker protocol is 1 (Core A6-2).
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        (which == WorkerBuild::Current).then_some(WorkerRef {
            build: which,
            file_name: None,
        })
    }

    /// The same worker under another file name: the wrapper's name, which Core receives as the path (Core E1-1).
    fn worker_named(&self, file_name: &str) -> Option<WorkerRef> {
        Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: Some(file_name.to_string()),
        })
    }

    /// The driver drops its `Core`; the workers keep running (Core LC-12) and stay in the guards' groups. The data
    /// directory's rows stay.
    fn drop_handle(&mut self, handle: &str) {
        self.handles.remove(handle);
    }

    fn has_control(&self, op: &str) -> bool {
        self.tapped
            && matches!(
                op,
                "edges_quiet"
                    | "break_control"
                    | "corrupt_registry_row"
                    | "payload_alive"
                    | "lose_worker"
            )
    }

    /// Core TH-1: the runner's compile-time answer for the facade's `Core` (`with_core_type`); `None` without one.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        self.core_send_not_sync
    }

    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        match op {
            "edges_quiet" if self.tapped => self.edges_quiet(handle, args),
            "break_control" if self.tapped => self.break_control(handle, args),
            "corrupt_registry_row" if self.tapped => self.corrupt_registry_row(handle, args),
            "payload_alive" if self.tapped => self.payload_alive(handle, args),
            "lose_worker" if self.tapped => self.lose_worker(handle, args),
            _ => Err(ControlError::Unsupported),
        }
    }

    /// `check_crates` reads the workspace, which is the same for both harnesses (Core A5-1).
    fn statement(&mut self, kind: &str, spec: &Value) -> Result<Value, ControlError> {
        match kind {
            "check_crates" => {
                crate::statements::check_crates(crate::statements::workspace_root(), spec)
            }
            _ => Err(ControlError::Unsupported),
        }
    }

    /// The guarded wrapper of the prebuilt probe. The probe receives the script as `argv[1]`, unchanged.
    fn probe_binary(&self) -> String {
        self.probe.display().to_string()
    }

    fn attach_stream(
        &mut self,
        _handle: &str,
        _core: &mut dyn CoreApi,
        _client: ClientId,
        _session: &SessionId,
        _options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError> {
        Err(no_route("attach_stream"))
    }
}

/// Whether the process `identity` is live: a member of its own process group that is not a zombie. The payload leads its
/// group (`setsid`), so its group id is its pid.
fn runs(identity: ProcessIdentity) -> io::Result<bool> {
    let pid = platform::pid(identity.pid)?;
    Ok(platform::live_members(pid)?.iter().any(|m| m.pid == pid))
}

/// The process and storage controls on real sessions of the prebuilt candidate.
#[cfg(test)]
mod slow_controls {
    use super::*;
    use botster_test_process::Deadline;
    use std::time::Instant;

    fn sid(name: &str) -> SessionId {
        SessionId(name.into())
    }

    /// Pumps `core` until `done` holds, by the cleanup deadline. The host pumps after a wake (TM-6).
    fn pump_until(core: &mut dyn CoreApi, mut done: impl FnMut(&mut dyn CoreApi) -> bool) {
        let deadline = Deadline::cleanup();
        let wake = core.wake_handle();
        loop {
            loop {
                let report = core.pump(Now {
                    monotonic: Instant::now(),
                    unix: 1_000_000,
                });
                core.poll_events(64);
                if !report.more {
                    break;
                }
            }
            if done(core) {
                return;
            }
            assert!(!deadline.expired(), "the host never got there");
            // timer: deadline — the cleanup bound ends a host that never gets there.
            wake.wait(deadline.remaining());
        }
    }

    fn state(core: &dyn CoreApi, session: &str) -> SessionState {
        core.get(&sid(session)).expect("the session").state
    }

    /// Creates and starts `session` with a payload that runs until a signal ends it (the probe's `hold`).
    fn running(harness: &RealCoreHarness, core: &mut dyn CoreApi, session: &str) {
        let hold = json!({ "program": [{ "hold": {} }] }).to_string();
        let request = SpawnRequest {
            argv: vec![harness.probe_binary(), hold],
            env: BTreeMap::new(),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            labels: BTreeMap::new(),
            color_profile: None,
            notification_policy: None,
            size_policy: None,
        };
        core.begin(Op::Create {
            session: sid(session),
            request,
        })
        .expect("create");
        pump_until(core, |c| c.get(&sid(session)).is_ok());
        core.begin(Op::Start { id: sid(session) }).expect("start");
        pump_until(core, |c| state(c, session) == SessionState::Running);
    }

    /// Core AD-6, AD-2, A10-2, EV-5(c), on real sessions:
    /// - `lose_worker` signals only a recorded worker whose pid and start time match: an identity with another start time
    ///   (a reused pid) is refused, and the worker keeps running. The worker's group holds no payload, so the kill never
    ///   targets the payload's group. After the kill the worker is gone, and a second `lose_worker` signals nothing.
    /// - `payload_alive` is true for a running payload and false after `Stop` ended it.
    /// - `corrupt_registry_row` stores the damaged bytes of Core's own row, which Core's decoder rejects, and the record of
    ///   the session's processes keeps what Core wrote.
    #[test]
    fn the_process_and_storage_controls_act_only_on_the_recorded_processes() {
        let dir = Candidate::beside_test_binary().expect("the candidate directory");
        let candidate = Candidate::locate(&dir).unwrap_or_else(|error| panic!("{error}"));
        let mut harness = RealCoreHarness::new(candidate).unwrap();
        let data_dir = harness.data_dir("a");
        let spec = OpenSpec {
            handle: "a".into(),
            data_dir: data_dir.clone(),
            worker: harness.worker(WorkerBuild::Current),
            limits: serde_json::to_value(CoreLimits::default()).unwrap(),
        };
        let mut core = harness.open(&spec).expect("open");
        running(&harness, core.as_mut(), "s1");
        running(&harness, core.as_mut(), "s2");
        let alive = |h: &mut RealCoreHarness, s: &str| {
            h.control("a", "payload_alive", &json!({ "session": s }))
        };
        assert_eq!(alive(&mut harness, "s1"), Ok(json!({ "alive": true })));

        let s1 = harness.session_processes(&data_dir, &sid("s1")).unwrap();
        let worker = s1.worker.expect("the worker is recorded");
        let payload = s1.payload.expect("the payload is recorded");
        let payload_pid = platform::pid(payload.pid).unwrap();
        let worker_group = platform::live_members(platform::pid(worker.pid).unwrap()).unwrap();
        assert!(
            worker_group.iter().all(|m| m.pid != payload_pid),
            "the payload is not in the worker's group: {worker_group:?}"
        );
        let reused = ProcessIdentity {
            pid: worker.pid,
            start_time: worker.start_time + 1,
        };
        let tap = harness.tap("a").unwrap();
        assert_eq!(lock(&tap).kill_group(reused), IdentityState::Reused);
        assert_eq!(lock(&tap).identity_state(worker), IdentityState::Matches);
        assert!(runs(worker).unwrap(), "a reused pid is never signalled");

        assert_eq!(
            harness.control("a", "lose_worker", &json!({ "session": "s1" })),
            Ok(Value::Null)
        );
        pump_until(core.as_mut(), |_| {
            lock(&tap).identity_state(worker) == IdentityState::Absent
        });
        assert!(matches!(
            harness.control("a", "lose_worker", &json!({ "session": "s1" })),
            Err(ControlError::Bad(_))
        ));
        assert_eq!(
            harness.control(
                "a",
                "lose_worker",
                &json!({ "session": "s2", "reason": "worker_unreachable" })
            ),
            Err(ControlError::Unsupported)
        );
        assert!(matches!(
            harness.control("a", "lose_worker", &json!({ "session": "s9" })),
            Err(ControlError::Bad(_))
        ));

        core.begin(Op::Stop { id: sid("s2") }).expect("stop");
        pump_until(core.as_mut(), |c| {
            matches!(state(c, "s2"), SessionState::Exited(_))
        });
        assert_eq!(alive(&mut harness, "s2"), Ok(json!({ "alive": false })));

        let key = row_key(&sid("s2"));
        let stored = lock(&tap).stored_row(&key).expect("the row of s2");
        assert!(Row::decode(&sid("s2"), &stored).is_some());
        assert_eq!(
            harness.control("a", "corrupt_registry_row", &json!({ "session": "s2" })),
            Ok(Value::Null)
        );
        let now = lock(&tap).stored_row(&key).expect("the damaged row");
        assert_eq!(now, damaged(&stored));
        assert!(Row::decode(&sid("s2"), &now).is_none());
        assert!(harness.session_processes(&data_dir, &sid("s2")).is_some());
        drop(tap);
        drop(core);
    }
}

#[cfg(test)]
mod slow_tests {
    use super::*;

    /// Binaries that do not exist: enough for a harness that starts no process.
    fn nonexistent() -> Candidate {
        Candidate {
            worker: PathBuf::from("/nonexistent/worker"),
            probe: PathBuf::from("/nonexistent/probe"),
            anchor: PathBuf::from("/nonexistent/anchor"),
        }
    }

    /// The tapped harness serves only the controls that it builds at its tap; the plain harness of the pass-through test
    /// serves none.
    #[test]
    fn only_the_tapped_harness_has_controls_and_only_its_own() {
        let tapped = RealCoreHarness::new(nonexistent()).unwrap();
        assert!(tapped.has_control("edges_quiet"));
        assert!(tapped.has_control("break_control"));
        assert!(!tapped.has_control("pty_output"));
        assert!(!tapped.has_control("fail_next"));
        let plain = RealCoreHarness::plain(nonexistent()).unwrap();
        assert!(!plain.has_control("edges_quiet"));
        assert!(!plain.has_control("break_control"));
    }

    /// A control that a harness does not serve is `unsupported_control`, never a pass and never a bad argument: on the
    /// plain harness every control, on the tapped harness every control that it does not build.
    #[test]
    fn a_control_that_the_harness_does_not_serve_is_unsupported() {
        let mut plain = RealCoreHarness::plain(nonexistent()).unwrap();
        for op in ["edges_quiet", "break_control"] {
            let result = plain.control("h", op, &json!({}));
            assert!(
                matches!(result, Err(ControlError::Unsupported)),
                "{op}: {result:?}"
            );
        }
        let mut tapped = RealCoreHarness::new(nonexistent()).unwrap();
        let result = tapped.control("h", "pty_output", &json!({}));
        assert!(
            matches!(result, Err(ControlError::Unsupported)),
            "{result:?}"
        );
        let result = tapped.control("h", "edges_quiet", &json!({}));
        assert!(matches!(result, Err(ControlError::Bad(_))), "{result:?}");
    }

    /// Core TH-1: the harness answers for the facade's `Core` only what the runner told it at compile time.
    #[test]
    fn the_core_type_answer_is_the_runners_and_none_without_one() {
        let harness = RealCoreHarness::new(nonexistent()).unwrap();
        assert_eq!(harness.core_is_send_not_sync(), None);
        let harness = harness.with_core_type(true);
        assert_eq!(harness.core_is_send_not_sync(), Some(true));
        let harness = harness.with_core_type(false);
        assert_eq!(harness.core_is_send_not_sync(), Some(false));
    }

    /// R-46: the runner's clock is the one Core sees, but real processes make progress, so the driver must not jump it. The
    /// harness starts no process, so binaries that do not exist are enough.
    #[test]
    fn the_real_harness_injects_the_clock_but_not_its_progress() {
        let harness = RealCoreHarness::new(nonexistent()).unwrap();
        assert!(harness.injects_clock());
        assert!(!harness.progress_is_injected());
    }

    /// Core E1-1, A6-2: the real tier has the current worker, under its own name or another file name, and no previous
    /// build.
    #[test]
    fn the_real_harness_has_the_current_worker_under_any_name_and_no_previous_one() {
        let harness = RealCoreHarness::new(nonexistent()).unwrap();
        assert_eq!(
            harness.worker(WorkerBuild::Current),
            Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: None
            })
        );
        assert_eq!(harness.worker(WorkerBuild::Previous), None);
        assert_eq!(
            harness.worker_named("renamed-worker"),
            Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: Some("renamed-worker".into()),
            })
        );
    }

    /// After the driver drops a handle, the harness no longer names its data directory.
    #[test]
    fn a_dropped_handle_is_no_longer_open() {
        let mut harness = RealCoreHarness::new(nonexistent()).unwrap();
        harness.handles.insert(
            "h".into(),
            Handle {
                data_dir: PathBuf::from("/nonexistent/d/h"),
                tap: Weak::new(),
            },
        );
        assert_eq!(
            harness.data_dir_of("h"),
            Some(DataDirRef("/nonexistent/d/h".into()))
        );
        harness.drop_handle("h");
        assert_eq!(harness.data_dir_of("h"), None);
    }

    /// Core E1-1: a worker may have any file name, the probe's too (#220 RH-F1). The role selects the binary, and the
    /// worker's wrapper and the probe's never share a path.
    #[test]
    fn a_worker_named_like_the_probe_still_runs_the_worker() {
        let mut harness = RealCoreHarness::new(nonexistent()).unwrap();
        let grace = CoreLimits::default().stop_grace;
        let worker = harness.worker_wrapper(PROBE, grace).unwrap();
        let probe = harness.wrapper(Role::Probe, PROBE, grace).unwrap();
        assert_ne!(worker, probe);
        assert_eq!(worker.file_name(), probe.file_name());
        let worker_script = std::fs::read_to_string(&worker).unwrap();
        let probe_script = std::fs::read_to_string(&probe).unwrap();
        assert!(
            worker_script.contains("/nonexistent/worker"),
            "{worker_script}"
        );
        assert!(
            !worker_script.contains("/nonexistent/probe"),
            "{worker_script}"
        );
        assert!(
            probe_script.contains("/nonexistent/probe"),
            "{probe_script}"
        );
        assert!(
            !probe_script.contains("/nonexistent/worker"),
            "{probe_script}"
        );
    }

    /// A failed trial prints the harness, so its debug form names the binaries under test and the root directory.
    #[test]
    fn the_debug_form_names_the_candidate_and_the_root() {
        let harness = RealCoreHarness::new(nonexistent()).unwrap();
        let debug = format!("{harness:?}");
        assert!(debug.starts_with("RealCoreHarness {"), "{debug}");
        assert!(debug.contains("/nonexistent/worker"), "{debug}");
        assert!(
            debug.contains(&format!("{:?}", harness.root.path())),
            "{debug}"
        );
    }
}
