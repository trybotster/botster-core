//! The generated JSON Schemas of the UI types (design section 8). `cargo xtask codegen` writes them to `schemas/ui/`.

use schemars::{schema_for, JsonSchema};

fn render<T: JsonSchema>() -> String {
    let mut text = serde_json::to_string_pretty(&schema_for!(T)).expect("a schema serializes");
    text.push('\n');
    text
}

/// The (file name, text) pairs under `schemas/ui/`.
pub fn all() -> Vec<(&'static str, String)> {
    vec![
        (
            "view-document.schema.json",
            render::<crate::doc::ViewDocument>(),
        ),
        (
            "realize-input.schema.json",
            render::<crate::input::RealizeInput>(),
        ),
        (
            "realized-tree.schema.json",
            render::<crate::node::RealizedTree>(),
        ),
        (
            "admission-failure.schema.json",
            render::<crate::admission::AdmissionFailure>(),
        ),
        (
            "admission-context.schema.json",
            render::<crate::admission::AdmissionContext>(),
        ),
        ("action-ok.schema.json", render::<crate::doc::ActionOk>()),
        (
            "action-error.schema.json",
            render::<crate::doc::ActionError>(),
        ),
        (
            "navigation-target.schema.json",
            render::<crate::doc::NavigationTarget>(),
        ),
    ]
}
