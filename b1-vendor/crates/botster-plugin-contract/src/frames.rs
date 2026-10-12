//! The ten frames of HP section 2 with the fields of Appendix B, A2-25 and A2-53.
//!
//! Clause: HP section 2, HS-1, HS-2, IV-1, IV-5, CA-1, CA-2, CR-2, Appendix B, A2-25.
//!
//! Every frame payload also carries the connection `epoch` (HP section 2, MD-4). An unknown field in a payload is ignored (HP section 18).

use crate::common::{Effect, Epoch, OpenObject, SessionRef, WireId};
use crate::limits::PluginLimits;
use crate::result::PluginResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// The feature names the contract itself defines (HP section 11, MT-7). Other names belong to the Hub's feature list in `Hello.features`.
pub const FEATURE_MCP_PROXY: &str = "mcp_proxy";

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A grant: a surface and an optional scope (HP section 3, HS-1, MN-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Grant {
    pub surface: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub scope: Option<String>,
}

/// A manifest capability (HP MN-1, MN-2, NT-2, CP-8). Also the entry of `Ready.grants`. `allow` is only on surface `net` (HP MN-1): the decoder and the schema refuse it elsewhere.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(extend("if" = {"required": ["allow"]}, "then" = {"properties": {"surface": {"const": "net"}}}))]
pub struct Capability {
    pub surface: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub scope: Option<String>,
    /// Only on a capability the plugin can do without (HP CP-8, MN-1).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub optional: Option<bool>,
    /// Only on surface `net` (HP NT-2, MN-1).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub allow: Option<Vec<String>>,
}

impl<'de> Deserialize<'de> for Capability {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Wire {
            surface: String,
            #[serde(default, deserialize_with = "crate::common::opt_no_null")]
            scope: Option<String>,
            #[serde(default, deserialize_with = "crate::common::opt_no_null")]
            optional: Option<bool>,
            #[serde(default, deserialize_with = "crate::common::opt_no_null")]
            allow: Option<Vec<String>>,
        }
        let w = Wire::deserialize(d)?;
        if w.allow.is_some() && w.surface != "net" {
            return Err(serde::de::Error::custom(
                "`allow` is only valid on surface `net` (HP MN-1)",
            ));
        }
        Ok(Capability {
            surface: w.surface,
            scope: w.scope,
            optional: w.optional,
            allow: w.allow,
        })
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A resolved interface binding (HP HS-1, PD-5, A2-25). `provider` is absent for an unmet optional one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HelloBinding {
    pub interface: String,
    pub interface_version: String,
    /// `null` for an unmet optional binding (HP HS-1, PD-5, PD-8); an absent key is accepted as well (A2-25).
    #[serde(default)]
    #[schemars(schema_with = "crate::schema::string_or_null")]
    pub provider: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub provider_version: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub interface_hash: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A priority name and its rank (HP EV-11, A2-25). v1: `high` 2, `normal` 1, `low` 0.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Priority {
    pub name: String,
    pub rank: u32,
}

/// The v1 priorities (HP EV-11).
pub fn v1_priorities() -> Vec<Priority> {
    [("high", 2), ("normal", 1), ("low", 0)]
        .into_iter()
        .map(|(n, r)| Priority {
            name: n.into(),
            rank: r,
        })
        .collect()
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Hub to plugin: the first frame on lane 0 (HP HS-1, LD-2, A2-25).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hello {
    pub epoch: Epoch,
    pub protocol_version: u32,
    pub min_protocol_version: u32,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2)))]
    pub features: Vec<String>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Grant>(), 0..2)))]
    pub grants: Vec<Grant>,
    pub limits: PluginLimits,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<HelloBinding>(), 0..2)))]
    pub bindings: Vec<HelloBinding>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Priority>(), 0..2)))]
    pub priorities: Vec<Priority>,
    /// An opaque instance id used for fencing and logs (HP section 2).
    pub instance: String,
    pub plugin_key: String,
    pub resumed: bool,
    pub hub_id: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The kind of a handler, as a wire string (HP A2-5). A kind outside this set fails closed (HP VR-4).
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum HandlerKind {
    McpTool,
    McpPrompt,
    McpResource,
    UiAction,
    Event,
    EventGap,
    EntityProvider,
    Timer,
    StoreWatch,
    Operation,
    Webhook,
    Relay,
    FsWatch,
}

impl HandlerKind {
    pub const ALL: [HandlerKind; 13] = [
        HandlerKind::McpTool,
        HandlerKind::McpPrompt,
        HandlerKind::McpResource,
        HandlerKind::UiAction,
        HandlerKind::Event,
        HandlerKind::EventGap,
        HandlerKind::EntityProvider,
        HandlerKind::Timer,
        HandlerKind::StoreWatch,
        HandlerKind::Operation,
        HandlerKind::Webhook,
        HandlerKind::Relay,
        HandlerKind::FsWatch,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            HandlerKind::McpTool => "mcp_tool",
            HandlerKind::McpPrompt => "mcp_prompt",
            HandlerKind::McpResource => "mcp_resource",
            HandlerKind::UiAction => "ui_action",
            HandlerKind::Event => "event",
            HandlerKind::EventGap => "event_gap",
            HandlerKind::EntityProvider => "entity_provider",
            HandlerKind::Timer => "timer",
            HandlerKind::StoreWatch => "store_watch",
            HandlerKind::Operation => "operation",
            HandlerKind::Webhook => "webhook",
            HandlerKind::Relay => "relay",
            HandlerKind::FsWatch => "fs_watch",
        }
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A handler registration: id, kind and descriptor (HP HS-2, IV-8, A2-5). The descriptor body depends on the kind; see `handlers`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HandlerDecl {
    pub id: String,
    pub kind: HandlerKind,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub descriptor: Value,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An MCP tool registration (HP MT-1, PC-1). The contract states these fields; other keys are accepted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolDecl {
    pub name: String,
    pub description: String,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub input_schema: Value,
    pub handler: String,
    /// `["plugins"]` opts in to plugin calls (HP PC-1).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(proptest::option::of(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2))))]
    pub callable_by: Option<Vec<String>>,
    #[serde(flatten)]
    #[cfg_attr(test, strategy(crate::props::extra()))]
    pub extra: OpenObject,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An MCP prompt registration (HP MT-1). Other keys are accepted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PromptDecl {
    pub name: String,
    pub description: String,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub arguments: Value,
    pub handler: String,
    #[serde(flatten)]
    #[cfg_attr(test, strategy(crate::props::extra()))]
    pub extra: OpenObject,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An MCP resource registration (HP MT-1, A2-5 descriptor). Other keys are accepted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResourceDecl {
    pub uri_template: String,
    pub name: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub mime_type: Option<String>,
    pub handler: String,
    #[serde(flatten)]
    #[cfg_attr(test, strategy(crate::props::extra()))]
    pub extra: OpenObject,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An event audience (HP EV-1, MN-3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    Plugins,
    Clients,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The `state_source` of an event declaration (HP EV-1, EV-12, MN-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct StateSource {
    pub read: String,
    pub grant: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// An event a plugin declares that it emits (HP EV-1, MN-3, Appendix B). `priority` is a name of `Hello.priorities` (default `normal`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeclaredEvent {
    pub name: String,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub payload_schema: Value,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Audience>(), 0..2)))]
    pub audience: Vec<Audience>,
    pub public: bool,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub priority: Option<String>,
    /// The event may carry a coalescing `key` (HP EV-13, MN-3). Present in the manifest entry.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub keyed: Option<bool>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub state_source: Option<StateSource>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The `{owner, name}` pattern of a subscription (HP EV-2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventPattern {
    pub owner: String,
    pub name: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Which sessions a subscription covers (HP EV-2, CP-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SubjectsScope {
    Own,
    Any,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A subscription entry (HP EV-2, MN-3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    pub handler_id: String,
    pub pattern: EventPattern,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub subjects: Option<SubjectsScope>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub min_priority: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The events section of `Ready` and of the manifest (HP HS-2, MN-3, EV-15).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EventsDecl {
    #[serde(default)]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<DeclaredEvent>(), 0..2)))]
    pub declared: Vec<DeclaredEvent>,
    #[serde(default)]
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Subscription>(), 0..2)))]
    pub subscribed: Vec<Subscription>,
    /// The handler id of the `event_gap` handler (HP EV-15).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub gap_handler: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A timer to re-arm (HP HS-2, CA-12, A2-25, A2-53).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct TimerDecl {
    pub timer_id: String,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub delay_ms: u64,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub handler_id: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A store watch to re-arm (HP HS-2, ST-6, A2-14, A2-25, A2-53).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct WatchDecl {
    pub collection: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub index: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(crate::props::opt_vec_value()))]
    pub equals: Option<Vec<Value>>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub handler_id: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// A filesystem watch to re-arm (HP FS-3, A2-25). `handler_id` is mandatory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FsWatchDecl {
    pub root: String,
    pub path: String,
    pub handler_id: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The action-to-handler mapping of a contributed session action (HP VW-8, A2-25).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionActionMap {
    pub action_id: String,
    pub handler_id: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The operation-completion subscription (HP CA-16, A2-5): the handler of kind `operation` that receives it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct OperationsDecl {
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub subscribed: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The host facts of `Ready` (HP HS-2, LH-2, LH-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct HostInfo {
    pub name: String,
    pub version: String,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2)))]
    pub globals: Vec<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The feature names a plugin needs (HP HS-4). The two sets are distinct names.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Features {
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2)))]
    pub required: Vec<String>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<String>(), 0..2)))]
    pub optional: Vec<String>,
}

impl Features {
    /// A name in both sets breaks HS-4 ("the two sets are distinct").
    pub fn overlap(&self) -> Vec<&str> {
        self.required
            .iter()
            .filter(|n| self.optional.contains(n))
            .map(String::as_str)
            .collect()
    }

    /// The required names the Hub does not offer (HP HS-4: the load fails with `feature_missing{names}`).
    pub fn missing<'a>(&'a self, offered: &[String]) -> Vec<&'a str> {
        self.required
            .iter()
            .filter(|n| !offered.contains(n))
            .map(String::as_str)
            .collect()
    }

    /// The optional names that the Hub offers (HP section 18: selected by intersection).
    pub fn selected_optional<'a>(&'a self, offered: &[String]) -> Vec<&'a str> {
        self.optional
            .iter()
            .filter(|n| offered.contains(n))
            .map(String::as_str)
            .collect()
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Plugin to Hub: the proposed registration (HP HS-2, Appendix B, A2-25).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Ready {
    pub epoch: Epoch,
    pub protocol_version: u32,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<HandlerDecl>(), 0..2)))]
    pub handlers: Vec<HandlerDecl>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<ToolDecl>(), 0..2)))]
    pub tools: Vec<ToolDecl>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<PromptDecl>(), 0..2)))]
    pub prompts: Vec<PromptDecl>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<ResourceDecl>(), 0..2)))]
    pub resources: Vec<ResourceDecl>,
    pub events: EventsDecl,
    /// Entity family declarations (HP EN-1). The contract states no field names beyond EN-1's list; see `entities`.
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<crate::entities::EntityFamilyDecl>(), 0..2)))]
    pub entity_providers: Vec<crate::entities::EntityFamilyDecl>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<TimerDecl>(), 0..2)))]
    pub timers: Vec<TimerDecl>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<WatchDecl>(), 0..2)))]
    pub watches: Vec<WatchDecl>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<FsWatchDecl>(), 0..2)))]
    pub fs_watches: Vec<FsWatchDecl>,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<SessionActionMap>(), 0..2)))]
    pub session_actions: Vec<SessionActionMap>,
    pub operations: OperationsDecl,
    #[cfg_attr(test, strategy(proptest::collection::vec(proptest::prelude::any::<Capability>(), 0..2)))]
    pub grants: Vec<Capability>,
    pub host: HostInfo,
    pub features: Features,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Hub to plugin: the Hub published the registration (HP HS-2). The payload is the epoch only.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Activated {
    pub epoch: Epoch,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Who asked (HP section 7, VC-1). A kind outside this set fails closed (HP VR-4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Caller {
    Operator {
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::common::opt_no_null"
        )]
        credential_id: Option<String>,
        #[serde(
            default,
            skip_serializing_if = "Option::is_none",
            deserialize_with = "crate::common::opt_no_null"
        )]
        hub_id: Option<String>,
    },
    Plugin {
        plugin_key: String,
        instance: String,
    },
    Session {
        session: SessionRef,
    },
    System,
}

/// The `_meta` key under which the Hub forwards the verified caller to a proxied MCP server (HP MP-5).
/// The Hub overwrites any caller-supplied `botster/*` key.
pub const MCP_CALLER_META_KEY: &str = "botster/caller";

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The caller that the Hub forwards to a proxied MCP server under [`MCP_CALLER_META_KEY`] (HP MP-5): `{kind, session?}`.
/// It is advisory to the server. It never carries a credential, so the other fields of [`Caller`] stay out.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct McpCallerMeta {
    /// The `kind` tag of the verified [`Caller`].
    pub kind: String,
    /// Present only for a session caller.
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub session: Option<SessionRef>,
}

impl From<&Caller> for McpCallerMeta {
    fn from(caller: &Caller) -> Self {
        let (kind, session) = match caller {
            Caller::Operator { .. } => ("operator", None),
            Caller::Plugin { .. } => ("plugin", None),
            Caller::Session { session } => ("session", Some(session.clone())),
            Caller::System => ("system", None),
        };
        McpCallerMeta {
            kind: kind.into(),
            session,
        }
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// How a handler was reached (HP VC-5, A2-5). Informative and non-exhaustive: an unknown origin is opaque.
/// The known strings are the 13 handler kinds; `command`, `hook` and `session_action` are removed (HP A2-5).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Origin(pub String);

impl JsonSchema for Origin {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "Origin".into()
    }
    fn json_schema(_: &mut schemars::SchemaGenerator) -> schemars::Schema {
        let known: Vec<&str> = HandlerKind::ALL.iter().map(|k| k.as_str()).collect();
        schemars::json_schema!({
            "description": "HP VC-5, A2-5: how the handler was reached. Non-exhaustive; an unknown string is opaque.",
            "type": "string",
            "examples": known
        })
    }
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The source of an event invocation (HP VC-7, EV-9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct EventSourceRef {
    pub kind: SourceKind,
    pub instance: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub session: Option<SessionRef>,
    /// Absent means this Hub (HP EV-9).
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub hub_id: Option<String>,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The kind of an event source (HP EV-9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Hub,
    Plugin,
    Terminal,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Hub to plugin: run one handler (HP IV-1, Appendix B).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Invoke {
    pub epoch: Epoch,
    pub request_id: WireId,
    pub handler: String,
    pub kind: HandlerKind,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub payload: Value,
    pub caller: Caller,
    pub origin: Origin,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    pub source: Option<EventSourceRef>,
    /// The opaque descriptor identity, revalidated at dispatch (HP IV-8).
    pub descriptor_id: String,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub deadline_ms: u64,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Hub to plugin: cancel one invocation (HP IV-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Cancel {
    pub epoch: Epoch,
    pub request_id: WireId,
    pub reason: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Plugin to Hub: the handler finished (HP IV-2). The frame type is named `Result` on the wire.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ResultFrame {
    pub epoch: Epoch,
    pub request_id: WireId,
    pub result: PluginResult,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// The context of a host call: exactly one of three (HP section 2, CA-3).
/// Wire form: `{"request": <request_id>}`, `"bootstrap"` or `"autonomous"`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CallContext {
    Request(WireId),
    Bootstrap,
    Autonomous,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Plugin to Hub: ask the Hub to do one operation (HP CA-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HostCall {
    pub epoch: Epoch,
    pub call_id: WireId,
    pub context: CallContext,
    /// `<area>.<name>` (HP CA-1).
    pub call: String,
    #[cfg_attr(test, strategy(crate::props::value()))]
    pub args: Value,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub max_result_bytes: u64,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Hub to plugin: the Hub's answer (HP CA-2, CA-5). `effect` describes that call only (HP ER-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HostResult {
    pub epoch: Epoch,
    pub call_id: WireId,
    pub result: PluginResult,
    pub effect: Effect,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Hub to plugin: orderly stop or refused registration (HP LD-4, HS-2). `reason` is a typed reason string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Shutdown {
    pub epoch: Epoch,
    #[cfg_attr(test, strategy(crate::props::wire_int()))]
    pub deadline_ms: u64,
    pub reason: String,
}

#[cfg_attr(test, derive(test_strategy::Arbitrary))]
/// Plugin to Hub, or Hub to plugin for an unknown frame: the host cannot continue (HP CR-2, VR-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Fault {
    pub epoch: Epoch,
    pub code: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "crate::common::opt_no_null"
    )]
    #[cfg_attr(test, strategy(crate::props::opt_value()))]
    pub detail: Option<Value>,
}
