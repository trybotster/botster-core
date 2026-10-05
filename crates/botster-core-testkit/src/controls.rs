//! Control registration for the testkit. Each module registers the controls that it owns.

use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use serde_json::Value;
use std::collections::{btree_map::Entry, BTreeMap};

/// A control handler receives the harness, the handle name, and the control arguments.
pub type ControlHandler = fn(&mut TestkitHarness, &str, &Value) -> Result<Value, ControlError>;

/// The handlers for one harness. Control names must be unique across modules.
#[derive(Debug, Default)]
pub struct ControlRegistry {
    handlers: BTreeMap<&'static str, ControlHandler>,
}

impl ControlRegistry {
    /// Registers one control. A duplicate name indicates an error in module registration.
    ///
    /// # Panics
    /// Panics if a module has already registered `name`.
    pub fn register(&mut self, name: &'static str, handler: ControlHandler) {
        match self.handlers.entry(name) {
            Entry::Vacant(entry) => {
                entry.insert(handler);
            }
            Entry::Occupied(_) => panic!("duplicate testkit control: {name}"),
        }
    }

    /// Returns whether a module registered this control.
    pub fn contains(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }

    /// Returns the handler for this control.
    pub fn handler(&self, name: &str) -> Option<ControlHandler> {
        self.handlers.get(name).copied()
    }
}

/// Collects module registrations. New controls belong in their module's registration function.
pub(crate) fn registered_controls() -> ControlRegistry {
    let mut registry = ControlRegistry::default();
    crate::refusal::register_controls(&mut registry);
    crate::worker_controls::register_controls(&mut registry);
    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed(
        harness: &mut TestkitHarness,
        handle: &str,
        args: &Value,
    ) -> Result<Value, ControlError> {
        Ok(serde_json::json!({"seed": harness.seed(), "handle": handle, "args": args}))
    }

    #[test]
    fn registered_handler_receives_the_harness_handle_and_arguments() {
        let mut registry = ControlRegistry::default();
        assert!(!registry.contains("seed"));
        assert!(registry.handler("seed").is_none());
        registry.register("seed", seed);
        assert!(registry.contains("seed"));
        let mut harness = TestkitHarness::new(17);
        let args = serde_json::json!({"input": 3});
        let result = registry.handler("seed").unwrap()(&mut harness, "h", &args).unwrap();
        assert_eq!(
            result,
            serde_json::json!({"seed": 17, "handle": "h", "args": args})
        );
        assert!(!registry.contains("unknown"));
        assert!(registry.handler("unknown").is_none());
    }

    #[test]
    #[should_panic(expected = "duplicate testkit control: seed")]
    fn duplicate_registration_fails() {
        let mut registry = ControlRegistry::default();
        registry.register("seed", seed);
        registry.register("seed", seed);
    }
}
