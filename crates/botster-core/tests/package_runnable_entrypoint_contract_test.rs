//! Package runnable entrypoint contract acceptance tests.

use botster_core::{
    validate_package_runnable_entrypoints, ExtensionKind, PackageManifest, RunnableEntrypoint,
    RunnableEntrypointEnvironmentRequirement, RunnableEntrypointInjection,
    RunnableEntrypointInjectionKind, RunnableEntrypointInjectionTarget, RunnableEntrypointKind,
    RunnableEntrypointLaunchMode, RunnableEntrypointLaunchResult, RunnableEntrypointProcessState,
    RunnableEntrypointReadiness, RunnableEntrypointResultField, RunnableEntrypointValidationError,
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
        RunnableEntrypointInjection {
            kind: RunnableEntrypointInjectionKind::HubSocket,
            target: RunnableEntrypointInjectionTarget::Environment {
                name: "BOTSTER_HUB_SOCKET".to_string(),
            },
            required: true,
            description: Some("Host-selected hub socket".to_string()),
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
                process_state: RunnableEntrypointProcessState::NotStarted,
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
                process_state: RunnableEntrypointProcessState::NotStarted,
            },
        ],
        surfaces: Vec::new(),
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
    assert!(json["runnable_entrypoints"][0]
        .get("process_state")
        .is_none());

    let decoded: PackageManifest = serde_json::from_value(json).expect("deserialize manifest");
    assert_eq!(decoded, manifest);
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
            RunnableEntrypointInjectionKind::HubSocket,
        ])
        .expect("serialize injection kinds"),
        serde_json::json!(["hub_connection", "data_dir", "hub_socket"])
    );
}

#[test]
fn package_runnable_entrypoint_validation_rejects_contract_violations() {
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
    manifest.runnable_entrypoints[0].injections.pop();
    assert_eq!(
        validate_package_runnable_entrypoints(&manifest),
        Err(
            RunnableEntrypointValidationError::MissingRequiredInjection {
                entrypoint_id: "web".to_string(),
                kind: RunnableEntrypointInjectionKind::HubSocket,
            }
        )
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
