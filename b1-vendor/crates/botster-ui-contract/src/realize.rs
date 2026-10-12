//! The reference `realize` (section 12 of ui-contract 1.0).
//!
//! Clause: UI-RZ-1 (pure and deterministic: no I/O, no clock, no randomness), UI-RZ-2, UI-RZ-2a, UI-RZ-2b, UI-RZ-4, UI-RZ-5,
//! and the shapes of UI-NV-1 to UI-NV-7 (ui-contract 1.4).
//! Every branch cites the clause that requires it.

use crate::doc::*;
use crate::ids::{child_id, draft_key, row_key};
use crate::input::*;
use crate::node::*;
use crate::value::{compare, display_text, eq, map_key, truthy};
use serde_json::{json, Map, Value};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet, HashSet};

/// The copy keys of generic copy (UI-NV-6, UI-CP-2): `realize` emits a key, never a string.
pub const DEFAULT_EMPTY_TITLE: &str = "generic_empty";
pub const DEFAULT_ERROR_TITLE: &str = "generic_error";
/// The copy key of an action disabled by an absent bound operand (UI-RC-5, UI-NV-6).
pub const DEFAULT_NOTHING_SELECTED: &str = "nothing_selected";

/// Realizes `input.document` (UI-RZ-1). The result depends on the input only.
///
/// UI-NV-4 "one pass": the tree and `local_changes` are those of a state in which the changes are already applied.
/// A draft that `realize` unsets can change the options of another field, so the pass repeats until no new draft is
/// unset. Each repeat adds at least one draft key, so the loop ends.
pub fn realize(input: &RealizeInput) -> RealizedTree {
    let mut found: BTreeSet<String> = BTreeSet::new();
    loop {
        let mut r = Realizer::new(input, found.clone());
        let mut tree = r.run();
        let before = found.len();
        found.extend(tree.local_changes.unset_drafts.iter().cloned());
        if found.len() == before {
            // The final pass already treats every found key as unset, so the changes come from `found`.
            tree.local_changes.unset_drafts = found.iter().cloned().collect();
            tree.local_changes.stamped_errors = found
                .iter()
                .map(|k| StampedError {
                    draft_key: k.clone(),
                    code: local_error("option_unavailable"),
                })
                .collect();
            return tree;
        }
    }
}

/// The tree that replaces a view that the client cannot show (UI-CF-1, UI-VR-2): one local diagnostic.
/// The copy key `view_unavailable` carries `{ view_id, reason }` (UI-NV-6); `reason` is the diagnostic code. The code is open
/// (UI-NV-4): this helper accepts any code. The codes that this crate chooses go through [`diagnostic_code`].
pub fn diagnostic_tree(view_id: &str, code: &str, detail: &str) -> RealizedTree {
    let root = UiNode::new(child_id(view_id, "container"), UiNodeKind::EmptyState).prop(
        "title",
        copy_key_args(
            "view_unavailable",
            &[("view_id", view_id), ("reason", code)],
        ),
    );
    RealizedTree {
        root,
        overlays: vec![],
        diagnostics: vec![Diagnostic {
            node_id: None,
            code: code.to_string(),
            detail: Some(detail.to_string()),
        }],
        local_changes: LocalChanges::default(),
    }
}

#[derive(Clone)]
struct RefState {
    row: Option<Value>,
    reason: Option<&'static str>,
    decl: Ref,
}

#[derive(Clone, Default)]
struct Env<'e> {
    /// The row that `{ path }` reads.
    path_row: Option<&'e Value>,
    /// The row that `{ row }` reads (UI-RC-3).
    row: Option<&'e Value>,
    refs: Option<&'e BTreeMap<String, RefState>>,
    form_node: Option<&'e str>,
    form_schema: Option<&'e Value>,
}

struct ActionSite<'a> {
    action: &'a Action,
    part_id: &'a str,
    /// The kind of the row part that holds the action, for a row action.
    row_kind: Option<&'a str>,
    /// The refs of the row part (UI-RF-3): a row form resolves them for its row.
    refs: Option<&'a BTreeMap<String, Ref>>,
}

/// A button that is in the tree (UI-NV-4: an overlay entry whose button is not in the tree is kept and not shown).
#[derive(Clone)]
struct ButtonSite<'a> {
    action: &'a Action,
    row_kind: Option<&'a str>,
    refs: Option<&'a BTreeMap<String, Ref>>,
    row_key: Option<String>,
}

struct Realizer<'a> {
    input: &'a RealizeInput,
    actions: BTreeMap<&'a str, ActionSite<'a>>,
    slot_kind: BTreeMap<&'a str, &'a str>,
    selections: BTreeMap<String, String>,
    /// Draft keys that earlier passes unset, with the error `option_unavailable` (UI-NV-4 "as if already applied").
    found: BTreeSet<String>,
    /// The buttons of the tree, by node id.
    buttons: BTreeMap<String, ButtonSite<'a>>,
    diags: Vec<Diagnostic>,
    changes: LocalChanges,
}

impl<'a> Realizer<'a> {
    fn new(input: &'a RealizeInput, found: BTreeSet<String>) -> Self {
        Realizer {
            input,
            actions: BTreeMap::new(),
            slot_kind: BTreeMap::new(),
            selections: input.local_state.selections.clone(),
            found,
            buttons: BTreeMap::new(),
            diags: vec![],
            changes: LocalChanges::default(),
        }
    }

    fn diag(&mut self, code: &str, node: Option<&str>, detail: &str) {
        self.diags.push(Diagnostic::new(node, code, detail));
    }

    /// The draft of `key` as `realize` sees it (UI-NV-5): `None` means no draft. A draft that is unset, here or in an
    /// earlier pass, shows no value and hides the default.
    fn draft(&self, key: &str) -> DraftView {
        if self.found.contains(key) {
            return DraftView::Unset;
        }
        match self.input.local_state.drafts.get(key) {
            Some(Draft::Value(v)) => DraftView::Value(v.clone()),
            Some(Draft::Unset(true)) => DraftView::Unset,
            // `{ clear: true }` is a secret rule (UI-FM-8); it is no value here.
            _ => DraftView::None,
        }
    }

    fn run(&mut self) -> RealizedTree {
        let doc = &self.input.document;
        let caps = &self.input.capabilities;
        // UI-CF-1: an unsupported version or feature is a local diagnostic in place of the view.
        let major = doc.ui_contract_view.split('.').next().unwrap_or("");
        let version_ok = caps
            .ui_contract_view
            .iter()
            .any(|v| v == &doc.ui_contract_view || v.split('.').next() == Some(major));
        if !version_ok {
            return diagnostic_tree(
                &doc.id,
                diagnostic_code("unsupported_view_version"),
                "ui_contract_view",
            );
        }
        if let Some(missing) = doc.requires.iter().find(|f| !caps.has_feature(f)) {
            return diagnostic_tree(
                &doc.id,
                diagnostic_code("unsupported_view_feature"),
                missing,
            );
        }
        self.index(&doc.root);
        self.clear_selections();
        let root = self
            .part(&doc.root)
            .unwrap_or_else(|| UiNode::new(child_id(&doc.id, "container"), UiNodeKind::Stack));
        let overlays = self.overlays();
        let mut tree = RealizedTree {
            root,
            overlays,
            diagnostics: vec![],
            local_changes: LocalChanges::default(),
        };
        // UI-RZ-5: a duplicate node id is a diagnostic and never a crash.
        let mut seen = HashSet::new();
        let mut dups = vec![];
        for id in tree.identity_ids() {
            if !seen.insert(id.clone()) {
                dups.push(id);
            }
        }
        for id in dups {
            self.diag("duplicate_node_id", Some(&id), "duplicate node id");
        }
        // UI-NV-4 `dropped_view_state`: a `focus` or `scroll` entry whose node left the tree.
        let ids: HashSet<String> = tree.identity_ids().into_iter().collect();
        let ls = &self.input.local_state;
        if let Some(f) = ls.focus.as_ref().filter(|f| !ids.contains(*f)) {
            self.changes.dropped_view_state.push(f.clone());
        }
        for node in ls.scroll.keys().filter(|k| !ids.contains(*k)) {
            if !self.changes.dropped_view_state.contains(node) {
                self.changes.dropped_view_state.push(node.clone());
            }
        }
        tree.diagnostics = std::mem::take(&mut self.diags);
        tree.local_changes = std::mem::take(&mut self.changes);
        tree
    }

    // ---- indexing -------------------------------------------------------------------------------------------

    fn index(&mut self, part: &'a Part) {
        match part {
            Part::List(p) => {
                if let Some(slot) = &p.selects {
                    self.slot_kind.insert(slot, &p.source.kind);
                }
                for a in &p.item.actions {
                    self.actions.insert(
                        &a.id,
                        ActionSite {
                            action: a,
                            part_id: &p.id,
                            row_kind: Some(&p.source.kind),
                            refs: Some(&p.refs),
                        },
                    );
                }
            }
            Part::Table(p) => {
                if let Some(slot) = &p.selects {
                    self.slot_kind.insert(slot, &p.source.kind);
                }
            }
            Part::Actions(p) => self.index_actions(&p.id, &p.items),
            Part::Toolbar(p) => self.index_actions(&p.id, &p.items),
            Part::Stack(p) => p.children.iter().for_each(|c| self.index(c)),
            Part::Inline(p) => p.children.iter().for_each(|c| self.index(c)),
            Part::Panel(p) => p.children.iter().for_each(|c| self.index(c)),
            Part::Section(p) => p.children.iter().for_each(|c| self.index(c)),
            _ => {}
        }
    }

    fn index_actions(&mut self, part_id: &'a str, items: &'a [Action]) {
        for a in items {
            self.actions.insert(
                &a.id,
                ActionSite {
                    action: a,
                    part_id,
                    row_kind: None,
                    refs: None,
                },
            );
        }
    }

    /// UI-SL-2, UI-NV-4: a slot clears when its row has left the kind, or when the document no longer declares it.
    /// A slot with no baseline stays (pending, UI-SL-5).
    fn clear_selections(&mut self) {
        let slots: Vec<(String, String)> = self
            .selections
            .iter()
            .map(|(s, k)| (s.clone(), k.clone()))
            .collect();
        for (slot, key) in slots {
            if !self.input.document.selections.iter().any(|d| d.id == slot) {
                self.selections.remove(&slot);
                self.changes.cleared_selections.push(slot);
                continue;
            }
            let Some(kind) = self.slot_kind.get(slot.as_str()).copied() else {
                continue;
            };
            let Some(snap) = self.input.kind_snapshots.get(kind) else {
                continue;
            };
            if snap.baseline
                && !snap
                    .rows
                    .iter()
                    .any(|r| row_key(r).as_deref() == Some(&key))
            {
                self.selections.remove(&slot);
                self.changes.cleared_selections.push(slot);
            }
        }
    }

    // ---- operands and predicates ----------------------------------------------------------------------------

    fn selected_row(&self, slot: &str) -> Option<&'a Value> {
        let key = self.selections.get(slot)?;
        let kind = self.slot_kind.get(slot)?;
        self.input
            .kind_snapshots
            .get(*kind)?
            .rows
            .iter()
            .find(|r| row_key(r).as_deref() == Some(key))
    }

    fn eval(&self, op: &Operand, env: &Env<'_>) -> Option<Value> {
        match op {
            Operand::Path(p) => env.path_row?.pointer(p).cloned(),
            Operand::Row(p) => env.row?.pointer(p).cloned(),
            Operand::Form(p) => {
                let node = env.form_node?;
                let name = p.trim_start_matches('/');
                match self.draft(&draft_key(node, name)) {
                    DraftView::Value(v) => return Some(v),
                    // UI-NV-5: an unset draft shows no value and hides the default.
                    DraftView::Unset => return None,
                    DraftView::None => {}
                }
                env.form_schema?
                    .get("properties")?
                    .get(name)?
                    .get("default")
                    .cloned()
            }
            Operand::Sel(s) => {
                let (slot, ptr) = split_slot(s);
                let row = self.selected_row(slot)?;
                if ptr.is_empty() {
                    Some(row.clone())
                } else {
                    row.pointer(ptr).cloned()
                }
            }
            Operand::Ref(s) => {
                let (id, ptr) = split_slot(s);
                let row = env.refs?.get(id)?.row.as_ref()?;
                if ptr.is_empty() {
                    Some(row.clone())
                } else {
                    row.pointer(ptr).cloned()
                }
            }
            Operand::Viewport(axis) => {
                Some(Value::String(self.input.viewport.class(*axis).to_string()))
            }
            Operand::Lit(v) => Some(v.clone()),
        }
    }

    fn pred(&self, p: &Pred, env: &Env<'_>) -> bool {
        match p {
            Pred::Eq((a, b)) => eq(self.eval(a, env).as_ref(), self.eval(b, env).as_ref()),
            Pred::Ne((a, b)) => !eq(self.eval(a, env).as_ref(), self.eval(b, env).as_ref()),
            Pred::Truthy(a) => truthy(self.eval(a, env).as_ref()),
            Pred::Not(q) => !self.pred(q, env),
            Pred::All(l) => l.iter().all(|q| self.pred(q, env)),
            Pred::Any(l) => l.iter().any(|q| self.pred(q, env)),
        }
    }

    fn tone(&mut self, t: &Option<OrUnknown<Tone>>) -> Option<Value> {
        match t {
            None => None,
            Some(OrUnknown::Known(t)) => Some(json!(t.as_str())),
            Some(OrUnknown::Unknown(name)) => {
                // UI-VR-2: an unknown tone is ignored and counted.
                self.diag("unknown_tone", None, name);
                None
            }
        }
    }

    // ---- refs -----------------------------------------------------------------------------------------------

    fn resolve_refs(
        &self,
        refs: &BTreeMap<String, Ref>,
        row: &Value,
    ) -> BTreeMap<String, RefState> {
        refs.iter()
            .map(|(id, decl)| {
                let (row, reason) = self.resolve_ref(decl, row);
                (
                    id.clone(),
                    RefState {
                        row,
                        reason,
                        decl: decl.clone(),
                    },
                )
            })
            .collect()
    }

    /// UI-RF-1, UI-RF-2.
    fn resolve_ref(&self, decl: &Ref, row: &Value) -> (Option<Value>, Option<&'static str>) {
        let Some(snap) = self.input.kind_snapshots.get(&decl.kind) else {
            return (None, absent("no_baseline"));
        };
        if !snap.baseline {
            return (None, absent("no_baseline"));
        }
        let key_value = match &decl.key {
            RefKey::Id(p) => row.pointer(p).cloned(),
            RefKey::Structured { hub_id, id } => {
                let hub = row.pointer(hub_id).and_then(Value::as_str);
                if let (Some(mine), Some(theirs)) = (snap.hub_id.as_deref(), hub) {
                    if mine != theirs {
                        return (None, absent("remote_hub_unsupported"));
                    }
                }
                row.pointer(id).cloned()
            }
        };
        let key = match key_value {
            Some(Value::String(s)) => s,
            Some(Value::Null) | None => return (None, absent("not_found")),
            Some(other) => other.to_string(),
        };
        match snap
            .rows
            .iter()
            .find(|r| row_key(r).as_deref() == Some(&key))
        {
            Some(r) => (Some(r.clone()), None),
            None => (None, absent("not_found")),
        }
    }

    // ---- parts ----------------------------------------------------------------------------------------------

    fn part(&mut self, part: &'a Part) -> Option<UiNode> {
        // UI-PT-3: `when` on a structural part reads the viewport only.
        if let Some(w) = part.when() {
            if !self.pred(w, &Env::default()) {
                return None;
            }
        }
        let caps = &self.input.capabilities;
        match part {
            Part::Stack(p) => {
                let children = self.children(&p.children);
                Some(
                    UiNode::new(&p.id, UiNodeKind::Stack)
                        .prop("direction", json!(dir_text(p.direction)))
                        .with_children(children),
                )
            }
            Part::Inline(p) => {
                let children = self.children(&p.children);
                Some(UiNode::new(&p.id, UiNodeKind::Inline).with_children(children))
            }
            Part::Panel(p) => {
                let children = self.children(&p.children);
                if caps.has_part("panel") {
                    Some(
                        UiNode::new(&p.id, UiNodeKind::Panel)
                            .prop("title", json!(p.title))
                            .with_children(children),
                    )
                } else {
                    // UI-CF-2: a heading and indented children.
                    Some(self.heading_fallback(&p.id, &p.title, children))
                }
            }
            Part::Section(p) => {
                let children = self.children(&p.children);
                if caps.has_part("section") {
                    Some(
                        UiNode::new(&p.id, UiNodeKind::Section)
                            .prop("title", json!(p.heading))
                            .with_children(children),
                    )
                } else {
                    Some(self.heading_fallback(&p.id, &p.heading, children))
                }
            }
            Part::Text(p) => {
                let value = match &p.value {
                    OperandOrLit::Operand(op) => self.eval(op, &Env::default()),
                    OperandOrLit::Literal(v) => Some(v.clone()),
                };
                let tone = self.tone(&p.tone);
                Some(
                    UiNode::new(&p.id, UiNodeKind::Text)
                        .prop("text", json!(display_text(value.as_ref())))
                        .prop("tone", tone),
                )
            }
            Part::EmptyState(p) => Some(
                UiNode::new(&p.id, UiNodeKind::EmptyState)
                    .prop("title", json!(p.title))
                    .prop("description", p.body.as_ref().map(|b| json!(b))),
            ),
            Part::StatusBadge(p) => Some(self.status_badge(p)),
            Part::Toolbar(p) => {
                Some(self.toolbar(&child_id(&p.id, "container"), &p.items, p.overflow, None))
            }
            Part::Actions(p) => {
                Some(self.toolbar(&child_id(&p.id, "container"), &p.items, p.overflow, None))
            }
            Part::List(p) => Some(self.list(p)),
            Part::Table(p) => Some(self.table(p)),
            Part::Detail(p) => Some(self.detail(p)),
            Part::Form(p) => Some(self.form_part(p)),
            Part::Embed(p) => Some(self.embed(p)),
        }
    }

    fn children(&mut self, parts: &'a [Part]) -> Vec<UiNode> {
        parts.iter().filter_map(|c| self.part(c)).collect()
    }

    fn heading_fallback(&mut self, id: &str, title: &str, children: Vec<UiNode>) -> UiNode {
        UiNode::new(id, UiNodeKind::Stack).with_children(vec![
            UiNode::new(child_id(id, "label"), UiNodeKind::Text)
                .prop("text", json!(title))
                .prop("variant", json!("emphasized")),
            UiNode::new(child_id(id, "container"), UiNodeKind::Stack)
                .prop("indent", json!(true))
                .with_children(children),
        ])
    }

    fn status_badge(&mut self, p: &'a StatusBadgePart) -> UiNode {
        let value = self.eval(&p.value, &Env::default());
        let (label, tone) = map_lookup(&p.map, value.as_ref());
        let tone = self.tone(&tone).unwrap_or(json!("neutral"));
        if self.input.capabilities.has_part("status_badge") {
            UiNode::new(&p.id, UiNodeKind::StatusBadge)
                .prop("label", json!(label))
                .prop("tone", tone)
        } else {
            // UI-CF-2: the label text plus the tone word in brackets.
            UiNode::new(&p.id, UiNodeKind::Text).prop(
                "text",
                json!(format!("{label} [{}]", tone.as_str().unwrap_or("neutral"))),
            )
        }
    }

    fn embed(&mut self, p: &'a EmbedPart) -> UiNode {
        // UI-RZ-2f: the node carries the asset reference; the renderer gets the URL.
        let asset = match &p.page.asset {
            OperandOrLit::Operand(op) => self.eval(op, &Env::default()),
            OperandOrLit::Literal(v) => Some(v.clone()),
        };
        let valid = asset
            .as_ref()
            .is_some_and(|a| serde_json::from_value::<AssetRef>(a.clone()).is_ok());
        // UI-EM-6: a renderer that does not embed shows the title and a link; the link has no channel.
        let embeds = self.input.capabilities.has_feature(FEATURE_EMBED);
        let channel = p.channel.as_ref().filter(|_| embeds).map(|c| {
            json!({
                "from_page": c.from_page.iter().map(|m| m.type_.clone()).collect::<Vec<_>>(),
                "to_page": c.to_page.iter().map(|m| m.type_.clone()).collect::<Vec<_>>(),
            })
        });
        let mut node = UiNode::new(&p.id, UiNodeKind::Embed)
            .prop("title", json!(p.title))
            .prop("state", json!(if valid { "ready" } else { "empty" }))
            .prop("channel", channel);
        if valid {
            // UI-RZ-2f: the node carries `asset_id`; a reference by name carries `name` (UI-EM-1).
            let (id, name) = match asset
                .as_ref()
                .map(|a| (a.get("asset_id").cloned(), a.get("name").cloned()))
            {
                Some((i, n)) => (i, n),
                None => (None, None),
            };
            node = node.prop("asset_id", id).prop("name", name);
        } else {
            node.children.push(copy_node(
                child_id(&p.id, "empty"),
                Some(&p.empty),
                DEFAULT_EMPTY_TITLE,
            ));
        }
        node
    }

    // ---- toolbars and buttons -------------------------------------------------------------------------------

    /// UI-PT-4, UI-AC-2. `row` is `Some` for the actions of a row part.
    fn toolbar(
        &mut self,
        id: &str,
        items: &'a [Action],
        overflow: Overflow,
        row_ctx: Option<(&str, &Env<'_>)>,
    ) -> UiNode {
        let default_env = Env::default();
        let (base, env) = match row_ctx {
            Some((row_node, env)) => (Some(row_node), env),
            None => (None, &default_env),
        };
        let mut clusters: Vec<(bool, Cluster)> = vec![];
        for a in items {
            let node_id = match base {
                Some(row_node) => child_id(row_node, &a.id),
                None => a.id.clone(),
            };
            let row_key = env.row.and_then(row_key);
            if let Some(c) = self.button(a, env, node_id, row_key.as_deref()) {
                clusters.push((a.priority == Some(Priority::Primary), c));
            }
        }
        // UI-PT-4: priority first, then declaration order (the sort is stable).
        clusters.sort_by_key(|(primary, _)| !*primary);
        let has_primary = clusters.iter().any(|(p, _)| *p);
        let has_lower = clusters.iter().any(|(p, _)| !*p);
        let mut children = vec![];
        let mut overflowed = vec![];
        let short = self.input.viewport.class(ViewportAxis::Width) == "compact";
        for (primary, c) in clusters {
            // UI-NV-7: `overflow: auto` never moves an item of the highest priority present, so with one priority
            // level nothing moves.
            if overflow == Overflow::Auto && short && has_primary && has_lower && !primary {
                overflowed.push(c);
            } else {
                children.push(c.button);
                children.extend(c.extras);
            }
        }
        if !overflowed.is_empty() {
            // UI-RZ-2: the overflow is a `menu` of menu items.
            let mut items = vec![];
            let mut after = vec![];
            for c in overflowed {
                items.push(menu_item(c.button));
                after.extend(c.extras);
            }
            children.push(UiNode::new(child_id(id, "item"), UiNodeKind::Menu).slot("items", items));
            // UI-NV-3: a node that follows a button in a menu follows the menu instead.
            children.extend(after);
        }
        UiNode::new(id, UiNodeKind::Toolbar).with_children(children)
    }

    /// The `descriptor` of an action in the `kind` rows: `actions.<handler>` (UI-AC-5, steward ruling R-2.6; UI-SA).
    /// The row of the action's own kind wins.
    fn action_entry(&self, a: &Action) -> Option<&'a Value> {
        let snap = self.input.kind_snapshots.get("kind")?;
        let row_kind = self.actions.get(a.id.as_str()).and_then(|s| s.row_kind);
        let mut found = None;
        for row in &snap.rows {
            let Some(entry) = row.pointer(&format!("/actions/{}", a.handler)) else {
                continue;
            };
            if row_kind.is_some() && row.get("kind").and_then(Value::as_str) == row_kind {
                return Some(entry);
            }
            found.get_or_insert(entry);
        }
        found
    }

    /// One action as a `button`, with the nodes that sit beside it (UI-RC-1, UI-RC-5, UI-AC-3, UI-AC-5, UI-SA-1 to UI-SA-5).
    fn button(
        &mut self,
        a: &'a Action,
        env: &Env<'_>,
        node_id: String,
        row_key: Option<&str>,
    ) -> Option<Cluster> {
        // UI-AC-5: a false `when` hides the action.
        if let Some(w) = &a.when {
            if !self.pred(w, env) {
                return None;
            }
        }
        let entry = self.action_entry(a);
        // UI-SA-1 to UI-SA-3: the pointers of a contributed action read the row's `metadata[<owner>]`.
        let pointed = |name: &str| -> Option<Value> {
            let ptr = entry?.get(name)?.as_str()?;
            let owner = entry?.get("owner")?.as_str()?;
            env.row?.get("metadata")?.get(owner)?.pointer(ptr).cloned()
        };
        // UI-SA-1: a non-empty string is the label. Anything else shows nothing and keeps the declared label.
        let label = pointed("label_pointer")
            .and_then(|v| v.as_str().map(str::to_string))
            .filter(|l| !l.is_empty())
            .unwrap_or_else(|| a.label.clone());
        let mut node = UiNode::new(&node_id, UiNodeKind::Button)
            .prop("label", json!(label))
            .prop("action_ref", json!(a.id))
            .prop(
                "confirm",
                a.confirm
                    .as_ref()
                    .map(|c| json!({"message": c.message, "tone": c.tone.as_str()})),
            )
            // UI-AC-2: priority changes emphasis only.
            .prop(
                "variant",
                (a.priority == Some(Priority::Primary)).then(|| json!("emphasized")),
            );
        // UI-RC-2: the client resolves the operands and sends the values as ordinary arguments.
        let resolved: Map<String, Value> = a
            .args
            .iter()
            .filter(|(_, op)| !matches!(op, Operand::Form(_)))
            .filter_map(|(k, op)| self.eval(op, env).map(|v| (k.clone(), v)))
            .collect();
        // UI-RC-1: a row action targets the row; UI-RC-2: the resolved arguments travel with the action.
        let mut payload = Map::new();
        if let Some(k) = row_key {
            payload.insert("target".into(), json!(k));
        }
        if !resolved.is_empty() {
            payload.insert("args".into(), Value::Object(resolved));
        }
        node = node.prop(
            "action",
            Some(if payload.is_empty() {
                json!({"id": a.id})
            } else {
                json!({"id": a.id, "payload": payload})
            }),
        );
        let denied = entry
            .and_then(|e| e.get("allowed")?.as_bool().map(|ok| (ok, e)))
            .filter(|(ok, _)| !ok);
        if let Some((_, e)) = denied {
            // UI-AC-5: the Hub's `allowed` and `reasons` disable the action.
            let reasons: Vec<&str> = e
                .get("reasons")
                .and_then(Value::as_array)
                .map(|l| l.iter().filter_map(Value::as_str).collect())
                .unwrap_or_default();
            node = node
                .prop("disabled", json!(true))
                .prop("disabled_reason", json!(reasons.join("; ")));
        } else if a
            .args
            .values()
            .any(|op| !matches!(op, Operand::Form(_)) && self.eval(op, env).is_none())
        {
            // UI-RC-5, UI-AC-8: an absent bound operand disables the action. The reason is the declared text, or the key.
            let reason = a
                .disabled_reason
                .as_ref()
                .map(|r| json!(r))
                .unwrap_or_else(|| copy_key(DEFAULT_NOTHING_SELECTED));
            node = node
                .prop("disabled", json!(true))
                .prop("disabled_reason", reason);
        }
        self.buttons.insert(
            node_id.clone(),
            ButtonSite {
                action: a,
                row_kind: self.actions.get(a.id.as_str()).and_then(|s| s.row_kind),
                refs: self.actions.get(a.id.as_str()).and_then(|s| s.refs),
                row_key: row_key.map(str::to_string),
            },
        );
        // The nodes beside the button, in order: its error (UI-NV-3), its warning (UI-SA-5), its status (UI-SA-2)
        // and its link (UI-SA-3).
        let mut extras = vec![];
        if let Some(msg) = self.input.local_state.errors.actions.get(&node_id) {
            extras.push(error_text(child_id(&node_id, "error"), msg));
        }
        let warning = a
            .warning
            .as_deref()
            .or_else(|| entry.and_then(|e| e.get("warning")?.as_str()));
        if let Some(w) = warning {
            extras.push(warning_text(child_id(&node_id, "warning"), w));
        }
        if let Some(st) = pointed("status_pointer").and_then(|v| v.as_str().map(str::to_string)) {
            extras.push(
                UiNode::new(child_id(&node_id, "status"), UiNodeKind::Text)
                    .prop("text", json!(st))
                    .prop("tone", json!("neutral")),
            );
        }
        // UI-SA-1: a pointer that resolves to a non-string shows nothing.
        if let Some(u) = pointed("url_pointer").filter(Value::is_string) {
            let link = self.link_node(child_id(&node_id, "link"), &u);
            extras.push(link);
        }
        Some(Cluster {
            button: node,
            extras,
        })
    }

    /// A link value as a `text` node (UI-FD-1, UI-CF-5(a), UI-SA-3, UI-RZ-2e).
    fn link_node(&self, id: String, value: &Value) -> UiNode {
        let external_open = self.input.capabilities.has_feature(FEATURE_EXTERNAL_OPEN);
        match value.as_str().and_then(|t| valid_link(t).map(|l| (t, l))) {
            Some((t, l)) if external_open => UiNode::new(id, UiNodeKind::Text)
                .prop("text", json!(l.host))
                .prop("href", json!(t)),
            Some((_, l)) => UiNode::new(id, UiNodeKind::Text).prop("text", json!(l.full)),
            None => {
                UiNode::new(id, UiNodeKind::Text).prop("text", json!(display_text(Some(value))))
            }
        }
    }

    // ---- data parts: rows -----------------------------------------------------------------------------------

    /// The rows of a source after `where` and order (UI-RZ-4). `None` means there is no baseline yet.
    #[allow(clippy::type_complexity)]
    fn source_rows(
        &mut self,
        source: &'a Source,
        part_order: &'a [OrderKey],
        refs: &'a BTreeMap<String, Ref>,
    ) -> Result<Vec<(String, &'a Value, BTreeMap<String, RefState>)>, SourceState> {
        let Some(snap) = self.input.kind_snapshots.get(&source.kind) else {
            return Err(SourceState::Loading);
        };
        if let Some(e) = &snap.error {
            return Err(SourceState::Error(e.message.clone()));
        }
        if !snap.baseline {
            return Err(SourceState::Loading);
        }
        let mut rows = vec![];
        for row in &snap.rows {
            let Some(key) = row_key(row) else {
                self.diag("row_without_id", None, &source.kind);
                continue;
            };
            let states = self.resolve_refs(refs, row);
            if let Some(w) = &source.where_ {
                let env = Env {
                    path_row: Some(row),
                    row: Some(row),
                    refs: Some(&states),
                    ..Env::default()
                };
                if !self.pred(w, &env) {
                    continue;
                }
            }
            rows.push((key, row, states));
        }
        let order = if part_order.is_empty() {
            &source.order
        } else {
            part_order
        };
        rows.sort_by(|(ka, ra, _), (kb, rb, _)| {
            for k in order {
                let mut c = compare(ra.pointer(&k.field), rb.pointer(&k.field));
                if k.dir == Dir::Desc {
                    c = c.reverse();
                }
                if c != Ordering::Equal {
                    return c;
                }
            }
            ka.as_bytes().cmp(kb.as_bytes())
        });
        Ok(rows)
    }

    fn state_node(
        &mut self,
        container: &str,
        part: &str,
        state: SourceState,
        empty: &Option<CopyText>,
        error: &Option<CopyText>,
    ) -> StateParts {
        let _ = part;
        match state {
            // UI-CP-1: a neutral loading indicator before the baseline.
            SourceState::Loading => StateParts::new("loading", None, None),
            // UI-CP-1, UI-NV-1: the Hub's message is the `detail` prop of the node that shows the error.
            SourceState::Error(msg) => StateParts::new(
                "error",
                Some(copy_node(
                    child_id(container, "error"),
                    error.as_ref(),
                    DEFAULT_ERROR_TITLE,
                )),
                msg,
            ),
            SourceState::Empty => StateParts::new(
                "empty",
                Some(copy_node(
                    child_id(container, "empty"),
                    empty.as_ref(),
                    DEFAULT_EMPTY_TITLE,
                )),
                None,
            ),
        }
    }

    fn list(&mut self, p: &'a ListPart) -> UiNode {
        let container = child_id(&p.id, "container");
        let mut node = UiNode::new(&container, UiNodeKind::List)
            .prop("selects", p.selects.as_ref().map(|s| json!(s)));
        let rows = match self.source_rows(&p.source, &p.order, &p.refs) {
            Err(s) => {
                let st = self.state_node(&container, &p.id, s, &p.empty, &p.error);
                node = st.apply(node);
                node.children.extend(st.child);
                return node;
            }
            Ok(rows) => rows,
        };
        if rows.is_empty() {
            let st = self.state_node(&container, &p.id, SourceState::Empty, &p.empty, &p.error);
            node = st.apply(node);
            node.children.extend(st.child);
            return node;
        }
        for (key, row, states) in rows {
            let row_node = child_id(&p.id, &key);
            let env = Env {
                path_row: Some(row),
                row: Some(row),
                refs: Some(&states),
                ..Env::default()
            };
            let mut item = self.row_node(
                UiNodeKind::ListItem,
                &row_node,
                &key,
                p.selects.as_deref(),
                p.activate.as_ref(),
                &env,
                &p.id,
            );
            for f in &p.item.fields {
                let fl = FieldLike::from_field(f);
                let fnode = self.field(&fl, &env, child_id(&row_node, &f.id), &p.id);
                item.children.push(fnode);
            }
            if !p.item.actions.is_empty() {
                let tb = self.toolbar(
                    &child_id(&row_node, "action"),
                    &p.item.actions,
                    Overflow::Never,
                    Some((&row_node, &env)),
                );
                item = item.slot("actions", vec![tb]);
            }
            node.children.push(item);
        }
        node
    }

    #[allow(clippy::too_many_arguments)]
    fn row_node(
        &mut self,
        kind: UiNodeKind,
        row_node: &str,
        key: &str,
        selects: Option<&str>,
        activate: Option<&'a Activate>,
        env: &Env<'_>,
        part: &str,
    ) -> UiNode {
        let selected =
            selects.is_some_and(|s| self.selections.get(s).map(String::as_str) == Some(key));
        let act = activate.and_then(|a| self.activate_prop(a, env, Some(key), part));
        UiNode::new(row_node, kind)
            .prop("value", json!(key))
            .prop("selects", selects.map(|s| json!(s)))
            .prop("selected", selected.then(|| json!(true)))
            .prop("activate", act)
    }

    /// A `table` is one node. Its rows are entries of the `rows` prop, each with its row node id (UI-ID-2, steward ruling R-2.3).
    fn table(&mut self, p: &'a TablePart) -> UiNode {
        let grid = self.input.capabilities.has_part("table");
        // UI-CF-2: without a column grid, a list of `label: value` lines.
        let mut node = UiNode::new(
            &p.id,
            if grid {
                UiNodeKind::Table
            } else {
                UiNodeKind::List
            },
        )
        .prop("selects", p.selects.as_ref().map(|s| json!(s)));
        if grid {
            let cols: Vec<Value> = p
                .columns
                .iter()
                .map(|c| {
                    let mut m = Map::new();
                    m.insert("id".into(), json!(c.id));
                    m.insert("label".into(), json!(c.label));
                    if let Some(a) = c.align {
                        m.insert(
                            "align".into(),
                            json!(match a {
                                Align::Start => "start",
                                Align::Center => "center",
                                Align::End => "end",
                            }),
                        );
                    }
                    Value::Object(m)
                })
                .collect();
            node = node.prop("columns", json!(cols));
        }
        let rows = match self.source_rows(&p.source, &p.order, &p.refs) {
            Err(s) => {
                let st = self.state_node(&p.id, &p.id, s, &p.empty, &p.error);
                return table_state(node, st, grid);
            }
            Ok(rows) => rows,
        };
        if rows.is_empty() {
            let st = self.state_node(&p.id, &p.id, SourceState::Empty, &p.empty, &p.error);
            return table_state(node, st, grid);
        }
        let mut entries: Vec<Value> = vec![];
        for (key, row, states) in rows {
            let row_node = child_id(&p.id, &key);
            let env = Env {
                path_row: Some(row),
                row: Some(row),
                refs: Some(&states),
                ..Env::default()
            };
            let selected = p
                .selects
                .as_deref()
                .is_some_and(|s| self.selections.get(s).map(String::as_str) == Some(key.as_str()));
            let activate = p
                .activate
                .as_ref()
                .and_then(|a| self.activate_prop(a, &env, Some(&key), &p.id));
            let mut cells = Map::new();
            let mut lines: Vec<UiNode> = vec![];
            for c in &p.columns {
                let fl = FieldLike::from_column(c);
                let cell = child_id(&child_id(&row_node, &c.id), "cell");
                let cn = self.field(&fl, &env, cell, &p.id);
                if grid {
                    cells.insert(c.id.clone(), cell_value(&cn));
                } else {
                    let text = display_text(cn.props.get("text").or_else(|| cn.props.get("label")));
                    lines.push(
                        UiNode::new(cn.id, UiNodeKind::Text)
                            .prop("text", json!(format!("{}: {text}", c.label))),
                    );
                }
            }
            if grid {
                let mut entry = Map::new();
                entry.insert("id".into(), json!(row_node));
                entry.insert("cells".into(), Value::Object(cells));
                if selected {
                    entry.insert("selected".into(), json!(true));
                }
                if let Some(a) = activate {
                    entry.insert("activate".into(), a);
                }
                entries.push(Value::Object(entry));
            } else {
                let item = self
                    .row_node(
                        UiNodeKind::ListItem,
                        &row_node,
                        &key,
                        p.selects.as_deref(),
                        p.activate.as_ref(),
                        &env,
                        &p.id,
                    )
                    .with_children(lines);
                node.children.push(item);
            }
        }
        if grid {
            node = node.prop("rows", json!(entries));
        }
        node
    }

    fn detail(&mut self, p: &'a DetailPart) -> UiNode {
        let container = child_id(&p.id, "container");
        let labeled_lines = !self.input.capabilities.has_part("detail");
        let kind = if labeled_lines {
            UiNodeKind::Stack
        } else {
            UiNodeKind::Panel
        };
        let node = UiNode::new(&container, kind);
        // The row: by id of a kind, or the selected row of a slot (UI-SL-2, UI-PT-5).
        let (kind_name, key): (&str, Option<String>) = match &p.source {
            DetailSource::ByKind { kind, id } => {
                let v = match id {
                    OperandOrLit::Operand(op) => self.eval(op, &Env::default()),
                    OperandOrLit::Literal(v) => Some(v.clone()),
                };
                (
                    kind,
                    v.map(|v| match v {
                        Value::String(s) => s,
                        o => o.to_string(),
                    }),
                )
            }
            DetailSource::BySel { sel } => match self.slot_kind.get(sel.as_str()) {
                Some(k) => (k, self.selections.get(sel).cloned()),
                None => ("", None),
            },
        };
        let snap = self.input.kind_snapshots.get(kind_name);
        let Some(key) = key else {
            let st = self.state_node(&container, &p.id, SourceState::Empty, &p.empty, &p.error);
            return detail_state(node, st);
        };
        let state = match snap {
            None => Some(SourceState::Loading),
            Some(s) if s.error.is_some() => Some(SourceState::Error(
                s.error.as_ref().and_then(|e| e.message.clone()),
            )),
            Some(s) if !s.baseline => Some(SourceState::Loading),
            _ => None,
        };
        if let Some(st) = state {
            let st = self.state_node(&container, &p.id, st, &p.empty, &p.error);
            return detail_state(node, st);
        }
        let row = snap.and_then(|s| s.rows.iter().find(|r| row_key(r).as_deref() == Some(&key)));
        let Some(row) = row else {
            let st = self.state_node(&container, &p.id, SourceState::Empty, &p.empty, &p.error);
            return detail_state(node, st);
        };
        let states = self.resolve_refs(&p.refs, row);
        let env = Env {
            path_row: Some(row),
            row: Some(row),
            refs: Some(&states),
            ..Env::default()
        };
        let mut n = node;
        for f in &p.fields {
            let fl = FieldLike::from_field(f);
            let mut fnode = self.field(&fl, &env, f.id.clone(), &p.id);
            if labeled_lines {
                fnode = self.labeled_line(&fl, fnode);
            }
            n.children.push(fnode);
        }
        n
    }

    /// UI-CF-2: `detail` as labeled lines.
    fn labeled_line(&mut self, f: &FieldLike<'_>, node: UiNode) -> UiNode {
        // A node that already carries a label keeps it. A bare value node becomes one `label: value` line.
        if node.kind == UiNodeKind::Inline {
            let text: Vec<String> = node
                .children
                .iter()
                .map(|c| display_text(c.props.get("text")))
                .collect();
            return UiNode::new(node.id, UiNodeKind::Text).prop("text", json!(text.join(": ")));
        }
        match (&f.label, node.props.get("text")) {
            (Some(l), Some(v)) => {
                let t = format!("{l}: {}", display_text(Some(v)));
                UiNode::new(node.id, UiNodeKind::Text).prop("text", json!(t))
            }
            _ => node,
        }
    }

    // ---- fields ---------------------------------------------------------------------------------------------

    /// Realizes a field into a node (UI-RZ-2, UI-RF-2, UI-RF-3, UI-VR-2). `base` is the node id of the field.
    fn field(&mut self, f: &FieldLike<'a>, env: &Env<'_>, base: String, part: &str) -> UiNode {
        let caps_metric = self.input.capabilities.has_feature(FEATURE_DISPLAY_METRIC);
        let operand = f.operand();
        let value = operand.as_ref().and_then(|o| self.eval(o, env));
        // UI-RF-2: an absent ref shows its copy; the field is never an error.
        let absent_ref = match &operand {
            Some(Operand::Ref(s)) => {
                let (id, _) = split_slot(s);
                env.refs
                    .and_then(|m| m.get(id))
                    .filter(|st| st.row.is_none())
                    .cloned()
            }
            _ => None,
        };
        let ref_activate = match &operand {
            Some(Operand::Ref(s)) => {
                let (id, _) = split_slot(s);
                env.refs
                    .and_then(|m| m.get(id))
                    .and_then(|st| st.decl.activate.clone().map(|a| (a, st.clone())))
            }
            _ => None,
        };
        let display = match f.display {
            None => Display::Text,
            Some(OrUnknown::Unknown(_)) => {
                // UI-VR-2: an unknown display is ignored and counted.
                self.diag("unknown_display", Some(&base), "display");
                Display::Text
            }
            Some(OrUnknown::Known(d)) => d,
        };
        let activate = ref_activate.and_then(|(a, st)| self.ref_activate_prop(&a, &st, env, part));
        let (shown, tone_value) = if let Some(st) = &absent_ref {
            let t = self.tone(&st.decl.absent.tone);
            (st.decl.absent.text.clone(), t)
        } else if value.is_none() {
            let t = self.tone(f.tone);
            (f.absent.clone().unwrap_or_default(), t)
        } else {
            let (label, tone) = map_lookup(f.map, value.as_ref());
            let t = self.tone(&tone).or_else(|| self.tone(f.tone));
            (label, t)
        };
        // UI-FD-1: `link` shows a valid http or https URL as its host; anything else is plain text, never activatable.
        let link = (display == Display::Link && absent_ref.is_none())
            .then(|| value.as_ref().and_then(Value::as_str).and_then(valid_link))
            .flatten();
        let external_open = self.input.capabilities.has_feature(FEATURE_EXTERNAL_OPEN);
        let (shown, href) = match (&link, external_open) {
            // UI-FD-1: the host in ASCII form and a non-default port, opened outside the app.
            (Some(l), true) => (
                l.host.clone(),
                value.as_ref().and_then(Value::as_str).map(str::to_string),
            ),
            // UI-CF-5(a): without `external_open`, the full validated URL as text and no control.
            (Some(l), false) => (l.full.clone(), None),
            (None, _) => (shown, None),
        };
        let absent_reason = absent_ref
            .as_ref()
            .and_then(|st| st.reason)
            .map(|r| json!(r));
        let tone_or_neutral = tone_value.clone().unwrap_or(json!("neutral"));
        // The nodes that carry the value.
        let make = |id: String| -> UiNode {
            if absent_ref.is_some() {
                return UiNode::new(id, UiNodeKind::Text)
                    .prop("text", json!(shown))
                    .prop("tone", tone_value.clone())
                    .prop("absent_reason", absent_reason.clone())
                    .prop("activate", activate.clone());
            }
            match display {
                Display::Metric if caps_metric => UiNode::new(id, UiNodeKind::Metric)
                    .prop("label", f.label.as_ref().map(|l| json!(l)))
                    .prop("value", json!(shown))
                    .prop("tone", Some(tone_or_neutral.clone()))
                    .prop("activate", activate.clone()),
                Display::Metric => UiNode::new(id, UiNodeKind::Text).prop(
                    "text",
                    json!(format!("{}: {shown}", f.label.clone().unwrap_or_default())),
                ),
                Display::Badge => UiNode::new(id, UiNodeKind::Badge)
                    .prop("label", json!(shown))
                    .prop("tone", Some(tone_or_neutral.clone()))
                    .prop("activate", activate.clone()),
                _ => UiNode::new(id, UiNodeKind::Text)
                    .prop("text", json!(shown))
                    .prop("tone", tone_value.clone())
                    .prop("href", href.as_ref().map(|h| json!(h)))
                    .prop("activate", activate.clone()),
            }
        };
        // A metric carries its own label. Other displays with a label get a container (UI-RZ-2).
        let own_label = display == Display::Metric && absent_ref.is_none();
        match (&f.label, own_label) {
            (Some(label), false) => UiNode::new(child_id(&base, "container"), UiNodeKind::Inline)
                .with_children(vec![
                    UiNode::new(child_id(&base, "label"), UiNodeKind::Text)
                        .prop("text", json!(label)),
                    make(child_id(&base, "value")),
                ]),
            _ => make(base),
        }
    }

    // ---- activation -----------------------------------------------------------------------------------------

    /// UI-SL-3 and UI-SL-5, UI-SL-7 (realized as props, UI-RZ-2e).
    fn activate_prop(
        &mut self,
        a: &'a Activate,
        env: &Env<'_>,
        key: Option<&str>,
        part: &str,
    ) -> Option<Value> {
        match a {
            Activate::Select(slot) => Some(json!({"select": slot})),
            Activate::Invoke(action) => Some(json!({"invoke": action, "row_key": key})),
            Activate::Open(ref_id) => {
                let st = env.refs.and_then(|m| m.get(ref_id)).cloned()?;
                self.ref_open(&st, part)
            }
            Activate::Navigate(n) => {
                let mut select = Map::new();
                for (slot, op) in &n.select {
                    match self.eval(op, env) {
                        Some(Value::String(s)) => {
                            select.insert(slot.clone(), json!(s));
                        }
                        Some(other) => {
                            select.insert(slot.clone(), json!(other.to_string()));
                        }
                        None => self.diag("navigate_slot_absent", None, slot),
                    }
                }
                Some(json!({"navigate": {"view": n.view, "select": select}}))
            }
            Activate::Url(op) => self.url_activation(op, env, part),
        }
    }

    /// UI-SL-7: the resolved value follows UI-FD-1; a refused value makes the activation inert, with a local diagnostic.
    fn url_activation(&mut self, op: &Operand, env: &Env<'_>, part: &str) -> Option<Value> {
        match self.eval(op, env) {
            Some(Value::String(u)) if valid_link(&u).is_some() => self
                .input
                .capabilities
                .has_feature(FEATURE_EXTERNAL_OPEN)
                // UI-CF-5(a): a renderer that cannot open a URL does not offer the activation.
                .then(|| json!({"url": u})),
            _ => {
                self.diag("url_refused", None, part);
                None
            }
        }
    }

    fn ref_activate_prop(
        &mut self,
        a: &Activate,
        st: &RefState,
        env: &Env<'_>,
        part: &str,
    ) -> Option<Value> {
        match a {
            // UI-SL-7: `{ url }` may be the activation of a `Ref`; the rule is that of UI-FD-1.
            Activate::Url(op) => self.url_activation(op, env, part),
            Activate::Open(_) => self.ref_open(st, part),
            Activate::Select(slot) => st
                .row
                .as_ref()
                .and_then(row_key)
                .map(|k| json!({"select": slot, "row_key": k})),
            Activate::Invoke(action) => st
                .row
                .as_ref()
                .and_then(row_key)
                .map(|k| json!({"invoke": action, "row_key": k})),
            _ => None,
        }
    }

    /// UI-SL-3: `open` shows the client's own screen, or falls back to the row's fields.
    fn ref_open(&mut self, st: &RefState, _part: &str) -> Option<Value> {
        let key = st.row.as_ref().and_then(row_key)?;
        // UI-SL-3, UI-CF-6: the screen is bound to `<kind>@<MAJOR>`. The MAJOR is the `schema_version` of the
        // descriptor of the kind, which the `kind` rows carry (DS-1, ruling R-2.6).
        let major = self
            .input
            .kind_snapshots
            .get("kind")
            .and_then(|s| {
                s.rows
                    .iter()
                    .find(|r| r.get("kind").and_then(Value::as_str) == Some(st.decl.kind.as_str()))
            })
            .and_then(|r| r.get("schema_version")?.as_str())
            .and_then(|v| v.split('.').next());
        if !major.is_some_and(|m| self.input.capabilities.has_screen(&st.decl.kind, m)) {
            // UI-SL-3, UI-CF-2: the client shows the row's fields instead. The activation keeps its target, and the
            // diagnostic tells the renderer that it has no screen.
            self.diag("open_without_screen", None, &st.decl.kind);
        }
        Some(json!({"open": {"kind": st.decl.kind, "key": key}}))
    }

    // ---- forms ----------------------------------------------------------------------------------------------

    fn form_part(&mut self, p: &'a FormPart) -> UiNode {
        let Some(site) = self
            .actions
            .get(p.action.as_str())
            .map(|s| (s.action, s.part_id))
        else {
            // The action is unknown. Admission refuses this document (UI-AD-1).
            return UiNode::new(child_id(&p.id, "container"), UiNodeKind::Form);
        };
        let action = site.0;
        let id = child_id(&p.id, "container");
        match self.form(
            &id,
            action,
            &p.schema,
            &Env::default(),
            p.submit.as_ref(),
            p.cancel.as_ref(),
        ) {
            Ok(n) => n,
            // The diagnostic of the refusal was recorded where it arose (UI-FM-2, UI-CF-3).
            Err(_) => {
                UiNode::new(id, UiNodeKind::EmptyState).prop("title", copy_key("form_unavailable"))
            }
        }
    }

    /// One action form (UI-RZ-2, UI-FM-2 to UI-FM-6, UI-FM-8, UI-CF-3). `Err` is a refusal with a diagnostic.
    #[allow(clippy::too_many_arguments)]
    fn form(
        &mut self,
        form_node: &str,
        action: &'a Action,
        schema: &'a Value,
        outer: &Env<'_>,
        submit: Option<&'a ButtonSpec>,
        cancel: Option<&'a ButtonSpec>,
    ) -> Result<UiNode, String> {
        let env = Env {
            form_node: Some(form_node),
            form_schema: Some(schema),
            ..outer.clone()
        };
        let required: BTreeSet<&str> = schema
            .get("required")
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default();
        let props: Vec<(&String, &Value)> = schema
            .get("properties")
            .and_then(Value::as_object)
            .map(|m| m.iter().collect())
            .unwrap_or_default();
        struct Built {
            order: Option<f64>,
            decl: usize,
            group: Option<String>,
            advanced: bool,
            node: UiNode,
        }
        let mut built: Vec<Built> = vec![];
        for (decl, (name, spec)) in props.iter().enumerate() {
            let ui = spec.get("ui").cloned().unwrap_or(Value::Null);
            // UI-FM-3: `visible_when` hides a field. The client omits a hidden field from the request.
            if let Some(vw) = ui
                .get("visible_when")
                .and_then(|v| serde_json::from_value::<Pred>(v.clone()).ok())
            {
                if !self.pred(&vw, &env) {
                    continue;
                }
            }
            let is_required = required.contains(name.as_str());
            // UI-FM-3: `required_when` is a client pre-check only.
            let required_when = ui
                .get("required_when")
                .and_then(|v| serde_json::from_value::<Pred>(v.clone()).ok())
                .is_some_and(|p| self.pred(&p, &env));
            let field_node =
                self.form_field(form_node, name, spec, &ui, is_required, required_when, &env)?;
            built.push(Built {
                order: ui.get("order").and_then(Value::as_f64),
                decl,
                group: ui.get("group").and_then(Value::as_str).map(str::to_string),
                advanced: ui.get("advanced").and_then(Value::as_bool).unwrap_or(false),
                node: field_node,
            });
        }
        // UI-FM-6: order, then declaration.
        built.sort_by(|a, b| {
            let (x, y) = (a.order.unwrap_or(f64::MAX), b.order.unwrap_or(f64::MAX));
            x.partial_cmp(&y)
                .unwrap_or(Ordering::Equal)
                .then(a.decl.cmp(&b.decl))
        });
        let item_root = child_id(form_node, "item");
        let mut children: Vec<UiNode> = vec![];
        let mut sections: Vec<(String, Vec<&Built>)> = vec![];
        let mut optional: Vec<&Built> = vec![];
        for b in &built {
            match &b.group {
                Some(g) => match sections.iter_mut().find(|(n, _)| n == g) {
                    Some((_, l)) => l.push(b),
                    None => sections.push((g.clone(), vec![b])),
                },
                None if b.advanced => optional.push(b),
                None => children.push(b.node.clone()),
            }
        }
        for (name, list) in sections {
            // UI-FM-6: a section collapses only when every field in it is advanced.
            let collapsed = list.iter().all(|b| b.advanced);
            children.push(
                UiNode::new(
                    child_id(&item_root, &format!("group:{name}")),
                    UiNodeKind::FormSection,
                )
                .prop("label", json!(name))
                .prop("collapsed", collapsed.then(|| json!(true)))
                .with_children(list.iter().map(|b| b.node.clone()).collect()),
            );
        }
        if !optional.is_empty() {
            children.push(
                UiNode::new(child_id(&item_root, "optional"), UiNodeKind::FormSection)
                    .prop("label", copy_key("optional_section"))
                    .prop("collapsed", json!(true))
                    .with_children(optional.iter().map(|b| b.node.clone()).collect()),
            );
        }
        let toolbar_id = child_id(form_node, "action");
        // UI-PT-5, UI-NV-6: a label is authored text, or the default copy key `submit` or `cancel`.
        let submit_btn = UiNode::new(child_id(&toolbar_id, "submit"), UiNodeKind::Button)
            .prop(
                "label",
                submit
                    .and_then(|s| s.label.as_ref())
                    .map(|l| json!(l))
                    .unwrap_or_else(|| copy_key("submit")),
            )
            .prop("action_ref", json!(action.id));
        let cancel_btn = UiNode::new(child_id(&toolbar_id, "cancel"), UiNodeKind::Button).prop(
            "label",
            cancel
                .and_then(|s| s.label.as_ref())
                .map(|l| json!(l))
                .unwrap_or_else(|| copy_key("cancel")),
        );
        children.push(
            UiNode::new(toolbar_id, UiNodeKind::Toolbar)
                .with_children(vec![submit_btn, cancel_btn]),
        );
        // UI-NV-3, UI-NV-5: the form-level message is the first child of the form.
        if let Some(msg) = self.input.local_state.errors.forms.get(form_node) {
            children.insert(0, error_text(child_id(form_node, "error"), msg));
        }
        Ok(UiNode::new(form_node, UiNodeKind::Form)
            .prop("action", json!({"id": action.id}))
            .prop(
                "submit_label",
                json!(submit
                    .and_then(|s| s.label.clone())
                    .unwrap_or_else(|| action.label.clone())),
            )
            .with_children(children))
    }

    /// One property as a `form_field` (UI-RZ-2). Returns the node and whether its value is empty.
    #[allow(clippy::too_many_arguments)]
    fn form_field(
        &mut self,
        form_node: &str,
        name: &str,
        spec: &'a Value,
        ui: &Value,
        required: bool,
        required_when: bool,
        env: &Env<'_>,
    ) -> Result<UiNode, String> {
        let caps = &self.input.capabilities;
        let key = draft_key(form_node, name);
        let ff_id = key.clone();
        let widget_id = child_id(&ff_id, "widget");
        let ty = spec.get("type").and_then(Value::as_str).unwrap_or("string");
        let secret = spec.get("secret").and_then(Value::as_bool).unwrap_or(false);
        let hint = ui.get("widget").and_then(Value::as_str);
        if let Some(h) = hint {
            // UI-VR-2: an unknown widget hint is ignored and counted.
            if ![
                "text", "textarea", "select", "checkbox", "number", "password",
            ]
            .contains(&h)
            {
                self.diag("unknown_widget", Some(&ff_id), h);
            }
        }
        let read_only = ui
            .get("read_only")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        // UI-NV-5: a draft value is shown; an unset draft shows no value and hides the default; no draft shows the default.
        let current = match self.draft(&key) {
            DraftView::Value(v) => Some(v),
            DraftView::Unset => None,
            DraftView::None => {
                // UI-FM-5: a default is a literal, or an operand that is not a form operand. An absent
                // `default_from` operand gives no default (UI-SL-8).
                spec.get("default").cloned().or_else(|| {
                    let op = spec.get("default_from")?;
                    let op: Operand = serde_json::from_value(op.clone()).ok()?;
                    self.eval(&op, env)
                })
            }
        };
        // UI-NV-4: an error that `realize` stamped in an earlier pass replaces an earlier error (UI-NV-5 `fields`).
        let mut field_error: Option<ErrorValue> = if self.found.contains(&key) {
            Some(ErrorValue {
                code: local_error("option_unavailable"),
                message: None,
            })
        } else {
            self.input.local_state.errors.fields.get(&key).cloned()
        };
        let mut extra_children: Vec<UiNode> = vec![];
        let mut w: UiNode;
        // UI-FM-2: a password widget or a secret property never falls back to an echoing input.
        if secret || hint == Some("password") {
            if !caps.has_widget("password") {
                self.diag("masked_input_unavailable", Some(&ff_id), name);
                return Err(format!("no masked input for '{name}'"));
            }
            // UI-FM-8: a secret property never receives its value. The form shows `set: bool`, which the row of a
            // row action carries at the property (SF-3).
            let set = env
                .row
                .and_then(|r| r.pointer(&format!("/{}/set", pointer_token(name))))
                .and_then(Value::as_bool);
            // UI-RZ-2a: the draft of the user shows (masked); the stored value and the default never do.
            let typed = match self.draft(&key) {
                DraftView::Value(v) => Some(v),
                _ => None,
            };
            w = UiNode::new(&widget_id, UiNodeKind::TextInput)
                .prop("masked", json!(true))
                .prop("value", typed)
                .prop("secret_set", set.map(|b| json!(b)));
        } else if let Some(src) = ui.get("options_source") {
            let (options, empty_copy) = self.options(src, env);
            let mut select = UiNode::new(&widget_id, UiNodeKind::Select);
            let mut value = current.clone();
            // UI-FM-4, UI-NV-4: a shown value, from the draft or the default, that is no longer an option is unset
            // and marked. The tree is that of the state in which this is already applied.
            if let (Some(v), Some(opts)) = (&value, &options) {
                if !opts.iter().any(|o| o.get("value") == Some(v)) {
                    value = None;
                    if !self.changes.unset_drafts.contains(&key) {
                        self.changes.unset_drafts.push(key.clone());
                    }
                    field_error = Some(ErrorValue {
                        code: local_error("option_unavailable"),
                        message: None,
                    });
                }
            }
            match options {
                Some(opts) => {
                    if opts.is_empty() {
                        // UI-CP-4: an option source with no options shows its own empty copy inside the field.
                        extra_children.push(copy_node(
                            child_id(&widget_id, "empty"),
                            empty_copy.as_ref(),
                            DEFAULT_EMPTY_TITLE,
                        ));
                    }
                    select = select.prop("options", json!(opts));
                }
                None => select = select.prop("disabled", json!(true)),
            }
            w = select.prop("value", value.clone());
        } else if let Some(options) = spec.get("enum").and_then(Value::as_array) {
            let opts: Vec<Value> = options
                .iter()
                .map(|o| json!({"value": o, "label": display_text(Some(o))}))
                .collect();
            w = UiNode::new(&widget_id, UiNodeKind::Select)
                .prop("options", json!(opts))
                .prop("value", current.clone());
        } else {
            match ty {
                "boolean" => {
                    w = UiNode::new(&widget_id, UiNodeKind::Checkbox)
                        .prop("checked", current.clone());
                }
                "integer" | "number" => {
                    w = UiNode::new(&widget_id, UiNodeKind::TextInput)
                        .prop("input", json!("number"))
                        .prop("value", current.clone());
                }
                "array" | "object" => {
                    // UI-CF-3.
                    let (node, refuse) =
                        self.structured_widget(&widget_id, ty, spec, current.as_ref(), required);
                    if refuse {
                        self.diag("uneditable_required_property", Some(&ff_id), name);
                        return Err(format!("property '{name}' has no editor"));
                    }
                    w = node;
                }
                _ => {
                    // A widget hint that the renderer lacks is ignored (UI-CF-2, UI-FM-2).
                    let kind = match hint {
                        Some("textarea") if caps.has_widget("textarea") => UiNodeKind::Textarea,
                        _ => UiNodeKind::TextInput,
                    };
                    w = UiNode::new(&widget_id, kind).prop("value", current.clone());
                }
            }
        }
        let label = name.to_string();
        let is_required = required || required_when;
        w = w
            .prop("name", json!(name))
            .prop("label", json!(label))
            .prop("placeholder", ui.get("placeholder").cloned())
            .prop("read_only", read_only.then(|| json!(true)))
            .prop("required", is_required.then(|| json!(true)));
        // The field schema of the stolen vocabulary (UI-RZ-2): the kind, name, label and hints of the field.
        let field_kind = match w.kind {
            UiNodeKind::Textarea => "textarea",
            UiNodeKind::Checkbox => "checkbox",
            UiNodeKind::Select => "select",
            _ => "text",
        };
        let mut schema_prop = Map::new();
        schema_prop.insert("kind".into(), json!(field_kind));
        schema_prop.insert("name".into(), json!(name));
        schema_prop.insert("label".into(), json!(label));
        if let Some(h) = ui.get("help") {
            schema_prop.insert("description".into(), h.clone());
        }
        if let Some(p) = ui.get("placeholder") {
            schema_prop.insert("placeholder".into(), p.clone());
        }
        if is_required {
            schema_prop.insert("required".into(), json!(true));
        }
        let mut ff = UiNode::new(&ff_id, UiNodeKind::FormField)
            .prop("schema", Value::Object(schema_prop))
            .prop("help", ui.get("help").cloned())
            .prop(
                "error",
                field_error.map(|e| serde_json::to_value(e).unwrap_or(Value::Null)),
            )
            .prop("required", is_required.then(|| json!(true)));
        ff.children.push(w);
        ff.children.extend(extra_children);
        Ok(ff)
    }

    /// UI-CF-3: an array or object property with no editor of its own.
    fn structured_widget(
        &mut self,
        id: &str,
        ty: &str,
        spec: &Value,
        current: Option<&Value>,
        required: bool,
    ) -> (UiNode, bool) {
        let json_text = |v: Option<&Value>| v.map(|v| v.to_string());
        let readonly = |v: Option<&Value>| {
            UiNode::new(id, UiNodeKind::Textarea)
                .prop("read_only", json!(true))
                .prop("value", json_text(v).map(Value::String))
        };
        let uneditable = |current: Option<&Value>| {
            required && current.is_none() && spec.get("default").is_none()
        };
        if ty == "array" {
            let strings = spec
                .get("items")
                .and_then(|i| i.get("type"))
                .and_then(Value::as_str)
                == Some("string");
            if strings {
                let lines = match current {
                    Some(Value::Array(a))
                        if a.iter()
                            .all(|e| e.as_str().is_some_and(|s| !s.contains('\n'))) =>
                    {
                        Some(
                            a.iter()
                                .filter_map(Value::as_str)
                                .collect::<Vec<_>>()
                                .join("\n"),
                        )
                    }
                    None => Some(String::new()),
                    _ => None,
                };
                return match lines {
                    Some(l) => (
                        UiNode::new(id, UiNodeKind::Textarea).prop("value", json!(l)),
                        false,
                    ),
                    None => (readonly(current), false),
                };
            }
            return (readonly(current), uneditable(current));
        }
        let all_strings = spec
            .get("properties")
            .and_then(Value::as_object)
            .is_none_or(|m| {
                m.values()
                    .all(|p| p.get("type").and_then(Value::as_str) == Some("string"))
            });
        if all_strings {
            match current {
                None => {
                    return (
                        UiNode::new(id, UiNodeKind::Textarea).prop("value", json!("")),
                        false,
                    )
                }
                Some(Value::Object(m)) => {
                    let lossless = m.iter().all(|(k, v)| {
                        !k.contains('=')
                            && !k.contains('\n')
                            && v.as_str().is_some_and(|s| !s.contains('\n'))
                    });
                    if lossless {
                        let lines: Vec<String> = m
                            .iter()
                            .map(|(k, v)| format!("{k}={}", v.as_str().unwrap_or("")))
                            .collect();
                        return (
                            UiNode::new(id, UiNodeKind::Textarea)
                                .prop("value", json!(lines.join("\n"))),
                            false,
                        );
                    }
                    return (readonly(current), false);
                }
                _ => {}
            }
        }
        (readonly(current), uneditable(current))
    }

    /// The options of an `options_source` (UI-FM-4, UI-CP-4). `None` options means no baseline yet.
    fn options(&mut self, src: &Value, env: &Env<'_>) -> (Option<Vec<Value>>, Option<CopyText>) {
        let kind = src.get("kind").and_then(Value::as_str).unwrap_or("");
        let value_field = src
            .get("value_field")
            .and_then(Value::as_str)
            .unwrap_or("/id");
        let label_field = src
            .get("label_field")
            .and_then(Value::as_str)
            .unwrap_or("/id");
        let where_ = src
            .get("where")
            .and_then(|v| serde_json::from_value::<Pred>(v.clone()).ok());
        let empty = src
            .get("empty")
            .and_then(|v| serde_json::from_value::<CopyText>(v.clone()).ok());
        let order: Vec<OrderKey> = src
            .get("order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default();
        let Some(snap) = self.input.kind_snapshots.get(kind).filter(|s| s.baseline) else {
            return (None, empty);
        };
        let excluded: Vec<Value> = match src.get("exclude") {
            Some(ex) => {
                let ek = ex.get("kind").and_then(Value::as_str).unwrap_or("");
                let ef = ex
                    .get("value_field")
                    .and_then(Value::as_str)
                    .unwrap_or("/id");
                let ew = ex
                    .get("where")
                    .and_then(|v| serde_json::from_value::<Pred>(v.clone()).ok());
                self.input
                    .kind_snapshots
                    .get(ek)
                    .map(|s| {
                        s.rows
                            .iter()
                            .filter(|r| {
                                ew.as_ref().is_none_or(|w| {
                                    self.pred(
                                        w,
                                        &Env {
                                            path_row: Some(r),
                                            ..env.clone()
                                        },
                                    )
                                })
                            })
                            .filter_map(|r| r.pointer(ef).cloned())
                            .collect()
                    })
                    .unwrap_or_default()
            }
            None => vec![],
        };
        let mut rows: Vec<&Value> = snap
            .rows
            .iter()
            .filter(|r| {
                where_.as_ref().is_none_or(|w| {
                    self.pred(
                        w,
                        &Env {
                            path_row: Some(r),
                            ..env.clone()
                        },
                    )
                })
            })
            .filter(|r| r.pointer(value_field).is_none_or(|v| !excluded.contains(v)))
            .collect();
        rows.sort_by(|a, b| {
            for k in &order {
                let mut c = compare(a.pointer(&k.field), b.pointer(&k.field));
                if k.dir == Dir::Desc {
                    c = c.reverse();
                }
                if c != Ordering::Equal {
                    return c;
                }
            }
            row_key(a).cmp(&row_key(b))
        });
        let opts = rows
            .iter()
            .filter_map(|r| {
                let v = r.pointer(value_field)?;
                Some(json!({"value": v, "label": display_text(r.pointer(label_field))}))
            })
            .collect();
        (Some(opts), empty)
    }

    // ---- overlays -------------------------------------------------------------------------------------------

    /// UI-RZ-2b, UI-NV-4: for each button of the tree that has an entry, its form entry when one exists, else its confirm
    /// entry, whatever their order. Dialogs keep the order in which each button first appears in the list. An entry
    /// whose button is not in the tree is kept and not shown.
    fn overlays(&mut self) -> Vec<UiNode> {
        let mut out = vec![];
        let overlays = &self.input.local_state.overlays;
        let mut order: Vec<&str> = vec![];
        for ov in overlays {
            if !order.contains(&ov.button()) {
                order.push(ov.button());
            }
        }
        for button_id in order {
            let Some(site) = self.buttons.get(button_id).cloned() else {
                continue;
            };
            let has_form = overlays
                .iter()
                .any(|o| matches!(o, Overlay::Form(b) if b.button == button_id));
            let has_confirm = overlays
                .iter()
                .any(|o| matches!(o, Overlay::Confirm(b) if b.button == button_id));
            let action = site.action;
            let row: Option<&'a Value> = match (&site.row_key, site.row_kind) {
                (Some(k), Some(kind)) => self
                    .input
                    .kind_snapshots
                    .get(kind)
                    .and_then(|s| s.rows.iter().find(|r| row_key(r).as_deref() == Some(k))),
                _ => None,
            };
            let dialog_id = child_id(button_id, "container");
            // UI-RF-3: the row's refs are resolved for the form of a row action.
            let states = match (row, site.refs) {
                (Some(r), Some(refs)) => self.resolve_refs(refs, r),
                _ => BTreeMap::new(),
            };
            let env = Env {
                path_row: row,
                row,
                refs: Some(&states),
                ..Env::default()
            };
            // UI-SA-5: the warning of the action, from the document or from the contributed descriptor.
            let warning = action.warning.as_deref().or_else(|| {
                self.action_entry(action)
                    .and_then(|e| e.get("warning")?.as_str())
            });
            let form_schema = action
                .args_schema
                .as_ref()
                .filter(|s| has_user_fields(action, s));
            if has_form && form_schema.is_some() {
                let schema = form_schema.unwrap_or(&Value::Null);
                // UI-NV-3: the form of a dialog is `child(<dialog>, "item")`.
                let form_id = child_id(&dialog_id, "item");
                let mut body = vec![];
                // UI-SA-5: the warning stays visible in the form.
                body.extend(warning.map(|w| warning_text(child_id(&dialog_id, "warning"), w)));
                match self.form(&form_id, action, schema, &env, None, None) {
                    Ok(form) => body.push(form),
                    Err(_) => body.push(
                        UiNode::new(child_id(&dialog_id, "label"), UiNodeKind::EmptyState)
                            .prop("title", copy_key("form_unavailable")),
                    ),
                }
                out.push(
                    UiNode::new(dialog_id, UiNodeKind::Dialog)
                        .prop("title", json!(action.label))
                        .slot("body", body),
                );
            } else if has_confirm || has_form {
                let Some(c) = &action.confirm else {
                    continue;
                };
                let mut body = vec![UiNode::new(child_id(&dialog_id, "label"), UiNodeKind::Text)
                    .prop("text", json!(c.message))
                    .prop("tone", json!(c.tone.as_str()))];
                body.extend(warning.map(|w| warning_text(child_id(&dialog_id, "warning"), w)));
                out.push(
                    UiNode::new(&dialog_id, UiNodeKind::Dialog)
                        .prop("title", json!(action.label))
                        .slot("body", body)
                        .slot(
                            "actions",
                            vec![
                                UiNode::new(child_id(&dialog_id, "confirm"), UiNodeKind::Button)
                                    .prop("label", copy_key("confirm"))
                                    .prop("action_ref", json!(action.id))
                                    .prop(
                                        "action",
                                        json!({"id": action.id, "payload": site.row_key.as_ref().map(|k| json!({"target": k}))}),
                                    )
                                    .prop("tone", json!(c.tone.as_str())),
                                UiNode::new(child_id(&dialog_id, "cancel"), UiNodeKind::Button)
                                    .prop("label", copy_key("cancel")),
                            ],
                        ),
                );
            }
        }
        out
    }
}

/// A button and the nodes beside it (UI-NV-3, UI-SA-5).
struct Cluster {
    button: UiNode,
    extras: Vec<UiNode>,
}

/// An `error`-tone `text` node (UI-NV-3, UI-AC-7(a)). The string is authored text (UI-NV-5).
fn error_text(id: String, message: &str) -> UiNode {
    UiNode::new(id, UiNodeKind::Text)
        .prop("text", json!(message))
        .prop("tone", json!("danger"))
}

/// The action warning: a `text` node in the `warning` tone (UI-SA-5).
fn warning_text(id: String, message: &str) -> UiNode {
    UiNode::new(id, UiNodeKind::Text)
        .prop("text", json!(message))
        .prop("tone", json!("warning"))
}

/// A draft as `realize` reads it (UI-NV-5).
enum DraftView {
    None,
    Value(Value),
    Unset,
}

enum SourceState {
    Loading,
    Error(Option<String>),
    Empty,
}

/// The `state` prop, the state child and the `detail` prop of a part in a state (UI-CP-1, UI-NV-1).
struct StateParts {
    state: &'static str,
    child: Option<UiNode>,
    detail: Option<String>,
}

impl StateParts {
    fn new(state: &'static str, child: Option<UiNode>, detail: Option<String>) -> Self {
        StateParts {
            state,
            child,
            detail,
        }
    }

    /// Sets `state` and `detail` on the node of the part.
    fn apply(&self, node: UiNode) -> UiNode {
        node.prop("state", json!(self.state))
            .prop("detail", self.detail.as_ref().map(|d| json!(d)))
    }
}

/// A field or a column, seen the same way (section 5 of ui-contract 1.0).
struct FieldLike<'f> {
    label: Option<String>,
    path: Option<&'f Path>,
    value: Option<Operand>,
    display: Option<OrUnknown<Display>>,
    map: &'f BTreeMap<String, MapEntry>,
    tone: &'f Option<OrUnknown<Tone>>,
    absent: Option<String>,
}

impl<'f> FieldLike<'f> {
    fn from_field(f: &'f Field) -> Self {
        FieldLike {
            label: f.label.clone(),
            path: f.path.as_ref(),
            value: f.value.clone(),
            display: f.display.clone(),
            map: &f.map,
            tone: &f.tone,
            absent: f.absent.clone(),
        }
    }
    fn from_column(c: &'f Column) -> Self {
        FieldLike {
            label: None,
            path: c.path.as_ref(),
            value: c.ref_.as_ref().map(|r| Operand::Ref(r.clone())),
            display: c.display.clone(),
            map: &c.map,
            tone: &c.tone,
            absent: c.absent.clone(),
        }
    }
    fn operand(&self) -> Option<Operand> {
        self.path
            .map(|p| Operand::Path(p.clone()))
            .or_else(|| self.value.clone())
    }
}

/// The label and tone of a value in a map (section 5 of ui-contract 1.0, UI-FD-2). The key of a non-string value is its
/// RFC 8785 text; an object or an array is never a key. A value with no key shows its raw text in the neutral tone.
fn map_lookup(
    map: &BTreeMap<String, MapEntry>,
    value: Option<&Value>,
) -> (String, Option<OrUnknown<Tone>>) {
    let text = display_text(value);
    match value.and_then(map_key).and_then(|k| map.get(&k)) {
        Some(e) => (e.label.clone(), e.tone.clone()),
        None => (
            text,
            (!map.is_empty() && value.is_some()).then_some(OrUnknown::Known(Tone::Neutral)),
        ),
    }
}

/// Escapes one JSON Pointer token (RFC 6901).
fn pointer_token(token: &str) -> String {
    token.replace('~', "~0").replace('/', "~1")
}

/// The feature names that realize reads from `Capabilities.features` (UI-CF-6).
pub const FEATURE_DISPLAY_METRIC: &str = "display_metric";
pub const FEATURE_DISPLAY_BADGE: &str = "display_badge";
pub const FEATURE_EXTERNAL_OPEN: &str = "external_open";
pub const FEATURE_EMBED: &str = "embed";

/// A validated link (UI-FD-1): the host to show, and the full URL.
struct Link {
    /// The ASCII (punycode) host and a non-default port.
    host: String,
    full: String,
}

/// UI-FD-1, UI-SL-7: the value must be an `http` or `https` URL with a host and without userinfo.
fn valid_link(text: &str) -> Option<Link> {
    let url = url::Url::parse(text).ok()?;
    if !matches!(url.scheme(), "http" | "https")
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return None;
    }
    let host = url.host_str().filter(|h| !h.is_empty())?;
    let host = match url.port() {
        Some(p) => format!("{host}:{p}"),
        None => host.to_string(),
    };
    Some(Link {
        host,
        full: url.as_str().to_string(),
    })
}

/// Splits `"<slot><Path>"` into the slot id and the pointer (UI-BD-3).
pub fn split_slot(s: &str) -> (&str, &str) {
    match s.find('/') {
        Some(i) => (&s[..i], &s[i..]),
        None => (s, ""),
    }
}

fn dir_text(d: Direction) -> &'static str {
    match d {
        Direction::Column => "column",
        Direction::Row => "row",
    }
}

/// A node for a copy (UI-CP-1, UI-CP-2). Authored copy is a string, a default is a copy key (UI-NV-6).
fn copy_node(id: String, copy: Option<&CopyText>, default_title: &str) -> UiNode {
    let title = match copy {
        Some(c) => json!(c.title),
        None => copy_key(default_title),
    };
    UiNode::new(id, UiNodeKind::EmptyState)
        .prop("title", title)
        .prop(
            "description",
            copy.and_then(|c| c.body.as_ref()).map(|b| json!(b)),
        )
}

/// UI-AC-4: an action has a user-entered field when its schema has a property that `args` does not bind.
pub fn has_user_fields(action: &Action, schema: &Value) -> bool {
    schema
        .get("properties")
        .and_then(Value::as_object)
        .is_some_and(|m| m.keys().any(|k| !action.args.contains_key(k)))
}

/// An overflowed button as a menu item (UI-RZ-2).
fn menu_item(button: UiNode) -> UiNode {
    let mut item = UiNode::new(button.id, UiNodeKind::MenuItem);
    item.props = button.props;
    item.props.remove("variant");
    item
}

/// A table cell: the text of a plain text node, or the node itself (UI-RZ-2).
fn cell_value(cell: &UiNode) -> Value {
    let plain = cell.kind == UiNodeKind::Text
        && cell.children.is_empty()
        && cell.props.keys().all(|k| k == "text");
    if plain {
        cell.props.get("text").cloned().unwrap_or(json!(""))
    } else {
        serde_json::to_value(cell).unwrap_or(Value::Null)
    }
}

/// A table in a state (UI-CP-1): the `state` prop and the `empty_state` prop; the fallback list holds a child.
fn table_state(node: UiNode, st: StateParts, grid: bool) -> UiNode {
    let node = st.apply(node);
    match (st.child, grid) {
        (Some(c), true) => {
            let empty =
                json!({"title": c.props.get("title"), "description": c.props.get("description")});
            node.prop("empty_state", empty)
        }
        (Some(c), false) => node.with_children(vec![c]),
        (None, _) => node,
    }
}

/// A `detail` in a state (UI-CP-1): the state child goes in the `empty` slot of a panel, or is a child of the labeled
/// lines of the UI-CF-2 fallback, which has no `state` prop.
fn detail_state(node: UiNode, st: StateParts) -> UiNode {
    if node.kind == UiNodeKind::Panel {
        let child = st.child.clone();
        let node = st.apply(node);
        match child {
            Some(c) => node.slot("empty", vec![c]),
            None => node,
        }
    } else {
        let mut node = node;
        node.children.extend(st.child);
        node
    }
}

/// An `absent_reason` of [`ABSENT_REASONS`] (UI-RF-1).
fn absent(reason: &'static str) -> Option<&'static str> {
    debug_assert!(
        ABSENT_REASONS.contains(&reason),
        "{reason} is not an absent_reason (UI-NV-2)"
    );
    Some(reason)
}

/// A local error code of [`LOCAL_ERROR_CODES`] (UI-FM-4).
fn local_error(code: &str) -> String {
    debug_assert!(
        LOCAL_ERROR_CODES.contains(&code),
        "{code} is not a local error code (UI-NV-2)"
    );
    code.to_string()
}

/// A diagnostic code that this crate chooses, one of [`DIAGNOSTIC_CODES`] (UI-NV-4). A caller-supplied code stays open: a renderer
/// shows an unknown code as a generic diagnostic.
pub(crate) fn diagnostic_code(code: &'static str) -> &'static str {
    debug_assert!(
        DIAGNOSTIC_CODES.contains(&code),
        "{code} is not a diagnostic code of this crate (UI-NV-4)"
    );
    code
}

/// A copy key: the renderer turns it into text (UI-NV-6). The key is one of [`COPY_KEYS`].
fn copy_key(key: &str) -> Value {
    debug_assert!(COPY_KEYS.contains(&key), "{key} is not a UI-NV-6 copy key");
    json!(Text::copy(key))
}

/// A copy key with `args` (UI-NV-6: `view_unavailable`). The arg names are those of [`COPY_KEY_ARGS`].
fn copy_key_args(key: &str, args: &[(&str, &str)]) -> Value {
    debug_assert!(
        COPY_KEY_ARGS
            .iter()
            .any(|(k, names)| *k == key && names.iter().eq(args.iter().map(|(n, _)| n))),
        "{key} does not take the args {args:?} (UI-NV-6)"
    );
    json!(Text::Copy(CopyKey {
        copy: key.to_string(),
        args: Some(
            args.iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect()
        ),
    }))
}
