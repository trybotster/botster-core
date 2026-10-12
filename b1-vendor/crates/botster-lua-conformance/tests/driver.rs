//! Runner self-test (design 6.3, 7.2 rule 7). No Lua host exists in Phase 0, so these tests drive the Lua adapter against `FakePlugin`, a frame-level stand-in that runs no Lua, to
//! show two things: the package of a transcript reaches the subject as written (LB-17b), and the quiet-until step that several transcripts rely on FAILS when the subject
//! sends the forbidden frame and PASSES when it does not. Without the second test, a negative assertion could be a blind oracle.

use botster_conformance::{
    parse_transcript, run_transcript, Limits, Outcome, SchemaResolver, SeedSet, StepDriver,
};
use botster_lua_conformance::{LuaAdapter, LuaHostSubject, LuaPackage};
use botster_plugin_conformance::{
    expand::expand, Backpressured, HpDriver, HpSchemas, HpSubject, Unsupported, HP_TRANSCRIPTS,
};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

/// FakePlugin as a Lua host: it records the package that the adapter hands over, and runs the fixture of the transcript.
struct FrameLevelHost {
    inner: Box<dyn HpSubject>,
    seen: Rc<RefCell<Option<LuaPackage>>>,
}

impl HpSubject for FrameLevelHost {
    fn is_fake(&self) -> bool {
        false
    }
    fn has_feature(&self, name: &str) -> bool {
        self.inner.has_feature(name)
    }
    fn has_control(&self, op: &str) -> bool {
        self.inner.has_control(op)
    }
    fn configure(&mut self, setup: &Value) -> Result<(), String> {
        self.inner.configure(setup)
    }
    fn write(&mut self, conn: &str, bytes: &[u8]) -> Result<(), Backpressured> {
        self.inner.write(conn, bytes)
    }
    fn read(&mut self, conn: &str) -> Option<Vec<u8>> {
        self.inner.read(conn)
    }
    fn conns(&self) -> Vec<String> {
        self.inner.conns()
    }
    fn closed(&self, conn: &str) -> Option<String> {
        self.inner.closed(conn)
    }
    fn close_channel(&mut self, conn: &str) {
        self.inner.close_channel(conn);
    }
    fn advance_clock(&mut self, ms: u64) {
        self.inner.advance_clock(ms);
    }
    fn control(&mut self, op: &Value) -> Result<Value, Unsupported> {
        self.inner.control(op)
    }
}

impl LuaHostSubject for FrameLevelHost {
    fn load_package(&mut self, package: &LuaPackage) -> Result<(), String> {
        *self.seen.borrow_mut() = Some(package.clone());
        Ok(())
    }
}

fn transcript(fixture: &str, quiet: Value) -> botster_conformance::Transcript {
    let steps = json!([
        {"send_frame": {"lane": "control", "type": "Hello", "payload": {"$ref": "hello", "epoch": 7}}},
        {"expect": {"lane": "control", "type": "Ready", "payload": {"epoch": 7}}, "because": "HP HS-1"},
        {"send_frame": {"lane": "control", "type": "Activated", "payload": {"epoch": 7}}},
        {"send_frame": {"lane": "invoke", "type": "Invoke", "payload": {"$ref": "invoke", "epoch": 7, "request_id": 5, "handler": "h1"}}},
        quiet,
    ]);
    let doc = json!({"id": "conf::self_test", "contract": "lb", "clause": "LB-1", "subject": "lua_host",
        "requires": {"controls": ["settle"]},
        "setup": {"fixture": fixture, "package": {"manifest": {"name": "demo"}, "files": {"main.lua": "return 1", "a/b.lua": "return 2"}}},
        "steps": steps});
    let mut t = parse_transcript("conf::self_test.json", &doc.to_string()).unwrap();
    let templates: Value = serde_json::from_str(
        HP_TRANSCRIPTS
            .get_file("fixtures/frames.json")
            .unwrap()
            .contents_utf8()
            .unwrap(),
    )
    .unwrap();
    t.steps = t
        .steps
        .iter()
        .map(|s| expand(s, &templates).unwrap())
        .collect();
    t
}

fn quiet_on_hostcall() -> Value {
    json!({"expect_quiet_until": {"lane": "call", "type": "HostCall"}, "barrier": {"control": {"op": "settle"}},
           "until": {"settled": true}, "because": "HP LH-1"})
}

fn run(t: &botster_conformance::Transcript, seen: &Rc<RefCell<Option<LuaPackage>>>) -> Outcome {
    let make = |seed: u64| -> Box<dyn StepDriver> {
        let inner = botster_plugin_conformance::fake::host(seed);
        let host = FrameLevelHost {
            inner,
            seen: seen.clone(),
        };
        Box::new(HpDriver::new(
            Box::new(LuaAdapter(Box::new(host))),
            &HP_TRANSCRIPTS,
        ))
    };
    let schemas: &dyn SchemaResolver = &HpSchemas::new();
    run_transcript(
        t,
        &make,
        &SeedSet { seeds: vec![0] },
        schemas,
        &Limits::default(),
    )
}

// LB-17b: the files of `setup.package` reach the subject unchanged, each by its path relative to the package root.
#[test]
fn the_package_reaches_the_subject_as_written() {
    let seen = Rc::new(RefCell::new(None));
    let t = transcript("host_echo", quiet_on_hostcall());
    assert_eq!(run(&t, &seen), Outcome::Passed);
    let pkg = seen.borrow().clone().expect("load_package was called");
    assert_eq!(
        pkg.files.get("a/b.lua").map(String::as_str),
        Some("return 2")
    );
    assert_eq!(pkg.files.len(), 2);
    assert_eq!(pkg.manifest["name"], "demo");
}

// The quiet step passes when the subject sends no HostCall (host_echo returns at once) ...
#[test]
fn quiet_until_passes_without_a_host_call() {
    let t = transcript("host_echo", quiet_on_hostcall());
    assert_eq!(run(&t, &Rc::new(RefCell::new(None))), Outcome::Passed);
}

// ... and FAILS at that step when the subject does send one (host_calls makes a HostCall in handler h1), so the negative assertions of the suite are not blind.
#[test]
fn quiet_until_fails_when_the_subject_sends_a_host_call() {
    let t = transcript("host_calls", quiet_on_hostcall());
    match run(&t, &Rc::new(RefCell::new(None))) {
        Outcome::Failed { step, .. } => assert_eq!(step, 4, "fails at the quiet step"),
        other => panic!("expected a failure at the quiet step, got {other:?}"),
    }
}
