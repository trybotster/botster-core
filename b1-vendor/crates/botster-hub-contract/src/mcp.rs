//! The data of the `mcp` binding that is shaped like the envelope (HC 9.4, section 10): the tool arguments of MC-2 and the resource URIs of TB-M.

use crate::refs::{ResourceRef, SessionRef};
use crate::wire::{opt_no_null, OpenObject};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A `ref` argument: a `SessionRef` or a `ResourceRef` (P5).
///
/// Clause: HC MC-2, P5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AnyRef {
    Resource(ResourceRef),
    Session(SessionRef),
}

/// The common optional request fields of a mutating tool.
///
/// Clause: HC MC-2, EN-1.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ToolRequestFields {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub idempotency_key: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub operation_id: Option<String>,
}

/// `list { kind, filter?, cursor?, limit? }`.
///
/// Clause: HC MC-2, VB-1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListTool {
    pub kind: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub filter: Option<OpenObject>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub cursor: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub limit: Option<u64>,
}

/// `get { ref }`.
///
/// Clause: HC MC-2.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct GetTool {
    #[serde(rename = "ref")]
    pub reference: AnyRef,
}

/// `create { kind, args, idempotency_key?, operation_id? }`.
///
/// Clause: HC MC-2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CreateTool {
    pub kind: String,
    pub args: Value,
    #[serde(flatten)]
    pub request: ToolRequestFields,
}

/// `update { ref, patch, if_rev?, idempotency_key? }`.
///
/// Clause: HC MC-2, VB-4.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UpdateTool {
    #[serde(rename = "ref")]
    pub reference: AnyRef,
    pub patch: OpenObject,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub if_rev: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub idempotency_key: Option<String>,
}

/// `delete { ref, args?, idempotency_key?, operation_id? }`.
///
/// Clause: HC MC-2, A2 H-8.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DeleteTool {
    #[serde(rename = "ref")]
    pub reference: AnyRef,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub args: Option<Value>,
    #[serde(flatten)]
    pub request: ToolRequestFields,
}

/// `invoke { ref, action, args, idempotency_key?, operation_id?, descriptor_id? }`. `ref` maps to `kind` and `target`, and `action` to `req.action` (EN-1a).
///
/// Clause: HC MC-2, A4 EN-1a.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InvokeTool {
    #[serde(rename = "ref")]
    pub reference: AnyRef,
    pub action: String,
    pub args: Value,
    #[serde(flatten)]
    pub request: ToolRequestFields,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub descriptor_id: Option<String>,
}

/// The URI of a row resource, `botster://<hub_id>/<kind>/<id>`.
///
/// Clause: HC TB-M.
pub fn row_uri(hub_id: &str, kind: &str, id: &str) -> String {
    format!("botster://{hub_id}/{kind}/{id}")
}

/// The URI of a kind's list resource, `botster://<hub_id>/<kind>`.
///
/// Clause: HC TB-M.
pub fn list_uri(hub_id: &str, kind: &str) -> String {
    format!("botster://{hub_id}/{kind}")
}
