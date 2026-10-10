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
//! Every other control gives `unsupported_control`, which is never a pass.

use crate::candidate::{Candidate, PROBE, WORKER};
use crate::controls::parse;
use crate::edge_tap::{EdgeTap, Rows, Tap};
use crate::harness::{limits_of, no_route};
use botster_core::RealEdges;
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_core_edges::edges::ProcessIdentity;
use botster_core_host::driver::HostDriver;
use botster_test_process::Guard;
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError, Weak};
use std::time::Duration;

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
        harness.probe = harness.wrapper(PROBE, CoreLimits::default().stop_grace)?;
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

    /// The wrapper `<root>/w/<grace>/<file_name>` of the guard of `grace`: it execs the verified binary of `file_name`'s
    /// kind (the probe for `PROBE`, else the worker) through that guard's anchor.
    fn wrapper(&mut self, file_name: &str, grace: Duration) -> io::Result<PathBuf> {
        let program = if file_name == PROBE {
            self.candidate.probe.clone()
        } else {
            self.candidate.worker.clone()
        };
        let dir = self
            .root
            .path()
            .join("w")
            .join(grace.as_nanos().to_string());
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
        let path = self.wrapper(file_name, grace)?;
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
            .wrapper(PROBE, grace)
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
        self.tapped && matches!(op, "edges_quiet" | "break_control")
    }

    /// Core TH-1: the runner's compile-time answer for the facade's `Core` (`with_core_type`); `None` without one.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        self.core_send_not_sync
    }

    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        match op {
            "edges_quiet" if self.tapped => self.edges_quiet(handle, args),
            "break_control" if self.tapped => self.break_control(handle, args),
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

#[cfg(test)]
mod slow_tests {
    use super::*;

    /// R-46: the runner's clock is the one Core sees, but real processes make progress, so the driver must not jump it. The
    /// harness starts no process, so binaries that do not exist are enough.
    #[test]
    fn the_real_harness_injects_the_clock_but_not_its_progress() {
        let harness = RealCoreHarness::new(Candidate {
            worker: PathBuf::from("/nonexistent/worker"),
            probe: PathBuf::from("/nonexistent/probe"),
            anchor: PathBuf::from("/nonexistent/anchor"),
        })
        .unwrap();
        assert!(harness.injects_clock());
        assert!(!harness.progress_is_injected());
    }

    /// A failed trial prints the harness, so its debug form names the binaries under test and the root directory.
    #[test]
    fn the_debug_form_names_the_candidate_and_the_root() {
        let harness = RealCoreHarness::new(Candidate {
            worker: PathBuf::from("/nonexistent/worker"),
            probe: PathBuf::from("/nonexistent/probe"),
            anchor: PathBuf::from("/nonexistent/anchor"),
        })
        .unwrap();
        let debug = format!("{harness:?}");
        assert!(debug.starts_with("RealCoreHarness {"), "{debug}");
        assert!(debug.contains("/nonexistent/worker"), "{debug}");
        assert!(
            debug.contains(&format!("{:?}", harness.root.path())),
            "{debug}"
        );
    }
}
