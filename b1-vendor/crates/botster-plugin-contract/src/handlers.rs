//! The handler kinds: descriptor, grant, `Invoke.payload` and `Result.value` of each (HP A2-5).
//!
//! Clause: HP A2-5, CP-2, MT-1, MT-4, EV-15, CA-16, NT-7, WP-2, FS-3, VW-3, VW-8, IV-8.

use crate::common::OpenObject;
use crate::entities::{EntityProviderPayload, EntityProviderResult};
use crate::events::{EventGapRecord, EventRecord};
use crate::frames::HandlerKind;
use crate::operations::OperationRecord;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Descriptor of an `mcp_tool` (HP A2-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct McpToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

/// Descriptor of an `mcp_prompt` (HP A2-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct McpPromptDescriptor {
    pub name: String,
    pub description: String,
    pub arguments: Value,
}

/// Descriptor of an `mcp_resource` (HP A2-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct McpResourceDescriptor {
    pub uri_template: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub mime_type: Option<String>,
}

/// Descriptor of a `ui_action` (HP A2-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct UiActionDescriptor {
    pub action_id: String,
}

/// Descriptor of an `entity_provider` (HP A2-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EntityProviderDescriptor {
    pub kind: String,
}

/// Descriptor of a `webhook` (HP A2-5, NT-7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct WebhookDescriptor {
    pub path: String,
}

/// The empty descriptor of `event`, `event_gap`, `timer`, `store_watch`, `operation`, `relay` and `fs_watch` (HP A2-5: the registration names the handler by `handler_id`, A2-53).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EmptyDescriptor {}

/// `Invoke.payload` of an `mcp_prompt` is its `arguments`; of an `mcp_tool` its `arguments` (HP A2-5, MT-4). The value is the plugin's own schema.
pub type McpArguments = OpenObject;

/// `Invoke.payload` of an `mcp_resource` (HP A2-5).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct McpResourcePayload {
    pub uri: String,
}

/// One content block of an `mcp_resource` result (HP A2-5): `text` or `blob_b64`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceContent {
    pub uri: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub mime_type: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub text: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub blob_b64: Option<String>,
}

/// `Result.value` of an `mcp_resource` (HP A2-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct McpResourceResult {
    pub contents: Vec<ResourceContent>,
}

/// `Result.value` of an `mcp_prompt` (HP A2-5). The messages are MCP prompt messages (HP MC-0 revision).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct McpPromptResult {
    pub messages: Vec<Value>,
}

/// `Invoke.payload` of a `ui_action` (HP A2-5, VW-3, VW-8).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UiActionPayload {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub view_id: Option<String>,
    pub action_id: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub target: Option<Value>,
    pub input: Value,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub idempotency_key: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub operation_id: Option<String>,
}

/// `Result.value` of a `ui_action`: `{}` or `{operation_id}` (HP A2-5). A UI contract `ActionOk` shape (UI-AC-1) is carried in the open fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UiActionResult {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub operation_id: Option<String>,
    #[serde(flatten)]
    pub extra: OpenObject,
}

/// `Invoke.payload` of a `timer` (HP A2-5, CA-12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimerPayload {
    pub timer_id: String,
}

/// `Invoke.payload` of a `store_watch` (HP A2-5, ST-6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StoreWatchPayload {
    pub watch_id: String,
    pub collection: String,
}

/// `Invoke.payload` of an `fs_watch` (HP A2-5, FS-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FsWatchPayload {
    pub watch_id: String,
    pub root: String,
}

/// `Invoke.payload` of a `webhook` (HP A2-5, NT-7). `headers` is a list of `[name, value]` pairs as in A2-17.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WebhookPayload {
    pub path: String,
    pub headers: Vec<[String; 2]>,
    pub body_b64: String,
}

/// `Result.value` of a `webhook` (HP A2-5, NT-7): `status` is 100 to 599.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WebhookResult {
    pub status: u16,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub headers: Option<Vec<[String; 2]>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub body_b64: Option<String>,
}

/// `Invoke.payload` of a `relay` handler (HP A2-5, WP-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RelayPayload {
    pub relay_id: String,
    pub envelope_b64: String,
}

/// The empty `Result.value` `{}` of the handlers that return nothing (HP A2-5).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EmptyValue {}

/// One row of the handler-kind table (HP A2-5, CP-2): schemas are generated functions so the table stays data.
pub struct HandlerSpec {
    pub kind: HandlerKind,
    /// The grant that admits the kind (HP CP-2).
    pub grant: &'static str,
    pub descriptor: fn() -> Value,
    pub payload: fn() -> Value,
    pub result: fn() -> Value,
    pub clauses: &'static str,
}

fn sch<T: JsonSchema>() -> Value {
    crate::schema::inline::<T>()
}

/// The handler kinds, one row per wire `kind` string (HP A2-5).
pub const HANDLERS: &[HandlerSpec] = &[
    HandlerSpec {
        kind: HandlerKind::McpTool,
        grant: "mcp",
        descriptor: sch::<McpToolDescriptor>,
        payload: sch::<McpArguments>,
        result: sch::<Value>,
        clauses: "A2-5, MT-1, MT-4, MT-5, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::McpPrompt,
        grant: "mcp",
        descriptor: sch::<McpPromptDescriptor>,
        payload: sch::<McpArguments>,
        result: sch::<McpPromptResult>,
        clauses: "A2-5, MT-1, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::McpResource,
        grant: "mcp",
        descriptor: sch::<McpResourceDescriptor>,
        payload: sch::<McpResourcePayload>,
        result: sch::<McpResourceResult>,
        clauses: "A2-5, MT-1, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::UiAction,
        grant: "surfaces",
        descriptor: sch::<UiActionDescriptor>,
        payload: sch::<UiActionPayload>,
        result: sch::<UiActionResult>,
        clauses: "A2-5, VW-3, VW-8, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::Event,
        grant: "none",
        descriptor: sch::<EmptyDescriptor>,
        payload: sch::<EventRecord>,
        result: sch::<EmptyValue>,
        clauses: "A2-5, EV-15, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::EventGap,
        grant: "none",
        descriptor: sch::<EmptyDescriptor>,
        payload: sch::<EventGapRecord>,
        result: sch::<EmptyValue>,
        clauses: "A2-5, EV-15, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::EntityProvider,
        grant: "none",
        descriptor: sch::<EntityProviderDescriptor>,
        payload: sch::<EntityProviderPayload>,
        result: sch::<EntityProviderResult>,
        clauses: "A2-5, EN-2, CP-2, A5-1",
    },
    HandlerSpec {
        kind: HandlerKind::Timer,
        grant: "none",
        descriptor: sch::<EmptyDescriptor>,
        payload: sch::<TimerPayload>,
        result: sch::<EmptyValue>,
        clauses: "A2-5, CA-12, A2-53, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::StoreWatch,
        grant: "none",
        descriptor: sch::<EmptyDescriptor>,
        payload: sch::<StoreWatchPayload>,
        result: sch::<EmptyValue>,
        clauses: "A2-5, ST-6, A2-53, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::Operation,
        grant: "none",
        descriptor: sch::<EmptyDescriptor>,
        payload: sch::<OperationRecord>,
        result: sch::<EmptyValue>,
        clauses: "A2-5, CA-16, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::Webhook,
        grant: "net (scope net_inbound)",
        descriptor: sch::<WebhookDescriptor>,
        payload: sch::<WebhookPayload>,
        result: sch::<WebhookResult>,
        clauses: "A2-5, NT-7, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::Relay,
        grant: "relay (scope relay_signaling)",
        descriptor: sch::<EmptyDescriptor>,
        payload: sch::<RelayPayload>,
        result: sch::<EmptyValue>,
        clauses: "A2-5, WP-2, CP-2",
    },
    HandlerSpec {
        kind: HandlerKind::FsWatch,
        grant: "fs (scope fs:<root>)",
        descriptor: sch::<EmptyDescriptor>,
        payload: sch::<FsWatchPayload>,
        result: sch::<EmptyValue>,
        clauses: "A2-5, FS-3, CP-2",
    },
];

/// Checks that a descriptor has the shape its kind requires (HP A2-5). `Err` carries the serde message.
pub fn check_descriptor(kind: HandlerKind, descriptor: &Value) -> Result<(), String> {
    fn go<T: for<'de> Deserialize<'de>>(v: &Value) -> Result<(), String> {
        serde_json::from_value::<T>(v.clone())
            .map(|_| ())
            .map_err(|e| e.to_string())
    }
    match kind {
        HandlerKind::McpTool => go::<McpToolDescriptor>(descriptor),
        HandlerKind::McpPrompt => go::<McpPromptDescriptor>(descriptor),
        HandlerKind::McpResource => go::<McpResourceDescriptor>(descriptor),
        HandlerKind::UiAction => go::<UiActionDescriptor>(descriptor),
        HandlerKind::EntityProvider => go::<EntityProviderDescriptor>(descriptor),
        HandlerKind::Webhook => go::<WebhookDescriptor>(descriptor),
        HandlerKind::Event
        | HandlerKind::EventGap
        | HandlerKind::Timer
        | HandlerKind::StoreWatch
        | HandlerKind::Operation
        | HandlerKind::Relay
        | HandlerKind::FsWatch => go::<EmptyDescriptor>(descriptor),
    }
}
