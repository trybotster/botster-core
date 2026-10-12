//! The document model: the view declaration that a plugin authors.
//!
//! Clause: UI-ID-1, UI-PT-1, UI-PT-5, UI-BD-1, UI-RF-1, UI-FM-1, UI-SL-3, UI-AC-1, UI-CP-1, UI-VR-1,
//! UI-SL-5, UI-PL-1, UI-NB-1, UI-EM-1, UI-AC-8, UI-SA-5 (v1.1 to v1.3 types are present, see each item).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// A JSON Pointer into a row, for example `/name` (UI-BD-1, section 4 of ui-contract 1.0).
pub type Path = String;

/// A value that is one of the known variants, or a name that this release does not know.
/// UI-VR-2 says a client ignores an unknown optional `tone`, `display` or `widget` and counts it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OrUnknown<T> {
    Known(T),
    Unknown(String),
}

impl<T> OrUnknown<T> {
    pub fn known(&self) -> Option<&T> {
        match self {
            OrUnknown::Known(t) => Some(t),
            OrUnknown::Unknown(_) => None,
        }
    }
}

/// Clause: UI-CF-4. A tone maps to a style, never to a fixed color.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    Neutral,
    Info,
    Success,
    Warning,
    Danger,
}

impl Tone {
    pub fn as_str(self) -> &'static str {
        match self {
            Tone::Neutral => "neutral",
            Tone::Info => "info",
            Tone::Success => "success",
            Tone::Warning => "warning",
            Tone::Danger => "danger",
        }
    }
}

/// Clause: UI-PT-3 (the `viewport` operand axes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ViewportAxis {
    Width,
    Height,
    Pointer,
}

/// An operand (UI-BD-1, UI-BD-3). Each variant is written as an object with one key.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Operand {
    /// A field of the bound row.
    Path(Path),
    /// The row that an action or item sits on.
    Row(Path),
    /// The current value of a form field (`/<property name>`).
    Form(Path),
    /// `"<selection id><Path>"`, the selected row of a slot.
    Sel(String),
    /// `"<ref id><Path>"`, a field of a resolved ref.
    Ref(String),
    Viewport(ViewportAxis),
    Lit(Value),
}

/// A literal JSON value or an operand (`text.value`, `detail.source.id`, UI-PT-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum OperandOrLit {
    Operand(Operand),
    Literal(Value),
}

/// A predicate (UI-BD-1, UI-BD-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Pred {
    Eq((Operand, Operand)),
    Ne((Operand, Operand)),
    Truthy(Operand),
    Not(Box<Pred>),
    All(Vec<Pred>),
    Any(Vec<Pred>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Dir {
    Asc,
    Desc,
}

/// One ordering key (UI-RZ-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OrderKey {
    pub field: Path,
    pub dir: Dir,
}

/// A source of rows of one kind (UI-BD-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Source {
    pub kind: String,
    #[serde(default, rename = "where", skip_serializing_if = "Option::is_none")]
    pub where_: Option<Pred>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<OrderKey>,
}

/// The source of a `detail`: one row by id, or the selected row of a slot (UI-PT-5, UI-SL-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum DetailSource {
    ByKind { kind: String, id: OperandOrLit },
    BySel { sel: String },
}

/// The key of a ref (UI-RF-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum RefKey {
    Id(Path),
    Structured { hub_id: Path, id: Path },
}

/// What a ref shows when it has no target (UI-RF-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AbsentCopy {
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<OrUnknown<Tone>>,
}

/// A typed reference to a row of another kind (UI-RF-1 to UI-RF-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Ref {
    pub kind: String,
    pub key: RefKey,
    pub absent: AbsentCopy,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activate: Option<Activate>,
}

/// The target of a navigation (UI-SL-5, UI-AC-6, UI-SL-6): a view of the same package and preset slots.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Navigate {
    pub view: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub select: BTreeMap<String, Operand>,
}

/// An activation (UI-SL-3; `navigate` and `url` are UI-SL-5 and UI-SL-7 of ui-contract 1.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Activate {
    Select(String),
    Invoke(String),
    Open(String),
    Navigate(Navigate),
    Url(Operand),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Display {
    Text,
    Badge,
    Metric,
    Number,
    /// UI-FD-1 (ui-contract 1.1).
    Link,
}

/// One entry of a `Field.map` (section 5 of ui-contract 1.0, UI-FD-2).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MapEntry {
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<OrUnknown<Tone>>,
}

/// A field (the one definition of section 5 of ui-contract 1.0).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Field {
    pub id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<Path>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub value: Option<Operand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<OrUnknown<Display>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map: BTreeMap<String, MapEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<OrUnknown<Tone>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absent: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Align {
    Start,
    Center,
    End,
}

/// A table column (section 3 of ui-contract 1.0): a field with `label`, and `path` or `ref`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Column {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path: Option<Path>,
    /// `"<ref id><Path>"` (UI-RF-3).
    #[serde(default, rename = "ref", skip_serializing_if = "Option::is_none")]
    pub ref_: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<OrUnknown<Display>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map: BTreeMap<String, MapEntry>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<OrUnknown<Tone>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub absent: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub align: Option<Align>,
}

/// Copy for a state (UI-CP-1 to UI-CP-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CopyText {
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<OrUnknown<Tone>>,
}

/// Clause: UI-AC-3.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Confirm {
    pub message: String,
    pub tone: Tone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Priority {
    Primary,
    Secondary,
}

/// An action (section 9 of ui-contract 1.0; `disabled_reason` is UI-AC-8, `warning` is UI-SA-5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Action {
    pub id: String,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The registered `ui_action` handler (UI-AC-1, UI-AD-1).
    pub handler: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub args_schema: Option<Value>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Operand>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub confirm: Option<Confirm>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub priority: Option<Priority>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disabled_reason: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

/// The `item` of a `list` (UI-PT-5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Item {
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub actions: Vec<Action>,
}

/// `{ label? }` of `form.submit` and `form.cancel` (UI-PT-5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ButtonSpec {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Overflow {
    Auto,
    #[default]
    Never,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    #[default]
    Column,
    Row,
}

/// A client-local selection slot (UI-SL-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SelectionDecl {
    pub id: String,
}

/// Home placement (UI-PL-1, ui-contract 1.1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Placement {
    pub area: String,
    pub size: String,
    pub order: i64,
}

/// A navigation badge (UI-NB-1, ui-contract 1.1): one row and one field.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Badge {
    pub source: BadgeSource,
    pub display: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<OrUnknown<Tone>>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct BadgeSource {
    pub kind: String,
    pub id: String,
    pub field: Path,
}

/// One message that a page may send (UI-EM-5, UI-EM-14): `invoke` or `select`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FromPage {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invoke: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub select: Option<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub args: BTreeMap<String, Operand>,
}

/// One push of rows to a page (UI-EM-5, UI-EM-14).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToPage {
    #[serde(rename = "type")]
    pub type_: String,
    pub source: Source,
    pub fields: Vec<Path>,
    pub on: String,
}

/// The message channel of an `embed` (UI-EM-5).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Channel {
    #[serde(default)]
    pub from_page: Vec<FromPage>,
    #[serde(default)]
    pub to_page: Vec<ToPage>,
}

/// The page of an `embed` (UI-EM-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmbedPage {
    pub asset: OperandOrLit,
}

/// An asset reference that `page.asset` resolves to (UI-EM-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum AssetRef {
    ById { asset_id: String },
    ByName { name: String },
}

/// The data and structural parts (UI-PT-1; `embed` is UI-EM-1, behind the feature `ui_embed`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Part {
    List(ListPart),
    Table(TablePart),
    Detail(DetailPart),
    Form(FormPart),
    Actions(ActionsPart),
    StatusBadge(StatusBadgePart),
    Stack(StackPart),
    Inline(InlinePart),
    Panel(PanelPart),
    Section(SectionPart),
    Toolbar(ToolbarPart),
    Text(TextPart),
    EmptyState(EmptyStatePart),
    Embed(EmbedPart),
}

impl Part {
    pub fn id(&self) -> &str {
        match self {
            Part::List(p) => &p.id,
            Part::Table(p) => &p.id,
            Part::Detail(p) => &p.id,
            Part::Form(p) => &p.id,
            Part::Actions(p) => &p.id,
            Part::StatusBadge(p) => &p.id,
            Part::Stack(p) => &p.id,
            Part::Inline(p) => &p.id,
            Part::Panel(p) => &p.id,
            Part::Section(p) => &p.id,
            Part::Toolbar(p) => &p.id,
            Part::Text(p) => &p.id,
            Part::EmptyState(p) => &p.id,
            Part::Embed(p) => &p.id,
        }
    }

    /// The `when` of a structural part (UI-PT-3).
    pub fn when(&self) -> Option<&Pred> {
        match self {
            Part::Stack(p) => p.when.as_ref(),
            Part::Inline(p) => p.when.as_ref(),
            Part::Panel(p) => p.when.as_ref(),
            Part::Section(p) => p.when.as_ref(),
            Part::Toolbar(p) => p.when.as_ref(),
            Part::Text(p) => p.when.as_ref(),
            Part::EmptyState(p) => p.when.as_ref(),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListPart {
    pub id: String,
    pub source: Source,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refs: BTreeMap<String, Ref>,
    pub item: Item,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<OrderKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activate: Option<Activate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selects: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<CopyText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CopyText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TablePart {
    pub id: String,
    pub source: Source,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refs: BTreeMap<String, Ref>,
    pub columns: Vec<Column>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub order: Vec<OrderKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub activate: Option<Activate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selects: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<CopyText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CopyText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DetailPart {
    pub id: String,
    pub source: DetailSource,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refs: BTreeMap<String, Ref>,
    pub fields: Vec<Field>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub empty: Option<CopyText>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<CopyText>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FormPart {
    pub id: String,
    /// The id of the action that this form submits.
    pub action: String,
    pub schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub submit: Option<ButtonSpec>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cancel: Option<ButtonSpec>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActionsPart {
    pub id: String,
    pub items: Vec<Action>,
    #[serde(default)]
    pub overflow: Overflow,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StatusBadgePart {
    pub id: String,
    pub value: Operand,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub map: BTreeMap<String, MapEntry>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StackPart {
    pub id: String,
    #[serde(default)]
    pub direction: Direction,
    pub children: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub alternatives: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InlinePart {
    pub id: String,
    pub children: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub alternatives: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PanelPart {
    pub id: String,
    pub title: String,
    pub children: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub alternatives: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SectionPart {
    pub id: String,
    pub heading: String,
    pub children: Vec<Part>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
    #[serde(default, skip_serializing_if = "is_false")]
    pub alternatives: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ToolbarPart {
    pub id: String,
    pub items: Vec<Action>,
    #[serde(default)]
    pub overflow: Overflow,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct TextPart {
    pub id: String,
    pub value: OperandOrLit,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tone: Option<OrUnknown<Tone>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmptyStatePart {
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub when: Option<Pred>,
}

/// UI-EM-1 (ui-contract 1.2). A document that uses it sets `requires: ["ui_embed"]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EmbedPart {
    pub id: String,
    pub title: String,
    pub page: EmbedPage,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<Channel>,
    pub empty: CopyText,
    pub error: CopyText,
}

fn is_false(b: &bool) -> bool {
    !*b
}

/// The view document (section 2 of ui-contract 1.0; `placement` is UI-PL-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ViewDocument {
    /// `"M.m"` (UI-VR-1).
    pub ui_contract_view: String,
    pub id: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requires: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selections: Vec<SelectionDecl>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    pub root: Part,
}

/// The wire form of a field error (UI-FM-7, UI-ID-4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FieldError {
    pub field_id: String,
    pub pointer: String,
    pub code: String,
    pub message: String,
}

/// The `ok` value of a `ui_action` (UI-AC-1, UI-AC-1b, UI-AC-6). A successful result carries no replacement tree.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ActionOk {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub navigate: Option<NavigationTarget>,
}

/// The failure of an action as the client reads it (UI-AC-1b, UI-FM-7).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct ActionError {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub field_errors: Vec<FieldError>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

/// A navigation target with row keys (UI-SL-6, UI-AC-6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NavigationTarget {
    pub view: String,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub select: BTreeMap<String, String>,
}
