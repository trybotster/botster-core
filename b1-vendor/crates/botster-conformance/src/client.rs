//! The step vocabulary of the `client` subject (design 5.5, lead ruling of ui-amendments).
//!
//! A client is a renderer: Web, the TUI or a client core. The steps are renderer-neutral. They name a node by the id that
//! `realize` gives it, and they name what the client does by typed observations. They hold no DOM selector and no
//! widget name. There are three steps:
//!
//! * `client_load`: the runner hands the client an admitted document and the Hub's data;
//! * `client_act`: one input to the client. It is a user action, an answer or an event of the Hub, or an event of the
//!   frame or of the view;
//! * `client_expect`: one typed observation of the client.
//!
//! `setup.admission` is an admission context, and `setup.app_origin` is the origin of the app, for a clause that compares
//! an origin (UI-EM-11).
//!
//! Phase 0 checks a `client` transcript structurally (steward ruling R-1.4): every step parses as one of these types,
//! the document passes admission, and every embedded value parses as its contract type. There is no fake client. A real
//! client implements the three steps in Phase 1. This crate holds no contract type, so the documents stay JSON values.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

/// One step of a `client` transcript.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientStep {
    /// The runner hands the client a valid admitted document and Hub data (UI-AD-1, UI-RZ-3).
    ClientLoad(Box<ClientLoad>),
    /// One input to the client.
    ClientAct(ClientAct),
    /// One typed observation of the client.
    ClientExpect(ClientExpect),
}

/// `client_load`: the inputs of `realize` (ui-contract section 12, UI-NV-5), except that the document is admitted first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientLoad {
    /// A view document. It must pass admission (`setup.admission` gives the context).
    pub document: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind_snapshots: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_state: Option<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub viewport: Option<Value>,
    /// The `ui_capabilities` of the client under test (UI-CF-6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<Value>,
    /// Properties that a realized `embed` node carries although admission refuses them in a document (UI-EM-10,
    /// UI-EM-11): `{ "<embed node id>": { sandbox?, src?, srcdoc?, allow? } }`. The renderer must refuse them. The
    /// structural check does not admit these.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub embed_properties: Option<Value>,
}

/// `client_act`: an input to the client. `node_id` is the node that the input is aimed at, when it has one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientAct {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub action: Action,
}

/// An input to the client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    /// Click, tap or Enter on the node (UI-SL-4, UI-AC-3).
    Activate {},
    /// Press a key while the node has focus: `Tab`, `Shift+Tab` or `Enter` (UI-CF-4).
    Key { key: String },
    /// Edit one field of the form node (UI-RZ-2d, UI-FM-1).
    Edit { field: String, value: Value },
    /// Submit the form node (UI-AC-4).
    Submit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        values: Option<Map<String, Value>>,
    },
    /// Answer the open confirmation with its confirm button (UI-AC-3).
    Confirm {},
    /// Answer the open confirmation or form with its cancel button (UI-AC-3).
    Cancel {},
    /// The Hub answers a request of the client. `request` is the 1-based number of the request, in the order in which
    /// the client sent them. `ok` is the `ok` value, `err` the Error (UI-AC-1b).
    HubResult {
        request: u32,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        ok: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        err: Option<Value>,
    },
    /// The rows of a watched kind change (UI-NV-5 `kind_snapshots`).
    HubRows {
        kind: String,
        #[serde(default = "yes")]
        baseline: bool,
        rows: Vec<Value>,
    },
    /// The client re-reads `describe` and the descriptor differs (UI-AC-7(b)).
    DescriptorChanged {},
    /// The frame of an `embed` sends a message (UI-EM-5). `via` is `port` (the transferred port) or `window`.
    PageMessage {
        via: MessageVia,
        #[serde(rename = "type")]
        type_: String,
        payload: Value,
    },
    /// The frame loads a document. The first load is the asset document. A later load is a navigation (UI-EM-5).
    FrameLoad {},
    /// The capability token of the frame passes `expires_at` (UI-EM-7).
    TokenExpiry {},
    /// Something happens to the view of the frame (UI-EM-13).
    ViewEvent { event: ViewEvent },
    /// The transport to the Hub drops and returns (UI-AC-2b: a new incarnation).
    Reconnect {},
    /// The user asks for a reload of an errored frame (UI-EM-13, UI-EM-15).
    Reload {},
}

fn yes() -> bool {
    true
}

/// Where a frame message comes from (UI-EM-5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MessageVia {
    Port,
    Window,
}

/// What happens to the view of a frame (UI-EM-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ViewEvent {
    Removed,
    HiddenAndUnmounted,
    Revoked,
    PackageReload,
    AssetChanged,
}

/// `client_expect`: one typed observation. `node_id` names the node it is about, when it is about one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ClientExpect {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_id: Option<String>,
    pub observation: Observation,
    /// `"<contract> <clause id>"`, for example `"UI UI-AC-3"`. A sentence of that clause fixes the observed value
    /// (design 5.6 rule 6).
    pub because: String,
}

/// What a client shows or does. Each variant cites the clause that it observes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub enum Observation {
    /// The node shows this `Text` (a string or a copy key, UI-NV-6) in this tone (UI-AC-2b, UI-AC-7, UI-EM-15).
    Shows {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        text: Option<Value>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tone: Option<String>,
    },
    /// The node has keyboard focus (UI-CF-4).
    Focused(bool),
    /// The renderer drew the realized tree with no unsupported-node and no unsupported-prop diagnostic (UI-RZ-2c).
    Drawn(bool),
    /// The node is disabled or enabled (UI-AC-5, UI-AC-8).
    Disabled(bool),
    /// The action of the node is in its pending state (UI-AC-2b).
    Pending(bool),
    /// The reason that the disabled node shows (UI-AC-8).
    DisabledReason(Value),
    /// The node shows the pending text of an accepted action, not a predicate or `disabled_reason` text (UI-AC-8).
    ShowsPendingText(bool),
    /// The node shows the label of a terminal state that the client does not list, as non-exhaustive "other" text
    /// (UI-AC-2b).
    ShowsOther(bool),
    /// The node shows that the outcome is unknown (UI-AC-2b, UI-AC-7(c)).
    OutcomeUnknown(bool),
    /// The client has sent a request that matches. `args` and `target` are matchers of the engine (design 5.2: a literal,
    /// `{"$absent": true}`, `{"$any": true}`). The client has sent this many requests in all, or at least one that matches (UI-AC-3, UI-AC-7(c), UI-EM-5).
    RequestSent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        action: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        target: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        args: Option<Value>,
        /// The number of requests with this action. Absent means one or more.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        count: Option<u32>,
    },
    /// The client has sent a `describe` request that no `descriptor_changed` has answered yet (UI-AC-7(b)).
    DescribeRequested(bool),
    /// A dialog is open for the node: its form or its confirmation (UI-AC-4, UI-EM-5).
    Dialog {
        kind: DialogKind,
        /// The values that the form shows (UI-EM-5: prefilled from a page payload).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        values: Option<Map<String, Value>>,
        /// True when the dialog is drawn inside a frame. The client draws dialogs itself, so false (UI-EM-9).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        inside_frame: Option<bool>,
    },
    /// No dialog is open (UI-AC-3, UI-EM-5).
    NoDialog(bool),
    /// The client shows a local diagnostic (UI-CF-1, UI-AC-6, UI-EM-10). An absent `code` means any code, for a clause
    /// that fixes no code.
    Diagnostic {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        code: Option<String>,
    },
    /// The frame of an `embed` node (UI-EM-3, UI-EM-9, UI-EM-11).
    Frame(FrameObservation),
    /// The state of the message channel of an `embed` node (UI-EM-5, UI-EM-13).
    Channel(ChannelState),
    /// The client dropped the last message that it received from the frame (UI-EM-5, UI-EM-14).
    MessageDropped(bool),
    /// Every message that the client sent to the page, except the init message, has one of these types: the page
    /// receives no result or failure of its own message (UI-EM-14).
    PageReceivedOnly { types: Vec<String> },
    /// The client pushed exactly this message to the page: the payload matches exactly, so an extra member fails
    /// (UI-EM-5, UI-EM-14).
    Pushed {
        #[serde(rename = "type")]
        type_: String,
        payload: Value,
    },
    /// The `embed` node shows its `error` state, with or without a reload action (UI-EM-13, UI-EM-15).
    ErrorState {
        /// Whether a reload action is offered. Absent means the clause fixes only the error state.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reload: Option<bool>,
    },
    /// The renderer shows the link fallback of an `embed` (UI-EM-6, UI-EM-8).
    Link {
        /// The line that opens the page in a browser is shown. Absent means the clause does not fix it.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        open_in_browser: Option<bool>,
        /// The text shows the full URL, with its token (UI-EM-8: only after an explicit activation).
        reveals_url: bool,
    },
    /// A follow-up link to a view, with preset slots (UI-AC-6).
    FollowUp {
        view: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        select: Option<Map<String, Value>>,
    },
    /// The client moved to this view (UI-AC-6, UI-SL-5).
    MovedTo { view: String },
    /// The client did not move away from the current view (UI-AC-6: not while a draft or a confirmation is open).
    Stayed(bool),
    /// A slot holds this row key, or nothing (UI-SL-5).
    Slot {
        slot: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        row_key: Option<String>,
    },
    /// No log, diagnostic, notice or error text of the client holds a capability token (UI-EM-8).
    LogsHoldNoToken(bool),
}

/// The kind of a dialog (UI-AC-4).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DialogKind {
    Form,
    Confirm,
}

/// The state of a message channel (UI-EM-13).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ChannelState {
    Open,
    Closed,
}

/// The origin of a frame `src` compared with the app origin, after URL parsing (UI-EM-11).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SrcOrigin {
    DistinctFromApp,
    SameAsApp,
}

/// What the frame of an `embed` node is, if a frame is loaded (UI-EM-3, UI-EM-9, UI-EM-11). An absent field is not checked.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FrameObservation {
    /// False means that no frame is loaded (UI-EM-10, UI-EM-11).
    pub loaded: bool,
    /// The complete set of sandbox tokens (UI-EM-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sandbox: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrerpolicy: Option<String>,
    /// The complete set of `allow` features (UI-EM-3).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub allow: Option<Vec<String>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub src_origin: Option<SrcOrigin>,
    /// The client draws the frame inside its own box and never over anything outside it (UI-EM-9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confined_to_box: Option<bool>,
    /// The box has a visible border (UI-EM-9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bordered: Option<bool>,
    /// The client draws its label, with the plugin and the title, outside the frame (UI-EM-9).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_outside: Option<bool>,
    /// The app bearer, cookie or session token is in the `src`, a message or a request of the frame (UI-EM-4).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub carries_app_credentials: Option<bool>,
}

/// Parses the steps of a `client` transcript. `Err` names the first step that is not a [`ClientStep`], or the rule that
/// the sequence breaks: the first step is `client_load`.
pub fn parse_client_steps(steps: &[Value]) -> Result<Vec<ClientStep>, String> {
    let mut out = Vec::with_capacity(steps.len());
    for (i, v) in steps.iter().enumerate() {
        let step: ClientStep =
            serde_json::from_value(v.clone()).map_err(|e| format!("step {i}: {e}"))?;
        out.push(step);
    }
    if !matches!(out.first(), Some(ClientStep::ClientLoad(_))) {
        return Err("the first step of a client transcript is client_load".into());
    }
    Ok(out)
}

/// The generated JSON Schema of [`ClientStep`], as the text of `schemas/client-step.schema.json`.
pub fn client_step_schema_json() -> String {
    let schema = schemars::schema_for!(ClientStep);
    let mut text = serde_json::to_string_pretty(&schema).expect("schema serializes");
    text.push('\n');
    text
}
