//! Package UI surface descriptor contract acceptance tests.

use botster_core::{
    ExtensionKind, PackageManifest, PackageNavigationEntry, PackageNavigationTarget,
    PackageSurfaceDescriptor, PackageSurfaceKind, PackageSurfaceOperation,
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
        runnable_entrypoints: Vec::new(),
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
        navigation: Vec::new(),
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
    assert_eq!(decoded.navigation, Vec::new());
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
fn package_navigation_entries_round_trip_without_ordering_authority() {
    let mut manifest = surface_manifest();
    manifest.navigation = vec![PackageNavigationEntry {
        id: "workbench".to_string(),
        label: "Workbench".to_string(),
        icon: Some("workspace".to_string()),
        description: Some("Open the package workbench".to_string()),
        target: PackageNavigationTarget::Surface {
            surface_id: "main".to_string(),
        },
    }];

    let json = serde_json::to_value(&manifest).expect("serialize manifest with navigation");
    assert_eq!(json["navigation"][0]["id"], "workbench");
    assert_eq!(json["navigation"][0]["label"], "Workbench");
    assert_eq!(json["navigation"][0]["icon"], "workspace");
    assert_eq!(json["navigation"][0]["target"]["kind"], "surface");
    assert_eq!(json["navigation"][0]["target"]["surface_id"], "main");
    assert_eq!(json["surfaces"][0]["kind"], "app");
    assert!(json["navigation"][0].get("order").is_none());
    assert!(json["navigation"][0].get("priority").is_none());
    assert!(json["navigation"][0].get("pinned").is_none());
    assert!(json["navigation"][0].get("hidden").is_none());
    assert!(json["navigation"][0].get("placement").is_none());
    assert!(json["navigation"][0].get("layout").is_none());
    assert!(json["navigation"][0].get("sidebar").is_none());

    let decoded: PackageManifest = serde_json::from_value(json).expect("deserialize manifest");
    assert_eq!(decoded, manifest);
}

#[test]
fn package_navigation_rejects_plugin_ordering_or_shell_authority_fields() {
    for forbidden in [
        "order",
        "priority",
        "pinned",
        "hidden",
        "placement",
        "layout",
        "sidebar",
    ] {
        let mut json = serde_json::json!({
            "name": "nav-plugin",
            "version": "0.1.0",
            "kind": "plugin",
            "botster": ">=0.1.0",
            "source": null,
            "capabilities": [],
            "entrypoints": [],
            "surfaces": [{
                "id": "main",
                "kind": "app",
                "title": "Workbench"
            }],
            "navigation": [{
                "id": "workbench",
                "label": "Workbench",
                "target": { "kind": "surface", "surface_id": "main" }
            }]
        });
        json["navigation"][0][forbidden] = serde_json::json!(true);

        let err = serde_json::from_value::<PackageManifest>(json)
            .expect_err("navigation authority field should be rejected");
        assert!(
            err.to_string().contains(forbidden),
            "expected `{err}` to mention forbidden field `{forbidden}`"
        );
    }
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
