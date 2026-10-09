//! Control registration for the testkit. Each module registers the controls that it owns.

use crate::harness::TestkitHarness;
use botster_core_conformance::ControlError;
use botster_core_contract::prelude::SessionId;
use botster_core_host::session::{row_key, Row};
use serde::de::DeserializeOwned;
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
    crate::process_controls::register_controls(&mut registry);
    crate::wake_controls::register_controls(&mut registry);
    crate::pty_controls::register_controls(&mut registry);
    crate::start_controls::register_controls(&mut registry);
    crate::adopt_controls::register_controls(&mut registry);
    crate::resume_controls::register_controls(&mut registry);
    registry
}

/// The arguments of a control. The step's own keys, `op` and `handle`, are removed at the top level only; every other key
/// must be an argument of the control (`T` denies unknown fields), so a misspelt argument is `Bad`, never ignored.
pub(crate) fn parse<T: DeserializeOwned>(args: &Value) -> Result<T, ControlError> {
    let mut args = args.clone();
    if let Some(object) = args.as_object_mut() {
        object.remove("op");
        object.remove("handle");
    }
    serde_json::from_value(args).map_err(|e| ControlError::Bad(format!("the arguments: {e}")))
}

/// The registry row of `session` in the data directory of `handle`, decoded by the host's own decoder (`Row::decode`): the
/// testkit reads what Core stored and has no second reading of it.
pub(crate) fn session_row(
    harness: &TestkitHarness,
    handle: &str,
    session: &SessionId,
) -> Result<Row, ControlError> {
    let dir = harness
        .directory_of(handle)
        .ok_or_else(|| ControlError::Bad(format!("the handle '{handle}' is not open")))?;
    let bytes = harness
        .directories()
        .row(dir, &row_key(session))
        .ok_or_else(|| ControlError::Bad(format!("no row of the session {}", session.0)))?;
    Row::decode(session, &bytes).ok_or_else(|| {
        ControlError::Bad(format!(
            "the row of the session {} is not readable",
            session.0
        ))
    })
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
