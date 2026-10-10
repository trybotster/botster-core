//! The runner gives a control the values that earlier steps bound, and a testkit that injects the clock gets `advance_clock` (Core TM-1).

use botster_conformance::{
    run_transcript, Bindings, Deadline, Limits, Outcome, SeedSet, StepDriver, Transcript,
};
use botster_core_conformance::fake::FakeCoreHarness;
use botster_core_conformance::{
    driver_for, ControlError, CoreDriver, CoreHarness, CoreSchemas, DataDirRef, OpenSpec,
    RouteClient, WorkerBuild, WorkerRef, CORE_TRANSCRIPTS,
};
use botster_core_contract::prelude::*;
use serde_json::{json, Value};

/// FakeCore under the name of a testkit: not a fake, with an injected clock, and a control `echo` that returns its arguments.
struct Testkit(FakeCoreHarness);

impl CoreHarness for Testkit {
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
    fn injects_clock(&self) -> bool {
        true
    }
    fn has_control(&self, op: &str) -> bool {
        op == "echo" || self.0.has_control(op)
    }
    fn control(&mut self, handle: &str, op: &str, args: &Value) -> Result<Value, ControlError> {
        if op == "echo" {
            return Ok(args.clone());
        }
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

#[test]
fn a_control_receives_the_values_that_earlier_steps_bound() {
    let mut driver = CoreDriver::new(Box::new(Testkit(FakeCoreHarness::new(0))));
    let mut bindings = Bindings::new();
    let mut run = |step: Value| {
        driver
            .exec(&step, &mut bindings, &Deadline::none())
            .unwrap_or_else(|e| panic!("{step}: {e:?}"))
    };
    run(json!({"let": {"$n": {"add": [3, 4]}}}));
    // The matcher sees the substituted value: 7, not the text "$n".
    run(
        json!({"control": {"op": "echo", "n": "$n", "nested": {"list": ["$n"]}}, "expect": {"n": 7, "nested": {"list": [7]}}}),
    );
    // A name that no step bound is a step error, never a literal handed to the harness.
    let step = json!({"control": {"op": "echo", "n": "$missing"}});
    assert!(driver
        .exec(&step, &mut bindings, &Deadline::none())
        .is_err());
}

/// R-46: by default a harness's progress is injected exactly when its clock is. The fake and a testkit keep the idle-wait jump; a
/// harness with real progress overrides the answer.
#[test]
fn the_progress_of_a_harness_is_injected_by_default_when_its_clock_is() {
    let fake = FakeCoreHarness::new(0);
    assert!(fake.injects_clock() && fake.progress_is_injected());
    assert!(Testkit(FakeCoreHarness::new(0)).progress_is_injected());
}

#[test]
fn a_harness_that_injects_the_clock_gets_advance_clock_though_it_is_not_a_fake() {
    let all = botster_conformance::load_dir(&CORE_TRANSCRIPTS).unwrap();
    // Stop with a grace of 100 ms ends only when the injected clock moves past it.
    let t: &Transcript = all
        .iter()
        .find(|t| t.id == "conf::lc_5_stop_ends_payload")
        .unwrap();
    let make = |seed: u64| driver_for(Box::new(Testkit(FakeCoreHarness::new(seed))));
    let limits = Limits {
        max_polls: 10_000,
        step_timeout: None,
    };
    let seeds = SeedSet::parse("0-1").unwrap();
    assert_eq!(
        run_transcript(t, &make, &seeds, &CoreSchemas, &limits),
        Outcome::Passed
    );
}
