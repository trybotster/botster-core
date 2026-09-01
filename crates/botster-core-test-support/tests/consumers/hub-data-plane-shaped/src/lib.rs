//! Isolated Hub-shaped owner thread for the Core wake pump seam.

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    use botster_core::{
        ClientId, CoreSessionMetadata, RequestId, ResizePayload, SessionId,
        SessionSpawnRequest, SpawnEnvironment, SpawnWorkingDirectory, SubscriptionId,
        TerminalCapabilitySet,
    };
    use botster_core_daemon::{
        CoreDaemon, CoreDaemonConfig, SpawnSessionRequest, WakePumpWait,
    };
    use botster_core_test_support::terminal_adapter::SharedFakeTerminalAdapter;

    enum HubRequest {
        Exercise {
            completed: mpsc::SyncSender<()>,
        },
    }

    fn exercise_control_path(daemon: &mut CoreDaemon) {
        let session_id = SessionId("hub-data-plane-session".into());
        let client_id = ClientId("hub-data-plane-client".into());
        let subscription_id = SubscriptionId("hub-data-plane-sub".into());
        daemon
            .spawn(
                SpawnSessionRequest {
                    request: SessionSpawnRequest {
                        request_id: RequestId("hub-data-plane-spawn".into()),
                        session_id: session_id.clone(),
                        executable: "sh".into(),
                        arguments: vec!["-c".into(), "cat".into()],
                        working_directory: SpawnWorkingDirectory { path: ".".into() },
                        environment: SpawnEnvironment::default(),
                        initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
                    },
                    metadata: CoreSessionMetadata::new(),
                },
                1,
            )
            .expect("spawn");
        daemon
            .expect_terminal_adapter(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
            )
            .expect("declare adapter");
        daemon
            .attach(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                2,
            )
            .expect("attach");
        let generation = daemon
            .terminal_subscription_generation(&session_id, &subscription_id)
            .expect("generation");
        daemon
            .bind_waking_terminal_adapter(
                client_id.clone(),
                session_id.clone(),
                subscription_id.clone(),
                generation,
                TerminalCapabilitySet::empty(),
                Box::new(SharedFakeTerminalAdapter::auto_complete()),
            )
            .expect("bind waking adapter");
        daemon
            .input(client_id.clone(), session_id.clone(), b"hello\n".to_vec(), 3)
            .expect("input");
        daemon
            .resize(client_id.clone(), session_id.clone(), 30, 100, 4)
            .expect("resize");
        let _ = daemon
            .session_registry_state(&session_id)
            .expect("exact registry state");
        daemon
            .detach(client_id, session_id, subscription_id, 5)
            .expect("detach");
    }

    #[test]
    fn daemon_is_constructed_driven_stopped_and_shutdown_on_one_thread() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let data_dir = std::env::temp_dir().join(format!("hub-data-plane-{nonce}"));
        let (request_tx, request_rx) = mpsc::sync_channel::<HubRequest>(4);
        let (control_tx, control_rx) = mpsc::sync_channel(1);

        let owner = std::thread::spawn(move || {
            let mut daemon = CoreDaemon::new(CoreDaemonConfig::new(data_dir));
            control_tx
                .send(daemon.wake_pump_control())
                .expect("publish control");
            loop {
                match daemon.wait_pump(Duration::from_secs(30)) {
                    WakePumpWait::Wakes(batch) => {
                        daemon.pump_woken(&batch, 6).expect("targeted pump");
                    }
                    WakePumpWait::Interrupted => {}
                    WakePumpWait::Stopped => break,
                    _ => continue,
                }
                for request in request_rx.try_iter().take(4) {
                    match request {
                        HubRequest::Exercise { completed } => {
                            exercise_control_path(&mut daemon);
                            completed.send(()).expect("request completion");
                        }
                    }
                }
            }
            daemon.shutdown(None, 7).expect("ordered Core shutdown");
        });

        let control = control_rx.recv().expect("receive control");
        let (completed_tx, completed_rx) = mpsc::sync_channel(1);
        request_tx
            .send(HubRequest::Exercise {
                completed: completed_tx,
            })
            .expect("bounded request admission");
        control.interrupt();
        completed_rx.recv().expect("bounded request completed");
        control.request_stop();
        owner.join().expect("owner thread joined");
    }
}
