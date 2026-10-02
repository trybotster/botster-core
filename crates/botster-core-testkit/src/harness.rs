//! `TestkitHarness` (plan 4.2, default tier): the `CoreHarness` over the testkit.
//!
//! `open` builds the real `Core`: the host driver of `botster-core-host` over in-memory edges (`crate::core`). The harness holds
//! no stand-in for Core (BUILD.md: no hand-written stand-in for a real component). A transcript that needs a control or an
//! edge that does not exist yet gets `unsupported_control`, which is never a pass; an id stays in `conformance/core-pending.txt`
//! until it passes.

use crate::core::{core_features, Directories, RunInputs};
use crate::oracle::Oracle;
use crate::refusal::{RefusalHandle, RefusalLayer, ScriptError};
use crate::scheduler::SchedulerHandle;
use crate::worker::{TestkitCore, Workers};
use botster_core_conformance::{
    ControlError, CoreHarness, DataDirRef, OpenSpec, RouteClient, WorkerBuild, WorkerRef,
};
use botster_core_contract::prelude::*;
use botster_route_codec::prelude::{hex_decode, HexBytes};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::time::Instant;

/// The controls of the program edge that the harness dispatches to a session's payload.
const PROGRAM_CONTROLS: &[&str] = &[
    "pty_input",
    "pty_output",
    "pty_output_unread",
    "pty_blocked",
    "pty_chunk",
    "pty_accept",
    "pty_fail_after",
    "program_write_size",
    "program_write_once",
];

/// The terminal oracles (R-7): an independent libghostty terminal replays what reached the session's PTY.
const ORACLE_CONTROLS: &[&str] = &[
    "oracle_modes",
    "oracle_state",
    "oracle_cursor",
    "oracle_screen",
    "oracle_notification",
    "oracle_encode",
];

/// The controls of the process edge that the harness dispatches to a session's worker.
const WORKER_CONTROLS: &[&str] = &["process_end_worker", "lose_worker", "break_control"];

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

    /// The program controls (`docs/core-testkit-controls.md` and FakeCore's list): they act on the program edge of the
    /// session's payload, through its `ProgramControl` (Core A5-1, A5-2, A5-3).
    fn program_control(
        &mut self,
        handle: &str,
        op: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        let bad = |why: String| ControlError::Bad(why);
        let session = args
            .get("session")
            .and_then(Value::as_str)
            .map(|s| SessionId(s.to_string()))
            .ok_or_else(|| bad(format!("{op} needs 'session'")))?;
        let control = self
            .workers
            .program_control(handle, &session)
            .ok_or_else(|| bad(format!("{op}: session {} has no payload", session.0)))?;
        let count = |name: &str| {
            args.get(name)
                .and_then(Value::as_u64)
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| bad(format!("{op} needs '{name}', a count")))
        };
        let hex = |name: &str| {
            args.get(name)
                .and_then(Value::as_str)
                .and_then(|text| hex_decode(text).ok())
                .ok_or_else(|| bad(format!("{op} needs '{name}', hex bytes")))
        };
        match op {
            "pty_input" => {
                return Ok(
                    json!({ "bytes": { "$bytes_hex": HexBytes(control.input_log()).to_hex() } }),
                )
            }
            "pty_output_unread" => return Ok(json!({ "bytes": control.output_unread() })),
            // `scripted` holds facts for a fake that has no terminal; the real model derives them (R-7), so it is not read.
            "pty_output" => control.write_plain(&hex("bytes_hex")?),
            "program_write_once" => control.write_once(&hex("bytes_hex")?),
            "pty_blocked" => {
                control.set_blocked(args.get("on").and_then(Value::as_bool).unwrap_or(true))
            }
            "pty_chunk" => control.input_chunk(Some(count("bytes")?)),
            "pty_accept" => control.accept_at_most(count("bytes")?),
            "pty_fail_after" => control.fail_after(count("bytes")?),
            "program_write_size" => control.write_size(Some(count("bytes")?)),
            _ => return Err(ControlError::Unsupported),
        }
        Ok(Value::Null)
    }

    /// The process controls of a session's worker (Core A5-1, A5-3): `process_end_worker` and FakeCore's `lose_worker` (reason
    /// `worker_gone`, its default) end the worker process at this point; `break_control` breaks its control link while it
    /// lives.
    fn worker_control(
        &mut self,
        handle: &str,
        op: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        let session = args
            .get("session")
            .and_then(Value::as_str)
            .map(|s| SessionId(s.to_string()))
            .ok_or_else(|| ControlError::Bad(format!("{op} needs 'session'")))?;
        let done = match (op, args.get("reason").and_then(Value::as_str)) {
            ("break_control", _) => self.workers.break_link(handle, &session),
            // FakeCore's default reason: the worker process ends (`Lost(WorkerGone)`).
            ("process_end_worker", _) | ("lose_worker", None | Some("worker_gone")) => {
                self.workers.end_worker(handle, &session)
            }
            // A live worker whose link is withheld (`Lost(WorkerUnreachable)`) is the process edge's `withhold_control_link`
            // (P6); ending the worker would give the other state, so it is not offered here.
            ("lose_worker", Some("worker_unreachable")) => return Err(ControlError::Unsupported),
            (_, Some(other)) => {
                return Err(ControlError::Bad(format!("{op}: unknown reason {other}")))
            }
            _ => return Err(ControlError::Unsupported),
        };
        if done {
            Ok(Value::Null)
        } else {
            Err(ControlError::Bad(format!(
                "{op}: session {} has no worker",
                session.0
            )))
        }
    }

    /// The terminal oracles of a session (R-7): a fresh libghostty terminal replays the session's PTY log and answers.
    fn oracle_control(
        &mut self,
        handle: &str,
        op: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        let session = args
            .get("session")
            .and_then(Value::as_str)
            .map(|s| SessionId(s.to_string()))
            .ok_or_else(|| ControlError::Bad(format!("{op} needs 'session'")))?;
        let control = self
            .workers
            .program_control(handle, &session)
            .ok_or_else(|| {
                ControlError::Bad(format!("{op}: session {} has no payload", session.0))
            })?;
        let oracle = Oracle::replay(&control.model_log())?;
        match op {
            "oracle_modes" => Ok(oracle.modes()),
            "oracle_state" => Ok(oracle.state()),
            "oracle_cursor" => Ok(oracle.cursor()),
            "oracle_screen" => oracle.screen(
                args.get("history")
                    .and_then(Value::as_bool)
                    .unwrap_or(false),
            ),
            "oracle_notification" => oracle.notification(),
            "oracle_encode" => {
                let kind = args
                    .get("kind")
                    .and_then(Value::as_str)
                    .ok_or_else(|| ControlError::Bad("oracle_encode needs 'kind'".into()))?;
                let input = args
                    .get("input")
                    .ok_or_else(|| ControlError::Bad("oracle_encode needs 'input'".into()))?;
                oracle.encode(kind, input, args.get("modes"))
            }
            _ => Err(ControlError::Unsupported),
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
            Some(Box::new(self.workers.spawner(&spec.data_dir.0))),
        )?;
        let core = Box::new(TestkitCore::new(
            opened.driver,
            opened.wake,
            self.workers.clone(),
            &spec.handle,
            &spec.data_dir.0,
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
            || PROGRAM_CONTROLS.contains(&op)
            || WORKER_CONTROLS.contains(&op)
            || ORACLE_CONTROLS.contains(&op)
    }

    /// Core TH-1 has no concrete Core type to ask yet.
    fn core_is_send_not_sync(&self) -> Option<bool> {
        None
    }

    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        match op {
            "fail_next" => self.fail_next(handle, args),
            op if PROGRAM_CONTROLS.contains(&op) => self.program_control(handle, op, args),
            op if WORKER_CONTROLS.contains(&op) => self.worker_control(handle, op, args),
            op if ORACLE_CONTROLS.contains(&op) => self.oracle_control(handle, op, args),
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
            harness.control("h", "descendants", &json!({})),
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

    /// Starts session `s1` on `core` (Create, then Start), pumping until the Start completes. The program holds.
    fn start_s1(core: &mut dyn CoreApi) {
        let request = SpawnRequest {
            argv: vec![
                botster_probe_script::PROBE_BINARY.to_string(),
                r#"{"program":[{"hold":{}}]}"#.to_string(),
            ],
            env: std::collections::BTreeMap::new(),
            cwd: "/".into(),
            size: Size {
                rows: 24,
                cols: 80,
                cell_px: None,
            },
            labels: std::collections::BTreeMap::new(),
            color_profile: None,
            notification_policy: None,
            size_policy: None,
        };
        let s1 = SessionId("s1".into());
        core.begin(Op::Create {
            session: s1.clone(),
            request,
        })
        .unwrap();
        let start = std::time::Instant::now();
        let now = |i: u64| Now {
            monotonic: start + std::time::Duration::from_millis(i),
            unix: 1,
        };
        let mut started = None;
        for i in 0..200 {
            core.pump(now(i));
            for event in core.poll_events(64) {
                if matches!(event, Event::Completed { op, .. } if Some(op) == started) {
                    return;
                }
                if matches!(&event, Event::Completed { .. }) && started.is_none() {
                    started = Some(core.begin(Op::Start { id: s1.clone() }).unwrap());
                }
            }
        }
        panic!("s1 did not start");
    }

    /// F9: two data directories each mint the instance `1-1`; a program control on one handle reaches that handle's payload
    /// only.
    #[test]
    fn program_controls_reach_the_handle_of_their_own_directory() {
        let mut harness = TestkitHarness::new(0);
        let mut a = harness.open(&spec()).expect("a Core");
        let mut b = harness
            .open(&OpenSpec {
                handle: "h2".into(),
                data_dir: DataDirRef("d2".into()),
                ..spec()
            })
            .expect("a second Core");
        start_s1(a.as_mut());
        start_s1(b.as_mut());
        harness
            .control(
                "h",
                "pty_output",
                &json!({"session": "s1", "bytes_hex": "6162"}),
            )
            .unwrap();
        let unread = |harness: &mut TestkitHarness, handle| {
            harness
                .control(handle, "pty_output_unread", &json!({"session": "s1"}))
                .unwrap()["bytes"]
                .as_u64()
                .unwrap()
        };
        assert_eq!(unread(&mut harness, "h"), 2);
        assert_eq!(
            unread(&mut harness, "h2"),
            0,
            "the other directory's payload is untouched"
        );
    }
}
