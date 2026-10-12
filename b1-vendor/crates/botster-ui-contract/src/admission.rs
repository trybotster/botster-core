//! Admission: the checks that the Hub runs on a view at package load.
//!
//! Clause: UI-AD-1 (the list of checks), UI-AD-2, UI-AD-3, UI-PT-1, UI-PT-3, UI-PT-5, UI-ID-1, UI-BD-2, UI-BD-3,
//! UI-FM-1, UI-FM-3, UI-VR-1, UI-VR-3, UI-VR-4, UI-LM-1 (no numeric limit), UI-CF-3, UI-EM-11.
//!
//! Admission resolves, replaces and filters nothing (P6). One failure fails the load with the path of the defect.

use crate::doc::*;
use crate::ids::{is_author_id, is_role_word};
use crate::input::Viewport;
use crate::realize::split_slot;
use crate::value::{eq, truthy};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

/// The typed admission failures (UI-AD-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AdmissionCode {
    /// UI-VR-1, UI-VR-3.
    UnsupportedVersion,
    /// UI-VR-4: a feature that the document uses is not in `requires`, or `requires` names an unknown feature.
    MissingRequires,
    /// UI-PT-1.
    UnsupportedPart,
    /// UI-EM-11.
    UnknownProperty,
    /// UI-PT-5, UI-AD-1 (predicates in the grammar).
    BadPropertyType,
    /// UI-ID-1.
    InvalidId,
    DuplicateId,
    ReservedId,
    /// UI-BD-2.
    KindNotAdmitted,
    /// UI-AD-1: a path outside the projection allowlist.
    PathNotAllowed,
    /// UI-BD-3.
    UnboundContext,
    UnknownSlot,
    UnknownRef,
    UnknownAction,
    /// UI-AD-1: no registered `ui_action` handler.
    UnknownHandler,
    /// UI-FM-1.
    SchemaMismatch,
    SchemaUnsupported,
    UnknownBoundProperty,
    ArgTypeMismatch,
    /// UI-FM-3.
    VisibleWhenRequired,
    /// UI-CF-3.
    BadDefault,
    /// UI-PT-3.
    WhenNotExclusive,
    BadAlternatives,
    /// UI-SL-5.
    UnknownView,
    /// UI-AD-3.
    BadPlacement,
    LinkNotText,
    /// UI-AD-4, UI-EM-12: a `to_page` source that is not a kind of this package.
    EmbedSourceNotOwn,
    /// UI-FD-2: a document number outside the double range.
    NumberOutOfRange,
    /// UI-FD-2: a map key that could never match.
    NonCanonicalMapKey,
}

/// A typed admission failure with the path of the defect (UI-AD-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AdmissionFailure {
    pub code: AdmissionCode,
    /// A JSON Pointer into the document.
    pub path: String,
    pub message: String,
}

/// The Hub's descriptor of a kind, as admission reads it (UI-AD-1, Hub to client DS-1).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct KindDescriptor {
    /// The `projection_allowlist` of the descriptor (UI-AD-1).
    #[serde(default)]
    pub projection_allowlist: Vec<Path>,
    /// The package that owns the kind, when it is not a Hub kind (UI-EM-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    /// The JSON type of a field, by path, when the kind's schema states it (UI-FM-1, UI-AD-3).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub field_types: BTreeMap<Path, String>,
}

/// A registered `ui_action` handler and its canonical schema (UI-FM-1).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct HandlerDescriptor {
    pub schema: Value,
}

/// What admission knows about the package and the Hub.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AdmissionContext {
    /// The kinds that the package may watch (UI-BD-2). A kind that is not listed is not admitted.
    #[serde(default)]
    pub kinds: BTreeMap<String, KindDescriptor>,
    /// Registered handlers, by name (UI-AD-1).
    #[serde(default)]
    pub handlers: BTreeMap<String, HandlerDescriptor>,
    /// The key of the package that the document belongs to (UI-AD-4, UI-EM-12).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub package: Option<String>,
    /// The ids of the package's views (UI-SL-5).
    #[serde(default)]
    pub views: Vec<String>,
    /// The MAJOR versions that the Hub supports (UI-VR-3).
    #[serde(default = "default_majors")]
    pub supported_majors: Vec<u32>,
    /// The feature names that the Hub knows (UI-VR-4).
    #[serde(default = "default_features")]
    pub features: Vec<String>,
}

fn default_majors() -> Vec<u32> {
    vec![1]
}

fn default_features() -> Vec<String> {
    vec![
        "ui_1_1".into(),
        "ui_embed".into(),
        "ui_1_3".into(),
        "ui_1_4".into(),
    ]
}

impl Default for AdmissionContext {
    fn default() -> Self {
        AdmissionContext {
            kinds: BTreeMap::new(),
            handlers: BTreeMap::new(),
            package: None,
            views: vec![],
            supported_majors: default_majors(),
            features: default_features(),
        }
    }
}

type Res<T> = Result<T, AdmissionFailure>;

fn fail<T>(code: AdmissionCode, path: &str, message: impl Into<String>) -> Res<T> {
    Err(AdmissionFailure {
        code,
        path: path.to_string(),
        message: message.into(),
    })
}

/// Admits a view document (UI-AD-1). `Ok` is the typed document.
pub fn admit(doc: &Value, ctx: &AdmissionContext) -> Res<ViewDocument> {
    let typed = parse(doc)?;
    let mut c = Checker::new(&typed, ctx);
    c.run()?;
    Ok(typed)
}

/// Admits a view document that arrives as JSON text (UI-AD-1, UI-FD-2). A number outside the double range is refused
/// `number_out_of_range`: a JSON value cannot hold one, so this check reads the text.
pub fn admit_text(text: &str, ctx: &AdmissionContext) -> Res<ViewDocument> {
    match serde_json::from_str::<Value>(text) {
        Ok(doc) => admit(&doc, ctx),
        Err(e) if e.to_string().contains("number out of range") => fail(
            AdmissionCode::NumberOutOfRange,
            "",
            "a number outside the double range",
        ),
        Err(e) => fail(AdmissionCode::BadPropertyType, "", e.to_string()),
    }
}

/// Admits a navigation badge (UI-AD-3, UI-NB-1): the source names an admitted kind, and the field is in its allowlist.
pub fn admit_badge(badge: &Value, ctx: &AdmissionContext) -> Res<crate::doc::Badge> {
    let typed: crate::doc::Badge =
        serde_json::from_value(badge.clone()).map_err(|e| AdmissionFailure {
            code: AdmissionCode::BadPropertyType,
            path: String::new(),
            message: e.to_string(),
        })?;
    if !matches!(typed.display.as_str(), "dot" | "count") {
        return fail(
            AdmissionCode::BadPropertyType,
            "/display",
            "dot or count is required",
        );
    }
    let doc = ViewDocument {
        ui_contract_view: "1.1".into(),
        id: "badge".into(),
        title: "badge".into(),
        requires: vec![],
        selections: vec![],
        placement: None,
        root: Part::Stack(StackPart {
            id: "badge".into(),
            direction: Direction::Column,
            children: vec![],
            when: None,
            alternatives: false,
        }),
    };
    let c = Checker::new(&doc, ctx);
    c.kind(&typed.source.kind, "/source/kind")?;
    c.allowed(&typed.source.kind, &typed.source.field, "/source/field")?;
    Ok(typed)
}

/// UI-AD-2: a loaded view is `stale` when a path that it uses left a kind's allowlist. `stale` is a run-time
/// diagnostic of the view (steward ruling R-2.5), not an admission refusal. `Some` is that diagnostic.
pub fn check_stale(doc: &ViewDocument, ctx: &AdmissionContext) -> Option<crate::node::Diagnostic> {
    let mut c = Checker::new(doc, ctx);
    let failure = c
        .run()
        .err()
        .filter(|f| f.code == AdmissionCode::PathNotAllowed)?;
    Some(crate::node::Diagnostic::new(None, "stale", &failure.path))
}

// ---- structure: known parts, known properties, property types ---------------------------------------------------

const COMMON: &[&str] = &["id", "type"];

fn part_keys(ty: &str) -> Option<&'static [&'static str]> {
    Some(match ty {
        "list" => &[
            "source", "refs", "item", "order", "activate", "selects", "empty", "error",
        ],
        "table" => &[
            "source", "refs", "columns", "order", "activate", "selects", "empty", "error",
        ],
        "detail" => &["source", "refs", "fields", "empty", "error"],
        "form" => &["action", "schema", "submit", "cancel"],
        "actions" => &["items", "overflow"],
        "status_badge" => &["value", "map"],
        "stack" => &["direction", "children", "when", "alternatives"],
        "inline" => &["children", "when", "alternatives"],
        "panel" => &["title", "children", "when", "alternatives"],
        "section" => &["heading", "children", "when", "alternatives"],
        "toolbar" => &["items", "overflow", "when"],
        "text" => &["value", "tone", "when"],
        "empty_state" => &["title", "body", "when"],
        "embed" => &["title", "page", "channel", "empty", "error"],
        _ => return None,
    })
}

fn parse(doc: &Value) -> Res<ViewDocument> {
    let Some(obj) = doc.as_object() else {
        return fail(
            AdmissionCode::BadPropertyType,
            "",
            "the document is not an object",
        );
    };
    let version = obj.get("ui_contract_view").and_then(Value::as_str);
    if version.is_none() {
        return fail(
            AdmissionCode::BadPropertyType,
            "/ui_contract_view",
            "a string is required",
        );
    }
    // UI-LM-1: no numeric limit. The walk recurses over the document only.
    let mut shell = doc.clone();
    let mut parts = vec![];
    if let Some(root) = obj.get("root") {
        walk_parts(root, "/root", &mut parts)?;
        shell["root"] = json!({"type": "empty_state", "id": "_", "title": "_"});
    } else {
        return fail(
            AdmissionCode::BadPropertyType,
            "/root",
            "a part is required",
        );
    }
    // Type check the shell (the document fields), then rebuild the whole document.
    let typed: ViewDocument = serde_json::from_value(shell).map_err(|e| AdmissionFailure {
        code: AdmissionCode::BadPropertyType,
        path: String::new(),
        message: e.to_string(),
    })?;
    drop(typed);
    serde_json::from_value(doc.clone()).map_err(|e| AdmissionFailure {
        code: AdmissionCode::BadPropertyType,
        path: String::new(),
        message: e.to_string(),
    })
}

/// Checks each part on its own, so a failure names the part (UI-PT-1, UI-PT-5, UI-EM-11).
fn walk_parts(part: &Value, path: &str, out: &mut Vec<String>) -> Res<()> {
    let Some(obj) = part.as_object() else {
        return fail(AdmissionCode::BadPropertyType, path, "a part is an object");
    };
    let Some(ty) = obj.get("type").and_then(Value::as_str) else {
        return fail(AdmissionCode::UnsupportedPart, path, "a part needs a type");
    };
    let Some(keys) = part_keys(ty) else {
        // UI-PT-1: a part outside the closed set.
        return fail(
            AdmissionCode::UnsupportedPart,
            &format!("{path}/type"),
            format!("unsupported part '{ty}'"),
        );
    };
    for k in obj.keys() {
        if !COMMON.contains(&k.as_str()) && !keys.contains(&k.as_str()) {
            // UI-EM-11 (4): `sandbox`, `src`, `srcdoc` and `allow` on an embed are unknown properties.
            return fail(
                AdmissionCode::UnknownProperty,
                &format!("{path}/{k}"),
                format!("unknown property '{k}' on '{ty}'"),
            );
        }
    }
    // UI-PT-5: property types.
    if let Some(v) = obj.get("alternatives") {
        if !v.is_boolean() {
            return fail(
                AdmissionCode::BadPropertyType,
                &format!("{path}/alternatives"),
                "a boolean is required",
            );
        }
    }
    if let Some(v) = obj.get("overflow") {
        if !matches!(v.as_str(), Some("auto" | "never")) {
            return fail(
                AdmissionCode::BadPropertyType,
                &format!("{path}/overflow"),
                "auto or never is required",
            );
        }
    }
    for k in ["submit", "cancel"] {
        if let Some(v) = obj.get(k) {
            let ok = v.as_object().is_some_and(|o| {
                o.keys().all(|k| k == "label") && o.get("label").is_none_or(Value::is_string)
            });
            if !ok {
                return fail(
                    AdmissionCode::BadPropertyType,
                    &format!("{path}/{k}"),
                    "an object with an optional string label is required",
                );
            }
        }
    }
    if let Some(v) = obj.get("item") {
        let ok = v.as_object().is_some_and(|o| {
            o.keys().all(|k| k == "fields" || k == "actions")
                && o.get("fields").is_none_or(Value::is_array)
                && o.get("actions").is_none_or(Value::is_array)
        });
        if !ok {
            return fail(
                AdmissionCode::BadPropertyType,
                &format!("{path}/item"),
                "an item has fields and actions",
            );
        }
    }
    if let Some(Value::Array(children)) = obj.get("children") {
        for (i, c) in children.iter().enumerate() {
            walk_parts(c, &format!("{path}/children/{i}"), out)?;
        }
    }
    // The part itself must type check without its children.
    let mut shallow = part.clone();
    if shallow.get("children").is_some() {
        shallow["children"] = json!([]);
    }
    serde_json::from_value::<Part>(shallow).map_err(|e| AdmissionFailure {
        code: AdmissionCode::BadPropertyType,
        path: path.to_string(),
        message: e.to_string(),
    })?;
    out.push(path.to_string());
    Ok(())
}

// ---- semantics --------------------------------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Scope<'a> {
    /// The kind that `{ path }` reads. `Some("")` is a kind that is not known.
    path_kind: Option<&'a str>,
    row_kind: Option<&'a str>,
    refs: BTreeMap<&'a str, &'a str>,
    form: bool,
    when_only: bool,
}

struct Checker<'a> {
    doc: &'a ViewDocument,
    ctx: &'a AdmissionContext,
    slots: BTreeSet<&'a str>,
    slot_kind: BTreeMap<&'a str, &'a str>,
    all_refs: BTreeSet<&'a str>,
    actions: BTreeMap<&'a str, &'a Action>,
    uses_1_1: bool,
    uses_embed: bool,
}

impl<'a> Checker<'a> {
    fn new(doc: &'a ViewDocument, ctx: &'a AdmissionContext) -> Self {
        Checker {
            doc,
            ctx,
            slots: BTreeSet::new(),
            slot_kind: BTreeMap::new(),
            all_refs: BTreeSet::new(),
            actions: BTreeMap::new(),
            uses_1_1: false,
            uses_embed: false,
        }
    }

    fn run(&mut self) -> Res<()> {
        let doc = self.doc;
        // UI-VR-1, UI-VR-3: the MAJOR must be supported.
        // The form is exactly `M.m` (UI-VR-1), then the MAJOR must be supported.
        let major = parse_major_minor(&doc.ui_contract_view).map(|(m, _)| m);
        if !major.is_some_and(|m| self.ctx.supported_majors.contains(&m)) {
            return fail(
                AdmissionCode::UnsupportedVersion,
                "/ui_contract_view",
                format!("version {}", doc.ui_contract_view),
            );
        }
        for (i, f) in doc.requires.iter().enumerate() {
            if !self.ctx.features.contains(f) {
                return fail(
                    AdmissionCode::MissingRequires,
                    &format!("/requires/{i}"),
                    format!("unknown feature '{f}'"),
                );
            }
        }
        // UI-ID-1: unique ids across all kinds, well formed, never a role word.
        let mut seen: BTreeMap<String, String> = BTreeMap::new();
        self.collect_ids(&doc.root, "/root", &mut seen)?;
        for (i, s) in doc.selections.iter().enumerate() {
            self.id(&s.id, &format!("/selections/{i}/id"), &mut seen)?;
            self.slots.insert(&s.id);
        }
        self.index(&doc.root);
        if let Some(p) = &doc.placement {
            // UI-AD-3.
            self.uses_1_1 = true;
            if p.area != "home" || !matches!(p.size.as_str(), "small" | "medium" | "wide" | "full")
            {
                return fail(
                    AdmissionCode::BadPlacement,
                    "/placement",
                    "area is home; size is small, medium, wide or full",
                );
            }
        }
        self.part(&doc.root, "/root", &Scope::default())?;
        // UI-VR-4: a feature that the document uses must be in `requires`.
        if self.uses_1_1 && !doc.requires.iter().any(|r| r == "ui_1_1") {
            return fail(AdmissionCode::MissingRequires, "/requires", "ui_1_1");
        }
        if self.uses_embed && !doc.requires.iter().any(|r| r == "ui_embed") {
            return fail(AdmissionCode::MissingRequires, "/requires", "ui_embed");
        }
        Ok(())
    }

    fn id(&self, id: &str, path: &str, seen: &mut BTreeMap<String, String>) -> Res<()> {
        if !is_author_id(id) {
            return fail(
                AdmissionCode::InvalidId,
                path,
                format!("'{id}' is not a valid id"),
            );
        }
        if is_role_word(id) {
            return fail(
                AdmissionCode::ReservedId,
                path,
                format!("'{id}' is a reserved role word"),
            );
        }
        if let Some(first) = seen.insert(id.to_string(), path.to_string()) {
            return fail(
                AdmissionCode::DuplicateId,
                path,
                format!("'{id}' is already used at {first}"),
            );
        }
        Ok(())
    }

    fn collect_ids(&self, part: &Part, path: &str, seen: &mut BTreeMap<String, String>) -> Res<()> {
        self.id(part.id(), &format!("{path}/id"), seen)?;
        match part {
            Part::List(p) => {
                for (i, f) in p.item.fields.iter().enumerate() {
                    self.id(&f.id, &format!("{path}/item/fields/{i}/id"), seen)?;
                }
                for (i, a) in p.item.actions.iter().enumerate() {
                    self.id(&a.id, &format!("{path}/item/actions/{i}/id"), seen)?;
                }
                for r in p.refs.keys() {
                    self.id(r, &format!("{path}/refs/{r}"), seen)?;
                }
            }
            Part::Table(p) => {
                for (i, c) in p.columns.iter().enumerate() {
                    self.id(&c.id, &format!("{path}/columns/{i}/id"), seen)?;
                }
                for r in p.refs.keys() {
                    self.id(r, &format!("{path}/refs/{r}"), seen)?;
                }
            }
            Part::Detail(p) => {
                for (i, f) in p.fields.iter().enumerate() {
                    self.id(&f.id, &format!("{path}/fields/{i}/id"), seen)?;
                }
                for r in p.refs.keys() {
                    self.id(r, &format!("{path}/refs/{r}"), seen)?;
                }
            }
            Part::Actions(p) => self.action_ids(&p.items, path, seen)?,
            Part::Toolbar(p) => self.action_ids(&p.items, path, seen)?,
            Part::Stack(p) => self.child_ids(&p.children, path, seen)?,
            Part::Inline(p) => self.child_ids(&p.children, path, seen)?,
            Part::Panel(p) => self.child_ids(&p.children, path, seen)?,
            Part::Section(p) => self.child_ids(&p.children, path, seen)?,
            _ => {}
        }
        Ok(())
    }

    fn action_ids(
        &self,
        items: &[Action],
        path: &str,
        seen: &mut BTreeMap<String, String>,
    ) -> Res<()> {
        for (i, a) in items.iter().enumerate() {
            self.id(&a.id, &format!("{path}/items/{i}/id"), seen)?;
        }
        Ok(())
    }

    fn child_ids(
        &self,
        children: &[Part],
        path: &str,
        seen: &mut BTreeMap<String, String>,
    ) -> Res<()> {
        for (i, c) in children.iter().enumerate() {
            self.collect_ids(c, &format!("{path}/children/{i}"), seen)?;
        }
        Ok(())
    }

    fn index(&mut self, part: &'a Part) {
        match part {
            Part::List(p) => {
                if let Some(s) = &p.selects {
                    self.slot_kind.insert(s, &p.source.kind);
                }
                self.all_refs.extend(p.refs.keys().map(String::as_str));
                for a in &p.item.actions {
                    self.actions.insert(&a.id, a);
                }
            }
            Part::Table(p) => {
                if let Some(s) = &p.selects {
                    self.slot_kind.insert(s, &p.source.kind);
                }
                self.all_refs.extend(p.refs.keys().map(String::as_str));
            }
            Part::Detail(p) => self.all_refs.extend(p.refs.keys().map(String::as_str)),
            Part::Actions(p) => p.items.iter().for_each(|a| {
                self.actions.insert(&a.id, a);
            }),
            Part::Toolbar(p) => p.items.iter().for_each(|a| {
                self.actions.insert(&a.id, a);
            }),
            Part::Embed(_) => self.uses_embed = true,
            Part::Stack(p) => p.children.iter().for_each(|c| self.index(c)),
            Part::Inline(p) => p.children.iter().for_each(|c| self.index(c)),
            Part::Panel(p) => p.children.iter().for_each(|c| self.index(c)),
            Part::Section(p) => p.children.iter().for_each(|c| self.index(c)),
            _ => {}
        }
    }

    // ---- kinds, paths, operands ---------------------------------------------------------------------------

    fn kind(&self, kind: &str, path: &str) -> Res<&'a KindDescriptor> {
        // UI-BD-2.
        match self.ctx.kinds.get(kind) {
            Some(k) => Ok(k),
            None => fail(
                AdmissionCode::KindNotAdmitted,
                path,
                format!("kind '{kind}' is not admitted"),
            ),
        }
    }

    /// UI-AD-1: a path in the kind's projection allowlist. A sub-path of an allowed path is allowed.
    fn allowed(&self, kind: &str, p: &str, path: &str) -> Res<()> {
        let Some(k) = self.ctx.kinds.get(kind) else {
            return Ok(());
        };
        if k.projection_allowlist.iter().any(|e| {
            p == e || (p.starts_with(e.as_str()) && p.as_bytes().get(e.len()) == Some(&b'/'))
        }) {
            return Ok(());
        }
        fail(
            AdmissionCode::PathNotAllowed,
            path,
            format!("'{p}' is not in the allowlist of '{kind}'"),
        )
    }

    fn operand(&mut self, op: &Operand, scope: &Scope<'a>, path: &str) -> Res<()> {
        let unbound = |what: &str| -> Res<()> {
            fail(
                AdmissionCode::UnboundContext,
                path,
                format!("{what} is not in scope here"),
            )
        };
        match op {
            Operand::Path(p) => match scope.path_kind {
                Some(k) if !scope.when_only => self.allowed(k, p, path),
                _ => unbound("path"),
            },
            Operand::Row(p) => match scope.row_kind {
                Some(k) if !scope.when_only => self.allowed(k, p, path),
                _ => unbound("row"),
            },
            Operand::Form(_) => {
                if scope.form && !scope.when_only {
                    Ok(())
                } else {
                    unbound("form")
                }
            }
            Operand::Sel(s) => {
                if scope.when_only {
                    return unbound("sel");
                }
                let (slot, ptr) = split_slot(s);
                if !self.slots.contains(slot) {
                    return fail(
                        AdmissionCode::UnknownSlot,
                        path,
                        format!("slot '{slot}' is not declared"),
                    );
                }
                match (self.slot_kind.get(slot), ptr.is_empty()) {
                    (Some(k), false) => self.allowed(k, ptr, path),
                    _ => Ok(()),
                }
            }
            Operand::Ref(s) => {
                if scope.when_only {
                    return unbound("ref");
                }
                let (id, ptr) = split_slot(s);
                match scope.refs.get(id) {
                    Some(k) if !ptr.is_empty() => self.allowed(k, ptr, path),
                    Some(_) => Ok(()),
                    None if self.all_refs.contains(id) => unbound("ref"),
                    None => fail(
                        AdmissionCode::UnknownRef,
                        path,
                        format!("ref '{id}' is not declared"),
                    ),
                }
            }
            Operand::Viewport(_) => {
                if scope.when_only {
                    Ok(())
                } else {
                    unbound("viewport")
                }
            }
            Operand::Lit(_) => Ok(()),
        }
    }

    fn pred(&mut self, p: &Pred, scope: &Scope<'a>, path: &str) -> Res<()> {
        match p {
            Pred::Eq((a, b)) | Pred::Ne((a, b)) => {
                let key = if matches!(p, Pred::Eq(_)) { "eq" } else { "ne" };
                self.operand(a, scope, &format!("{path}/{key}/0"))?;
                self.operand(b, scope, &format!("{path}/{key}/1"))
            }
            Pred::Truthy(a) => self.operand(a, scope, &format!("{path}/truthy")),
            Pred::Not(q) => self.pred(q, scope, &format!("{path}/not")),
            Pred::All(l) | Pred::Any(l) => {
                let key = if matches!(p, Pred::All(_)) {
                    "all"
                } else {
                    "any"
                };
                for (i, q) in l.iter().enumerate() {
                    self.pred(q, scope, &format!("{path}/{key}/{i}"))?;
                }
                Ok(())
            }
        }
    }

    fn activate(&mut self, a: &Activate, scope: &Scope<'a>, path: &str, in_ref: bool) -> Res<()> {
        match a {
            Activate::Select(s) => {
                if !self.slots.contains(s.as_str()) {
                    return fail(AdmissionCode::UnknownSlot, path, format!("slot '{s}'"));
                }
            }
            Activate::Invoke(id) => {
                if !self.actions.contains_key(id.as_str()) {
                    return fail(AdmissionCode::UnknownAction, path, format!("action '{id}'"));
                }
            }
            Activate::Open(r) => {
                if !scope.refs.contains_key(r.as_str()) {
                    return fail(AdmissionCode::UnknownRef, path, format!("ref '{r}'"));
                }
            }
            Activate::Navigate(n) => {
                self.uses_1_1 = true;
                // UI-SL-5: `<package>/<view id>` names a view of the same package.
                let known = match n.view.rsplit_once('/') {
                    Some((pkg, view)) => {
                        self.ctx.package.as_deref().is_none_or(|p| p == pkg)
                            && self.ctx.views.iter().any(|v| v == view)
                    }
                    None => false,
                };
                if !known {
                    // UI-SL-5.
                    return fail(
                        AdmissionCode::UnknownView,
                        path,
                        format!("view '{}'", n.view),
                    );
                }
                for (slot, op) in &n.select {
                    self.operand(op, scope, &format!("{path}/navigate/select/{slot}"))?;
                }
            }
            Activate::Url(op) => {
                self.uses_1_1 = true;
                // UI-SL-7: not allowed on an action button; here it is on a row or a ref.
                let _ = in_ref;
                self.operand(op, scope, path)?;
            }
        }
        Ok(())
    }

    // ---- parts ----------------------------------------------------------------------------------------------

    fn part(&mut self, part: &'a Part, path: &str, outer: &Scope<'a>) -> Res<()> {
        let _ = outer;
        match part {
            Part::Stack(p) => self.structural(&p.when, p.alternatives, &p.children, path),
            Part::Inline(p) => self.structural(&p.when, p.alternatives, &p.children, path),
            Part::Panel(p) => self.structural(&p.when, p.alternatives, &p.children, path),
            Part::Section(p) => self.structural(&p.when, p.alternatives, &p.children, path),
            Part::Toolbar(p) => {
                self.when(&p.when, path)?;
                self.actions_list(&p.items, &Scope::default(), path)
            }
            Part::Actions(p) => self.actions_list(&p.items, &Scope::default(), path),
            Part::Text(p) => {
                self.when(&p.when, path)?;
                if let OperandOrLit::Operand(op) = &p.value {
                    self.operand(op, &Scope::default(), &format!("{path}/value"))?;
                }
                Ok(())
            }
            Part::EmptyState(p) => self.when(&p.when, path),
            Part::StatusBadge(p) => {
                self.operand(&p.value, &Scope::default(), &format!("{path}/value"))
            }
            Part::List(p) => {
                let scope = self.row_scope(&p.source, &p.refs, path)?;
                self.row_part(
                    &p.source,
                    &p.order,
                    &p.refs,
                    p.activate.as_ref(),
                    p.selects.as_deref(),
                    &scope,
                    path,
                )?;
                // UI-BD-3: `row` is in scope in `item`, and not in `source.where`.
                let mut item_scope = scope.clone();
                item_scope.row_kind = Some(&p.source.kind);
                for (i, f) in p.item.fields.iter().enumerate() {
                    self.field(f, &item_scope, &format!("{path}/item/fields/{i}"))?;
                }
                self.primary_count(&p.item.actions, &format!("{path}/item/actions"))?;
                for (i, a) in p.item.actions.iter().enumerate() {
                    self.action(a, &item_scope, &format!("{path}/item/actions/{i}"))?;
                }
                Ok(())
            }
            Part::Table(p) => {
                let scope = self.row_scope(&p.source, &p.refs, path)?;
                self.row_part(
                    &p.source,
                    &p.order,
                    &p.refs,
                    p.activate.as_ref(),
                    p.selects.as_deref(),
                    &scope,
                    path,
                )?;
                for (i, c) in p.columns.iter().enumerate() {
                    let cp = format!("{path}/columns/{i}");
                    if let Some(pp) = &c.path {
                        self.operand(&Operand::Path(pp.clone()), &scope, &cp)?;
                    }
                    if let Some(r) = &c.ref_ {
                        self.operand(&Operand::Ref(r.clone()), &scope, &cp)?;
                    }
                    self.map_keys(&c.map, c.path.as_deref(), scope.path_kind, &cp)?;
                }
                Ok(())
            }
            Part::Detail(p) => {
                let (kind, mut scope): (&str, Scope<'a>) = match &p.source {
                    DetailSource::ByKind { kind, id } => {
                        self.kind(kind, &format!("{path}/source/kind"))?;
                        if let OperandOrLit::Operand(op) = id {
                            self.operand(op, &Scope::default(), &format!("{path}/source/id"))?;
                        }
                        (kind, Scope::default())
                    }
                    DetailSource::BySel { sel } => {
                        if !self.slots.contains(sel.as_str()) {
                            return fail(
                                AdmissionCode::UnknownSlot,
                                &format!("{path}/source/sel"),
                                format!("slot '{sel}'"),
                            );
                        }
                        (
                            self.slot_kind.get(sel.as_str()).copied().unwrap_or(""),
                            Scope::default(),
                        )
                    }
                };
                scope.path_kind = Some(kind);
                scope.row_kind = Some(kind);
                for (id, r) in &p.refs {
                    self.kind(&r.kind, &format!("{path}/refs/{id}/kind"))?;
                    scope.refs.insert(id, &r.kind);
                }
                self.ref_keys(&p.refs, kind, path)?;
                for (i, f) in p.fields.iter().enumerate() {
                    self.field(f, &scope, &format!("{path}/fields/{i}"))?;
                }
                Ok(())
            }
            Part::Form(p) => {
                let Some(action) = self.actions.get(p.action.as_str()).copied() else {
                    return fail(
                        AdmissionCode::UnknownAction,
                        &format!("{path}/action"),
                        format!("action '{}'", p.action),
                    );
                };
                self.action_schema(
                    action,
                    Some(&p.schema),
                    &Scope::default(),
                    &format!("{path}/schema"),
                )
            }
            Part::Embed(p) => self.embed(p, path),
        }
    }

    /// UI-AD-4: the page asset is in scope, every `from_page` entry names an action or slot of the document, every
    /// `to_page` source is an admitted kind of this package with allowlisted fields, and a type is unique per direction.
    fn embed(&mut self, p: &'a EmbedPart, path: &str) -> Res<()> {
        if let OperandOrLit::Operand(op) = &p.page.asset {
            self.operand(op, &Scope::default(), &format!("{path}/page/asset"))?;
        }
        let Some(ch) = &p.channel else {
            return Ok(());
        };
        let mut seen = BTreeSet::new();
        for (i, m) in ch.from_page.iter().enumerate() {
            let at = format!("{path}/channel/from_page/{i}");
            if !seen.insert(m.type_.as_str()) {
                return fail(
                    AdmissionCode::DuplicateId,
                    &format!("{at}/type"),
                    "a message type is unique per direction",
                );
            }
            match (&m.invoke, &m.select) {
                (Some(a), None) => {
                    if !self.actions.contains_key(a.as_str()) {
                        return fail(
                            AdmissionCode::UnknownAction,
                            &format!("{at}/invoke"),
                            format!("action '{a}'"),
                        );
                    }
                }
                (None, Some(slot)) => {
                    if !self.slots.contains(slot.as_str()) {
                        return fail(
                            AdmissionCode::UnknownSlot,
                            &format!("{at}/select"),
                            format!("slot '{slot}'"),
                        );
                    }
                }
                _ => {
                    return fail(
                        AdmissionCode::BadPropertyType,
                        &at,
                        "a from_page entry has exactly one of invoke and select",
                    )
                }
            }
            for (name, op) in &m.args {
                self.operand(op, &Scope::default(), &format!("{at}/args/{name}"))?;
            }
        }
        let mut seen = BTreeSet::new();
        for (i, m) in ch.to_page.iter().enumerate() {
            let at = format!("{path}/channel/to_page/{i}");
            if !seen.insert(m.type_.as_str()) {
                return fail(
                    AdmissionCode::DuplicateId,
                    &format!("{at}/type"),
                    "a message type is unique per direction",
                );
            }
            if m.on != "change" {
                return fail(
                    AdmissionCode::BadPropertyType,
                    &format!("{at}/on"),
                    "change is required",
                );
            }
            let desc = self.kind(&m.source.kind, &format!("{at}/source/kind"))?;
            // UI-EM-12: a Hub kind, or a kind of another package, is never a source.
            let own = match (&desc.owner, &self.ctx.package) {
                (Some(o), Some(pkg)) => o == pkg,
                (Some(o), None) => o != "hub",
                (None, _) => false,
            };
            if !own {
                return fail(
                    AdmissionCode::EmbedSourceNotOwn,
                    &format!("{at}/source/kind"),
                    format!("kind '{}' is not owned by this package", m.source.kind),
                );
            }
            for f in &m.fields {
                self.allowed(&m.source.kind, f, &format!("{at}/fields"))?;
            }
            let scope = Scope {
                path_kind: Some(&m.source.kind),
                ..Scope::default()
            };
            if let Some(w) = &m.source.where_ {
                self.pred(w, &scope, &format!("{at}/source/where"))?;
            }
        }
        Ok(())
    }

    fn when(&mut self, when: &Option<Pred>, path: &str) -> Res<()> {
        if let Some(w) = when {
            // UI-PT-3: `when` reads the viewport only.
            let scope = Scope {
                when_only: true,
                ..Scope::default()
            };
            self.pred(w, &scope, &format!("{path}/when"))?;
        }
        Ok(())
    }

    fn structural(
        &mut self,
        when: &Option<Pred>,
        alternatives: bool,
        children: &'a [Part],
        path: &str,
    ) -> Res<()> {
        self.when(when, path)?;
        if alternatives {
            // UI-PT-3: alternatives all carry a `when`, and no two are true together in any of the 27 classes.
            let mut preds = vec![];
            for (i, c) in children.iter().enumerate() {
                match c.when() {
                    Some(w) => preds.push((i, w)),
                    None => {
                        return fail(
                            AdmissionCode::BadAlternatives,
                            &format!("{path}/children/{i}"),
                            "an alternative needs a when",
                        )
                    }
                }
            }
            for (ai, (i, a)) in preds.iter().enumerate() {
                for (j, b) in preds.iter().skip(ai + 1) {
                    for vp in Viewport::all() {
                        if viewport_pred(a, &vp) && viewport_pred(b, &vp) {
                            return fail(
                                AdmissionCode::WhenNotExclusive,
                                &format!("{path}/children/{j}"),
                                format!("children {i} and {j} are both true in one viewport class"),
                            );
                        }
                    }
                }
            }
        }
        for (i, c) in children.iter().enumerate() {
            self.part(c, &format!("{path}/children/{i}"), &Scope::default())?;
        }
        Ok(())
    }

    fn row_scope(
        &mut self,
        source: &'a Source,
        refs: &'a BTreeMap<String, Ref>,
        path: &str,
    ) -> Res<Scope<'a>> {
        self.kind(&source.kind, &format!("{path}/source/kind"))?;
        let mut scope = Scope {
            path_kind: Some(&source.kind),
            ..Scope::default()
        };
        for (id, r) in refs {
            self.kind(&r.kind, &format!("{path}/refs/{id}/kind"))?;
            scope.refs.insert(id, &r.kind);
        }
        Ok(scope)
    }

    #[allow(clippy::too_many_arguments)]
    fn row_part(
        &mut self,
        source: &'a Source,
        order: &[OrderKey],
        refs: &'a BTreeMap<String, Ref>,
        activate: Option<&Activate>,
        selects: Option<&str>,
        scope: &Scope<'a>,
        path: &str,
    ) -> Res<()> {
        self.ref_keys(refs, &source.kind, path)?;
        if let Some(w) = &source.where_ {
            self.pred(w, scope, &format!("{path}/source/where"))?;
        }
        for k in source.order.iter().chain(order) {
            self.allowed(&source.kind, &k.field, &format!("{path}/order"))?;
        }
        if let Some(s) = selects {
            if !self.slots.contains(s) {
                return fail(
                    AdmissionCode::UnknownSlot,
                    &format!("{path}/selects"),
                    format!("slot '{s}'"),
                );
            }
        }
        if let Some(a) = activate {
            self.activate(a, scope, &format!("{path}/activate"), false)?;
        }
        for (id, r) in refs {
            if let Some(a) = &r.activate {
                self.activate(a, scope, &format!("{path}/refs/{id}/activate"), true)?;
            }
        }
        Ok(())
    }

    fn ref_keys(&self, refs: &BTreeMap<String, Ref>, kind: &str, path: &str) -> Res<()> {
        for (id, r) in refs {
            let paths: Vec<&String> = match &r.key {
                RefKey::Id(p) => vec![p],
                RefKey::Structured { hub_id, id } => vec![hub_id, id],
            };
            for p in paths {
                self.allowed(kind, p, &format!("{path}/refs/{id}/key"))?;
            }
        }
        Ok(())
    }

    fn field(&mut self, f: &Field, scope: &Scope<'a>, path: &str) -> Res<()> {
        match (&f.path, &f.value) {
            (Some(p), None) => self.operand(&Operand::Path(p.clone()), scope, path)?,
            (None, Some(v)) => self.operand(v, scope, path)?,
            _ => {
                return fail(
                    AdmissionCode::BadPropertyType,
                    path,
                    "a field has exactly one of path and value",
                )
            }
        }
        self.map_keys(&f.map, f.path.as_deref(), scope.path_kind, path)?;
        if matches!(f.display, Some(OrUnknown::Known(Display::Link))) {
            self.uses_1_1 = true;
            // UI-AD-3: `link` only on a text-valued field.
            if let (Some(p), Some(k)) = (&f.path, scope.path_kind) {
                if let Some(t) = self.ctx.kinds.get(k).and_then(|d| d.field_types.get(p)) {
                    if t != "string" {
                        return fail(AdmissionCode::LinkNotText, path, format!("'{p}' is {t}"));
                    }
                }
            }
        }
        Ok(())
    }

    /// UI-FD-2: a map key is refused only when the map is over a field that the kind's schema declares numeric or
    /// boolean, and the key could never match there.
    fn map_keys(
        &self,
        map: &BTreeMap<String, MapEntry>,
        field: Option<&str>,
        kind: Option<&str>,
        path: &str,
    ) -> Res<()> {
        let ty = field
            .zip(kind)
            .and_then(|(p, k)| self.ctx.kinds.get(k)?.field_types.get(p));
        let Some(ty) = ty else {
            return Ok(());
        };
        for key in map.keys() {
            let canonical = match ty.as_str() {
                "number" | "integer" => serde_json::from_str::<serde_json::Number>(key)
                    .ok()
                    .and_then(|n| n.as_f64())
                    .filter(|f| f.is_finite())
                    .map(crate::value::es_number)
                    .filter(|c| c != key),
                "boolean"
                    if key.eq_ignore_ascii_case("true") || key.eq_ignore_ascii_case("false") =>
                {
                    Some(key.to_ascii_lowercase()).filter(|c| c != key)
                }
                _ => None,
            };
            if let Some(c) = canonical {
                return fail(
                    AdmissionCode::NonCanonicalMapKey,
                    &format!("{path}/map/{key}"),
                    format!("write the key as {c}"),
                );
            }
        }
        Ok(())
    }

    /// UI-AC-2: at most one primary action per toolbar.
    fn primary_count(&self, items: &[Action], path: &str) -> Res<()> {
        let mut primaries = items
            .iter()
            .enumerate()
            .filter(|(_, a)| a.priority == Some(Priority::Primary));
        if let (Some(_), Some((i, _))) = (primaries.next(), primaries.next()) {
            // UI-AC-2: the code is `bad_property_type`, the UI-AD-5 code of a property that no rule allows.
            return fail(
                AdmissionCode::BadPropertyType,
                &format!("{path}/{i}/priority"),
                "a toolbar has at most one primary action",
            );
        }
        Ok(())
    }

    fn actions_list(&mut self, items: &'a [Action], scope: &Scope<'a>, path: &str) -> Res<()> {
        self.primary_count(items, &format!("{path}/items"))?;
        for (i, a) in items.iter().enumerate() {
            self.action(a, scope, &format!("{path}/items/{i}"))?;
        }
        Ok(())
    }

    // ---- actions and forms ----------------------------------------------------------------------------------

    fn action(&mut self, a: &'a Action, scope: &Scope<'a>, path: &str) -> Res<()> {
        if let Some(w) = &a.when {
            self.pred(w, scope, &format!("{path}/when"))?;
        }
        let mut args_scope = scope.clone();
        args_scope.form = true;
        for (name, op) in &a.args {
            self.operand(op, &args_scope, &format!("{path}/args/{name}"))?;
        }
        self.action_schema(
            a,
            a.args_schema.as_ref(),
            scope,
            &format!("{path}/args_schema"),
        )
    }

    /// UI-FM-1: the view's schema equals the action's canonical schema, apart from `ui` hints and bound properties.
    fn action_schema(
        &mut self,
        a: &'a Action,
        view_schema: Option<&'a Value>,
        scope: &Scope<'a>,
        path: &str,
    ) -> Res<()> {
        let Some(handler) = self.ctx.handlers.get(&a.handler) else {
            return fail(
                AdmissionCode::UnknownHandler,
                path,
                format!("no registered handler '{}'", a.handler),
            );
        };
        let canonical = &handler.schema;
        check_subset(canonical, path)?;
        if let Some(s) = view_schema {
            check_subset(s, path)?;
            self.check_hints(canonical, s, scope, path)?;
        }
        let canon_props = canonical.get("properties").and_then(Value::as_object);
        for (name, op) in &a.args {
            let Some(prop) = canon_props.and_then(|m| m.get(name)) else {
                return fail(
                    AdmissionCode::UnknownBoundProperty,
                    path,
                    format!("'{name}' is not in the canonical schema"),
                );
            };
            if let (Some(want), Some(got)) = (
                prop.get("type").and_then(Value::as_str),
                self.operand_type(op, scope),
            ) {
                let ok = want == got || (want == "number" && got == "integer");
                if !ok {
                    return fail(
                        AdmissionCode::ArgTypeMismatch,
                        path,
                        format!("'{name}' is {want}; the operand is {got}"),
                    );
                }
            }
        }
        let bound: BTreeSet<&str> = a.args.keys().map(String::as_str).collect();
        let default_schema = json!({"type": "object"});
        let view = normalize(view_schema.unwrap_or(&default_schema), &bound);
        let canon = normalize(canonical, &bound);
        if view != canon {
            return fail(
                AdmissionCode::SchemaMismatch,
                path,
                "the schema differs from the action's canonical schema",
            );
        }
        Ok(())
    }

    fn operand_type(&self, op: &Operand, scope: &Scope<'a>) -> Option<String> {
        let field = |kind: Option<&str>, p: &str| -> Option<String> {
            self.ctx.kinds.get(kind?)?.field_types.get(p).cloned()
        };
        match op {
            Operand::Lit(v) => Some(
                match v {
                    Value::String(_) => "string",
                    Value::Bool(_) => "boolean",
                    Value::Number(n) if n.is_i64() || n.is_u64() => "integer",
                    Value::Number(_) => "number",
                    Value::Array(_) => "array",
                    Value::Object(_) => "object",
                    Value::Null => return None,
                }
                .to_string(),
            ),
            Operand::Path(p) => field(scope.path_kind, p),
            Operand::Row(p) => field(scope.row_kind, p),
            Operand::Sel(s) => {
                let (slot, ptr) = split_slot(s);
                field(self.slot_kind.get(slot).copied(), ptr)
            }
            Operand::Ref(s) => {
                let (id, ptr) = split_slot(s);
                field(scope.refs.get(id).copied(), ptr)
            }
            _ => None,
        }
    }

    /// UI-FM-3, UI-FM-4, UI-FM-5, UI-CF-3: the `ui` hints and the defaults of a view schema.
    /// `visible_when` is checked against the canonical `required` list, because the comparison removes bound properties.
    fn check_hints(
        &mut self,
        canonical: &Value,
        schema: &'a Value,
        scope: &Scope<'a>,
        path: &str,
    ) -> Res<()> {
        let mut required: BTreeSet<&str> = BTreeSet::new();
        for list in [schema.get("required"), canonical.get("required")] {
            required.extend(
                list.and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str),
            );
        }
        let Some(props) = schema.get("properties").and_then(Value::as_object) else {
            return Ok(());
        };
        let mut form_scope = scope.clone();
        form_scope.form = true;
        for (name, spec) in props {
            let pp = format!("{path}/properties/{name}");
            if is_role_word(name) || name.starts_with("group:") {
                return fail(
                    AdmissionCode::ReservedId,
                    &pp,
                    format!("'{name}' is a reserved word"),
                );
            }
            if let Some(ui) = spec.get("ui") {
                if ui.get("visible_when").is_some() && required.contains(name.as_str()) {
                    // UI-FM-3.
                    return fail(
                        AdmissionCode::VisibleWhenRequired,
                        &format!("{pp}/ui/visible_when"),
                        format!("'{name}' is required"),
                    );
                }
                for key in ["visible_when", "required_when"] {
                    if let Some(v) = ui.get(key) {
                        let p = self.pred_value(v, &format!("{pp}/ui/{key}"))?;
                        self.pred(&p, &form_scope, &format!("{pp}/ui/{key}"))?;
                    }
                }
                if let Some(src) = ui.get("options_source") {
                    self.options_source(src, &form_scope, &format!("{pp}/ui/options_source"))?;
                }
            }
            // UI-FM-5: `default_from` is an operand in scope.
            if let Some(v) = spec.get("default_from") {
                let Ok(op) = serde_json::from_value::<Operand>(v.clone()) else {
                    return fail(
                        AdmissionCode::BadPropertyType,
                        &format!("{pp}/default_from"),
                        "not an operand of the grammar",
                    );
                };
                self.operand(&op, &form_scope, &format!("{pp}/default_from"))?;
            }
            // UI-CF-3: an array default whose element holds a newline is refused.
            if spec.get("type").and_then(Value::as_str) == Some("array") {
                if let Some(Value::Array(d)) = spec.get("default") {
                    if d.iter()
                        .any(|e| e.as_str().is_some_and(|s| s.contains('\n')))
                    {
                        return fail(
                            AdmissionCode::BadDefault,
                            &format!("{pp}/default"),
                            "an element contains a newline",
                        );
                    }
                }
            }
        }
        Ok(())
    }

    fn pred_value(&self, v: &Value, path: &str) -> Res<Pred> {
        serde_json::from_value::<Pred>(v.clone()).or_else(|_| {
            fail(
                AdmissionCode::BadPropertyType,
                path,
                "not a predicate of the grammar",
            )
        })
    }

    /// UI-FM-4, UI-BD-2, UI-AD-1: the kinds, paths and predicates of an `options_source`, including `order` and `exclude`.
    fn options_source(&mut self, src: &'a Value, form_scope: &Scope<'a>, path: &str) -> Res<()> {
        let kind = src.get("kind").and_then(Value::as_str).unwrap_or("");
        self.kind(kind, &format!("{path}/kind"))?;
        let mut s = form_scope.clone();
        s.path_kind = Some(kind);
        if let Some(w) = src.get("where") {
            let p = self.pred_value(w, &format!("{path}/where"))?;
            self.pred(&p, &s, &format!("{path}/where"))?;
        }
        for f in ["value_field", "label_field"] {
            if let Some(p) = src.get(f).and_then(Value::as_str) {
                self.allowed(kind, p, &format!("{path}/{f}"))?;
            }
        }
        for (i, k) in src
            .get("order")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .enumerate()
        {
            if let Some(p) = k.get("field").and_then(Value::as_str) {
                self.allowed(kind, p, &format!("{path}/order/{i}/field"))?;
            }
        }
        if let Some(ex) = src.get("exclude") {
            let ek = ex.get("kind").and_then(Value::as_str).unwrap_or("");
            self.kind(ek, &format!("{path}/exclude/kind"))?;
            let mut es = form_scope.clone();
            es.path_kind = Some(ek);
            if let Some(w) = ex.get("where") {
                let p = self.pred_value(w, &format!("{path}/exclude/where"))?;
                self.pred(&p, &es, &format!("{path}/exclude/where"))?;
            }
            if let Some(p) = ex.get("value_field").and_then(Value::as_str) {
                self.allowed(ek, p, &format!("{path}/exclude/value_field"))?;
            }
        }
        Ok(())
    }
}

/// UI-FM-1: the closed schema subset. The keywords are `type`, `properties`, `required`, `enum`, `default`, `minimum`,
/// `maximum`, `minLength`, `maxLength` and `items`. The view adds `ui` (UI-FM-2), `default_from` (UI-FM-5) and
/// `secret` (UI-FM-8). Every other keyword is outside the subset.
fn check_subset(schema: &Value, path: &str) -> Res<()> {
    const KEYWORDS: [&str; 13] = [
        "type",
        "properties",
        "required",
        "enum",
        "default",
        "minimum",
        "maximum",
        "minLength",
        "maxLength",
        "items",
        "ui",
        "default_from",
        "secret",
    ];
    const TYPES: [&str; 6] = ["string", "integer", "number", "boolean", "array", "object"];
    let Some(obj) = schema.as_object() else {
        return fail(
            AdmissionCode::SchemaUnsupported,
            path,
            "a schema is an object",
        );
    };
    if let Some(k) = obj.keys().find(|k| !KEYWORDS.contains(&k.as_str())) {
        return fail(
            AdmissionCode::SchemaUnsupported,
            &format!("{path}/{k}"),
            format!("'{k}' is not in the subset"),
        );
    }
    if let Some(t) = obj.get("type") {
        if !t.as_str().is_some_and(|t| TYPES.contains(&t)) {
            return fail(
                AdmissionCode::SchemaUnsupported,
                &format!("{path}/type"),
                "type is outside the subset",
            );
        }
    }
    let kw_type = |key: &str, ok: fn(&Value) -> bool, what: &str| -> Res<()> {
        match obj.get(key) {
            Some(v) if !ok(v) => fail(
                AdmissionCode::SchemaUnsupported,
                &format!("{path}/{key}"),
                format!("{key} is {what}"),
            ),
            _ => Ok(()),
        }
    };
    kw_type(
        "required",
        |v| v.as_array().is_some_and(|l| l.iter().all(Value::is_string)),
        "a list of names",
    )?;
    kw_type("enum", Value::is_array, "a list")?;
    kw_type("minimum", Value::is_number, "a number")?;
    kw_type("maximum", Value::is_number, "a number")?;
    kw_type("minLength", Value::is_u64, "a count")?;
    kw_type("maxLength", Value::is_u64, "a count")?;
    kw_type("secret", Value::is_boolean, "a boolean")?;
    kw_type("properties", Value::is_object, "an object")?;
    if let Some(Value::Object(props)) = obj.get("properties") {
        for (k, v) in props {
            check_subset(v, &format!("{path}/properties/{k}"))?;
        }
    }
    if let Some(items) = obj.get("items") {
        check_subset(items, &format!("{path}/items"))?;
    }
    Ok(())
}

/// The comparison form of UI-FM-1: no `ui` hints, no `default_from`, no bound properties.
fn normalize(schema: &Value, bound: &BTreeSet<&str>) -> Value {
    fn strip(v: &Value) -> Value {
        match v {
            Value::Object(m) => Value::Object(
                m.iter()
                    .filter(|(k, _)| *k != "ui" && *k != "default_from")
                    .map(|(k, v)| (k.clone(), strip(v)))
                    .collect(),
            ),
            Value::Array(l) => Value::Array(l.iter().map(strip).collect()),
            other => other.clone(),
        }
    }
    let mut s = strip(schema);
    if let Some(m) = s.as_object_mut() {
        if let Some(Value::Object(props)) = m.get_mut("properties") {
            props.retain(|k, _| !bound.contains(k.as_str()));
            if props.is_empty() {
                m.remove("properties");
            }
        }
        if let Some(Value::Array(req)) = m.get_mut("required") {
            req.retain(|r| r.as_str().is_none_or(|r| !bound.contains(r)));
            if req.is_empty() {
                m.remove("required");
            }
        }
    }
    s
}

/// A predicate over the viewport only, for the enumeration of UI-PT-3.
fn viewport_pred(p: &Pred, vp: &Viewport) -> bool {
    let eval = |op: &Operand| -> Option<Value> {
        match op {
            Operand::Viewport(axis) => Some(Value::String(vp.class(*axis).to_string())),
            Operand::Lit(v) => Some(v.clone()),
            _ => None,
        }
    };
    match p {
        Pred::Eq((a, b)) => eq(eval(a).as_ref(), eval(b).as_ref()),
        Pred::Ne((a, b)) => !eq(eval(a).as_ref(), eval(b).as_ref()),
        Pred::Truthy(a) => truthy(eval(a).as_ref()),
        Pred::Not(q) => !viewport_pred(q, vp),
        Pred::All(l) => l.iter().all(|q| viewport_pred(q, vp)),
        Pred::Any(l) => l.iter().any(|q| viewport_pred(q, vp)),
    }
}

/// Reads `"M.m"`: two decimal numbers and one dot, nothing else (UI-VR-1).
fn parse_major_minor(text: &str) -> Option<(u32, u32)> {
    let (major, minor) = text.split_once('.')?;
    let digits = |t: &str| !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit());
    if !digits(major) || !digits(minor) {
        return None;
    }
    Some((major.parse().ok()?, minor.parse().ok()?))
}
