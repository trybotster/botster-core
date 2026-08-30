//! Isolated Hub-shaped consumer. Depends only on the opaque protocol crate.

use botster_terminal_protocol::{
    ensure_compatible, Attach, TerminalCapabilitySet, TerminalCompatibility,
    TerminalCompatibilityError, TerminalCompatibilityRequirement, TerminalFrame, FEATURE_RESIZE,
    FEATURE_TERMINAL_STREAMING,
};

pub fn forward_attach(session_id: &str, subscription_id: &str) -> String {
    let request = Attach {
        session_id: session_id.to_string(),
        subscription_id: subscription_id.to_string(),
    };
    serde_json::to_string(&request).expect("serialize attach")
}

pub fn forward_frame(bytes: &[u8]) -> Vec<u8> {
    TerminalFrame::from_bytes(bytes)
        .expect("opaque frame")
        .to_bytes()
        .expect("emit frame")
}

pub fn negotiated_capabilities(tokens: &[&str]) -> TerminalCapabilitySet {
    TerminalCapabilitySet::from_tokens(tokens.iter().copied()).expect("advertised tokens")
}

pub fn empty_capabilities() -> TerminalCapabilitySet {
    TerminalCapabilitySet::empty()
}

pub fn baseline_capabilities() -> TerminalCapabilitySet {
    negotiated_capabilities(&[FEATURE_TERMINAL_STREAMING, FEATURE_RESIZE])
}

pub fn ensure_advertised_duplex() -> Result<(), TerminalCompatibilityError> {
    ensure_compatible(
        &TerminalCompatibilityRequirement::for_duplex_binary_transport(),
        &TerminalCompatibility::current(),
    )
}
