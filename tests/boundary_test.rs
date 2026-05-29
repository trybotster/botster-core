//! Boundary contract tests.

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
