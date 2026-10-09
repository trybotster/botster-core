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
//! The harness builds no control yet: each control gives `unsupported_control`, which is never a pass.

use crate::candidate::{Candidate, PROBE, WORKER};
use crate::harness::{limits_of, no_route};
use botster_core::Core;
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_test_process::Guard;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

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
    /// The guards' sockets, the wrappers and the data directories.
    root: tempfile::TempDir,
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
        let mut harness = RealCoreHarness {
            guards: BTreeMap::new(),
            candidate,
            workers: BTreeMap::new(),
            probe: PathBuf::new(),
            core_send_not_sync: None,
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

    fn harness_failed(error: io::Error) -> CoreError {
        CoreError::new(
            ErrorCode::Internal,
            format!("the real harness could not prepare a wrapper: {error}"),
        )
    }
}

impl CoreHarness for RealCoreHarness {
    /// Opens the real `Core` over a real data directory, with the guarded worker wrapper as its worker path (LC-1, LC-2, 9B).
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
        let core = Core::open(OpenConfig {
            data_dir: PathBuf::from(&spec.data_dir.0),
            worker_path,
            limits,
        })?;
        self.probe = self
            .wrapper(PROBE, grace)
            .map_err(RealCoreHarness::harness_failed)?;
        Ok(Box::new(core))
    }

    /// A real Core follows real time (Core TM-1).
    fn injects_clock(&self) -> bool {
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

    /// The driver drops its `Core`; the workers keep running (Core LC-12) and stay in the guards' groups.
    fn drop_handle(&mut self, _handle: &str) {}

    fn has_control(&self, _op: &str) -> bool {
        false
    }

    /// Core TH-1: the runner's compile-time answer for the facade's `Core` (`with_core_type`); `None` without one.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        self.core_send_not_sync
    }

    fn control(&mut self, _handle: &str, _op: &str, _args: &Value) -> Result<Value, ControlError> {
        Err(ControlError::Unsupported)
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
