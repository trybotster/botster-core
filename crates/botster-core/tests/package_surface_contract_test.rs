//! Package UI surface descriptor contract acceptance tests.

use botster_core::{
    ExtensionKind, PackageManifest, PackageSurfaceDescriptor, PackageSurfaceKind,
    PackageSurfaceOperation,
};

fn surface_manifest() -> PackageManifest {
    PackageManifest {
        name: "surface-plugin".to_string(),
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
        surfaces: vec![
            PackageSurfaceDescriptor {
                id: "main".to_string(),
                kind: PackageSurfaceKind::App,
                title: "Project Workbench".to_string(),
                description: Some("Primary package surface".to_string()),
                icon: Some("project".to_string()),
                order: Some(1),
                category: Some("work".to_string()),
                supports: vec![
                    PackageSurfaceOperation::Render,
                    PackageSurfaceOperation::Action,
                ],
            },
            PackageSurfaceDescriptor {
                id: "settings".to_string(),
                kind: PackageSurfaceKind::Settings,
                title: "Settings".to_string(),
                description: None,
                icon: None,
                order: Some(2),
                category: None,
                supports: vec![PackageSurfaceOperation::Render],
            },
        ],
    }
}

#[test]
fn package_manifest_without_surfaces_keeps_serde_compatibility() {
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
        serde_json::from_value(json).expect("deserialize manifest without surfaces");

    assert_eq!(decoded.surfaces, Vec::new());
    assert_eq!(
        serde_json::to_value(decoded).expect("serialize manifest without surfaces"),
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
fn package_surfaces_round_trip_through_package_manifest() {
    let manifest = surface_manifest();
    let json = serde_json::to_value(&manifest).expect("serialize surface manifest");

    assert_eq!(json["surfaces"][0]["id"], "main");
    assert_eq!(json["surfaces"][0]["kind"], "app");
    assert_eq!(json["surfaces"][0]["title"], "Project Workbench");
    assert_eq!(
        json["surfaces"][0]["description"],
        "Primary package surface"
    );
    assert_eq!(json["surfaces"][0]["icon"], "project");
    assert_eq!(json["surfaces"][0]["order"], 1);
    assert_eq!(json["surfaces"][0]["category"], "work");
    assert_eq!(
        json["surfaces"][0]["supports"],
        serde_json::json!(["render", "action"])
    );
    assert_eq!(json["surfaces"][1]["kind"], "settings");
    assert!(json["surfaces"][1].get("description").is_none());

    let decoded: PackageManifest = serde_json::from_value(json).expect("deserialize manifest");
    assert_eq!(decoded, manifest);
}

#[test]
fn package_surface_kinds_cover_required_inventory() {
    let kinds = vec![
        PackageSurfaceKind::App,
        PackageSurfaceKind::Settings,
        PackageSurfaceKind::DashboardWidget,
        PackageSurfaceKind::Diagnostics,
    ];

    assert_eq!(
        serde_json::to_value(kinds).expect("serialize surface kinds"),
        serde_json::json!(["app", "settings", "dashboard_widget", "diagnostics"])
    );
}

#[test]
fn package_surface_example_deserializes() {
    let example = include_str!("../../../docs/examples/package-surfaces.json");
    let manifest: PackageManifest =
        serde_json::from_str(example).expect("deserialize package surface example");

    assert_eq!(manifest.name, "surface-demo");
    assert_eq!(manifest.surfaces.len(), 3);
    assert_eq!(manifest.surfaces[0].kind, PackageSurfaceKind::App);
    assert_eq!(
        manifest.surfaces[0].supports,
        vec![
            PackageSurfaceOperation::Render,
            PackageSurfaceOperation::Action
        ]
    );
}
