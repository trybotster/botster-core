#![allow(missing_docs)]

use botster_terminal_protocol::{
    ensure_compatible, TerminalCompatibility, TerminalCompatibilityRequirement, FEATURE_RESIZE,
    FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY, FEATURE_TERMINAL_STREAMING, PROTOCOL,
    PROTOCOL_VERSION,
};

fn baseline_descriptor() -> TerminalCompatibility {
    TerminalCompatibility {
        protocol: PROTOCOL.to_string(),
        protocol_version: PROTOCOL_VERSION,
        features: vec![
            FEATURE_TERMINAL_STREAMING.to_string(),
            FEATURE_RESIZE.to_string(),
        ],
        conformance_fixture_revision: 1,
    }
}

#[test]
fn default_requirement_accepts_descriptor_without_ready_then_history() {
    let requirement = TerminalCompatibilityRequirement::current();
    assert!(!requirement
        .required_features
        .iter()
        .any(|feature| feature == FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    ensure_compatible(&requirement, &baseline_descriptor()).expect("baseline must satisfy default");
}

#[test]
fn advertised_support_includes_optional_ready_then_history() {
    let advertised = TerminalCompatibility::current();
    assert!(advertised.supports_feature(FEATURE_TERMINAL_STREAMING));
    assert!(advertised.supports_feature(FEATURE_RESIZE));
    assert!(advertised.supports_feature(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    ensure_compatible(&TerminalCompatibilityRequirement::current(), &advertised)
        .expect("current advertised must satisfy default");
}

#[test]
fn ready_then_history_requirement_rejects_baseline_and_accepts_advertised() {
    let requirement = TerminalCompatibilityRequirement::for_ready_then_history_attach();
    assert!(requirement
        .required_features
        .iter()
        .any(|feature| feature == FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    let rejected = ensure_compatible(&requirement, &baseline_descriptor());
    assert!(
        rejected.is_err(),
        "baseline must fail the operation-specific requirement"
    );
    let diagnostic = rejected.expect_err("checked").diagnostic;
    assert!(
        diagnostic.contains("snapshot_delivery=ready_then_history"),
        "{diagnostic}"
    );
    ensure_compatible(&requirement, &TerminalCompatibility::current())
        .expect("advertised support must satisfy the operation-specific requirement");
}
