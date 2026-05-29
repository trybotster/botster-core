//! Boundary contract tests.

use std::fs;
use std::path::Path;

use botster_core::boundary::{responsibility, Layer};
use botster_core::capability::{Capability, CapabilitySurface};
use botster_core::extension::{ExtensionEntrypoint, ExtensionKind, ExtensionRuntime};
use botster_core::package::PackageManifest;

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
        "src/session.rs",
        "src/crypto.rs",
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
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    fs::read_to_string(path).expect("README.md should be readable")
}
