//! `TestkitHarness` (plan 4.2, default tier): the `CoreHarness` over the testkit.
//!
//! `open` builds the real `Core`: the host driver of `botster-core-host` over in-memory edges (`crate::core`). The harness holds
//! no stand-in for Core (BUILD.md: no hand-written stand-in for a real component). A transcript that needs a control or an
//! edge that does not exist yet gets `unsupported_control`, which is never a pass; an id stays in `conformance/core-pending.txt`
//! until it passes.

use crate::core::{core_features, Directories};
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::Value;
use std::time::Instant;

/// The default-tier harness for one seed (foundation design 6.1: seeds 0 to 31).
#[derive(Debug)]
pub struct TestkitHarness {
    seed: u64,
    start: Instant,
    directories: Directories,
}

impl TestkitHarness {
    pub fn new(seed: u64) -> TestkitHarness {
        TestkitHarness {
            seed,
            start: Instant::now(),
            directories: Directories::default(),
        }
    }

    /// The seed of this run. `with_seed` of every `Sim` that `open` builds takes it (Core A5-2).
    pub fn seed(&self) -> u64 {
        self.seed
    }

    fn no_route(what: &str) -> CoreError {
        CoreError::new(
            ErrorCode::Unsupported { what: None },
            format!("unsupported_control: {what} needs the route data plane (P4a)"),
        )
    }
}

impl CoreHarness for TestkitHarness {
    /// Builds the real `Core` over an in-memory data directory (LC-1, LC-2, 9B).
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        let limits: CoreLimits = serde_json::from_value(spec.limits.clone()).map_err(|error| {
            CoreError::new(
                ErrorCode::InvalidConfig {
                    field: "limits".to_string(),
                },
                format!("the limits are not CoreLimits: {error}"),
            )
        })?;
        let config = OpenConfig {
            data_dir: spec.data_dir.0.clone().into(),
            worker_path: spec.worker.as_ref().map(|w| {
                w.file_name
                    .clone()
                    .unwrap_or_else(|| "botster-worker".to_string())
                    .into()
            }),
            limits,
        };
        let (driver, _faults) = self.directories.open(
            &spec.data_dir.0,
            &config,
            self.seed,
            self.start,
            core_features(),
            None,
        )?;
        Ok(Box::new(driver))
    }

    /// The harness passes the clock, so `advance_clock` moves it (Core TM-1, A5-1).
    fn injects_clock(&self) -> bool {
        true
    }

    /// A data directory is a name here: the in-memory registry of a `Sim` is keyed by it, and survives a drop and a reopen
    /// (Core LC-12, AD-1) once P1 provides the `Storage` user.
    fn data_dir(&mut self, name: &str) -> DataDirRef {
        DataDirRef(name.to_string())
    }

    /// `Current` is the in-process `Worker`. There is no `Previous` while the worker protocol is 1 (Core A6-2: no old
    /// protocol is fabricated).
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        (which == WorkerBuild::Current).then_some(WorkerRef {
            build: which,
            file_name: None,
        })
    }

    /// The in-process worker has no file; Core receives a path and fixes no name for it (Core E1-1).
    fn worker_named(&self, file_name: &str) -> Option<WorkerRef> {
        Some(WorkerRef {
            build: WorkerBuild::Current,
            file_name: Some(file_name.to_string()),
        })
    }

    /// No handle exists, so there is nothing to drop (Core LC-12 is proven once `open` returns a Core).
    fn drop_handle(&mut self, _handle: &str) {}

    /// No control is built yet: the controls come from the testkit edges, with their packages (design 6.3).
    fn has_control(&self, _op: &str) -> bool {
        false
    }

    /// Core TH-1 has no concrete Core type to ask yet.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        None
    }

    fn control(&mut self, _handle: &str, _op: &str, _args: &Value) -> Result<Value, ControlError> {
        Err(ControlError::Unsupported)
    }

    /// `argv[0]` of a session program: the probe binary. The scripted program edge interprets the script of `argv[1]`
    /// in-process (Core A5-1).
    fn probe_binary(&self) -> String {
        botster_probe_script::PROBE_BINARY.to_string()
    }

    fn attach_stream(
        &mut self,
        _handle: &str,
        _core: &mut dyn CoreApi,
        _client: ClientId,
        _session: &SessionId,
        _options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError> {
        Err(TestkitHarness::no_route("attach_stream"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn spec() -> OpenSpec {
        OpenSpec {
            handle: "h".into(),
            data_dir: DataDirRef("d".into()),
            worker: Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: None,
            }),
            limits: json!({}),
        }
    }

    /// Core LC-1, LC-2, 9B: `open` builds the real Core, refuses a second open of the directory while the first lives, and a
    /// reopen after the drop works.
    #[test]
    fn open_builds_a_core_and_the_directory_is_exclusive_until_the_drop() {
        let mut harness = TestkitHarness::new(0);
        let first = harness.open(&spec()).expect("a Core");
        assert_eq!(first.list(), vec![]);
        let second = harness.open(&spec()).err().expect("the directory is held");
        assert_eq!(second.code, ErrorCode::DataDirInUse);
        drop(first);
        assert!(harness.open(&spec()).is_ok());
    }

    /// Core LC-1, 9B: no worker path is `MissingWorkerPath`, and a zero limit is `InvalidConfig`.
    #[test]
    fn open_checks_the_worker_path_and_the_limits() {
        let mut harness = TestkitHarness::new(0);
        let mut no_worker = spec();
        no_worker.worker = None;
        assert_eq!(
            harness.open(&no_worker).err().expect("refused").code,
            ErrorCode::MissingWorkerPath
        );
        let mut zero = spec();
        zero.limits = json!({"max_sessions": 0});
        assert!(matches!(
            harness.open(&zero).err().expect("refused").code,
            ErrorCode::InvalidConfig { .. }
        ));
    }

    /// Core A6-2: no previous worker is fabricated while the protocol is 1.
    #[test]
    fn only_the_current_worker_is_offered() {
        let harness = TestkitHarness::new(1);
        assert_eq!(
            harness.worker(WorkerBuild::Current),
            Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: None
            })
        );
        assert_eq!(harness.worker(WorkerBuild::Previous), None);
    }

    #[test]
    fn the_harness_names_the_probe_binary_and_holds_no_control() {
        let mut harness = TestkitHarness::new(2);
        assert_eq!(harness.probe_binary(), "botster-conformance-probe");
        assert_eq!(harness.data_dir("a"), DataDirRef("a".into()));
        assert!(!harness.has_control("kill_worker"));
        assert_eq!(
            harness.control("h", "kill_worker", &json!({})),
            Err(ControlError::Unsupported)
        );
        assert_eq!(harness.core_is_send_not_sync(), None);
        assert!(!harness.is_fake());
        assert_eq!(harness.seed(), 2);
    }
}
