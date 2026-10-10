//! The real-clock wait path (design 5.5, 6.1): against a subject that is not a fake, the clock follows real time, a wait stays idle until its
//! valid deadline, and only a runner limit gives `inconclusive`. A mock harness over FakeCore (reporting itself as real) reaches the path
//! without a real Core binary.

use botster_conformance::{run_transcript, Deadline, Limits, Outcome, SeedSet, Transcript};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{
    driver_for, ControlError, CoreHarness, CoreSchemas, DataDirRef, OpenSpec, RouteClient,
    WorkerBuild, WorkerRef, CORE_TRANSCRIPTS,
};
use botster_core_contract::prelude::*;
use serde_json::{json, Value};
use std::time::Duration;

struct NotAFake(FakeCoreHarness);

impl CoreHarness for NotAFake {
    fn open(&mut self, spec: &OpenSpec) -> Result<Box<dyn CoreApi>, CoreError> {
        self.0.open(spec)
    }
    fn data_dir(&mut self, name: &str) -> DataDirRef {
        self.0.data_dir(name)
    }
    fn worker(&self, which: WorkerBuild) -> Option<WorkerRef> {
        self.0.worker(which)
    }
    fn drop_handle(&mut self, handle: &str) {
        self.0.drop_handle(handle)
    }
    fn has_control(&self, op: &str) -> bool {
        self.0.has_control(op)
    }
    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        self.0.control(handle, op, args)
    }
    fn probe_binary(&self) -> String {
        self.0.probe_binary()
    }
    fn attach_stream(
        &mut self,
        handle: &str,
        core: &mut dyn CoreApi,
        client: ClientId,
        session: &SessionId,
        options: AttachOptions,
    ) -> Result<(AttachResult, Box<dyn RouteClient>), CoreError> {
        self.0.attach_stream(handle, core, client, session, options)
    }
}

fn limits() -> Limits {
    // The mock's wake handle never blocks, so a real-clock wait spins until its deadline: the poll limit must not end it first.
    Limits::real()
}

fn outcome(t: &Transcript, limits: &Limits) -> Outcome {
    let make = |seed: u64| driver_for(Box::new(NotAFake(FakeCoreHarness::new(seed))));
    run_transcript(
        t,
        &make,
        &SeedSet::parse("0-1").unwrap(),
        &CoreSchemas,
        limits,
    )
}

#[test]
fn a_wait_that_is_idle_until_its_valid_deadline_passes_on_a_real_clock() {
    let all = botster_conformance::load_dir(&CORE_TRANSCRIPTS).unwrap();
    let t = all
        .iter()
        .find(|t| t.id == "conf::lc_5_stop_ends_payload")
        .unwrap();
    assert_eq!(outcome(t, &limits()), Outcome::Passed);
}

#[test]
fn a_runner_limit_is_inconclusive_not_failed() {
    let t: Transcript = serde_json::from_value(json!({
        "id": "conf::lc_1_open_needs_worker_path", "contract": "core", "clause": "LC-1", "subject": "core",
        "steps": [{ "pump_until": { "event": { "SessionState": { "id": "never" } } }, "because": "Core OR-2" }]
    }))
    .unwrap();
    let short = Limits {
        max_polls: 10_000,
        step_timeout: Some(Duration::from_millis(200)),
    };
    assert!(matches!(outcome(&t, &short), Outcome::Inconclusive { .. }));
    let _ = Deadline::none();
}
