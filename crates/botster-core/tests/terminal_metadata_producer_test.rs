//! Terminal metadata producer tests.

use botster_core::{
    TerminalMetadataKind, TerminalMetadataLaneShaper, TerminalMetadataObservation,
    TerminalMetadataProducer, TerminalMetadataShapingOutcome,
};

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

#[test]
fn ignores_non_notification_osc9_progress() {
    let mut producer = TerminalMetadataProducer::new();

    assert!(producer.observe(b"\x1b]9;4;1;50\x07").is_empty());
}

#[test]
fn ignores_osc7_file_uri_without_path_segment() {
    let mut producer = TerminalMetadataProducer::new();

    assert!(producer.observe(b"\x1b]7;file://host\x07").is_empty());
}

#[test]
fn metadata_lane_shaper_reports_latest_win_dedup_rate_limit_and_drop_without_payloads() {
    let mut shaper = TerminalMetadataLaneShaper::new(2, 2);

    let accepted = shaper.push(TerminalMetadataObservation::TitleChanged("one".to_string()));
    assert_eq!(accepted[0].kind, Some(TerminalMetadataKind::Title));
    assert_eq!(
        accepted[0].outcome,
        TerminalMetadataShapingOutcome::Accepted
    );

    let latest = shaper.push(TerminalMetadataObservation::TitleChanged("two".to_string()));
    assert_eq!(latest[0].kind, Some(TerminalMetadataKind::Title));
    assert_eq!(latest[0].outcome, TerminalMetadataShapingOutcome::LatestWin);

    let deduped = shaper.push(TerminalMetadataObservation::TitleChanged("two".to_string()));
    assert_eq!(deduped[0].kind, Some(TerminalMetadataKind::Title));
    assert_eq!(
        deduped[0].outcome,
        TerminalMetadataShapingOutcome::Deduplicated
    );

    let accepted_cwd = shaper.push(TerminalMetadataObservation::CwdChanged("/repo".to_string()));
    assert_eq!(
        accepted_cwd[0].outcome,
        TerminalMetadataShapingOutcome::Accepted
    );

    let rate_limited = shaper.push(TerminalMetadataObservation::Bell);
    assert_eq!(rate_limited[0].kind, Some(TerminalMetadataKind::Bell));
    assert_eq!(
        rate_limited[0].outcome,
        TerminalMetadataShapingOutcome::RateLimited
    );

    let retained = shaper.drain();
    assert_eq!(
        retained,
        vec![
            TerminalMetadataObservation::TitleChanged("two".to_string()),
            TerminalMetadataObservation::CwdChanged("/repo".to_string()),
        ]
    );

    let mut full = TerminalMetadataLaneShaper::new(1, 8);
    assert_eq!(full.capacity(), 1);
    assert_eq!(full.retained_len(), 0);
    full.push(TerminalMetadataObservation::Bell);
    let dropped = full.push(TerminalMetadataObservation::Notification(
        botster_core::NotificationPayload {
            title: "hidden".to_string(),
            body: "hidden".to_string(),
        },
    ));
    assert_eq!(dropped[0].kind, Some(TerminalMetadataKind::Notification));
    assert_eq!(dropped[0].outcome, TerminalMetadataShapingOutcome::Dropped);
    assert_eq!(dropped[0].count, 1);
}
