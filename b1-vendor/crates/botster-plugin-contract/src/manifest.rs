//! The plugin manifest (`botster-plugin.json`) and the plugin definition.
//!
//! Clause: HP MN-1, MN-2, MN-3, PD-1 to PD-5, PK-1 to PK-4, A2-43, and the clauses named per field.
//!
//! The manifest is one JSON object with exactly the keys of MN-1 and an optional `$schema` string that is accepted and ignored (A2-43). An unknown key fails the load `manifest_unknown_key`, so the type refuses unknown keys.
//! Steward ruling R-6.1: the required keys are `botster_plugin_api`, `name`, `version` and `entrypoints`. Every other key is optional; an absent key means "not provided" (WF-4),
//! which equals the empty declaration (`[]` or `{}`), and `Ready` repeats that empty value. `scope` defaults to `device`; an absent `limits` requests the Hub defaults.
//! Fields inside a present declaration follow their own clause.
//! Where the contract names an element without its fields, the element is an open object with the stated fields typed (steward answer to question 2).

use crate::common::OpenObject;
use crate::entities::EntityFamilyDecl;
use crate::frames::{Capability, EventsDecl};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// The plugin key: lowercase letters, digits and `-`, at least one character; no `@` and no `__` follow (HP PD-1).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct PluginName(pub String);

impl PluginName {
    pub fn is_valid(s: &str) -> bool {
        !s.is_empty()
            && s.bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
    }
}

impl<'de> Deserialize<'de> for PluginName {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if PluginName::is_valid(&s) {
            Ok(PluginName(s))
        } else {
            Err(serde::de::Error::custom(
                "a plugin name uses lowercase letters, digits and `-` (HP PD-1)",
            ))
        }
    }
}

impl JsonSchema for PluginName {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "PluginName".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"description": "HP PD-1: lowercase letters, digits and `-`; no `@`, no `__`.", "type": "string", "pattern": "^[a-z0-9-]+$"})
    }
}

/// A provided interface `name@MAJOR.MINOR.PATCH` (HP PD-3); `name` uses the characters of PD-1.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct ProvidedInterface(pub String);

impl ProvidedInterface {
    pub fn is_valid(s: &str) -> bool {
        s.split_once('@').is_some_and(|(name, v)| {
            PluginName::is_valid(name)
                && semver::Version::parse(v).is_ok_and(|v| v.pre.is_empty() && v.build.is_empty())
        })
    }
}

impl<'de> Deserialize<'de> for ProvidedInterface {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if ProvidedInterface::is_valid(&s) {
            Ok(ProvidedInterface(s))
        } else {
            Err(serde::de::Error::custom(
                "an interface is `name@MAJOR.MINOR.PATCH` (HP PD-3)",
            ))
        }
    }
}

impl JsonSchema for ProvidedInterface {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ProvidedInterface".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"description": "HP PD-3: name@MAJOR.MINOR.PATCH.", "type": "string", "pattern": r"^[a-z0-9-]+@(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)$"})
    }
}

/// The semantic-version pattern of semver 2.0.0 (HP PD-1).
pub const SEMVER_PATTERN: &str = r"^(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-((0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*)(\.(0|[1-9][0-9]*|[0-9]*[a-zA-Z-][0-9a-zA-Z-]*))*))?(\+([0-9a-zA-Z-]+(\.[0-9a-zA-Z-]+)*))?$";

/// A semantic version `MAJOR.MINOR.PATCH` with optional pre-release and build parts (HP PD-1, semver 2.0.0).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct SemVer(pub String);

impl SemVer {
    /// Strict semver 2.0.0, checked by the `semver` crate.
    pub fn is_valid(s: &str) -> bool {
        semver::Version::parse(s).is_ok()
    }
}

impl<'de> Deserialize<'de> for SemVer {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let s = String::deserialize(d)?;
        if SemVer::is_valid(&s) {
            Ok(SemVer(s))
        } else {
            Err(serde::de::Error::custom(
                "a version is a semantic version MAJOR.MINOR.PATCH (HP PD-1)",
            ))
        }
    }
}

impl JsonSchema for SemVer {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "SemVer".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({"description": "HP PD-1: semver 2.0.0.", "type": "string", "pattern": SEMVER_PATTERN})
    }
}

/// The manifest file name at the package root (HP PK-1).
pub const MANIFEST_FILE: &str = "botster-plugin.json";
/// The agent definition file name (HP PK-6).
pub const AGENT_FILE: &str = "botster-agent.json";
/// The accessory file name (HP PK-6).
pub const ACCESSORY_FILE: &str = "botster-accessory.json";
/// The default `target_plugin_dir`. The default is contract: a change is MAJOR (HP A2-39).
pub const DEFAULT_TARGET_PLUGIN_DIR: &str = ".botster/plugins";
/// The plugin API major version that this contract defines (HP section 18).
pub const BOTSTER_PLUGIN_API: u32 = 1;
/// The path of an interface document inside its provider package (HP PD-3, A2-49): `interfaces/<name>@<MAJOR.MINOR.PATCH>.json`.
pub fn interface_document_path(name: &str, version: &str) -> String {
    format!("interfaces/{name}@{version}.json")
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The Lua entry (HP MN-1, A2-47): a path relative to the package root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServiceEntry {
    pub lua: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// `entrypoints: {service: {lua: <staged file>}}` (HP MN-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Entrypoints {
    pub service: ServiceEntry,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The two feature sets of `requirements.features` (HP PD-2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RequiredFeatures {
    #[serde(default)]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2)))]
    pub required: Vec<String>,
    #[serde(default)]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2)))]
    pub optional: Vec<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// `requirements: {botster_plugin_api, features, ui_contract?}` (HP PD-2). `ui_contract` is `"M.m"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    #[cfg_attr(test, strategy(proptest::strategy::Just(1u32)))]
    pub botster_plugin_api: u32,
    #[serde(default)]
    pub features: RequiredFeatures,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub ui_contract: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// One exported tool of a provided interface (HP PD-3). `effect` is compared for equality (PD-3b).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InterfaceExport {
    pub tool: String,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub args_schema: Value,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub result_schema: Value,
    pub effect: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// `provides: [{interface: "name@MAJOR.MINOR.PATCH", exports}]` (HP PD-3).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Provides {
    pub interface: ProvidedInterface,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<InterfaceExport>(), 0..2)))]
    pub exports: Vec<InterfaceExport>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// `requires: [{interface, range, optional?}]` (HP PD-5). `range` is a semver range over the `interface_version`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Requires {
    pub interface: String,
    pub range: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub optional: Option<bool>,
}

/// `interaction` of a session type (HP CA-18).
pub type InteractionClass = crate::rows::Interaction;
pub use crate::rows::Presentation;

/// The mode of a terminal-notification policy (HP CA-22).
#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum NotificationMode {
    All,
    None,
    Matching,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// `notifications: {mode, patterns}` (HP CA-22). `patterns` only with `matching`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Notifications {
    pub mode: NotificationMode,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2))))]
    pub patterns: Option<Vec<String>>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A session type in the manifest (HP MN-1, CA-13). The contract states only the fields below; other keys are accepted.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SessionTypeDecl {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interaction: Option<InteractionClass>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub presentation: Option<Presentation>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub allocate_port: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2))))]
    pub port_env: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub notifications: Option<Notifications>,
    #[serde(flatten)]
    #[cfg_attr(test, strategy(crate::props::extra()))]
    pub extra: OpenObject,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A configuration entry (HP MN-1, CF-1). The contract names the `type` key without its values, so it is a string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConfigDecl {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub required: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(crate::props::opt_value()))]
    pub default: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub secret: Option<bool>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A secret the plugin declares (HP MN-1, NT-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SecretDecl {
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub required: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub plugin_writable: Option<bool>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A filesystem root (HP MN-1, FS-1). `read` is always `true`; there is no write in v1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FsRootDecl {
    pub id: String,
    pub read: crate::common::ConstTrue,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A static route (HP MN-1, WP-5, A2-46). `root` may be `package:<relative dir>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaticRoute {
    pub route: String,
    pub root: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub index: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A staged asset (HP MN-1, AS-1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StagedAsset {
    pub name: String,
    pub path: String,
    pub content_type: crate::call_types::AssetContentType,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The restart policy of a declared MCP server (HP MP-8): `{max, window}` with `window` in milliseconds (WF-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpRestart {
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max: u64,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub window: u64,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A declared MCP server (HP MN-1, MP-1). A `stdio` entry needs `command`; an `http` entry needs `url`; any other combination fails the load `manifest_bad_mcp_server`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "transport", rename_all = "snake_case")]
pub enum McpServerDecl {
    Stdio(McpStdio),
    Http(McpHttp),
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A `stdio` MCP server (HP MP-1, MP-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpStdio {
    pub id: String,
    pub command: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2))))]
    pub args: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::btree_map(proptest::prelude::any::<String>(), proptest::prelude::any::<String>(), 0..2))))]
    pub env: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2))))]
    pub allow_tools: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub restart: Option<McpRestart>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An `http` MCP server (HP MP-1, MP-11).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpHttp {
    pub id: String,
    pub url: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::btree_map(proptest::prelude::any::<String>(), proptest::prelude::any::<String>(), 0..2))))]
    pub headers: Option<BTreeMap<String, String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2))))]
    pub allow_tools: Option<Vec<String>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub restart: Option<McpRestart>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The declared scalar type of an index field (HP ST-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum IndexFieldType {
    String,
    Integer,
    Boolean,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndexField {
    pub field: String,
    #[serde(rename = "type")]
    pub field_type: IndexFieldType,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A named, ordered index (HP ST-1). Every collection also has the implicit `id` index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IndexDecl {
    pub name: String,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<IndexField>(), 0..2)))]
    pub fields: Vec<IndexField>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A store collection (HP ST-1, ST-11). `schema_version` is the version written and `accepts` the versions read; the contract does not fix their JSON type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionDecl {
    pub name: String,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub schema_version: Value,
    #[cfg_attr(test, strategy(crate::props::vec_value()))]
    pub accepts: Vec<Value>,
    #[serde(default)]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<IndexDecl>(), 0..2)))]
    pub indexes: Vec<IndexDecl>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// `storage: {collections}` (HP ST-1, MN-1).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StorageDecl {
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<CollectionDecl>(), 0..2)))]
    pub collections: Vec<CollectionDecl>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The `subject_scope` of a notice reaction (HP VW-9). Default `own`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubjectScope {
    Own,
    Any,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The severity of a notice (HP VW-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Info,
    Success,
    Warning,
    Error,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A notice delivery path (HP VW-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DeliveryPath {
    InApp,
    Push,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A notice reaction (HP VW-9, MN-1). The pointers are JSON pointers into the event payload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoticeReaction {
    pub id: String,
    pub owner: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub subject_scope: Option<SubjectScope>,
    pub text_pointer: String,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub ttl_ms: u64,
    pub severity: Severity,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::vec(proptest::prelude::any::<DeliveryPath>(), 0..2))))]
    pub delivery: Option<Vec<DeliveryPath>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub url_pointer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub tag_pointer: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The package scope (HP CP-8, MN-1). Default `device`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PackageScope {
    Device,
    Target,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The client effect classes of DS-1 (HP VW-8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActionEffect {
    Read,
    Immediate,
    Committed,
    Accepted,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// One value of an `applies_to` term: a string or a list of strings (any of).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OneOrMany {
    One(String),
    Many(Vec<String>),
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The closed equality filter of a contributed session action (HP VW-8). Any other field fails the load `manifest_bad_applies_to`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AppliesTo {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub session_type_id: Option<OneOrMany>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub state: Option<OneOrMany>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interaction: Option<OneOrMany>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub role: Option<OneOrMany>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub has_port: Option<bool>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An action that a plugin contributes to Hub session rows (HP VW-8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SessionActionUi {
    pub action_id: String,
    pub label: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub warning: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(crate::props::opt_value()))]
    pub args_schema: Option<Value>,
    pub effect: ActionEffect,
    pub handler_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub durable_operation: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub applies_to: Option<AppliesTo>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub label_pointer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub status_pointer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub url_pointer: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The manifest of a plugin (HP MN-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// An editor hint that is accepted and ignored (HP A2-43).
    #[serde(
        rename = "$schema",
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub schema_ref: Option<String>,
    /// The plugin API major version the plugin targets (HP section 18). The value is `1`.
    #[schemars(range(min = 1, max = 1))]
    #[cfg_attr(test, strategy(proptest::strategy::Just(1u32)))]
    pub botster_plugin_api: u32,
    /// The plugin key (HP PD-1).
    pub name: PluginName,
    /// A semantic version (HP PD-1).
    pub version: SemVer,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub description: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub license: Option<String>,
    /// Informational only: the source recorded at install is the truth (HP PD-1, PM-1).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(crate::props::opt_value()))]
    pub source: Option<Value>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub requirements: Option<Requirements>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Provides>(), 0..2)))]
    pub provides: Vec<Provides>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Requires>(), 0..2)))]
    pub requires: Vec<Requires>,
    pub entrypoints: Entrypoints,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Capability>(), 0..2)))]
    pub capabilities: Vec<Capability>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<SessionTypeDecl>(), 0..2)))]
    pub session_types: Vec<SessionTypeDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<ConfigDecl>(), 0..2)))]
    pub config: Vec<ConfigDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<SecretDecl>(), 0..2)))]
    pub secrets: Vec<SecretDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<FsRootDecl>(), 0..2)))]
    pub fs_roots: Vec<FsRootDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<StaticRoute>(), 0..2)))]
    pub static_routes: Vec<StaticRoute>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<StagedAsset>(), 0..2)))]
    pub assets: Vec<StagedAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<McpServerDecl>(), 0..2)))]
    pub mcp_servers: Vec<McpServerDecl>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub storage: Option<StorageDecl>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub events: Option<EventsDecl>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<EntityFamilyDecl>(), 0..2)))]
    pub entity_families: Vec<EntityFamilyDecl>,
    /// View documents of the UI contract (HP VW-1, VW-6).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::strategy::Just(Vec::new())))]
    pub views: Vec<botster_ui_contract::prelude::ViewDocument>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<NoticeReaction>(), 0..2)))]
    pub notice_reactions: Vec<NoticeReaction>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub scope: Option<PackageScope>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<SessionActionUi>(), 0..2)))]
    pub session_actions_ui: Vec<SessionActionUi>,
    /// Optional requests for `PluginLimits`, bounded by the Hub parameters (HP MN-1). The keys are limit names.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(crate::props::opt_object()))]
    pub limits: Option<OpenObject>,
}
