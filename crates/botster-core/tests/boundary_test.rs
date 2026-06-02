//! Boundary contract tests.

use std::{fs, path::Path};

use botster_core::boundary::{responsibility, Layer};
use botster_core::capability::{Capability, CapabilitySurface};
use botster_core::extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
use botster_core::package::PackageManifest;

const README: &str = include_str!("../../../README.md");

fn policy_section() -> &'static str {
    README
        .split("## Extraction Compatibility Policy")
        .nth(1)
        .and_then(|section| section.split("## License").next())
        .expect("README must contain an Extraction Compatibility Policy section before License")
}

fn assert_policy_verdict(path: &str, verdict: &str) {
    let path = path.to_ascii_lowercase();
    let verdict = verdict.to_ascii_lowercase();
    let matching_row = policy_section()
        .lines()
        .map(|line| line.replace('`', "").to_ascii_lowercase())
        .find(|line| line.contains(&path) && line.contains(&verdict));

    assert!(
        matching_row.is_some(),
        "policy must classify `{path}` with verdict `{verdict}`"
    );
}

fn normalized_policy() -> String {
    policy_section()
        .replace('`', "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn rust_source_files(dir: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();

    for entry in fs::read_dir(dir).expect("source directory must be readable") {
        let path = entry.expect("source entry must be readable").path();
        if path.is_dir() {
            files.extend(rust_source_files(&path));
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("rs") {
            files.push(path);
        }
    }

    files
}

#[test]
fn layer_responsibilities_keep_cli_out_of_core_and_hub() {
    let core = responsibility(Layer::Core);
    let hub = responsibility(Layer::Hub);
    let cli = responsibility(Layer::Cli);

    assert!(core.owns.contains("reusable mechanisms"));
    assert!(hub.owns.contains("policy"));
    assert!(cli.owns.contains("operator commands"));
    assert!(core.does_not_own.contains("startup"));
}

#[test]
fn provider_is_a_privileged_extension_package() {
    let manifest = PackageManifest {
        name: "trybotster-cloud".to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Provider,
        botster: ">=0.1,<1.0".to_string(),
        source: None,
        capabilities: vec![
            Capability {
                surface: CapabilitySurface::ClientAdmission,
                scope: None,
            },
            Capability {
                surface: CapabilitySurface::SignalingRelay,
                scope: None,
            },
        ],
        entrypoints: vec![ExtensionEntrypoint {
            runtime: ExtensionRuntime::Lua,
            path: "bootstrap.lua".to_string(),
            bootstrap: true,
        }],
        host_profile: None,
    };

    assert_eq!(manifest.kind, ExtensionKind::Provider);
    assert!(manifest.entrypoints[0].bootstrap);
}

#[test]
fn readme_documents_layer_ownership_boundaries() {
    let readme = readme();

    for anchor in [
        "## Ownership Boundary",
        "| Core |",
        "| Hub |",
        "| CLI |",
        "| Client |",
        "| Provider/plugin |",
        "ExtensionKind::Plugin",
        "ExtensionKind::Provider",
        "Layer::Extension",
        "session/client data-plane actors",
        "not a separate `Layer::Provider` variant",
        "src/contract/session.rs",
        "src/identity/crypto.rs",
    ] {
        assert!(
            readme.contains(anchor),
            "README.md must document boundary anchor: {anchor}"
        );
    }
}

#[test]
fn readme_keeps_core_ban_list_explicit() {
    let readme = readme();

    for banned_surface in [
        "hub policy",
        "CLI startup",
        "Rails/cloud/Auth implementation",
        "concrete WebRTC negotiation policy",
        "React/TUI rendering",
        "Project Pipelines/GitHub/Cloudflare product logic",
        "legacy compatibility paths",
    ] {
        assert!(
            readme.contains(banned_surface),
            "README.md must explicitly ban {banned_surface} from botster-core"
        );
    }
}

#[test]
fn readme_requires_preserve_translate_drop_migration_choices() {
    let readme = readme();

    for anchor in [
        "## Migration Guidance",
        "There is no defer category.",
        "### Preserve",
        "### Translate",
        "### Drop",
    ] {
        assert!(
            readme.contains(anchor),
            "README.md must keep migration guidance anchor: {anchor}"
        );
    }
}

fn readme() -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../README.md");
    fs::read_to_string(path).expect("README.md should be readable")
}

#[test]
fn readme_documents_extraction_compatibility_decision_rules() {
    let policy = normalized_policy();

    assert!(policy.contains("preserve"));
    assert!(policy.contains("translate"));
    assert!(policy.contains("drop"));
    assert!(policy.contains("there is no defer bucket"));
    assert!(policy.contains("delete"));
    assert!(policy.contains("exclude"));
    assert!(policy.contains("fossilizing accidental coupling"));
}

#[test]
fn readme_classifies_preserved_core_contract_families() {
    let policy = normalized_policy();

    for contract in [
        "transport-neutral identifiers",
        "ingress and egress frames",
        "entity frames",
        "ui contract shapes",
        "package manifests",
        "capabilities",
        "extension metadata",
        "crypto or identity operation contracts",
    ] {
        assert!(
            policy.contains(contract),
            "policy must preserve `{contract}` as a core contract family"
        );
    }

    assert_policy_verdict("transport-neutral identifiers", "preserve");
}

#[test]
fn readme_classifies_ticket_named_excluded_paths() {
    assert_policy_verdict("context.json migration", "drop");
    assert_policy_verdict("legacy repo-cwd hub identity", "drop");
    assert_policy_verdict("old forwarder terminology", "translate");
    assert_policy_verdict("browser-only plugin stores", "drop");
    assert_policy_verdict("direct snapshot helpers", "translate");
    assert_policy_verdict("hub-owned pty relays", "drop");
    assert_policy_verdict("product-specific ui refresh behavior", "drop");
}

#[test]
fn readme_translates_forwarders_and_direct_snapshot_helpers() {
    let policy = normalized_policy();

    assert!(policy.contains("terminal subscriptions"));
    assert!(policy.contains("ptyforwarder"));
    assert!(policy.contains("stopforwarder"));
    assert!(policy.contains("create_pty_forwarder"));
    assert!(policy.contains("transport-neutral snapshot"));
    assert!(policy.contains("snapshot_and_subscribe"));
    assert!(policy.contains("session/client-worker ownership"));
}

#[test]
fn source_does_not_reintroduce_legacy_public_api_names() {
    let disallowed = [
        "PtyForwarder",
        "StopForwarder",
        "create_pty_forwarder",
        "snapshot_and_subscribe",
    ];

    for path in rust_source_files(Path::new("src")) {
        let contents = fs::read_to_string(&path).expect("source file must be readable");
        for token in disallowed {
            assert!(
                !contents.contains(token),
                "{path:?} must not reintroduce legacy public API `{token}`"
            );
        }
    }
}

#[test]
fn source_boundary_json_uses_are_limited_to_classified_escape_hatches() {
    // Keep these source markers in sync with the owner/reason inventory in
    // actor_contract_test.rs so new BoundaryJson fields are not only detected.
    let allowed_markers = [
        "pub struct BoundaryJson(pub serde_json::Value);",
        "payload: BoundaryJson,",
        "pub payload: BoundaryJson,",
        "pub body: BoundaryJson,",
        "pub metadata: Option<BoundaryJson>,",
        "pub payload: Option<BoundaryJson>,",
        "pub extension: Option<BoundaryJson>,",
    ];

    let mut boundary_uses = Vec::new();
    for path in rust_source_files(Path::new("src")) {
        let contents = fs::read_to_string(&path).expect("source file must be readable");
        for line in contents.lines() {
            let trimmed = line.trim_start();
            if !trimmed.contains("BoundaryJson")
                || trimmed.starts_with("use ")
                || trimmed.starts_with("pub use ")
                || trimmed.starts_with("//")
            {
                continue;
            }

            assert!(
                allowed_markers.contains(&trimmed),
                "{path:?} has an unclassified BoundaryJson use: {trimmed}"
            );
            boundary_uses.push(format!("{path:?}:{trimmed}"));
        }
    }

    assert!(
        boundary_uses
            .iter()
            .any(|line| line.contains("NotificationAction") || line.contains("extension")),
        "notification extension BoundaryJson uses must remain visible to the source inventory"
    );
}
