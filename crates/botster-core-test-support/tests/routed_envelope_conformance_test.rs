//! Routed envelope conformance helper tests.

#[cfg(feature = "local-runtime")]
use botster_core::{EnvelopeId, EnvelopeTarget};
#[cfg(feature = "local-runtime")]
use botster_core_test_support::conformance::host_coordination_envelope_fixture;

#[cfg(feature = "local-runtime")]
#[test]
fn hub_facing_conformance_helper_builds_semantic_tool_payload_above_core() {
    let envelope = host_coordination_envelope_fixture(
        "coordination-1",
        "native-tool",
        "host-coordination",
        br#"{"tool":"synthetic","intent":"coordinate"}"#.to_vec(),
    );

    assert_eq!(envelope.id, EnvelopeId("coordination-1".to_string()));
    assert_eq!(
        envelope.targets,
        vec![EnvelopeTarget::Topic {
            topic: "host-coordination".to_string(),
        }]
    );
    assert_eq!(
        envelope.payload.content_type,
        "application/vnd.botster.host-coordination+json"
    );
    assert!(envelope.payload.extension.is_none());
}
