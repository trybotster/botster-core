//! Terminal metadata producer tests.

use botster_core::{TerminalMetadataObservation, TerminalMetadataProducer};

#[test]
fn observes_supported_metadata_without_mutating_raw_bytes() {
    let raw = b"pre\x1b]0;Build\x07\x1b]7;file://host/work/repo\x07\x1b]133;A\x07\x07\x1b]9;Notice;Body\x07post";
    let mut producer = TerminalMetadataProducer::new();

    let observations = producer.observe(raw);

    assert_eq!(
        observations,
        vec![
            TerminalMetadataObservation::TitleChanged("Build".to_string()),
            TerminalMetadataObservation::CwdChanged("/work/repo".to_string()),
            TerminalMetadataObservation::PromptMark(botster_core::PromptMarkPayload {
                mark: "A".to_string(),
            }),
            TerminalMetadataObservation::Bell,
            TerminalMetadataObservation::Notification(botster_core::NotificationPayload {
                title: "Notice".to_string(),
                body: "Body".to_string(),
            }),
        ]
    );
    assert_eq!(
        raw,
        b"pre\x1b]0;Build\x07\x1b]7;file://host/work/repo\x07\x1b]133;A\x07\x07\x1b]9;Notice;Body\x07post"
    );
}

#[test]
fn split_osc_sequence_produces_one_observation() {
    let mut producer = TerminalMetadataProducer::new();

    assert!(producer.observe(b"\x1b]2;Bui").is_empty());
    assert_eq!(
        producer.observe(b"ld\x1b\\"),
        vec![TerminalMetadataObservation::TitleChanged(
            "Build".to_string()
        )]
    );
}

#[test]
fn unterminated_osc_state_is_bounded() {
    let mut producer = TerminalMetadataProducer::new();

    assert!(producer.observe(b"\x1b]2;").is_empty());
    for _ in 0..5000 {
        producer.observe(b"x");
    }

    assert!(
        producer.retained_len() <= 4096,
        "retained partial OSC state should remain bounded"
    );
}
