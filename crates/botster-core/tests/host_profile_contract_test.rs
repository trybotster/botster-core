//! Host-profile package contract acceptance tests.

use botster_core::{
    admit_host_profile, Capability, CapabilitySurface, ExtensionEntrypoint, ExtensionKind,
    ExtensionRuntime, HostProfileAdmissionError, HostProfileCompatibilityField,
    HostProfileMetadata, HostProfilePolicySection, PackageManifest, PackageSource,
};

const HOST_BOTSTER_VERSION: &str = "0.1.2";

fn network_capability() -> Capability {
    Capability {
        surface: CapabilitySurface::Network,
        scope: Some("api".to_string()),
    }
}

fn host_profile_metadata(required_capabilities: Vec<Capability>) -> HostProfileMetadata {
    HostProfileMetadata {
        profile_id: "botster-hub".to_string(),
        compatibility: ">=0.1.0".to_string(),
        precedence: 10,
        required_providers: vec!["network-provider".to_string()],
        required_capabilities,
        policy_sections: vec![
            HostProfilePolicySection::Startup,
            HostProfilePolicySection::Capabilities,
            HostProfilePolicySection::ClientAdmission,
        ],
    }
}

fn provider_manifest() -> PackageManifest {
    let capability = network_capability();

    PackageManifest {
        name: "botster-hub-profile".to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Provider,
        botster: ">=0.1.0".to_string(),
        source: Some(PackageSource::Git {
            repo: "https://example.invalid/botster-hub-profile.git".to_string(),
            reference: "v0.1.0".to_string(),
        }),
        capabilities: vec![capability.clone()],
        entrypoints: vec![ExtensionEntrypoint {
            runtime: ExtensionRuntime::Lua,
            path: "profile.lua".to_string(),
            bootstrap: true,
        }],
        dependencies: Vec::new(),
        features: Vec::new(),
        host_profile: Some(host_profile_metadata(vec![capability])),
        configuration: None,
        runnable_entrypoints: Vec::new(),
        surfaces: Vec::new(),
        navigation: Vec::new(),
    }
}

#[test]
fn host_profile_metadata_round_trips_through_package_manifest() {
    let manifest = provider_manifest();
    let json = serde_json::to_value(&manifest).expect("serialize host profile manifest");

    assert_eq!(json["host_profile"]["profile_id"], "botster-hub");
    assert_eq!(
        json["host_profile"]["policy_sections"],
        serde_json::json!(["startup", "capabilities", "client_admission"])
    );

    let decoded: PackageManifest =
        serde_json::from_value(json).expect("deserialize host profile manifest");
    assert_eq!(decoded, manifest);
}

#[test]
fn package_manifest_without_host_profile_keeps_serde_compatibility() {
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
        serde_json::from_value(json).expect("deserialize legacy manifest shape");

    assert_eq!(decoded.host_profile, None);
    assert_eq!(
        serde_json::to_value(decoded).expect("serialize legacy manifest shape"),
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
fn enabled_provider_with_source_bootstrap_and_required_capabilities_admits() {
    let manifest = provider_manifest();
    let admitted = admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION)
        .expect("admit provider host profile");

    assert_eq!(admitted.package_name, "botster-hub-profile");
    assert_eq!(admitted.package_version, "0.1.0");
    assert_eq!(
        admitted.metadata,
        host_profile_metadata(vec![network_capability()])
    );
}

#[test]
fn exact_compatibility_requirements_are_admitted() {
    let mut manifest = provider_manifest();
    manifest.botster = HOST_BOTSTER_VERSION.to_string();
    manifest
        .host_profile
        .as_mut()
        .expect("host profile metadata")
        .compatibility = HOST_BOTSTER_VERSION.to_string();

    assert!(admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION).is_ok());
}

#[test]
fn lower_bound_compatibility_requirements_are_admitted() {
    let manifest = provider_manifest();

    assert!(admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION).is_ok());
}

#[test]
fn ordinary_plugin_with_host_profile_metadata_is_rejected() {
    let mut manifest = provider_manifest();
    manifest.kind = ExtensionKind::Plugin;

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::NotProvider)
    );
}

#[test]
fn provider_without_host_profile_metadata_is_rejected() {
    let mut manifest = provider_manifest();
    manifest.host_profile = None;

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::MissingMetadata)
    );
}

#[test]
fn disabled_provider_is_rejected() {
    let manifest = provider_manifest();

    assert_eq!(
        admit_host_profile(&manifest, false, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::Disabled)
    );
}

#[test]
fn provider_without_source_provenance_is_rejected() {
    let mut manifest = provider_manifest();
    manifest.source = None;

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::MissingSource)
    );
}

#[test]
fn provider_without_bootstrap_entrypoint_is_rejected() {
    let mut manifest = provider_manifest();
    manifest.entrypoints[0].bootstrap = false;

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::MissingBootstrapEntrypoint)
    );
}

#[test]
fn provider_with_blank_bootstrap_entrypoint_path_is_rejected() {
    let mut manifest = provider_manifest();
    manifest.entrypoints[0].path = " ".to_string();

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::BlankBootstrapEntrypoint)
    );
}

#[test]
fn provider_missing_required_capability_is_rejected() {
    let mut manifest = provider_manifest();
    let required = network_capability();
    manifest.capabilities = Vec::new();

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::MissingRequiredCapability(
            required
        ))
    );
}

#[test]
fn provider_with_blank_profile_id_is_rejected() {
    let mut manifest = provider_manifest();
    manifest
        .host_profile
        .as_mut()
        .expect("host profile metadata")
        .profile_id = " ".to_string();

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::BlankProfileId)
    );
}

#[test]
fn provider_with_blank_profile_compatibility_is_rejected() {
    let mut manifest = provider_manifest();
    manifest
        .host_profile
        .as_mut()
        .expect("host profile metadata")
        .compatibility = " ".to_string();

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::BlankProfileCompatibility)
    );
}

#[test]
fn provider_with_blank_required_provider_is_rejected() {
    let mut manifest = provider_manifest();
    manifest
        .host_profile
        .as_mut()
        .expect("host profile metadata")
        .required_providers = vec!["network-provider".to_string(), " ".to_string()];

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::BlankRequiredProvider)
    );
}

#[test]
fn malformed_host_botster_version_is_rejected() {
    let manifest = provider_manifest();

    assert_eq!(
        admit_host_profile(&manifest, true, "0.1"),
        Err(HostProfileAdmissionError::InvalidHostBotsterVersion(
            "0.1".to_string()
        ))
    );
}

#[test]
fn unsupported_compatibility_requirement_is_rejected() {
    let mut manifest = provider_manifest();
    manifest.botster = "^0.1.0".to_string();

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(
            HostProfileAdmissionError::UnsupportedCompatibilityRequirement {
                field: HostProfileCompatibilityField::Package,
                requirement: "^0.1.0".to_string(),
            }
        )
    );
}

#[test]
fn malformed_compatibility_requirement_version_is_rejected() {
    let mut manifest = provider_manifest();
    manifest.botster = ">=0.1".to_string();

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(
            HostProfileAdmissionError::MalformedCompatibilityRequirement {
                field: HostProfileCompatibilityField::Package,
                requirement: ">=0.1".to_string(),
            }
        )
    );
}

#[test]
fn compatibility_mismatch_is_rejected() {
    let mut manifest = provider_manifest();
    manifest
        .host_profile
        .as_mut()
        .expect("host profile metadata")
        .compatibility = ">=0.2.0".to_string();

    assert_eq!(
        admit_host_profile(&manifest, true, HOST_BOTSTER_VERSION),
        Err(HostProfileAdmissionError::IncompatibleBotsterVersion {
            field: HostProfileCompatibilityField::Profile,
            requirement: ">=0.2.0".to_string(),
            host_version: HOST_BOTSTER_VERSION.to_string(),
        })
    );
}

#[test]
fn policy_sections_are_typed_enum_values() {
    let section: HostProfilePolicySection =
        serde_json::from_value(serde_json::json!("persistence"))
            .expect("deserialize typed policy section");

    assert_eq!(section, HostProfilePolicySection::Persistence);
}
