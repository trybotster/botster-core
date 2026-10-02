//! `TestkitHarness` (plan 4.2, default tier): the `CoreHarness` over the testkit.
//!
//! Until P1 provides the `HostEngine` there is no Core to open. `open` and `attach_stream` return an error that says so, and the
//! runner reports a transcript that needs them as not passing: an id stays in `conformance/core-pending.txt` until it passes.
//! The harness holds no stand-in for Core (BUILD.md: no hand-written stand-in for a real component).

use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::Value;

/// The default-tier harness for one seed (foundation design 6.1: seeds 0 to 31).
#[derive(Debug)]
pub struct TestkitHarness {
    seed: u64,
}

impl TestkitHarness {
    pub fn new(seed: u64) -> TestkitHarness {
        TestkitHarness { seed }
    }

    /// The seed of this run. `with_seed` of every `Sim` that `open` builds takes it (Core A5-2).
    pub fn seed(&self) -> u64 {
        self.seed
    }

    fn no_core(what: &str) -> CoreError {
        CoreError::new(
            ErrorCode::Internal,
            format!("no Core exists yet: {what} needs the HostEngine of P1, which this testkit milestone does not have"),
        )
    }
}

impl CoreHarness for TestkitHarness {
    fn open(&mut self, _spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        Err(TestkitHarness::no_core("open"))
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

    /// The testkit passes the clock: the `Sim` owns the virtual clock (Core TM-1, plan 2.3 `Clock`).
    fn injects_clock(&self) -> bool {
        true
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
        Err(TestkitHarness::no_core("attach_stream"))
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

    /// Until P1, `open` says that no Core exists. It is an error that is not `unsupported_control`, so the runner reports a
    /// failure with this text and never a pass.
    #[test]
    fn open_says_that_no_core_exists_yet() {
        let error = TestkitHarness::new(0).open(&spec()).err().expect("no Core");
        assert_eq!(error.code, ErrorCode::Internal);
        assert!(
            error.detail.contains("no Core exists yet"),
            "{}",
            error.detail
        );
        assert!(!error.detail.starts_with("unsupported_control:"));
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
        assert!(harness.injects_clock());
        assert_eq!(harness.seed(), 2);
    }
}
