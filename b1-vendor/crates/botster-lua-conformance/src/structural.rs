//! The Phase 0 subject (design 7.1, ruling R-1.4): the Lua host has no fake, so no behavior is modeled. This subject only reports that it is a stand-in. The runner then checks each
//! `structural` transcript's stimuli (every valid frame validates, every typed negative frame fails its schema) and runs nothing else.

use crate::{LuaHostSubject, LuaPackage};
use botster_plugin_conformance::{Backpressured, HpSubject, Unsupported};
use serde_json::Value;

/// A subject that does nothing. It selects the structural check; it never answers a frame.
#[derive(Debug, Default, Clone, Copy)]
pub struct StructuralLuaHost;

impl HpSubject for StructuralLuaHost {
    fn is_fake(&self) -> bool {
        true
    }
    fn has_feature(&self, _name: &str) -> bool {
        false
    }
    fn has_control(&self, _op: &str) -> bool {
        false
    }
    fn configure(&mut self, _setup: &Value) -> Result<(), String> {
        Ok(())
    }
    fn write(&mut self, _conn: &str, _bytes: &[u8]) -> Result<(), Backpressured> {
        Ok(())
    }
    fn read(&mut self, _conn: &str) -> Option<Vec<u8>> {
        None
    }
    fn conns(&self) -> Vec<String> {
        vec![]
    }
    fn closed(&self, _conn: &str) -> Option<String> {
        None
    }
    fn close_channel(&mut self, _conn: &str) {}
    fn advance_clock(&mut self, _ms: u64) {}
    fn control(&mut self, _op: &Value) -> Result<Value, Unsupported> {
        Err(Unsupported("structural subject".into()))
    }
}

impl LuaHostSubject for StructuralLuaHost {
    fn load_package(&mut self, _package: &LuaPackage) -> Result<(), String> {
        Ok(())
    }
}

/// The subject factory of the Phase 0 self-test.
pub fn subject(_seed: u64) -> Box<dyn LuaHostSubject> {
    Box::new(StructuralLuaHost)
}
