//! `describe` (HC section 4): the kind descriptor, its verbs and actions, and the effect classes of EN-7.

use crate::wire::{open_enum, opt_no_null};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

open_enum! {
    /// The effect class of a verb or an action.
    ///
    /// Clause: HC EN-7.
    pub enum EffectClass {
        /// No change.
        Read = "read",
        /// Done when `ok` arrives, not durable.
        Immediate = "immediate",
        /// Done and durable (fsynced) when `ok` arrives.
        Committed = "committed",
        /// Admitted; `ok` returns `{ operation }`.
        Accepted = "accepted",
    }
}

/// A verb of a kind, as `describe` lists it.
///
/// Clause: HC DS-1, DS-5.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct VerbDescriptor {
    pub effect: EffectClass,
    pub permission: String,
    /// A hint for this caller. The Hub enforces every call (DS-5).
    pub allowed: bool,
}

/// One reason an action is disabled: a section 11 code or a kind `reason`.
///
/// Clause: HC A2 H-13, DS-6.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct DescribedReason {
    pub code: String,
    pub message: String,
}

/// The closed equality filter of a contributed session action (HP VW-8): the fields that must equal the session's.
///
/// Clause: HC section 6 (`session` row), DS-1.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
pub struct AppliesTo {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub session_type_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub state: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub interaction: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub role: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub has_port: Option<bool>,
}

/// An action of a kind. `owner`, `descriptor_id` and the VW-8 fields appear on an action that a plugin contributes to another owner's kind.
///
/// Clause: HC DS-1, DS-5, DS-6, EN-7.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActionDescriptor {
    pub args_schema: Value,
    pub result_schema: Value,
    pub effect: EffectClass,
    /// Per action, never inferred from the resource (EN-7).
    pub durable_operation: bool,
    pub idempotent: bool,
    pub permission: String,
    pub allowed: bool,
    /// Disabled actions are listed with `allowed: false` and their reasons (DS-6).
    pub reasons: Vec<DescribedReason>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub owner: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub descriptor_id: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub warning: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub label_pointer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub status_pointer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub url_pointer: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub applies_to: Option<AppliesTo>,
}

open_enum! {
    /// How a plugin kind without a projection lists (VB-1): `current_read` pages are current reads, not one cut.
    ///
    /// Clause: HC VB-1.
    pub enum ListMode {
        CurrentRead = "current_read",
    }
}

/// The descriptor of one kind: the row of the watchable kind `kind`, and an element of the `describe` result.
///
/// Clause: HC DS-1, DS-2, DS-3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct KindDescriptor {
    pub kind: String,
    /// `hub` or a package key.
    pub owner: String,
    /// Changes whenever the owning plugin reloads or the kind's schema changes (DS-6).
    pub descriptor_id: String,
    /// `"M.m"` (section 12, Versioning).
    pub schema_version: String,
    /// The kind's JSON Schema (draft 2020-12).
    pub schema: Value,
    pub core_fields: Vec<String>,
    pub verbs: BTreeMap<String, VerbDescriptor>,
    pub actions: BTreeMap<String, ActionDescriptor>,
    pub watchable: bool,
    pub durable: bool,
    /// The fields that views and watches of this kind may read (DS-1, UI-AD-1).
    pub projection_allowlist: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub list: Option<ListMode>,
    /// The range filters that the kind declares for `list` (VB-1), by field name.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub range_filters: Option<Vec<String>>,
}

/// The `ok` value of `describe`. `ui_contract_view` names the view vocabulary version (section 7b).
///
/// Clause: HC DS-1, section 7b.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DescribeResult {
    pub kinds: Vec<KindDescriptor>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "opt_no_null"
    )]
    pub ui_contract_view: Option<String>,
}
