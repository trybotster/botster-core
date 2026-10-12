//! The adapter and the Phase 0 subject do exactly what their doc comments say: every `HpSubject` call reaches the Lua host unchanged, and the structural subject answers nothing.
//! These tests exist because the crate has no fake to run against: a delegation that returned a constant would pass every structural transcript.

use botster_lua_conformance::structural::{subject, StructuralLuaHost};
use botster_lua_conformance::{LuaAdapter, LuaHostSubject, LuaPackage};
use botster_plugin_conformance::{Backpressured, HpSubject, Unsupported};
use serde_json::{json, Value};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Log(Vec<String>);

/// A subject that records each call and answers with values no default would produce.
struct Recording(Rc<RefCell<Log>>, bool);

impl Recording {
    fn note(&self, text: String) {
        self.0.borrow_mut().0.push(text);
    }
}

impl HpSubject for Recording {
    fn is_fake(&self) -> bool {
        self.1
    }
    fn has_feature(&self, name: &str) -> bool {
        name == "yes"
    }
    fn has_control(&self, op: &str) -> bool {
        op == "settle"
    }
    fn configure(&mut self, setup: &Value) -> Result<(), String> {
        self.note(format!("configure {setup}"));
        Ok(())
    }
    fn write(&mut self, conn: &str, bytes: &[u8]) -> Result<(), Backpressured> {
        self.note(format!("write {conn} {bytes:?}"));
        if bytes.is_empty() {
            Err(Backpressured)
        } else {
            Ok(())
        }
    }
    fn read(&mut self, conn: &str) -> Option<Vec<u8>> {
        Some(conn.as_bytes().to_vec())
    }
    fn conns(&self) -> Vec<String> {
        vec!["a".into(), "b".into()]
    }
    fn closed(&self, conn: &str) -> Option<String> {
        Some(format!("closed {conn}"))
    }
    fn close_channel(&mut self, conn: &str) {
        self.note(format!("close {conn}"));
    }
    fn advance_clock(&mut self, ms: u64) {
        self.note(format!("clock {ms}"));
    }
    fn control(&mut self, op: &Value) -> Result<Value, Unsupported> {
        Ok(json!({"saw": op}))
    }
}

impl LuaHostSubject for Recording {
    fn load_package(&mut self, package: &LuaPackage) -> Result<(), String> {
        self.note(format!("load {}", package.files.len()));
        Ok(())
    }
}

#[test]
fn the_adapter_delegates_every_call() {
    let log = Rc::new(RefCell::new(Log::default()));
    let mut a = LuaAdapter(Box::new(Recording(log.clone(), true)));
    assert!(a.is_fake());
    // The adapter reports what the subject reports, whichever it is.
    assert!(!LuaAdapter(Box::new(Recording(log.clone(), false))).is_fake());
    assert!(a.has_feature("yes") && !a.has_feature("no"));
    assert!(a.has_control("settle") && !a.has_control("other"));
    assert_eq!(a.write("main", &[1]), Ok(()));
    assert_eq!(a.write("main", &[]), Err(Backpressured));
    assert_eq!(a.read("main"), Some(b"main".to_vec()));
    assert_eq!(a.conns(), vec!["a".to_string(), "b".to_string()]);
    assert_eq!(a.closed("main"), Some("closed main".to_string()));
    a.close_channel("cand");
    a.advance_clock(40);
    assert_eq!(
        a.control(&json!({"op": "settle"})),
        Ok(json!({"saw": {"op": "settle"}}))
    );
    let log = log.borrow();
    assert_eq!(
        log.0,
        vec!["write main [1]", "write main []", "close cand", "clock 40"]
    );
}

#[test]
fn configure_loads_the_package_before_it_passes_the_setup_on() {
    let log = Rc::new(RefCell::new(Log::default()));
    let mut a = LuaAdapter(Box::new(Recording(log.clone(), true)));
    let setup = json!({"package": {"manifest": {"name": "demo"}, "files": {"main.lua": "x", "a/b.lua": "y"}}, "limits": {}});
    assert_eq!(a.configure(&setup), Ok(()));
    assert_eq!(
        log.borrow().0,
        vec!["load 2".to_string(), format!("configure {setup}")]
    );
    // A setup with no package loads nothing.
    log.borrow_mut().0.clear();
    assert_eq!(a.configure(&json!({"limits": {}})), Ok(()));
    assert_eq!(
        log.borrow().0,
        vec![format!("configure {}", json!({"limits": {}}))]
    );
}

#[test]
fn a_malformed_package_is_an_error_that_names_the_gap() {
    let none = LuaPackage::from_setup(&json!({})).unwrap();
    assert!(none.is_none());
    let e = LuaPackage::from_setup(&json!({"package": {"files": {}}})).unwrap_err();
    assert!(e.contains("manifest"), "{e}");
    let e = LuaPackage::from_setup(&json!({"package": {"manifest": {}}})).unwrap_err();
    assert!(e.contains("files"), "{e}");
    let e = LuaPackage::from_setup(&json!({"package": {"manifest": {}, "files": {"a.lua": 5}}}))
        .unwrap_err();
    assert!(e.contains("a.lua"), "{e}");
    let ok = LuaPackage::from_setup(
        &json!({"package": {"manifest": {"n": 1}, "files": {"a.lua": "x"}}}),
    )
    .unwrap()
    .unwrap();
    assert_eq!(ok.manifest, json!({"n": 1}));
    assert_eq!(ok.files.get("a.lua").map(String::as_str), Some("x"));
}

// The Phase 0 subject selects the structural check and answers nothing.
#[test]
fn the_structural_subject_is_inert() {
    let mut s = StructuralLuaHost;
    assert!(s.is_fake());
    assert!(!s.has_feature("anything"));
    assert!(!s.has_control("settle"));
    assert_eq!(s.configure(&json!({})), Ok(()));
    assert_eq!(s.write("main", &[1]), Ok(()));
    assert_eq!(s.read("main"), None);
    assert!(s.conns().is_empty());
    assert_eq!(s.closed("main"), None);
    s.close_channel("main");
    s.advance_clock(1);
    assert_eq!(
        s.control(&json!({"op": "settle"})),
        Err(Unsupported("structural subject".into()))
    );
    assert_eq!(
        s.load_package(
            &LuaPackage::from_setup(&json!({"package": {"manifest": {}, "files": {}}}))
                .unwrap()
                .unwrap()
        ),
        Ok(())
    );
    assert!(subject(0).is_fake());
}

// `run_test` fails a transcript whose stimulus is not a valid frame. Without this test, a `run_test` that did nothing would pass every generated test.
static BAD: include_dir::Dir<'_> =
    include_dir::include_dir!("$CARGO_MANIFEST_DIR/tests/fixtures/bad");

#[test]
#[should_panic(expected = "does not validate against")]
fn run_test_fails_a_bad_stimulus() {
    botster_lua_conformance::run_test(
        "botster-lua-conformance",
        &BAD,
        "conf::bad_frame",
        &subject,
        &botster_conformance::NoSchemas,
    );
}
