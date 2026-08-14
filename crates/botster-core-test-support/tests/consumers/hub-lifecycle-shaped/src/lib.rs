//! Isolated Hub-shaped consumer of observe, wake, and bounded lifecycle pages.

use std::collections::BTreeMap;

use botster_core_daemon::{
    CoreDaemon, SessionLifecycleCursor, SessionLifecyclePage, SessionLifecyclePageError,
    SessionLifecycleRecord, SessionLifecycleResyncReason,
};

/// In-memory Hub-shaped session projection rebuilt from Core pages.
#[derive(Clone, Debug, Default)]
pub struct HubLifecycleProjection {
    /// Cursor consumed so far.
    pub cursor: Option<SessionLifecycleCursor>,
    /// Authoritative rows keyed by session id.
    pub sessions: BTreeMap<String, SessionLifecycleRecord>,
}

/// Why the Hub-shaped consume loop stopped.
#[derive(Debug)]
pub enum HubLifecycleConsumeError {
    /// The published page budget is below the empty successful page.
    BudgetTooSmall {
        /// Required encoded size.
        minimum_bytes: usize,
    },
    /// Future Core page error. Exhaustive matches without `_` are a defect.
    UnknownPageError,
}

/// Safe consume order: take, page until caught up or resync, take, re-page if woke.
///
/// Never page-then-take-then-sleep. Page never clears the wake.
pub fn consume_lifecycle_until_caught_up(
    daemon: &mut CoreDaemon,
    projection: &mut HubLifecycleProjection,
    max_changes: usize,
    max_bytes: usize,
) -> Result<(), HubLifecycleConsumeError> {
    let _ = daemon.take_journal_advanced_wake();
    page_until_caught_up(daemon, projection, max_changes, max_bytes)?;
    if daemon.take_journal_advanced_wake() {
        page_until_caught_up(daemon, projection, max_changes, max_bytes)?;
    }
    Ok(())
}

fn page_until_caught_up(
    daemon: &mut CoreDaemon,
    projection: &mut HubLifecycleProjection,
    max_changes: usize,
    max_bytes: usize,
) -> Result<(), HubLifecycleConsumeError> {
    loop {
        let after = match &projection.cursor {
            Some(cursor) => cursor.clone(),
            None => {
                install_baseline(daemon, projection)?;
                return Ok(());
            }
        };
        let page = match daemon.lifecycle_changes_page(&after, max_changes, max_bytes) {
            Ok(page) => page,
            Err(error) => return Err(map_page_error(error)),
        };
        if let Some(reason) = &page.resync_required {
            match reason {
                SessionLifecycleResyncReason::SourceChanged
                | SessionLifecycleResyncReason::CursorExpired { .. }
                | SessionLifecycleResyncReason::CursorAhead => {
                    install_baseline(daemon, projection)?;
                    return Ok(());
                }
                _ => return Err(HubLifecycleConsumeError::UnknownPageError),
            }
        }
        apply_page(projection, &page);
        if page.next == page.source_watermark {
            return Ok(());
        }
        if page.changes.is_empty() {
            // First change cannot fit a valid budget, or max_changes is 0.
            // This is not catch-up. Recovery is a fresh baseline, not sleep.
            install_baseline(daemon, projection)?;
            return Ok(());
        }
    }
}

fn install_baseline(
    daemon: &CoreDaemon,
    projection: &mut HubLifecycleProjection,
) -> Result<(), HubLifecycleConsumeError> {
    let baseline = daemon
        .lifecycle_baseline()
        .map_err(|_| HubLifecycleConsumeError::UnknownPageError)?;
    replace_projection(projection, &baseline.sessions, baseline.cursor);
    Ok(())
}

fn map_page_error(error: SessionLifecyclePageError) -> HubLifecycleConsumeError {
    match error {
        SessionLifecyclePageError::BudgetTooSmall { minimum_bytes } => {
            HubLifecycleConsumeError::BudgetTooSmall { minimum_bytes }
        }
        _ => HubLifecycleConsumeError::UnknownPageError,
    }
}

fn replace_projection(
    projection: &mut HubLifecycleProjection,
    sessions: &[SessionLifecycleRecord],
    cursor: SessionLifecycleCursor,
) {
    projection.sessions.clear();
    projection.sessions.extend(
        sessions
            .iter()
            .cloned()
            .map(|record| (record.session.session_id.0.clone(), record)),
    );
    projection.cursor = Some(cursor);
}

fn apply_page(projection: &mut HubLifecycleProjection, page: &SessionLifecyclePage) {
    for change in &page.changes {
        match &change.kind {
            botster_core_daemon::SessionLifecycleChangeKind::Upsert { record } => {
                projection
                    .sessions
                    .insert(record.session.session_id.0.clone(), record.clone());
            }
            botster_core_daemon::SessionLifecycleChangeKind::Removed { session_id } => {
                projection.sessions.remove(&session_id.0);
            }
            _ => {}
        }
    }
    projection.cursor = Some(page.next.clone());
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    use botster_core::{
        CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
        SpawnEnvironment, SpawnWorkingDirectory,
    };
    use botster_core_daemon::{CoreDaemon, CoreDaemonConfig, SpawnSessionRequest};

    #[derive(Clone, Copy, Debug)]
    enum InterleaveSeam {
        BeforeTake,
        BetweenTakeAndPage,
        AfterPageBeforeSecondTake,
        AfterSecondTake,
    }

    #[cfg(unix)]
    #[test]
    fn hub_shaped_consumer_observes_without_drain() {
        let data_dir = temp_data_dir("hub-lifecycle-observe");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let session_id = SessionId("hub-life-observe".to_string());
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("observe fixture spawn");
        let observed = daemon
            .observe_lifecycle(11)
            .expect("hub-shaped observe is the control-plane tick");
        assert!(observed.session_errors.is_empty());
        let _ = daemon.take_journal_advanced_wake();
        let _ = daemon.shutdown(Some(session_id), 20);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn hub_shaped_consumer_matches_budget_error_with_wildcard() {
        let data_dir = temp_data_dir("hub-lifecycle-budget");
        let daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let cursor = daemon
            .lifecycle_baseline()
            .expect("baseline")
            .cursor;
        let mapped = match daemon.lifecycle_changes_page(&cursor, 8, 0) {
            Ok(_) => panic!("zero budget must not return a successful page"),
            Err(error) => map_page_error(error),
        };
        assert!(matches!(
            mapped,
            HubLifecycleConsumeError::BudgetTooSmall { minimum_bytes } if minimum_bytes > 0
        ));
        let _ = fs::remove_dir_all(data_dir);
    }

    #[cfg(unix)]
    #[test]
    fn hub_shaped_consumer_baselines_when_first_change_does_not_fit() {
        let data_dir = temp_data_dir("hub-lifecycle-no-progress");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let session_id = SessionId("hub-life-no-progress".to_string());
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("spawn a change larger than the empty page");
        let source = daemon
            .lifecycle_baseline()
            .expect("watermark after spawn")
            .cursor;
        let after = SessionLifecycleCursor {
            source_id: source.source_id.clone(),
            sequence: 0,
        };
        let minimum_bytes = match daemon.lifecycle_changes_page(&after, 8, 0) {
            Err(SessionLifecyclePageError::BudgetTooSmall { minimum_bytes }) => minimum_bytes,
            other => panic!("expected BudgetTooSmall, got {other:?}"),
        };
        let empty = daemon
            .lifecycle_changes_page(&after, 8, minimum_bytes)
            .expect("exact minimum is an empty successful page");
        assert!(empty.resync_required.is_none());
        assert!(empty.changes.is_empty());
        assert_ne!(empty.next, empty.source_watermark);

        let mut projection = HubLifecycleProjection {
            cursor: Some(after),
            sessions: BTreeMap::new(),
        };
        consume_lifecycle_until_caught_up(&mut daemon, &mut projection, 8, minimum_bytes)
            .expect("no-progress page must recover, not report catch-up");
        assert_eq!(
            projection.cursor.as_ref(),
            Some(&empty.source_watermark),
            "baseline recovery must reach the source watermark"
        );
        assert!(
            projection.sessions.contains_key(&session_id.0),
            "baseline recovery must apply the oversized first change"
        );

        let _ = daemon.shutdown(Some(session_id), 20);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[cfg(unix)]
    #[test]
    fn hub_shaped_safe_loop_converges_across_wake_page_interleavings() {
        for seam in [
            InterleaveSeam::BeforeTake,
            InterleaveSeam::BetweenTakeAndPage,
            InterleaveSeam::AfterPageBeforeSecondTake,
            InterleaveSeam::AfterSecondTake,
        ] {
            prove_safe_loop_seam(seam);
        }
    }

    fn prove_safe_loop_seam(seam: InterleaveSeam) {
        let data_dir = temp_data_dir(&format!("hub-lifecycle-seam-{seam:?}"));
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let first = SessionId(format!("hub-life-a-{seam:?}"));
        daemon
            .spawn(spawn_request(&first), 10)
            .expect("seed append");
        let watermark = daemon
            .lifecycle_baseline()
            .expect("seed baseline")
            .cursor;
        let mut projection = HubLifecycleProjection {
            cursor: Some(SessionLifecycleCursor {
                source_id: watermark.source_id,
                sequence: 0,
            }),
            sessions: BTreeMap::new(),
        };

        let extra = SessionId(format!("hub-life-b-{seam:?}"));
        match seam {
            InterleaveSeam::BeforeTake => {
                append_extra(&mut daemon, &extra, 11);
                consume_lifecycle_until_caught_up(&mut daemon, &mut projection, 8, 16 * 1024)
                    .expect("safe loop");
            }
            InterleaveSeam::BetweenTakeAndPage => {
                let _ = daemon.take_journal_advanced_wake();
                append_extra(&mut daemon, &extra, 11);
                page_until_caught_up(&mut daemon, &mut projection, 8, 16 * 1024).expect("page");
                if daemon.take_journal_advanced_wake() {
                    page_until_caught_up(&mut daemon, &mut projection, 8, 16 * 1024)
                        .expect("second page");
                }
            }
            InterleaveSeam::AfterPageBeforeSecondTake => {
                let _ = daemon.take_journal_advanced_wake();
                page_until_caught_up(&mut daemon, &mut projection, 8, 16 * 1024).expect("page");
                append_extra(&mut daemon, &extra, 11);
                if daemon.take_journal_advanced_wake() {
                    page_until_caught_up(&mut daemon, &mut projection, 8, 16 * 1024)
                        .expect("re-page");
                }
            }
            InterleaveSeam::AfterSecondTake => {
                consume_lifecycle_until_caught_up(&mut daemon, &mut projection, 8, 16 * 1024)
                    .expect("safe loop");
                append_extra(&mut daemon, &extra, 11);
            }
        }

        let applied = projection.sessions.contains_key(&extra.0);
        if !applied {
            assert!(
                daemon.take_journal_advanced_wake(),
                "unapplied change must leave a pending wake at seam {seam:?}"
            );
        }
        let _ = daemon.shutdown(Some(first), 20);
        let _ = daemon.shutdown(Some(extra), 21);
        let _ = fs::remove_dir_all(data_dir);
    }

    fn append_extra(daemon: &mut CoreDaemon, session_id: &SessionId, now: u64) {
        daemon
            .spawn(spawn_request(session_id), now)
            .expect("interleaved append");
    }

    fn spawn_request(session_id: &SessionId) -> SpawnSessionRequest {
        SpawnSessionRequest {
            request: SessionSpawnRequest {
                request_id: RequestId(format!("{}-spawn", session_id.0)),
                session_id: session_id.clone(),
                executable: "sh".to_string(),
                arguments: vec!["-c".to_string(), "sleep 8".to_string()],
                working_directory: SpawnWorkingDirectory {
                    path: ".".to_string(),
                },
                environment: SpawnEnvironment::default(),
                initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
            },
            metadata: CoreSessionMetadata::new(),
        }
    }

    fn temp_data_dir(label: &str) -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        std::env::temp_dir().join(format!("botster-hub-life-{label}-{nanos}"))
    }
}
