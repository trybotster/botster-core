//! Package configuration schema contract acceptance tests.

use botster_core::{
    ExtensionKind, PackageConfigurationField, PackageConfigurationFieldType,
    PackageConfigurationGroup, PackageConfigurationOption, PackageConfigurationSchema,
    PackageConfigurationSecretValue, PackageConfigurationValidationHints,
    PackageConfigurationValue, PackageManifest,
};
use serde_json::Number;

fn number(value: i64) -> Number {
    Number::from(value)
}

fn decimal(value: f64) -> Number {
    Number::from_f64(value).expect("finite JSON number")
}

fn configured_manifest() -> PackageManifest {
    PackageManifest {
        name: "configurable-plugin".to_string(),
        version: "0.1.0".to_string(),
        kind: ExtensionKind::Plugin,
        botster: ">=0.1.0".to_string(),
        source: None,
        capabilities: Vec::new(),
        entrypoints: Vec::new(),
        host_profile: None,
        configuration: Some(PackageConfigurationSchema {
            groups: vec![PackageConfigurationGroup {
                id: "connection".to_string(),
                label: "Connection".to_string(),
                description: Some("Connection settings".to_string()),
                order: Some(1),
            }],
            fields: vec![
                PackageConfigurationField {
                    key: "endpoint".to_string(),
                    field_type: PackageConfigurationFieldType::Url,
                    label: "Endpoint URL".to_string(),
                    description: Some("Base URL for outbound requests".to_string()),
                    required: true,
                    default: Some(PackageConfigurationValue::Url {
                        value: "https://api.example.invalid".to_string(),
                    }),
                    validation: Some(PackageConfigurationValidationHints {
                        min_length: Some(8),
                        max_length: Some(200),
                        pattern: Some("^https://".to_string()),
                        min: None,
                        max: None,
                        allowed_extensions: Vec::new(),
                    }),
                    group: Some("connection".to_string()),
                    order: Some(1),
                    options: Vec::new(),
                },
                PackageConfigurationField {
                    key: "mode".to_string(),
                    field_type: PackageConfigurationFieldType::Select,
                    label: "Mode".to_string(),
                    description: None,
                    required: false,
                    default: Some(PackageConfigurationValue::Select {
                        value: "safe".to_string(),
                    }),
                    validation: None,
                    group: Some("connection".to_string()),
                    order: Some(2),
                    options: vec![
                        PackageConfigurationOption {
                            value: "safe".to_string(),
                            label: "Safe".to_string(),
                            description: Some("Prefer conservative behavior".to_string()),
                        },
                        PackageConfigurationOption {
                            value: "fast".to_string(),
                            label: "Fast".to_string(),
                            description: None,
                        },
                    ],
                },
                PackageConfigurationField {
                    key: "retries".to_string(),
                    field_type: PackageConfigurationFieldType::Integer,
                    label: "Retries".to_string(),
                    description: None,
                    required: false,
                    default: Some(PackageConfigurationValue::Integer { value: 3 }),
                    validation: Some(PackageConfigurationValidationHints {
                        min_length: None,
                        max_length: None,
                        pattern: None,
                        min: Some(number(0)),
                        max: Some(number(10)),
                        allowed_extensions: Vec::new(),
                    }),
                    group: Some("connection".to_string()),
                    order: Some(3),
                    options: Vec::new(),
                },
            ],
        }),
        surfaces: Vec::new(),
    }
}

#[test]
fn package_manifest_without_configuration_keeps_serde_compatibility() {
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
        serde_json::from_value(json).expect("deserialize manifest without configuration");

    assert_eq!(decoded.configuration, None);
    assert_eq!(
        serde_json::to_value(decoded).expect("serialize manifest without configuration"),
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
fn package_configuration_round_trips_through_package_manifest() {
    let manifest = configured_manifest();
    let json = serde_json::to_value(&manifest).expect("serialize configured manifest");

    assert_eq!(json["configuration"]["groups"][0]["id"], "connection");
    assert_eq!(json["configuration"]["fields"][0]["type"], "url");
    assert_eq!(json["configuration"]["fields"][0]["required"], true);
    assert_eq!(
        json["configuration"]["fields"][0]["validation"]["pattern"],
        "^https://"
    );
    assert_eq!(
        json["configuration"]["fields"][1]["options"][0]["value"],
        "safe"
    );
    assert_eq!(json["configuration"]["fields"][2]["validation"]["max"], 10);

    let decoded: PackageManifest =
        serde_json::from_value(json).expect("deserialize configured manifest");
    assert_eq!(decoded, manifest);
}

#[test]
fn package_configuration_field_types_cover_required_inventory() {
    let field_types = vec![
        PackageConfigurationFieldType::String,
        PackageConfigurationFieldType::Number,
        PackageConfigurationFieldType::Integer,
        PackageConfigurationFieldType::Boolean,
        PackageConfigurationFieldType::Select,
        PackageConfigurationFieldType::Path,
        PackageConfigurationFieldType::Url,
        PackageConfigurationFieldType::MultilineText,
        PackageConfigurationFieldType::Secret,
    ];

    assert_eq!(
        serde_json::to_value(field_types).expect("serialize field types"),
        serde_json::json!([
            "string",
            "number",
            "integer",
            "boolean",
            "select",
            "path",
            "url",
            "multiline_text",
            "secret"
        ])
    );
}

#[test]
fn package_configuration_value_types_round_trip_without_losing_numeric_shape() {
    let values = vec![
        PackageConfigurationValue::String {
            value: "name".to_string(),
        },
        PackageConfigurationValue::Number {
            value: decimal(1.5),
        },
        PackageConfigurationValue::Integer { value: 42 },
        PackageConfigurationValue::Boolean { value: true },
        PackageConfigurationValue::Select {
            value: "safe".to_string(),
        },
        PackageConfigurationValue::Path {
            value: "relative/path".to_string(),
        },
        PackageConfigurationValue::Url {
            value: "https://api.example.invalid".to_string(),
        },
        PackageConfigurationValue::MultilineText {
            value: "line one\nline two".to_string(),
        },
    ];

    let json = serde_json::to_value(&values).expect("serialize values");
    assert_eq!(
        json[1],
        serde_json::json!({ "type": "number", "value": 1.5 })
    );
    assert_eq!(
        json[2],
        serde_json::json!({ "type": "integer", "value": 42 })
    );

    let decoded: Vec<PackageConfigurationValue> =
        serde_json::from_value(json).expect("deserialize values");
    assert_eq!(decoded, values);
}

#[test]
fn package_configuration_required_defaults_to_false_when_omitted() {
    let field_json = serde_json::json!({
        "key": "notes",
        "type": "multiline_text",
        "label": "Notes"
    });

    let field: PackageConfigurationField =
        serde_json::from_value(field_json).expect("deserialize field without required");

    assert!(!field.required);
}

#[test]
fn package_configuration_secret_values_are_redacted_or_write_only() {
    let raw_secret = "super-secret-token";
    let secret = PackageConfigurationValue::Secret {
        state: PackageConfigurationSecretValue::WriteOnly,
    };

    let json = serde_json::to_string(&secret).expect("serialize secret value");

    assert!(!json.contains(raw_secret));
    assert!(!json.contains("value"));
    assert_eq!(json, r#"{"type":"secret","state":"write_only"}"#);

    let raw_secret_payload = serde_json::json!({
        "type": "secret",
        "state": "write_only",
        "value": raw_secret
    });
    let decoded = serde_json::from_value::<PackageConfigurationValue>(raw_secret_payload);

    assert!(decoded.is_err());
}

#[test]
fn package_configuration_example_json_deserializes() {
    let example = include_str!("../../../docs/examples/package-configuration-schema.json");
    let manifest: PackageManifest =
        serde_json::from_str(example).expect("deserialize package configuration example");

    assert_eq!(manifest.name, "example-package");
    assert!(manifest.configuration.is_some());
}
