//! `TestkitHarness` (plan 4.2, default tier): the `CoreHarness` over the testkit.
//!
//! `open` builds the real `Core`: the host driver of `botster-core-host` over in-memory edges (`crate::core`). The harness holds
//! no stand-in for Core (BUILD.md: no hand-written stand-in for a real component). A transcript that needs a control or an
//! edge that does not exist yet gets `unsupported_control`, which is never a pass; an id stays in `conformance/core-pending.txt`
//! until it passes.

use crate::core::{core_features, Directories, RunInputs};
use crate::refusal::{RefusalHandle, RefusalLayer, ScriptError};
use crate::scheduler::SchedulerHandle;
use crate::worker::{TestkitCore, Workers};
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

/// The default-tier harness for one seed (foundation design 6.1: seeds 0 to 31).
#[derive(Debug)]
pub struct TestkitHarness {
    seed: u64,
    start: Instant,
    directories: Directories,
    /// Every in-process worker of the run, in one `Sim` with the run's seeded stream (plan 4.1, A5-2).
    workers: Workers,
    /// The scripted synchronous refusals of each handle (plan 4.2a). The harness arms them; the layer in front of the handle's
    /// Core consumes them.
    refusals: BTreeMap<String, RefusalHandle>,
}

impl TestkitHarness {
    pub fn new(seed: u64) -> TestkitHarness {
        let start = Instant::now();
        TestkitHarness {
            seed,
            start,
            directories: Directories::default(),
            workers: Workers::new(SchedulerHandle::with_seed(seed), start),
            refusals: BTreeMap::new(),
        }
    }

    /// The seed of this run. `with_seed` of every `Sim` that `open` builds takes it (Core A5-2).
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Puts the refusal layer of the handle in front of its Core. `open` calls this with the Core that it built, so every Core
    /// that a transcript sees can be scripted (Core A5-3 timing 1).
    pub fn with_refusals(&mut self, handle: &str, core: Box<dyn CoreApi>) -> Box<dyn CoreApi> {
        let script = self.refusals.entry(handle.to_string()).or_default().clone();
        Box::new(RefusalLayer::new(core, script))
    }

    /// The control `fail_next`: the next call of `target` (an operation kind or a call name) is refused with `error` before it
    /// reaches Core. `occurrence` counts from 1 (default 1). A code that is not in the call's sync column is refused with the
    /// typed `ControlError::Refused` (Core A5-3).
    fn fail_next(&mut self, handle: &str, args: &Value) -> Result<Value, ControlError> {
        let bad = |why: String| ControlError::Bad(why);
        let target = args
            .get("target")
            .and_then(Value::as_str)
            .ok_or_else(|| bad("needs 'target': the call".into()))?;
        let error = args
            .get("error")
            .ok_or_else(|| bad("needs 'error'".into()))?;
        let occurrence = match args.get("occurrence") {
            None => 1,
            Some(n) => n
                .as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| bad("'occurrence' is a number".into()))?,
        };
        let script = self.refusals.entry(handle.to_string()).or_default();
        match script.arm(target, occurrence, error) {
            Ok(()) => Ok(Value::Null),
            Err(ScriptError::NotInSyncColumn { call, code }) => Err(ControlError::Refused(
                json!({"call": call, "code": code, "reason": "not_in_sync_column"}),
            )),
            Err(other) => Err(bad(format!("{other:?}"))),
        }
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
        let opened = self.directories.open(
            &spec.data_dir.0,
            &config,
            RunInputs {
                seed: self.seed,
                scheduler: self.workers.scheduler(),
                start: self.start,
            },
            core_features(),
            Some(Box::new(self.workers.spawner())),
        )?;
        let core = Box::new(TestkitCore::new(
            opened.driver,
            opened.wake,
            self.workers.clone(),
        ));
        Ok(self.with_refusals(&spec.handle, core))
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

    /// The controls that the testkit builds (design 6.3, `docs/core-testkit-controls.md`). The others come with the machines
    /// and edges that they need.
    fn has_control(&self, op: &str) -> bool {
        op == "fail_next"
    }

    /// Core TH-1 has no concrete Core type to ask yet.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        None
    }

    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        match op {
            "fail_next" => self.fail_next(handle, args),
            _ => Err(ControlError::Unsupported),
        }
    }

    /// The statement steps that need no Core: `check_crates` (Core A5-1). The others wait for a Core and the real harness.
    fn statement(&mut self, kind: &str, spec: &Value) -> Result<Value, ControlError> {
        match kind {
            "check_crates" => {
                crate::statements::check_crates(crate::statements::workspace_root(), spec)
            }
            _ => Err(ControlError::Unsupported),
        }
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
        assert_eq!(
            harness.worker_named("renamed-worker"),
            Some(WorkerRef {
                build: WorkerBuild::Current,
                file_name: Some("renamed-worker".into()),
            })
        );
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

    /// `fail_next` arms a refusal in the handle's script, and a code outside the sync column is refused with the typed value
    /// that `expect_refused` matches (Core A5-3).
    #[test]
    fn fail_next_arms_the_script_and_refuses_a_code_outside_the_sync_column() {
        let mut harness = TestkitHarness::new(0);
        assert!(harness.has_control("fail_next"));
        let armed = harness.control(
            "h",
            "fail_next",
            &json!({"target": "Create", "error": "SessionLimit"}),
        );
        assert_eq!(armed, Ok(Value::Null));
        assert!(!harness.refusals["h"].is_empty());
        let refused = harness.control(
            "h",
            "fail_next",
            &json!({"target": "Start", "error": {"StartFailed": {"ExecFailed": {"errno": 2}}}}),
        );
        assert_eq!(
            refused,
            Err(ControlError::Refused(
                json!({"call": "Start", "code": "StartFailed", "reason": "not_in_sync_column"})
            ))
        );
        // The refused entry is not armed, and another handle has its own script.
        assert!(harness.refusals["h"].take("Start").is_none());
        assert!(!harness.refusals.contains_key("other"));
    }

    #[test]
    fn fail_next_reports_bad_arguments() {
        let mut harness = TestkitHarness::new(0);
        for args in [
            json!({"error": "SessionLimit"}),
            json!({"target": "Create"}),
            json!({"target": "Create", "error": "SessionLimit", "occurrence": "x"}),
            json!({"target": "Nope", "error": "SessionLimit"}),
            json!({"target": "Create", "error": "SessionLimit", "occurrence": 0}),
        ] {
            assert!(
                matches!(
                    harness.control("h", "fail_next", &args),
                    Err(ControlError::Bad(_))
                ),
                "{args}"
            );
        }
        assert_eq!(
            harness.control("h", "lose_worker", &json!({})),
            Err(ControlError::Unsupported)
        );
        let nth = harness.control(
            "h",
            "fail_next",
            &json!({"target": "Create", "error": "SessionLimit", "occurrence": 3}),
        );
        assert_eq!(nth, Ok(Value::Null));
    }

    /// `check_crates` is answered from the workspace: this testkit is in no facade, and no listed crate depends on it. Another
    /// statement is not answered yet, and that is never a pass.
    #[test]
    fn the_harness_answers_check_crates_and_nothing_else_yet() {
        let mut harness = TestkitHarness::new(0);
        let spec = json!({"crate": "botster-core-testkit", "not_in": ["botster_core::prelude"], "no_dependency_of": ["botster-core-ffi"]});
        assert_eq!(
            harness.statement("check_crates", &spec),
            Ok(json!({"found_in": [], "depended_on_by": []}))
        );
        let missing = harness.statement("check_crates", &json!({}));
        assert!(matches!(missing, Err(ControlError::Bad(_))));
        for kind in [
            "run_suite",
            "error_codes_reachable",
            "run_deterministic",
            "other",
        ] {
            assert_eq!(
                harness.statement(kind, &spec),
                Err(ControlError::Unsupported),
                "{kind}"
            );
        }
    }
}
