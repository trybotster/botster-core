#![allow(missing_docs)]

use botster_terminal_protocol::{
    ensure_compatible, TerminalCapabilitySet, TerminalCapabilitySetError, TerminalCompatibility,
    TerminalCompatibilityRequirement, FEATURE_RESIZE, FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
    FEATURE_TERMINAL_STREAMING, FEATURE_TRANSPORT_DUPLEX_BINARY,
};

/// Current advertised support without one feature token.
fn current_without(feature: &str) -> TerminalCompatibility {
    let mut descriptor = TerminalCompatibility::current();
    descriptor.features.retain(|token| token != feature);
    descriptor
}

#[test]
fn advertised_support_includes_optional_ready_then_history() {
    let advertised = TerminalCompatibility::current();
    assert!(advertised.supports_feature(FEATURE_TERMINAL_STREAMING));
    assert!(advertised.supports_feature(FEATURE_RESIZE));
    assert!(advertised.supports_feature(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    assert!(advertised.supports_feature(FEATURE_TRANSPORT_DUPLEX_BINARY));
    ensure_compatible(&TerminalCompatibilityRequirement::current(), &advertised)
        .expect("current advertised must satisfy default");
}

#[test]
fn default_requirement_rejects_descriptor_without_duplex_binary() {
    let missing_duplex = current_without(FEATURE_TRANSPORT_DUPLEX_BINARY);
    let rejected = ensure_compatible(
        &TerminalCompatibilityRequirement::current(),
        &missing_duplex,
    );
    let diagnostic = rejected.expect_err("duplex token is required").diagnostic;
    assert!(
        diagnostic.contains(FEATURE_TRANSPORT_DUPLEX_BINARY),
        "{diagnostic}"
    );
}

#[test]
fn ready_then_history_requirement_rejects_its_absence_and_accepts_advertised() {
    let requirement = TerminalCompatibilityRequirement::for_ready_then_history_attach();
    assert!(requirement
        .required_features
        .iter()
        .any(|feature| feature == FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    let rejected = ensure_compatible(
        &requirement,
        &current_without(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY),
    );
    assert!(
        rejected.is_err(),
        "a descriptor without ready_then_history must fail the operation-specific requirement"
    );
    let diagnostic = rejected.expect_err("checked").diagnostic;
    assert!(
        diagnostic.contains("snapshot_delivery=ready_then_history"),
        "{diagnostic}"
    );
    ensure_compatible(&requirement, &TerminalCompatibility::current())
        .expect("advertised support must satisfy the operation-specific requirement");
}

#[test]
fn empty_capability_set_constructs() {
    let empty = TerminalCapabilitySet::empty();
    assert!(empty.is_empty());
    assert!(!empty.contains(FEATURE_TERMINAL_STREAMING));
    let from_empty = TerminalCapabilitySet::from_tokens(Vec::<&str>::new()).expect("empty list");
    assert_eq!(empty, from_empty);
}

#[test]
fn advertised_tokens_construct_an_ordered_set() {
    let set = TerminalCapabilitySet::from_tokens([
        FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
        FEATURE_RESIZE,
        FEATURE_TERMINAL_STREAMING,
        FEATURE_TRANSPORT_DUPLEX_BINARY,
        FEATURE_RESIZE,
    ])
    .expect("advertised tokens");
    assert!(set.contains(FEATURE_RESIZE));
    assert!(set.contains(FEATURE_TERMINAL_STREAMING));
    assert!(set.contains(FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY));
    let tokens: Vec<&str> = set.iter().collect();
    assert_eq!(
        tokens,
        vec![
            FEATURE_RESIZE,
            FEATURE_SNAPSHOT_DELIVERY_READY_THEN_HISTORY,
            FEATURE_TERMINAL_STREAMING,
            FEATURE_TRANSPORT_DUPLEX_BINARY,
        ]
    );
}

#[test]
fn unknown_tokens_fail_at_construction() {
    let error =
        TerminalCapabilitySet::from_tokens(["not-a-terminal-token"]).expect_err("unknown token");
    assert!(matches!(
        error,
        TerminalCapabilitySetError::UnknownToken { token } if token == "not-a-terminal-token"
    ));
}
