//! Local process runtime acceptance tests.

#[cfg(unix)]
mod unix {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{Duration, Instant};

    use botster_core::{
        LocalProcessRuntime, RequestId, ResizePayload, SessionId, SessionRuntime,
        SessionRuntimeErrorKind, SessionRuntimeInput, SessionRuntimeOutput, SessionSpawnRequest,
        SpawnEnvironment, SpawnEnvironmentVariable, SpawnWorkingDirectory,
    };

    fn session_id(value: &str) -> SessionId {
        SessionId(value.to_string())
    }

    fn request_id(value: &str) -> RequestId {
        RequestId(value.to_string())
    }

    fn shell_request(session_id: SessionId, script: &str) -> SessionSpawnRequest {
        SessionSpawnRequest {
            request_id: request_id("local-runtime-request"),
            session_id,
            executable: "sh".to_string(),
            arguments: vec!["-c".to_string(), script.to_string()],
            working_directory: SpawnWorkingDirectory {
                path: ".".to_string(),
            },
            environment: SpawnEnvironment::default(),
            initial_pty_size: Some(ResizePayload { rows: 24, cols: 80 }),
        }
    }

    fn collect_until<F>(
        runtime: &mut dyn SessionRuntime,
        session_id: &SessionId,
        mut predicate: F,
    ) -> Vec<SessionRuntimeOutput>
    where
        F: FnMut(&[SessionRuntimeOutput]) -> bool,
    {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut output = Vec::new();

        while Instant::now() < deadline {
            output.extend(
                runtime
                    .drain_output(session_id)
                    .expect("drain local process runtime output"),
            );
            if predicate(&output) {
                return output;
            }
            std::thread::sleep(Duration::from_millis(20));
        }

        output
    }

    fn output_text(output: &[SessionRuntimeOutput]) -> String {
        let bytes: Vec<u8> = output
            .iter()
            .filter_map(|event| match event {
                SessionRuntimeOutput::PtyOutput { data, .. } => Some(data.as_slice()),
                SessionRuntimeOutput::ProcessExited { .. } => None,
            })
            .flatten()
            .copied()
            .collect();
        String::from_utf8_lossy(&bytes).into_owned()
    }

    fn has_exit(output: &[SessionRuntimeOutput]) -> bool {
        output
            .iter()
            .any(|event| matches!(event, SessionRuntimeOutput::ProcessExited { .. }))
    }

    fn source_files(root: impl AsRef<Path>) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let mut pending = vec![root.as_ref().to_path_buf()];

        while let Some(path) = pending.pop() {
            for entry in fs::read_dir(path).expect("read source directory") {
                let entry = entry.expect("read source entry");
                let path = entry.path();

                if path.is_dir() {
                    pending.push(path);
                } else if path
                    .extension()
                    .is_some_and(|extension| extension == "rs" || extension == "md")
                {
                    files.push(path);
                }
            }
        }

        files.sort();
        files
    }

    #[test]
    fn local_process_runtime_spawns_simple_command_and_drains_output() {
        let mut runtime = LocalProcessRuntime::new();
        let session_id = session_id("local-runtime-output");

        runtime
            .spawn_session(shell_request(
                session_id.clone(),
                "printf 'botster-local-output\\n'",
            ))
            .expect("spawn local command");

        let output = collect_until(&mut runtime, &session_id, |output| {
            output_text(output).contains("botster-local-output") && has_exit(output)
        });

        assert!(output_text(&output).contains("botster-local-output"));
        assert!(has_exit(&output), "expected process exit, got {output:?}");
    }

    #[test]
    fn local_process_runtime_writes_input_to_pty() {
        let mut runtime = LocalProcessRuntime::new();
        let session_id = session_id("local-runtime-input");

        runtime
            .spawn_session(shell_request(session_id.clone(), "cat"))
            .expect("spawn echoing local command");
        runtime
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: b"botster-input-marker\n".to_vec(),
            })
            .expect("write local pty input");

        let output = collect_until(&mut runtime, &session_id, |output| {
            output_text(output).contains("botster-input-marker")
        });
        runtime
            .send_input(SessionRuntimeInput::Shutdown {
                session_id: session_id.clone(),
            })
            .expect("shutdown echoing local command");

        assert!(output_text(&output).contains("botster-input-marker"));
    }

    #[test]
    fn local_process_runtime_resizes_pty_when_supported() {
        let mut runtime = LocalProcessRuntime::new();
        let session_id = session_id("local-runtime-resize");

        runtime
            .spawn_session(shell_request(session_id.clone(), "sleep 1"))
            .expect("spawn resizable local command");

        runtime
            .send_input(SessionRuntimeInput::Resize {
                session_id: session_id.clone(),
                size: ResizePayload {
                    rows: 33,
                    cols: 120,
                },
            })
            .expect("resize local pty");
        runtime
            .send_input(SessionRuntimeInput::Shutdown { session_id })
            .expect("shutdown resized local command");
    }

    #[test]
    fn local_process_runtime_reports_spawn_failure() {
        let mut runtime = LocalProcessRuntime::new();
        let mut request = shell_request(session_id("local-runtime-spawn-failure"), "exit 0");
        request.executable = "definitely-missing-botster-core-runtime-test-command".to_string();

        let error = runtime
            .spawn_session(request)
            .expect_err("missing executable should fail");

        assert_eq!(error.kind, SessionRuntimeErrorKind::SpawnFailed);
        assert!(
            error
                .message
                .contains("definitely-missing-botster-core-runtime-test-command"),
            "spawn failure should include requested executable"
        );
    }

    #[test]
    fn local_process_runtime_reports_session_not_found() {
        let mut runtime = LocalProcessRuntime::new();
        let session_id = session_id("local-runtime-missing");

        let input_error = runtime
            .send_input(SessionRuntimeInput::PtyInput {
                session_id: session_id.clone(),
                data: b"ignored".to_vec(),
            })
            .expect_err("missing session input should fail");
        let output_error = runtime
            .drain_output(&session_id)
            .expect_err("missing session output should fail");

        assert_eq!(input_error.kind, SessionRuntimeErrorKind::SessionNotFound);
        assert_eq!(output_error.kind, SessionRuntimeErrorKind::SessionNotFound);
    }

    #[test]
    fn local_process_runtime_shutdown_cleans_up_child() {
        let mut runtime = LocalProcessRuntime::new();
        let session_id = session_id("local-runtime-shutdown");

        runtime
            .spawn_session(shell_request(session_id.clone(), "sleep 30"))
            .expect("spawn long running local command");
        runtime
            .send_input(SessionRuntimeInput::Shutdown {
                session_id: session_id.clone(),
            })
            .expect("shutdown long running local command");

        let output = collect_until(&mut runtime, &session_id, has_exit);

        assert!(has_exit(&output), "expected shutdown exit, got {output:?}");
    }

    #[test]
    fn local_process_runtime_can_be_used_through_public_session_runtime_trait() {
        let mut runtime: Box<dyn SessionRuntime> = Box::new(LocalProcessRuntime::new());
        let session_id = session_id("local-runtime-trait-object");
        let mut request = shell_request(
            session_id.clone(),
            "printf 'trait-runtime env-%s\\n' \"$BOTSTER_CORE_LOCAL_RUNTIME_TEST\"",
        );
        request
            .environment
            .variables
            .push(SpawnEnvironmentVariable {
                name: "BOTSTER_CORE_LOCAL_RUNTIME_TEST".to_string(),
                value: "1".to_string(),
            });

        let handle = runtime
            .spawn_session(request)
            .expect("spawn through public trait object");
        assert_eq!(handle.session_id, session_id);

        let output = collect_until(runtime.as_mut(), &handle.session_id, |output| {
            output_text(output).contains("trait-runtime env-1") && has_exit(output)
        });

        assert!(output_text(&output).contains("trait-runtime env-1"));
        assert!(has_exit(&output), "expected process exit, got {output:?}");
    }

    #[test]
    fn local_runtime_docs_and_tests_do_not_embed_private_paths_or_pii() {
        let banned_terms = [
            ["/", "Users", "/"].concat(),
            ["jason", "conigliari"].concat(),
            ["Project", "Pipelines"].concat(),
        ];
        let mut files = source_files("src/runtime");
        files.push(PathBuf::from("tests/local_process_runtime_test.rs"));
        files.push(PathBuf::from("../../README.md"));

        for source_file in files {
            let source = fs::read_to_string(&source_file).expect("read source file");

            for term in &banned_terms {
                assert!(
                    !source.contains(term.as_str()),
                    "local runtime file {} must not contain banned term {term}",
                    source_file.display()
                );
            }
        }
    }
}
