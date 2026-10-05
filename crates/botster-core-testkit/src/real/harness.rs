//! `RealCoreHarness` (plan 4.2, the real-process tier): the `CoreHarness` over the real `Core::open`, real `botster-worker`
//! processes and the real `botster-conformance-probe`, all prebuilt and verified against the candidate manifest.
//!
//! Lead ruling (2026-10-04): the harness owns the processes through public launch inputs only. `OpenConfig.worker_path` and
//! `argv[0]` of every session program are guarded wrappers (`botster-test-anchor`, see [`crate::anchor`]) that exec the
//! verified binaries. Core and the worker run unchanged, with no injection seam and no test branch. The harness's
//! [`AnchorGuard`] ends every group on every exit path of the test, its death included.
//!
//! The harness builds no control yet: each control gives `unsupported_control`, which is never a pass, and every id stays in
//! `conformance/core-pending.txt` until it passes on both harnesses (plan 4.2b).

use super::guard::{AnchorGuard, Finished};
use crate::anchor::Report;
use crate::candidate::{Candidate, PROBE, WORKER};
use crate::harness::{limits_of, no_route};
use botster_core::Core;
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::Value;
use std::collections::BTreeMap;
use std::io;
use std::path::PathBuf;
use std::time::Duration;

/// The real-tier harness. A real implementation ignores the seed (plan 4.2c).
#[derive(Debug)]
pub struct RealCoreHarness {
    candidate: Candidate,
    /// The worker wrapper of each file name and grace.
    workers: BTreeMap<(String, Duration), PathBuf>,
    /// The program wrapper that `probe_binary` names: the one of the latest `open`, with that open's `stop_grace`.
    probe: PathBuf,
    guard: AnchorGuard,
}

impl RealCoreHarness {
    /// A harness over the verified binaries of `candidate`. Before the first `open`, a session program gets the default
    /// `stop_grace` of the Core limits.
    pub fn new(candidate: Candidate) -> io::Result<RealCoreHarness> {
        let mut guard = AnchorGuard::new(&candidate.anchor)?;
        let probe = guard.wrapper(PROBE, &candidate.probe, CoreLimits::default().stop_grace)?;
        Ok(RealCoreHarness {
            candidate,
            workers: BTreeMap::new(),
            probe,
            guard,
        })
    }

    /// The groups that the harness's anchors hold: one for each worker and one for each session program that started.
    pub fn anchors(&mut self) -> io::Result<Vec<Report>> {
        self.guard.reports()
    }

    /// Ends every group now and returns what each anchor did. The drop does the same.
    pub fn finish(&mut self) -> io::Result<Vec<Finished>> {
        self.guard.finish()
    }

    fn worker_wrapper(&mut self, file_name: &str, grace: Duration) -> io::Result<PathBuf> {
        let key = (file_name.to_string(), grace);
        if let Some(path) = self.workers.get(&key) {
            return Ok(path.clone());
        }
        let path = self
            .guard
            .wrapper(file_name, &self.candidate.worker, grace)?;
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
            .guard
            .wrapper(PROBE, &self.candidate.probe, grace)
            .map_err(RealCoreHarness::harness_failed)?;
        Ok(Box::new(core))
    }

    /// A real Core follows real time (Core TM-1).
    fn injects_clock(&self) -> bool {
        false
    }

    /// A directory under the harness's root, kept across drop and reopen (Core LC-12, AD-1). `Core::open` creates it.
    fn data_dir(&mut self, name: &str) -> DataDirRef {
        DataDirRef(self.guard.root().join("d").join(name).display().to_string())
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

    /// The driver drops its `Core`; the workers keep running (Core LC-12) and stay in the guard's groups.
    fn drop_handle(&mut self, _handle: &str) {}

    fn has_control(&self, _op: &str) -> bool {
        false
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
