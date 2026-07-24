//! Package runnable entrypoint contract acceptance tests.

use botster_core::{
    validate_package_runnable_entrypoints, ExtensionKind, PackageManifest, RunnableEntrypoint,
    RunnableEntrypointEnvironmentRequirement, RunnableEntrypointHubConnection,
    RunnableEntrypointHubConnectionTransport, RunnableEntrypointHubConnectionValidationError,
    RunnableEntrypointInjection, RunnableEntrypointInjectionKind,
    RunnableEntrypointInjectionTarget, RunnableEntrypointKind, RunnableEntrypointLaunchMode,
    RunnableEntrypointLaunchResult, RunnableEntrypointProcessState, RunnableEntrypointReadiness,
    RunnableEntrypointResultField, RunnableEntrypointValidationError,
    RunnableEntrypointWorkingDirectory,
};

fn required_injections() -> Vec<RunnableEntrypointInjection> {
    vec![
        RunnableEntrypointInjection {
            kind: RunnableEntrypointInjectionKind::HubConnection,
            target: RunnableEntrypointInjectionTarget::Environment {
                name: "BOTSTER_HUB_CONNECTION".to_string(),
            },
            required: true,
            description: Some("Host-selected hub connection descriptor".to_string()),
        },
        RunnableEntrypointInjection {
            kind: RunnableEntrypointInjectionKind::DataDir,
            target: RunnableEntrypointInjectionTarget::Environment {
                name: "BOTSTER_PACKAGE_DATA_DIR".to_string(),
            },
            required: true,
            description: Some("Host-selected package data directory".to_string()),
        },
    ]
}

fn runnable_manifest() -> PackageManifest {
    PackageManifest {
        name: "client-app-package".to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Plugin,
        botster: ">=0.1.0".to_string(),
        source: None,
        capabilities: Vec::new(),
        entrypoints: Vec::new(),
        dependencies: Vec::new(),
        features: Vec::new(),
        host_profile: None,
        configuration: None,
        runnable_entrypoints: vec![
            RunnableEntrypoint {
                id: "web".to_string(),
                kind: RunnableEntrypointKind::WebApp,
                launch_mode: RunnableEntrypointLaunchMode::Background,
                command: "bin/web".to_string(),
                args: vec!["--port".to_string(), "0".to_string()],
                working_directory: Some(RunnableEntrypointWorkingDirectory::PackageRoot),
                injections: required_injections(),
                environment: vec![RunnableEntrypointEnvironmentRequirement {
                    name: "RUST_LOG".to_string(),
                    required: false,
                    default: Some("info".to_string()),
                    description: Some("Package logging level".to_string()),
                }],
                readiness: Some(RunnableEntrypointReadiness {
                    result_fields: vec![RunnableEntrypointResultField::LocalUrl],
                }),
            },
            RunnableEntrypoint {
                id: "terminal".to_string(),
                kind: RunnableEntrypointKind::TerminalApp,
                launch_mode: RunnableEntrypointLaunchMode::ForegroundStdio,
                command: "bin/tui".to_string(),
                args: Vec::new(),
                working_directory: Some(RunnableEntrypointWorkingDirectory::EntrypointDir),
                injections: required_injections(),
                environment: Vec::new(),
                readiness: None,
            },
        ],
        surfaces: Vec::new(),
        navigation: Vec::new(),
    }
}

#[test]
fn package_manifest_without_runnable_entrypoints_keeps_serde_compatibility() {
    let json = serde_json::json!({
        "name": "ordinary-plugin",
        "version": "0.1.0",
        "kind": "plugin",
        "botster": ">=0.1.0",
        "source": null,
        "capabilities": [],
        "entrypoints": []
    });

    let decoded: PackageManifest =
        serde_json::from_value(json).expect("deserialize manifest without runnable entrypoints");

    assert_eq!(decoded.runnable_entrypoints, Vec::new());
    assert_eq!(
        serde_json::to_value(decoded).expect("serialize manifest without runnable entrypoints"),
        serde_json::json!({
            "name": "ordinary-plugin",
            "version": "0.1.0",
            "kind": "plugin",
            "botster": ">=0.1.0",
            "source": null,
            "capabilities": [],
            "entrypoints": []
        })
    );
}

#[test]
fn package_runnable_entrypoints_round_trip_through_package_manifest() {
    let manifest = runnable_manifest();
    let json = serde_json::to_value(&manifest).expect("serialize runnable manifest");

    assert_eq!(json["runnable_entrypoints"][0]["id"], "web");
    assert_eq!(json["runnable_entrypoints"][0]["kind"], "web_app");
    assert_eq!(json["runnable_entrypoints"][0]["launch_mode"], "background");
    assert_eq!(
        json["runnable_entrypoints"][0]["working_directory"]["policy"],
        "package_root"
    );
    assert_eq!(
        json["runnable_entrypoints"][0]["injections"][0]["kind"],
        "hub_connection"
    );
    assert_eq!(
        json["runnable_entrypoints"][0]["readiness"]["result_fields"],
        serde_json::json!(["local_url"])
    );
    assert_eq!(json["runnable_entrypoints"][1]["kind"], "terminal_app");
    assert_eq!(
        json["runnable_entrypoints"][1]["launch_mode"],
        "foreground_stdio"
    );

    let decoded: PackageManifest = serde_json::from_value(json).expect("deserialize manifest");
    assert_eq!(decoded, manifest);
}

#[test]
fn package_runnable_entrypoints_round_trip_relative_workdir_and_argument_injection() {
    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[0].working_directory =
        Some(RunnableEntrypointWorkingDirectory::Relative {
            path: "apps/web".to_string(),
        });
    manifest.runnable_entrypoints[0].injections[0].target =
        RunnableEntrypointInjectionTarget::Argument {
            value: "{{hub_connection}}".to_string(),
        };

    let json = serde_json::to_value(&manifest).expect("serialize runnable manifest");

    assert_eq!(
        json["runnable_entrypoints"][0]["working_directory"],
        serde_json::json!({
            "policy": "relative",
            "path": "apps/web"
        })
    );
    assert_eq!(
        json["runnable_entrypoints"][0]["injections"][0]["target"],
        serde_json::json!({
            "type": "argument",
            "value": "{{hub_connection}}"
        })
    );

    let decoded: PackageManifest = serde_json::from_value(json).expect("deserialize manifest");
    assert_eq!(decoded, manifest);
    assert_eq!(validate_package_runnable_entrypoints(&decoded), Ok(()));
}

#[test]
fn runnable_entrypoint_vocabulary_covers_required_inventory() {
    assert_eq!(
        serde_json::to_value(vec![
            RunnableEntrypointKind::WebApp,
            RunnableEntrypointKind::TerminalApp
        ])
        .expect("serialize kinds"),
        serde_json::json!(["web_app", "terminal_app"])
    );
    assert_eq!(
        serde_json::to_value(vec![
            RunnableEntrypointLaunchMode::Background,
            RunnableEntrypointLaunchMode::ForegroundStdio
        ])
        .expect("serialize launch modes"),
        serde_json::json!(["background", "foreground_stdio"])
    );
    assert_eq!(
        serde_json::to_value(vec![
            RunnableEntrypointInjectionKind::HubConnection,
            RunnableEntrypointInjectionKind::DataDir,
        ])
        .expect("serialize injection kinds"),
        serde_json::json!(["hub_connection", "data_dir"])
    );

    assert!(
        serde_json::from_value::<RunnableEntrypointInjectionKind>(serde_json::json!("hub_socket"))
            .is_err(),
        "obsolete raw socket injection kind must be rejected"
    );
}

#[test]
fn package_runnable_entrypoint_validation_rejects_missing_ids_commands_and_injections() {
    let mut manifest = runnable_manifest();

    assert_eq!(validate_package_runnable_entrypoints(&manifest), Ok(()));

    manifest.runnable_entrypoints[0].id = " ".to_string();
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(RunnableEntrypointValidationError::BlankId)
    );

    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[1].id = "web".to_string();
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(RunnableEntrypointValidationError::DuplicateId(
            "web".to_string()
        ))
    );

    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[0].command = " ".to_string();
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(RunnableEntrypointValidationError::BlankCommand(
            "web".to_string()
        ))
    );

    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[0].injections.remove(0);
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(
            RunnableEntrypointValidationError::MissingRequiredInjection {
                entrypoint_id: "web".to_string(),
                kind: RunnableEntrypointInjectionKind::HubConnection,
            }
        )
    );
}

#[test]
fn runnable_entrypoint_hub_connection_pins_exact_json_and_round_trips() {
    let connection = RunnableEntrypointHubConnection {
        transport: RunnableEntrypointHubConnectionTransport::UnixSocket {
            path: "/var/run/botster/hub.sock".to_string(),
        },
    };

    assert_eq!(connection.validate(), Ok(()));
    assert_eq!(
        serde_json::to_value(&connection).expect("serialize Hub connection"),
        serde_json::json!({
            "transport": {
                "type": "unix_socket",
                "path": "/var/run/botster/hub.sock"
            }
        })
    );

    let decoded: RunnableEntrypointHubConnection = serde_json::from_value(serde_json::json!({
        "transport": {
            "type": "unix_socket",
            "path": "/var/run/botster/hub.sock"
        }
    }))
    .expect("deserialize Hub connection");
    assert_eq!(decoded, connection);
}

#[test]
fn runnable_entrypoint_hub_connection_serde_rejects_open_or_malformed_shapes() {
    let invalid = [
        serde_json::json!({}),
        serde_json::json!({
            "transport": {
                "type": "unix_socket"
            }
        }),
        serde_json::json!({
            "transport": {
                "type": "tcp",
                "path": "/var/run/botster/hub.sock"
            }
        }),
        serde_json::json!({
            "transport": "unix_socket"
        }),
        serde_json::json!({
            "transport": {
                "type": "unix_socket",
                "path": "/var/run/botster/hub.sock",
                "mode": "rw"
            }
        }),
        serde_json::json!({
            "transport": {
                "type": "unix_socket",
                "path": "/var/run/botster/hub.sock"
            },
            "credentials": "invented"
        }),
    ];

    for value in invalid {
        assert!(
            serde_json::from_value::<RunnableEntrypointHubConnection>(value).is_err(),
            "open or malformed Hub connection shape must fail"
        );
    }
}

#[test]
fn runnable_entrypoint_hub_connection_validation_rejects_every_invalid_path_class() {
    for path in ["", "   "] {
        let connection = RunnableEntrypointHubConnection {
            transport: RunnableEntrypointHubConnectionTransport::UnixSocket {
                path: path.to_string(),
            },
        };
        assert_eq!(
            connection.validate(),
            Err(RunnableEntrypointHubConnectionValidationError::BlankUnixSocketPath)
        );
    }

    for path in ["hub.sock", "./hub.sock", "var/run/hub.sock"] {
        let connection = RunnableEntrypointHubConnection {
            transport: RunnableEntrypointHubConnectionTransport::UnixSocket {
                path: path.to_string(),
            },
        };
        assert_eq!(
            connection.validate(),
            Err(
                RunnableEntrypointHubConnectionValidationError::RelativeUnixSocketPath(
                    path.to_string()
                )
            )
        );
    }
}

#[test]
fn hub_shaped_producer_serializes_connection_for_arbitrary_manifest_targets() {
    let connection = RunnableEntrypointHubConnection {
        transport: RunnableEntrypointHubConnectionTransport::UnixSocket {
            path: "/private/run/selected-hub.sock".to_string(),
        },
    };
    let serialized = serde_json::to_string(&connection).expect("serialize Hub-owned descriptor");

    let targets = [
        RunnableEntrypointInjectionTarget::Environment {
            name: "PACKAGE_SELECTED_CONNECTION".to_string(),
        },
        RunnableEntrypointInjectionTarget::Argument {
            value: "{{portable_connection}}".to_string(),
        },
    ];

    for target in targets {
        let injected = match target {
            RunnableEntrypointInjectionTarget::Environment { name } => {
                format!("{name}={serialized}")
            }
            RunnableEntrypointInjectionTarget::Argument { value } => {
                value.replace("{{portable_connection}}", &serialized)
            }
        };
        assert!(injected.contains(
            r#"{"transport":{"type":"unix_socket","path":"/private/run/selected-hub.sock"}}"#
        ));
    }
}

#[test]
fn package_runnable_entrypoint_validation_rejects_blank_metadata_values() {
    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[0].working_directory =
        Some(RunnableEntrypointWorkingDirectory::Relative {
            path: " ".to_string(),
        });
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(RunnableEntrypointValidationError::BlankRelativeWorkingDirectory("web".to_string()))
    );

    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[0].injections[0].target =
        RunnableEntrypointInjectionTarget::Environment {
            name: " ".to_string(),
        };
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(RunnableEntrypointValidationError::BlankInjectionEnvironment("web".to_string()))
    );

    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[0].injections[0].target =
        RunnableEntrypointInjectionTarget::Argument {
            value: " ".to_string(),
        };
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(RunnableEntrypointValidationError::BlankInjectionArgument(
            "web".to_string()
        ))
    );

    let mut manifest = runnable_manifest();
    manifest.runnable_entrypoints[0].environment[0].name = " ".to_string();
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(RunnableEntrypointValidationError::BlankEnvironmentRequirement("web".to_string()))
    );
}

#[test]
fn runnable_entrypoint_launch_result_carries_structured_output_without_policy() {
    let result = RunnableEntrypointLaunchResult {
        entrypoint_id: "web".to_string(),
        process_state: RunnableEntrypointProcessState::Running,
        local_url: Some("http://127.0.0.1:49152".to_string()),
    };

    assert_eq!(
        serde_json::to_value(result).expect("serialize launch result"),
        serde_json::json!({
            "entrypoint_id": "web",
            "process_state": "running",
            "local_url": "http://127.0.0.1:49152"
        })
    );
}

#[test]
fn runnable_entrypoint_launch_result_process_state_defaults_to_not_started() {
    let result: RunnableEntrypointLaunchResult = serde_json::from_value(serde_json::json!({
        "entrypoint_id": "web"
    }))
    .expect("deserialize launch result without process state");

    assert_eq!(result.entrypoint_id, "web");
    assert_eq!(
        result.process_state,
        RunnableEntrypointProcessState::NotStarted
    );
    assert_eq!(result.local_url, None);
}

#[test]
fn package_runnable_entrypoint_example_deserializes_and_validates() {
    let example = include_str!("../../../docs/examples/package-runnable-entrypoints.json");
    let manifest: PackageManifest =
        serde_json::from_str(example).expect("deserialize runnable entrypoint example");

    assert_eq!(manifest.name, "runnable-demo");
    assert_eq!(manifest.runnable_entrypoints.len(), 2);
    assert_eq!(
        manifest.runnable_entrypoints[0].kind,
        RunnableEntrypointKind::WebApp
    );
    assert_eq!(
        manifest.runnable_entrypoints[1].launch_mode,
        RunnableEntrypointLaunchMode::ForegroundStdio
    );
    assert_eq!(validate_package_runnable_entrypoints(&manifest), Ok(()));
}
