//! Isolated Hub-shaped consumer of observe, wake, and bounded lifecycle pages.

use std::collections::BTreeMap;

use std::time::Duration;

use botster_core::SessionLifecycleState;
use botster_core_daemon::{
    CoreDaemon, CoreDaemonError, LifecycleBaselineBudget, ObserveLifecycleBudget,
    ObserveLifecycleCursor, ObserveLifecycleSlice, RegistrySessionState, SessionLifecycleCursor,
    SessionLifecycleLookup, SessionLifecyclePage, SessionLifecyclePageError,
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
                | SessionLifecycleResyncReason::CursorAhead
                | SessionLifecycleResyncReason::SnapshotUnavailable => {
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
    daemon: &mut CoreDaemon,
    projection: &mut HubLifecycleProjection,
) -> Result<(), HubLifecycleConsumeError> {
    let mut snapshot = None;
    let mut after = None;
    let mut rows = Vec::new();
    loop {
        let page = match daemon.lifecycle_baseline_page(
            snapshot.as_ref(),
            after.as_ref(),
            LifecycleBaselineBudget {
                max_rows: 32,
                max_bytes: 64 * 1024,
                max_elapsed: Duration::MAX,
            },
        ) {
            Ok(page) => page,
            Err(error) => return Err(map_page_error(error)),
        };
        if let Some(reason) = &page.resync_required {
            match reason {
                SessionLifecycleResyncReason::SourceChanged
                | SessionLifecycleResyncReason::CursorExpired { .. }
                | SessionLifecycleResyncReason::CursorAhead
                | SessionLifecycleResyncReason::SnapshotUnavailable => {
                    if snapshot.is_none() {
                        return Err(HubLifecycleConsumeError::UnknownPageError);
                    }
                    snapshot = None;
                    after = None;
                    rows.clear();
                    continue;
                }
                _ => return Err(HubLifecycleConsumeError::UnknownPageError),
            }
        }
        rows.extend(page.sessions.iter().cloned());
        if page.complete {
            replace_projection(projection, &rows, page.snapshot_sequence);
            return Ok(());
        }
        snapshot = Some(page.snapshot_sequence);
        after = page.next;
        // Setup-only and index-in-progress yields keep the freeze identity
        // and set next = None. Retry the same snapshot.
    }
}

/// Stage A observe: one owner-turn slice. The caller owns the resume cursor.
pub fn observe_lifecycle_stage_a(
    daemon: &mut CoreDaemon,
    now_seconds: u64,
    resume: Option<&ObserveLifecycleCursor>,
    budget: ObserveLifecycleBudget,
) -> Result<ObserveLifecycleSlice, SessionLifecyclePageError> {
    daemon.observe_lifecycle_slice(now_seconds, resume, budget)
}

/// Hub-shaped ShutdownSession class for one exact-session Core lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HubSessionLifecycleClass {
    /// The session still has a non-exited control-plane row.
    Active,
    /// Core reported a reconciled exited row.
    Exited,
    /// Registry and engine both lack the session.
    Absent,
    /// Control-plane error. This is not Active.
    OperatorError,
}

/// Classify one exact-session lookup without Drain, baseline, or pagination.
///
/// A registry-only terminal row has no in-memory lifecycle until adoption.
/// That row is still ended when `registry_state` is Exited or Stale.
/// Miss, error, and future lookup variants stay off Active.
#[must_use]
pub fn classify_session_lifecycle(
    result: Result<SessionLifecycleLookup, CoreDaemonError>,
) -> HubSessionLifecycleClass {
    match result {
        Ok(SessionLifecycleLookup::Found(record)) => {
            let terminal_lifecycle = matches!(
                record.lifecycle,
                Some(SessionLifecycleState::Exited { .. })
                    | Some(SessionLifecycleState::Failed { .. })
            );
            let terminal_registry = matches!(
                record.session.registry_state,
                RegistrySessionState::Exited | RegistrySessionState::Stale
            );
            if terminal_lifecycle || terminal_registry {
                HubSessionLifecycleClass::Exited
            } else {
                HubSessionLifecycleClass::Active
            }
        }
        Ok(SessionLifecycleLookup::Absent) => HubSessionLifecycleClass::Absent,
        Err(_) => HubSessionLifecycleClass::OperatorError,
        Ok(_) => HubSessionLifecycleClass::OperatorError,
    }
}

/// Resume cursor from a progressing slice, if this pass can continue.
#[must_use]
pub fn observe_lifecycle_resume_cursor(
    slice: &ObserveLifecycleSlice,
) -> Option<ObserveLifecycleCursor> {
    if slice.complete || slice.resync_required.is_some() {
        return None;
    }
    Some(ObserveLifecycleCursor {
        pass_id: slice.pass_id.clone(),
        last_visited: slice.last_visited.clone(),
    })
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
        ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId, SessionSpawnRequest,
        SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
    };
    use botster_core_daemon::{
        CoreDaemon, CoreDaemonConfig, RegistryRecord, SessionRegistryStateLookup,
        SpawnSessionRequest,
    };

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
        let observed = observe_lifecycle_stage_a(
            &mut daemon,
            11,
            None,
            ObserveLifecycleBudget {
                max_sessions: 8,
                max_encoded_result_bytes: 16 * 1024,
                max_elapsed: std::time::Duration::from_secs(1),
            },
        )
        .expect("hub-shaped observe is the control-plane tick");
        assert!(observed.complete);
        assert!(observed.session_errors.is_empty());
        let _ = daemon.take_journal_advanced_wake();
        let _ = daemon.shutdown(Some(session_id), 20);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[cfg(unix)]
    #[test]
    fn hub_shaped_stage_a_yields_between_owner_turns() {
        let data_dir = temp_data_dir("hub-lifecycle-owner-turns");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let first = SessionId("a-hub-owner-turn".to_string());
        let second = SessionId("b-hub-owner-turn".to_string());
        daemon
            .spawn(spawn_request(&first), 10)
            .expect("first owner-turn spawn");
        daemon
            .spawn(spawn_request(&second), 11)
            .expect("second owner-turn spawn");
        let budget = ObserveLifecycleBudget {
            max_sessions: 1,
            max_encoded_result_bytes: 16 * 1024,
            max_elapsed: std::time::Duration::from_secs(1),
        };
        let setup_slice = observe_lifecycle_stage_a(
            &mut daemon,
            12,
            None,
            ObserveLifecycleBudget {
                max_sessions: 1,
                max_encoded_result_bytes: 16 * 1024,
                max_elapsed: std::time::Duration::ZERO,
            },
        )
        .expect("first owner turn can yield during setup");
        assert!(setup_slice.last_visited.is_none());
        assert!(!setup_slice.complete);
        let setup_resume = observe_lifecycle_resume_cursor(&setup_slice)
            .expect("setup yield has a resume cursor");
        let first_slice = observe_lifecycle_stage_a(&mut daemon, 13, Some(&setup_resume), budget)
            .expect("second owner turn visits first session");
        assert_eq!(first_slice.last_visited.as_ref(), Some(&first));
        assert!(!first_slice.complete);
        let resume = observe_lifecycle_resume_cursor(&first_slice)
            .expect("caller owns the resume cursor");
        let _ = daemon.take_journal_advanced_wake();
        let listed = daemon
            .list()
            .expect("host can do ready work between owner turns");
        assert!(listed.iter().any(|session| session.session_id == first));
        let second_slice =
            observe_lifecycle_stage_a(&mut daemon, 14, Some(&resume), budget)
                .expect("third owner turn");
        assert_eq!(second_slice.last_visited.as_ref(), Some(&second));
        assert!(second_slice.complete);
        let _ = daemon.shutdown(Some(first), 20);
        let _ = daemon.shutdown(Some(second), 21);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn classify_session_lifecycle_never_maps_miss_or_error_to_active() {
        assert_eq!(
            classify_session_lifecycle(Ok(SessionLifecycleLookup::Absent)),
            HubSessionLifecycleClass::Absent
        );
        assert_eq!(
            classify_session_lifecycle(Err(CoreDaemonError::Shutdown)),
            HubSessionLifecycleClass::OperatorError
        );
        let data_dir = temp_data_dir("hub-lifecycle-classify");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let unknown = botster_core::SessionId("hub-classify-missing".to_string());
        let looked_up = daemon.observe_session_lifecycle(&unknown, 10);
        assert_eq!(
            classify_session_lifecycle(looked_up),
            HubSessionLifecycleClass::Absent
        );
        let _ = fs::remove_dir_all(data_dir);
    }

    #[cfg(unix)]
    #[test]
    fn hub_pump_shaped_exact_subscription_membership() {
        let data_dir = temp_data_dir("hub-lifecycle-sub-generation");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let session_id = SessionId("hub-life-sub-generation".to_string());
        let client_id = ClientId("hub-life-sub-generation-client".to_string());
        let subscription_id = SubscriptionId("hub-life-sub-generation-sub".to_string());
        let absent = SubscriptionId("hub-life-sub-generation-absent".to_string());
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("hub-shaped membership spawn");
        daemon
            .attach(
                client_id,
                session_id.clone(),
                subscription_id.clone(),
                11,
            )
            .expect("hub-shaped attach");
        let inventory = daemon
            .list_terminal_subscriptions()
            .into_iter()
            .find(|row| row.session_id == session_id && row.subscription_id == subscription_id)
            .expect("attach-created owner");
        let live = daemon.terminal_subscription_generation(&session_id, &subscription_id);
        assert_eq!(live, Some(inventory.generation));
        assert_eq!(
            daemon.terminal_subscription_generation(&session_id, &absent),
            None
        );
        let _ = daemon.shutdown(Some(session_id), 20);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[cfg(unix)]
    #[test]
    fn hub_close_suppression_shaped_registry_state_is_non_mutating() {
        let data_dir = temp_data_dir("hub-lifecycle-registry-state");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let session_id = SessionId("hub-life-registry-state".to_string());
        daemon
            .spawn(spawn_request(&session_id), 10)
            .expect("close-suppression spawn");
        assert!(daemon.take_journal_advanced_wake());
        let cursor = daemon
            .lifecycle_baseline()
            .expect("watermark after spawn")
            .cursor;
        let looked_up = daemon
            .session_registry_state(&session_id)
            .expect("exact registry-state query");
        assert!(matches!(
            looked_up,
            SessionRegistryStateLookup::Found(RegistrySessionState::Running)
        ));
        assert!(
            !daemon.take_journal_advanced_wake(),
            "close-suppression query must leave the wake clear"
        );
        let page = daemon
            .lifecycle_changes_page(&cursor, 8, 16 * 1024)
            .expect("page after close-suppression query");
        assert!(page.resync_required.is_none());
        assert!(
            page.changes.is_empty(),
            "close-suppression query must not append lifecycle changes"
        );
        let _ = daemon.shutdown(Some(session_id), 20);
        let _ = fs::remove_dir_all(data_dir);
    }

    #[test]
    fn classify_registry_only_exited_row_is_not_active() {
        let data_dir = temp_data_dir("hub-lifecycle-registry-only-exited");
        let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(&data_dir));
        let session_id = SessionId("hub-registry-only-exited".to_string());
        let mut record = RegistryRecord::running(
            session_id.clone(),
            None,
            ResizePayload { rows: 24, cols: 80 },
            "dummy".to_string(),
            10,
        );
        record.mark(RegistrySessionState::Exited, 11);
        daemon
            .registry()
            .save(&record)
            .expect("seed registry-only Exited row");
        let looked_up = daemon
            .observe_session_lifecycle(&session_id, 12)
            .expect("exact query");
        match &looked_up {
            SessionLifecycleLookup::Found(found) => {
                assert!(
                    found.lifecycle.is_none(),
                    "fresh daemon has no adopted lifecycle: {found:?}"
                );
                assert_eq!(found.session.registry_state, RegistrySessionState::Exited);
            }
            other => panic!("expected Found registry-only row, got {other:?}"),
        }
        assert_eq!(
            classify_session_lifecycle(Ok(looked_up)),
            HubSessionLifecycleClass::Exited
        );
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
