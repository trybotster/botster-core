//! Package dependency and feature-gate contract acceptance tests.

use botster_core::{
    resolve_package_dependencies, Capability, CapabilitySurface, ExtensionKind, PackageAuthState,
    PackageBlockedReason, PackageConfigState, PackageDependency, PackageDependencyKind,
    PackageFeatureGate, PackageManifest, PackageRequirement, PackageRequirementStatus,
    PackageResolutionInput, PackageResolutionPackage, PackageResolutionState,
};

fn network_capability() -> Capability {
    Capability {
        surface: CapabilitySurface::Network,
        scope: Some("api".to_string()),
    }
}

fn marketplace_manifest() -> PackageManifest {
    PackageManifest {
        name: "workflow-plugin".to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Plugin,
        botster: ">=0.1.0".to_string(),
        source: None,
        capabilities: Vec::new(),
        entrypoints: Vec::new(),
        dependencies: vec![
            PackageDependency {
                id: "storage".to_string(),
                package: "storage-provider".to_string(),
                kind: PackageDependencyKind::Required,
                feature: None,
                requirements: Vec::new(),
            },
            PackageDependency {
                id: "issue-provider".to_string(),
                package: "issue-provider".to_string(),
                kind: PackageDependencyKind::Optional,
                feature: Some("issues".to_string()),
                requirements: vec![PackageRequirement::Provider {
                    provider: "issues".to_string(),
                }],
            },
            PackageDependency {
                id: "api-provider".to_string(),
                package: "api-provider".to_string(),
                kind: PackageDependencyKind::Optional,
                feature: Some("api_sync".to_string()),
                requirements: vec![PackageRequirement::Capability {
                    capability: network_capability(),
                }],
            },
        ],
        features: vec![
            PackageFeatureGate {
                id: "issues".to_string(),
                label: "Issues".to_string(),
                description: Some("Issue integration".to_string()),
                dependencies: vec!["issue-provider".to_string()],
                requirements: Vec::new(),
            },
            PackageFeatureGate {
                id: "api_sync".to_string(),
                label: "API sync".to_string(),
                description: None,
                dependencies: vec!["api-provider".to_string()],
                requirements: vec![
                    PackageRequirement::Auth {
                        key: "api_token".to_string(),
                    },
                    PackageRequirement::Config {
                        key: "endpoint".to_string(),
                    },
                ],
            },
        ],
        host_profile: None,
        configuration: None,
        runnable_entrypoints: Vec::new(),
        surfaces: Vec::new(),
        navigation: Vec::new(),
    }
}

fn enabled_package(name: &str) -> PackageResolutionPackage {
    PackageResolutionPackage {
        name: name.to_string(),
        enabled: true,
        providers: Vec::new(),
        capabilities: Vec::new(),
    }
}

fn capability_feature_manifest() -> PackageManifest {
    PackageManifest {
        name: "capability-feature-plugin".to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Plugin,
        botster: ">=0.1.0".to_string(),
        source: None,
        capabilities: Vec::new(),
        entrypoints: Vec::new(),
        dependencies: Vec::new(),
        features: vec![PackageFeatureGate {
            id: "network_sync".to_string(),
            label: "Network sync".to_string(),
            description: None,
            dependencies: Vec::new(),
            requirements: vec![PackageRequirement::Capability {
                capability: network_capability(),
            }],
        }],
        host_profile: None,
        configuration: None,
        runnable_entrypoints: Vec::new(),
        surfaces: Vec::new(),
        navigation: Vec::new(),
    }
}

#[test]
fn package_manifest_without_dependencies_keeps_serde_compatibility() {
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
        serde_json::from_value(json).expect("deserialize manifest without dependency metadata");

    assert_eq!(decoded.dependencies, Vec::new());
    assert_eq!(decoded.features, Vec::new());
    assert_eq!(
        serde_json::to_value(decoded).expect("serialize manifest without dependency metadata"),
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
fn package_dependency_manifest_round_trips_feature_gates_and_requirements() {
    let manifest = marketplace_manifest();
    let json = serde_json::to_value(&manifest).expect("serialize dependency manifest");

    assert_eq!(json["dependencies"][0]["kind"], "required");
    assert_eq!(json["dependencies"][1]["feature"], "issues");
    assert_eq!(
        json["dependencies"][1]["requirements"][0],
        serde_json::json!({ "type": "provider", "provider": "issues" })
    );
    assert_eq!(json["features"][0]["id"], "issues");
    assert_eq!(
        json["features"][1]["requirements"][0],
        serde_json::json!({ "type": "auth", "key": "api_token" })
    );

    let decoded: PackageManifest =
        serde_json::from_value(json).expect("deserialize dependency manifest");
    assert_eq!(decoded, manifest);
}

#[test]
fn hard_dependency_blocks_when_required_package_is_missing() {
    let matrix = resolve_package_dependencies(
        &marketplace_manifest(),
        &PackageResolutionInput {
            packages: Vec::new(),
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: Vec::new(),
            config: Vec::new(),
        },
    );

    assert_eq!(matrix.dependencies[0].id, "storage");
    assert_eq!(
        matrix.dependencies[0].state,
        PackageResolutionState::Blocked
    );
    assert_eq!(
        matrix.dependencies[0].blocked_reasons,
        vec![PackageBlockedReason::MissingPackage {
            package: "storage-provider".to_string(),
        }]
    );
}

#[test]
fn optional_provider_feature_blocks_when_dependency_package_is_missing() {
    let matrix = resolve_package_dependencies(
        &marketplace_manifest(),
        &PackageResolutionInput {
            packages: vec![enabled_package("storage-provider")],
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: Vec::new(),
            config: Vec::new(),
        },
    );

    assert_eq!(matrix.features[0].id, "issues");
    assert_eq!(matrix.features[0].state, PackageResolutionState::Blocked);
    assert_eq!(
        matrix.features[0].blocked_reasons,
        vec![PackageBlockedReason::MissingPackage {
            package: "issue-provider".to_string(),
        }]
    );
}

#[test]
fn optional_provider_feature_blocks_when_dependency_package_is_disabled() {
    let matrix = resolve_package_dependencies(
        &marketplace_manifest(),
        &PackageResolutionInput {
            packages: vec![
                enabled_package("storage-provider"),
                PackageResolutionPackage {
                    name: "issue-provider".to_string(),
                    enabled: false,
                    providers: vec!["issues".to_string()],
                    capabilities: Vec::new(),
                },
            ],
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: Vec::new(),
            config: Vec::new(),
        },
    );

    assert_eq!(matrix.features[0].state, PackageResolutionState::Blocked);
    assert_eq!(
        matrix.features[0].blocked_reasons,
        vec![PackageBlockedReason::DisabledPackage {
            package: "issue-provider".to_string(),
        }]
    );
}

#[test]
fn optional_provider_feature_blocks_when_enabled_dependency_lacks_provider() {
    let matrix = resolve_package_dependencies(
        &marketplace_manifest(),
        &PackageResolutionInput {
            packages: vec![
                enabled_package("storage-provider"),
                enabled_package("issue-provider"),
            ],
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: Vec::new(),
            config: Vec::new(),
        },
    );

    assert_eq!(matrix.features[0].state, PackageResolutionState::Blocked);
    assert_eq!(
        matrix.features[0].blocked_reasons,
        vec![PackageBlockedReason::MissingProvider {
            provider: "issues".to_string(),
        }]
    );
}

#[test]
fn feature_blocks_when_auth_or_config_is_missing() {
    let matrix = resolve_package_dependencies(
        &marketplace_manifest(),
        &PackageResolutionInput {
            packages: vec![
                enabled_package("storage-provider"),
                PackageResolutionPackage {
                    name: "api-provider".to_string(),
                    enabled: true,
                    providers: Vec::new(),
                    capabilities: vec![network_capability()],
                },
            ],
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: vec![PackageAuthState {
                key: "api_token".to_string(),
                status: PackageRequirementStatus::Missing,
            }],
            config: vec![PackageConfigState {
                key: "endpoint".to_string(),
                status: PackageRequirementStatus::Missing,
            }],
        },
    );

    assert_eq!(matrix.features[1].id, "api_sync");
    assert_eq!(matrix.features[1].state, PackageResolutionState::Blocked);
    assert_eq!(
        matrix.features[1].blocked_reasons,
        vec![
            PackageBlockedReason::MissingAuth {
                key: "api_token".to_string(),
            },
            PackageBlockedReason::MissingConfig {
                key: "endpoint".to_string(),
            },
        ]
    );
}

#[test]
fn feature_capability_requirement_blocks_and_resolves_from_host_capabilities() {
    let blocked = resolve_package_dependencies(
        &capability_feature_manifest(),
        &PackageResolutionInput {
            packages: Vec::new(),
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: Vec::new(),
            config: Vec::new(),
        },
    );

    assert_eq!(blocked.features[0].state, PackageResolutionState::Blocked);
    assert_eq!(
        blocked.features[0].blocked_reasons,
        vec![PackageBlockedReason::MissingCapability {
            package: None,
            capability: network_capability(),
        }]
    );

    let available = resolve_package_dependencies(
        &capability_feature_manifest(),
        &PackageResolutionInput {
            packages: Vec::new(),
            providers: Vec::new(),
            capabilities: vec![network_capability()],
            auth: Vec::new(),
            config: Vec::new(),
        },
    );

    assert_eq!(
        available.features[0].state,
        PackageResolutionState::Available
    );
    assert!(available.features[0].blocked_reasons.is_empty());
}

#[test]
fn dependency_and_feature_are_available_once_requirements_are_met() {
    let matrix = resolve_package_dependencies(
        &marketplace_manifest(),
        &PackageResolutionInput {
            packages: vec![
                enabled_package("storage-provider"),
                PackageResolutionPackage {
                    name: "issue-provider".to_string(),
                    enabled: true,
                    providers: vec!["issues".to_string()],
                    capabilities: Vec::new(),
                },
                PackageResolutionPackage {
                    name: "api-provider".to_string(),
                    enabled: true,
                    providers: Vec::new(),
                    capabilities: vec![network_capability()],
                },
            ],
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: vec![PackageAuthState {
                key: "api_token".to_string(),
                status: PackageRequirementStatus::Configured,
            }],
            config: vec![PackageConfigState {
                key: "endpoint".to_string(),
                status: PackageRequirementStatus::Configured,
            }],
        },
    );

    assert!(matrix
        .dependencies
        .iter()
        .all(|dependency| dependency.state == PackageResolutionState::Available));
    assert!(matrix
        .features
        .iter()
        .all(|feature| feature.state == PackageResolutionState::Available));
}

#[test]
fn package_dependency_example_deserializes_and_resolves_deterministically() {
    let example = include_str!("../../../docs/examples/package-dependencies.json");
    let manifest: PackageManifest =
        serde_json::from_str(example).expect("deserialize package dependency example");
    let matrix = resolve_package_dependencies(
        &manifest,
        &PackageResolutionInput {
            packages: vec![enabled_package("storage-provider")],
            providers: Vec::new(),
            capabilities: Vec::new(),
            auth: Vec::new(),
            config: Vec::new(),
        },
    );

    assert_eq!(manifest.dependencies.len(), 2);
    assert_eq!(manifest.features.len(), 1);
    assert_eq!(
        serde_json::to_value(matrix.features[0].blocked_reasons.clone())
            .expect("serialize blocked reasons"),
        serde_json::json!([
            { "type": "missing_package", "package": "issue-provider" },
            { "type": "missing_auth", "key": "issue_token" },
            { "type": "missing_config", "key": "issue_endpoint" }
        ])
    );
}
