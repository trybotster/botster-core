//! `TestkitHarness` (plan 4.2, default tier): the `CoreHarness` over the testkit.
//!
//! Until P1 provides the `HostEngine` there is no Core to open. `open` and `attach_stream` return an error that says so, and the
//! runner reports a transcript that needs them as not passing: an id stays in `conformance/core-pending.txt` until it passes.
//! The harness holds no stand-in for Core (BUILD.md: no hand-written stand-in for a real component).

use crate::refusal::{RefusalHandle, RefusalLayer, ScriptError};
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use serde_json::{json, Value};
use std::collections::BTreeMap;

/// The default-tier harness for one seed (foundation design 6.1: seeds 0 to 31).
#[derive(Debug)]
pub struct TestkitHarness {
    seed: u64,
    /// The scripted synchronous refusals of each handle (plan 4.2a). The harness arms them; the layer in front of the handle's
    /// Core consumes them.
    refusals: BTreeMap<String, RefusalHandle>,
}

impl TestkitHarness {
    pub fn new(seed: u64) -> TestkitHarness {
        TestkitHarness {
            seed,
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
}
