//! `cargo xtask crate-symbols [--check | --draft <file>]`: every code span of every contract file maps to crate symbols, or to `n/a`
//! with a reason, and each symbol resolves.
//!
//! docs/BUILD.md "Amendments": an amendment is not FINAL until its crate change is merged on the same final branch. This command is the
//! mechanical half of that rule, and the text drives it. The command extracts every single-line code span (`` `...` ``) of each file
//! that `contracts.toml` lists, with its line. `crate-symbols/<file stem>.toml` has one row per (line, span), in `[rows."<line>"]`:
//! each table row and each paragraph has its own mappings, so the same span under two owners has two rows. A contract file never
//! changes in place (the manifest job checks its hash; a new candidate is a new file), so a line number is a stable identity. A row is
//! one of:
//! - `["<symbol>", ...]`: the crate symbols that the span names;
//! - `"n/a: <reason>"`: a reviewed exemption of this one span (prose in code format, an example value, a path of the file system);
//! - `{ symbols = [...], uncovered = { <word> = "<reason>" } }`: symbols, and the words of the span that no symbol names.
//!
//! **Word coverage.** Each symbol-shaped word of a span (an identifier with a lowercase letter: CamelCase or snake_case) must equal a
//! segment of one of the row's symbols, or be in `uncovered`. The comparison ignores case and `_`, so `InvalidInput` = `invalid_input`.
//! A span with no symbol-shaped word (a number, a clause id such as `SW-1`) needs no row. The rule classes of [`RULES`] exempt a few
//! fixed span shapes. So `InvalidInput{field}` mapped to `core::ErrorCode::InvalidInput` alone fails: no symbol names `field`.
//!
//! Two kinds of symbol:
//! - **structural**, `<crate>::[<module>::]*<Item>[::<Member>[::<Field>]]`, resolved by a `syn` parse of the crate's non-test source.
//!   Only the item tree counts: a name in a comment, a string literal, a `macro_rules!` body or a `#[cfg(test)]` item never resolves,
//!   and a member must belong to the named item. Item-level macro invocations are read by the rules of [`item_macro`]; one with no rule
//!   fails the check, so a new macro cannot hide an item.
//! - **registry data**, `<scheme>:<arguments>`, resolved by a typed lookup in the registry that the contract crate exports (an error code,
//!   a failure reason, a limit, an event, a UI prop): see [`resolve_data`].
//!
//! **Whole tokens.** A `cli.*` symbol names one token of the CLI spec: the binary name, a command path (`plugin pin`), a flag
//! (`--unpin`), a key, a development checkout key, an MCP tool, a log label, a confirmation answer, or the full text of a message or a path. It covers the words of that token only where the span holds the
//! whole token, with no word character or `-` next to it, and it has no segments ([`cli_token`], [`whole_token_words`]). So
//! `cli.command:COMMANDS:mcp-serve` covers `mcp` and `serve` in `mcp-serve`, and never a lone `mcp`.
//!
//! **Crate gaps.** A span or a word that the contract defines and no crate symbol holds yet is a crate gap. Its reason starts
//! `GAP(<closer>): `, where the closer names the change that adds the symbol ([`GAP_CLOSERS`]). With `--base`, a gap that the base
//! tables do not have fails: the gap list only shrinks, as PENDING does ([`gaps`], [`check_gaps`]).
//!
//! [`PENDING`] is empty: every base file has a table. A file that contracts.toml lists without a table fails.

use crate::config::Config;
use anyhow::{bail, Context, Result};
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The directory of the tables, relative to the repo root.
const TABLES: &str = "crate-symbols";

/// The base files that have no table yet, with the sha256 of each file. This list only shrinks: the change that adds a file's table
/// deletes its entry here. The hash binds an entry to the file's bytes, so a new or a replaced file cannot use the exemption by name.
const PENDING: &[(&str, &str)] = &[];

/// The pending names that a base with no PENDING const allows: the seven base files when this check was adopted. Used only then.
const BOOTSTRAP: &[&str] = &[
    "core-contract-v1.17.md",
    "route-codec-v1-extract-rev36.md",
    "hub-client-contract-v1-candidate-parity45.md",
    "hub-plugin-contract-v1.1.joint-J4e.md",
    "hub-plugin-lua-binding-v1.1-r9.md",
    "ui-contract-v1-frozen-20260930.md",
    "cli-spec-v1-candidate34.md",
];

/// The rule classes: a span that matches one in full, and passes its check in [`rule_of`], needs no row. (name, pattern, reason).
/// A pattern is anchored to the whole span, so a rule never exempts a part of a span.
pub const RULES: &[(&str, &str, &str)] = &[(
    "conformance-id",
    r"^conf::[a-z0-9_]+$",
    "the id of a conformance test (a ledger or withdrawn id), not a crate symbol",
)];

/// The crate aliases of a structural symbol, and their source directories.
const CRATES: &[(&str, &str)] = &[
    ("core", "crates/botster-core-contract/src"),
    ("codec", "crates/botster-route-codec/src"),
    ("hub", "crates/botster-hub-contract/src"),
    ("plugin", "crates/botster-plugin-contract/src"),
    ("ui", "crates/botster-ui-contract/src"),
    ("cli", "crates/botster-cli-contract/src"),
];

/// The table of one contract file.
#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Table {
    /// Line number (1-based, as a string key), then span.
    #[serde(default)]
    pub rows: BTreeMap<String, BTreeMap<String, Row>>,
}

/// One row: what a span names.
#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Row {
    Symbols(Vec<String>),
    NotApplicable(String),
    Partial(Partial),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Partial {
    pub symbols: Vec<String>,
    pub uncovered: BTreeMap<String, String>,
}

impl Table {
    /// The table of `file`, or `None` when it has none.
    pub fn load(root: &Path, file: &str) -> Result<Option<Table>> {
        let path = table_path(root, file);
        if !path.exists() {
            return Ok(None);
        }
        let text =
            std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        toml::from_str(&text)
            .map(Some)
            .with_context(|| format!("parse {}", path.display()))
    }
}

fn table_path(root: &Path, file: &str) -> PathBuf {
    root.join(TABLES)
        .join(format!("{}.toml", file.strip_suffix(".md").unwrap_or(file)))
}

// ------------------------------------------------------------------ the text

/// Where a span occurs: the heading path above its line, and the line.
#[derive(Debug, Clone, PartialEq)]
pub struct Place {
    pub headings: String,
    pub text: String,
}

/// Every single-line code span of a Markdown text, trimmed, by (line, span). Fenced code blocks are not spans.
pub fn spans(text: &str) -> BTreeMap<(usize, String), Place> {
    let mut out = BTreeMap::new();
    let mut fenced = false;
    let mut path: Vec<(usize, String)> = Vec::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
            continue;
        }
        if fenced {
            continue;
        }
        if line.starts_with('#') {
            let level = line.chars().take_while(|c| *c == '#').count();
            path.retain(|(l, _)| *l < level);
            path.push((level, line[level..].trim().to_string()));
        }
        let mut rest = line;
        while let Some(start) = rest.find('`') {
            let ticks = rest[start..].chars().take_while(|c| *c == '`').count();
            let fence = &rest[start..start + ticks];
            let after = &rest[start + ticks..];
            let Some(end) = after.find(fence) else {
                break;
            };
            let span = after[..end].trim();
            if !span.is_empty() {
                out.insert(
                    (n + 1, span.to_string()),
                    Place {
                        headings: path
                            .iter()
                            .map(|(_, h)| h.as_str())
                            .collect::<Vec<_>>()
                            .join(" > "),
                        text: line.trim().to_string(),
                    },
                );
            }
            rest = &after[end + ticks..];
        }
    }
    out
}

/// The symbol-shaped words of a span: identifiers with a lowercase ASCII letter (`field`, `InvalidInput`, `max_key_text_bytes`; not
/// `ESC` or `SW`).
pub fn words(span: &str) -> BTreeSet<String> {
    static WORD: OnceLock<Regex> = OnceLock::new();
    let word = WORD.get_or_init(|| Regex::new(r"\b[A-Za-z_][A-Za-z0-9_]*\b").expect("regex"));
    word.find_iter(span)
        .map(|m| m.as_str())
        .filter(|w| w.chars().any(|c| c.is_ascii_lowercase()))
        .map(str::to_string)
        .collect()
}

/// The rule class that exempts a span, if one does: its pattern matches the whole span, and its check passes (`test_ids` holds the
/// known conformance ids).
pub fn rule_of(span: &str, test_ids: &BTreeSet<String>) -> Option<&'static str> {
    static COMPILED: OnceLock<Vec<Regex>> = OnceLock::new();
    let compiled = COMPILED.get_or_init(|| {
        RULES
            .iter()
            .map(|(_, p, _)| Regex::new(p).expect("rule regex"))
            .collect()
    });
    let (name, _, _) = RULES
        .iter()
        .zip(compiled)
        .find(|(_, re)| re.is_match(span))?
        .0;
    match *name {
        "conformance-id" => test_ids.contains(span).then_some(*name),
        _ => None,
    }
}

/// A span that needs a row: it has a symbol-shaped word and no rule class exempts it.
pub fn needs_row(span: &str, test_ids: &BTreeSet<String>) -> bool {
    !words(span).is_empty() && rule_of(span, test_ids).is_none()
}

/// A word or segment for comparison: ASCII letters and digits only, lowercased (`max_key_text_bytes` = `MaxKeyTextBytes`, `$schema` =
/// `schema`).
fn normal(word: &str) -> String {
    word.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

/// The segments of a symbol that can cover a word: the path segments after the crate, or the arguments of registry data.
pub fn segments(symbol: &str) -> Vec<String> {
    if symbol.starts_with("cli.") {
        return Vec::new();
    }
    match symbol.split_once("::") {
        Some((_, path)) => path.split("::").map(normal).collect(),
        None => symbol
            .split_once(':')
            .map_or("", |(_, args)| args)
            .split([':', '.', '/'])
            .map(normal)
            .collect(),
    }
}

/// The token of the CLI spec that a `cli.*` symbol names, or `None` for another scheme, a wrong arity or a name that the CLI crate
/// does not list. A family with live and dropped (or gone) names takes the const name as its first argument.
pub fn cli_token(symbol: &str) -> Option<&'static str> {
    use botster_cli_contract as cli;
    let (scheme, args) = symbol.split_once(':')?;
    let a: Vec<&str> = args.split(':').collect();
    let name =
        |list: &'static [&'static str], name: &str| list.iter().copied().find(|x| *x == name);
    let text = |list: &'static [cli::SpecText], clause: &str, n: &str| {
        list.iter()
            .find(|t| t.clause == clause && t.ordinal.to_string() == n)
            .map(|t| t.text)
    };
    match (scheme, a.as_slice()) {
        ("cli.binary", [binary]) => (*binary == cli::BINARY).then_some(cli::BINARY),
        ("cli.command", ["COMMANDS", path]) => name(cli::COMMANDS, path),
        ("cli.command", ["GONE_COMMANDS", path]) => cli::GONE_COMMANDS
            .iter()
            .find(|g| g.command == *path)
            .map(|g| g.command),
        ("cli.mcp_tool", [tool]) => name(cli::MCP_TOOLS, tool),
        ("cli.log_label", [label]) => name(cli::LOG_LABELS, label),
        ("cli.dev_checkout_key", [key]) => name(cli::DEV_CHECKOUT_KEYS, key),
        ("cli.flag", [list, command, flag]) => match *list {
            "FLAGS" => cli::FLAGS,
            "DROPPED_FLAGS" => cli::DROPPED_FLAGS,
            _ => return None,
        }
        .iter()
        .find(|f| f.command == *command && f.flag == *flag)
        .map(|f| f.flag),
        ("cli.confirm_answer", [answer]) => name(cli::CONFIRM_ANSWERS, answer),
        ("cli.context_key", ["CONTEXT_KEYS", key]) => name(cli::CONTEXT_KEYS, key),
        ("cli.context_key", ["DROPPED_CONTEXT_KEYS", key]) => name(cli::DROPPED_CONTEXT_KEYS, key),
        ("cli.config_key", ["CONFIG_KEYS", key]) => name(cli::CONFIG_KEYS, key),
        ("cli.config_key", ["DROPPED_CONFIG_KEYS", key]) => name(cli::DROPPED_CONFIG_KEYS, key),
        ("cli.status_field", ["STATUS_FIELDS", field]) => name(cli::STATUS_FIELDS, field),
        ("cli.status_field", ["DROPPED_STATUS_FIELDS", field]) => {
            name(cli::DROPPED_STATUS_FIELDS, field)
        }
        ("cli.message", [clause, n]) => text(cli::MESSAGES, clause, n),
        ("cli.path", [clause, n]) => text(cli::PATHS, clause, n),
        _ => None,
    }
}

/// The words (compared as [`normal`]) that `symbol` covers in `span` as a whole token: the words of its [`cli_token`], where `span`
/// holds that token with no letter, digit, `_` or `-` on either side. Empty for any other symbol.
pub fn whole_token_words(symbol: &str, span: &str) -> BTreeSet<String> {
    let Some(token) = cli_token(symbol).map(str::trim) else {
        return BTreeSet::new();
    };
    let edge =
        |c: Option<char>| c.is_none_or(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'));
    let whole = span.match_indices(token).any(|(at, _)| {
        edge(span[..at].chars().next_back()) && edge(span[at + token.len()..].chars().next())
    });
    if whole {
        words(token).iter().map(|w| normal(w)).collect()
    } else {
        BTreeSet::new()
    }
}

// ------------------------------------------------------------------ the check

/// The problems of one file's table against the file's text, and the count of resolved symbols. `resolve` resolves one symbol.
pub fn check_table(
    file: &str,
    text: &str,
    table: &Table,
    test_ids: &BTreeSet<String>,
    resolve: &dyn Fn(&str) -> std::result::Result<(), String>,
) -> (Vec<String>, usize) {
    let spans = spans(text);
    let mut problems = Vec::new();
    let mut resolved = 0;
    let mut rows: BTreeMap<(usize, String), &Row> = BTreeMap::new();
    for (line, by_span) in &table.rows {
        let Ok(n) = line.parse::<usize>() else {
            problems.push(format!("{file}: [rows.\"{line}\"] is not a line number"));
            continue;
        };
        for (span, row) in by_span {
            rows.insert((n, span.clone()), row);
        }
    }
    for ((line, span), place) in &spans {
        if needs_row(span, test_ids) && !rows.contains_key(&(*line, span.clone())) {
            problems.push(format!(
                "{file}:{line}: `{span}` has no row\n    in {}\n    {}",
                place.headings, place.text
            ));
        }
    }
    for ((line, span), row) in &rows {
        let at = format!("{file}:{line}: `{span}`");
        if !spans.contains_key(&(*line, span.clone())) {
            problems.push(format!(
                "{at}: the row is stale (the line has no such span)"
            ));
            continue;
        }
        let (symbols, uncovered): (&[String], Option<&BTreeMap<String, String>>) = match row {
            Row::NotApplicable(reason) => {
                if reason
                    .strip_prefix("n/a:")
                    .is_none_or(|r| r.trim().is_empty())
                {
                    problems.push(format!("{at}: a text row is `n/a: <reason>`"));
                }
                continue;
            }
            Row::Symbols(s) => (s, None),
            Row::Partial(p) => (&p.symbols, Some(&p.uncovered)),
        };
        if symbols.is_empty() {
            problems.push(format!(
                "{at}: the row lists no symbol (use `n/a: <reason>`)"
            ));
        }
        for symbol in symbols {
            match resolve(symbol) {
                Ok(()) => resolved += 1,
                Err(reason) => problems.push(format!("{at}: {symbol}: {reason}")),
            }
        }
        let covered: BTreeSet<String> = symbols
            .iter()
            .flat_map(|s| segments(s).into_iter().chain(whole_token_words(s, span)))
            .collect();
        let span_words = words(span);
        for word in &span_words {
            let excused = uncovered.is_some_and(|u| u.contains_key(word));
            if !covered.contains(&normal(word)) && !excused {
                problems.push(format!(
                    "{at}: no symbol names `{word}` (add one, or list it in `uncovered` with a reason)"
                ));
            }
        }
        for (word, reason) in uncovered.into_iter().flatten() {
            if !span_words.contains(word) {
                problems.push(format!(
                    "{at}: `uncovered` names `{word}`, which is not a word of the span"
                ));
            } else if reason.trim().is_empty() {
                problems.push(format!("{at}: `uncovered` gives no reason for `{word}`"));
            }
        }
    }
    (problems, resolved)
}

/// The changes that close a crate gap: the base-table PR of each contract, or `final36` for an amendment that waits for that final.
pub const GAP_CLOSERS: &[&str] = &["core", "codec", "hc", "hp", "lb", "ui", "cli", "final36"];

/// The crate gaps of a table, and the problems of its gap reasons. A crate gap is a span or a word that the contract defines and no
/// crate symbol holds yet. Its reason (of an `n/a` row, or of an `uncovered` word) is `GAP(<closer>): <reason>`, with a closer of
/// [`GAP_CLOSERS`]. The result maps each gap to its closer. A gap of an `n/a` row is keyed `<file>:<line>: `<span>``; a gap of a word adds ` <word>`. Like PENDING, the set
/// of gaps only shrinks against the base ([`check_gaps`]).
pub fn gaps(file: &str, table: &Table) -> (BTreeMap<String, String>, Vec<String>) {
    let marker = Regex::new(r"^GAP\(([a-z0-9]+)\): \S").expect("regex");
    let mut keys = BTreeMap::new();
    let mut problems = Vec::new();
    let mut note = |key: String, reason: &str| {
        if !reason.contains("GAP") {
            return;
        }
        match marker.captures(reason) {
            Some(c) if GAP_CLOSERS.contains(&&c[1]) => {
                keys.insert(key, c[1].to_string());
            }
            _ => problems.push(format!(
                "{key}: a gap reason is `GAP(<closer>): <reason>`, with a closer of {}",
                GAP_CLOSERS.join(", ")
            )),
        }
    };
    for (line, by_span) in &table.rows {
        for (span, row) in by_span {
            let at = format!("{file}:{line}: `{span}`");
            match row {
                Row::NotApplicable(reason) => {
                    note(at, reason.strip_prefix("n/a:").unwrap_or(reason).trim())
                }
                Row::Partial(p) => {
                    for (word, reason) in &p.uncovered {
                        note(format!("{at} {word}"), reason.trim());
                    }
                }
                Row::Symbols(_) => {}
            }
        }
    }
    (keys, problems)
}

/// The problems of the gaps against the base: a gap that the base does not have fails. `base` is `None` when the base has no
/// [`SOURCE`] (the adoption of the check).
pub fn check_gaps(
    gaps: &BTreeMap<String, String>,
    base: Option<&BTreeMap<String, String>>,
) -> Vec<String> {
    let Some(base) = base else {
        return Vec::new();
    };
    gaps.keys()
        .filter(|g| !base.contains_key(*g))
        .map(|g| {
            format!(
                "{g}: a new crate gap (the gap list only shrinks; add the crate symbol instead)"
            )
        })
        .collect()
}

/// The names of the PENDING const in a source text of this file, or `None` when the text has no PENDING const.
pub fn pending_names(source: &str) -> Option<BTreeSet<String>> {
    let start = source.find("const PENDING: &[(&str, &str)] = &[")?;
    let block = &source[start..];
    let block = &block[..block.find("];")?];
    let entry = Regex::new(r#"\(\s*"([^"]+)",\s*"[0-9a-f]{64}",?\s*\)"#).expect("regex");
    Some(
        entry
            .captures_iter(block)
            .map(|c| c[1].to_string())
            .collect(),
    )
}

/// This file, whose PENDING const at the base bounds the current one.
const SOURCE: &str = "xtask/src/crate_symbols.rs";

/// The pending names that the base allows. A base without this file is the adoption of the check: [`BOOTSTRAP`]. A base with this
/// file must have a PENDING const that [`pending_names`] reads; a damaged or changed format fails, and is never read as adoption.
pub fn allowed_pending(base_source: Option<&str>) -> Result<BTreeSet<String>> {
    match base_source {
        None => Ok(BOOTSTRAP.iter().map(|s| s.to_string()).collect()),
        Some(source) => {
            pending_names(source).context("the base has no PENDING const that this check can read")
        }
    }
}

/// The problems of the set of tables. `listed` holds each file of contracts.toml with the sha256 of its bytes; `tables` holds the files
/// that have a table; `allowed` holds the pending names of the base (or [`BOOTSTRAP`] when the base has no PENDING const), if a base
/// was given.
pub fn check_set(
    listed: &BTreeMap<String, String>,
    tables: &BTreeSet<String>,
    pending: &[(&str, &str)],
    allowed: Option<&BTreeSet<String>>,
) -> Vec<String> {
    let mut problems = Vec::new();
    for (file, hash) in pending {
        if allowed.is_some_and(|a| !a.contains(*file)) {
            problems.push(format!(
                "PENDING names {file}, which the base's PENDING does not name (the list only shrinks)"
            ));
        }
        match listed.get(*file) {
            None => problems.push(format!("PENDING names {file}, which contracts.toml does not list")),
            Some(actual) if actual != hash => problems.push(format!(
                "PENDING names {file} with sha256 {hash}, but the file has sha256 {actual} (a pending file never changes)"
            )),
            Some(_) if tables.contains(*file) => problems.push(format!(
                "{file} has a table and is still pending (delete its PENDING entry in xtask/src/crate_symbols.rs)"
            )),
            Some(_) => {}
        }
    }
    for file in listed.keys() {
        if !tables.contains(file) && !pending.iter().any(|(p, _)| p == file) {
            problems.push(format!(
                "{file} is in contracts.toml and has no table in {TABLES}/"
            ));
        }
    }
    for file in tables {
        if !listed.contains_key(file) {
            problems.push(format!(
                "{TABLES}/ has a table of {file}, which contracts.toml does not list"
            ));
        }
    }
    problems
}

// ------------------------------------------------------------------ the source index

/// What one item of a crate is, as far as a declaration can name it.
#[derive(Debug, Clone, PartialEq)]
pub enum Def {
    /// A struct with its named fields.
    Struct(BTreeSet<String>),
    /// An enum with its variants and each variant's named fields.
    Enum(BTreeMap<String, BTreeSet<String>>),
    /// A trait with its associated fns, consts and types.
    Trait(BTreeSet<String>),
    /// A const, static, fn, type alias or union: no member is declarable.
    Plain,
}

/// One definition of a name, with the module path where it is defined (`[]` for the crate root).
#[derive(Debug, Clone, PartialEq)]
pub struct Located {
    pub module: Vec<String>,
    pub def: Def,
}

/// The items of one crate by name (a name may occur more than once, in different modules), the associated items of each `impl` self
/// type keyed by the type's own (module, name), and the `macro_rules!` definitions that the template rule of [`item_macro`] expands.
/// One binding of a `use` declaration, with its path as written (`crate::`, `self::`, `super::` or relative to its module).
#[derive(Debug, Clone)]
enum Use {
    /// `use a::B;` or `use a::B as C;`: the name in scope (`B` or `C`) and the path it names.
    Named(String, Vec<String>),
    /// `use a::*;`: the path of the module.
    Glob(Vec<String>),
}

/// The bindings of one `use` tree, under the path `prefix`.
fn use_bindings(tree: &syn::UseTree, prefix: &mut Vec<String>, out: &mut Vec<Use>) {
    match tree {
        syn::UseTree::Path(p) => {
            prefix.push(p.ident.to_string());
            use_bindings(&p.tree, prefix, out);
            prefix.pop();
        }
        syn::UseTree::Name(n) if n.ident == "self" => {
            if let Some(last) = prefix.last() {
                out.push(Use::Named(last.clone(), prefix.clone()));
            }
        }
        syn::UseTree::Name(n) => {
            let path = [&prefix[..], &[n.ident.to_string()]].concat();
            out.push(Use::Named(n.ident.to_string(), path));
        }
        syn::UseTree::Rename(r) if r.rename != "_" => {
            let name = if r.ident == "self" {
                prefix.clone()
            } else {
                [&prefix[..], &[r.ident.to_string()]].concat()
            };
            out.push(Use::Named(r.rename.to_string(), name));
        }
        syn::UseTree::Rename(_) => {}
        syn::UseTree::Glob(_) => out.push(Use::Glob(prefix.clone())),
        syn::UseTree::Group(g) => {
            for tree in &g.items {
                use_bindings(tree, prefix, out);
            }
        }
    }
}

/// How many `use` hops a name may take to its definition. A longer chain (or a cycle) does not resolve.
const USE_DEPTH: usize = 16;

#[derive(Debug, Default)]
pub struct Index {
    pub items: BTreeMap<String, Vec<Located>>,
    pub assoc: BTreeMap<(Vec<String>, String), BTreeSet<String>>,
    /// The `impl` blocks, owner not yet resolved: (the impl's module, the self-type path, its members).
    impls: Vec<(Vec<String>, Vec<String>, BTreeSet<String>)>,
    /// Every type definition by (module, name), private ones included: an `impl` names its owner through them. Only
    /// [`Index::items`] is the crate's API.
    defined: BTreeSet<(Vec<String>, String)>,
    /// Every module path read (`[]` is the crate root).
    modules: BTreeSet<Vec<String>>,
    /// The `use` declarations of each module.
    uses: BTreeMap<Vec<String>, Vec<Use>>,
    macros: BTreeMap<String, TokenStream>,
    /// Template-macro invocations, expanded once the whole crate is read: a macro may be used in a module that comes before its
    /// definition in module order (through `use`).
    pending: Vec<(String, TokenStream, Vec<String>)>,
    /// The module path of the items being read.
    module: Vec<String>,
    /// The module files already read. A module that `#[cfg(feature)] pub mod x;` and `#[cfg(not(feature))] mod x;` both declare is
    /// one module, read once.
    read: BTreeSet<PathBuf>,
}

impl Index {
    /// Indexes the crate whose `lib.rs` is in `src`, following its module tree. `#[cfg(test)]` modules are not read.
    pub fn of_crate(src: &Path) -> Result<Index> {
        let mut index = Index::default();
        index.read_module_file(&src.join("lib.rs"), src)?;
        index.expand_pending()?;
        index.resolve_impls();
        Ok(index)
    }

    /// Indexes one source text, with no module files (for tests).
    #[cfg(test)]
    pub fn of_source(text: &str) -> Result<Index> {
        let mut index = Index::default();
        let file = syn::parse_file(text)?;
        index.items_of(&file.items, None)?;
        index.expand_pending()?;
        index.resolve_impls();
        Ok(index)
    }

    /// Expands each template-macro invocation with its macro's transcriber, and indexes the result in the invocation's module.
    fn expand_pending(&mut self) -> Result<()> {
        for (name, tokens, module) in std::mem::take(&mut self.pending) {
            let definition =
                self.macros.get(&name).cloned().with_context(|| {
                    format!("{name}! has no macro_rules! definition in the crate")
                })?;
            let expanded =
                expand_template(&definition, &tokens).with_context(|| format!("expand {name}!"))?;
            let file: syn::File =
                syn::parse2(expanded).with_context(|| format!("parse the expansion of {name}!"))?;
            let outer = std::mem::replace(&mut self.module, module);
            let read = self.items_of(&file.items, None);
            self.module = outer;
            read?;
        }
        Ok(())
    }

    /// Gives each `impl` block's members to the type that its self type names, found as Rust finds it: the path from the impl's
    /// module through its definitions (private ones included) and its `use` bindings. An owner that cannot be proved this way gets
    /// nothing: its members do not resolve. A private owner keeps its members out of the API, because it is not in `items`.
    fn resolve_impls(&mut self) {
        for (module, path, members) in std::mem::take(&mut self.impls) {
            if let Some(key) = self.owner(&module, &path, USE_DEPTH) {
                self.assoc.entry(key).or_default().extend(members);
            }
        }
    }

    /// The (module, name) of the type that `path` names in `module`.
    fn owner(
        &self,
        module: &[String],
        path: &[String],
        depth: usize,
    ) -> Option<(Vec<String>, String)> {
        let (name, qualifier) = path.split_last()?;
        if qualifier.is_empty() {
            return self.lookup(module, name, depth);
        }
        let m = self.module_of(module, qualifier, depth)?;
        self.lookup(&m, name, depth)
    }

    /// The module that the path `qualifier` names in `module`, or `None` when it is not a module of this crate.
    fn module_of(
        &self,
        module: &[String],
        qualifier: &[String],
        depth: usize,
    ) -> Option<Vec<String>> {
        let depth = depth.checked_sub(1)?;
        let (first, rest) = qualifier.split_first()?;
        let mut rest = rest;
        let mut m: Vec<String> = match first.as_str() {
            "crate" => Vec::new(),
            "self" => module.to_vec(),
            "super" => {
                // `super::super::x`: each further `super` goes up one more module.
                let mut m = module.split_last()?.1.to_vec();
                while let Some((_, tail)) = rest.split_first().filter(|(s, _)| *s == "super") {
                    m.pop()?;
                    rest = tail;
                }
                m
            }
            _ => {
                let child = [module, std::slice::from_ref(first)].concat();
                if self.modules.contains(&child) {
                    child
                } else {
                    let named: Option<Vec<Vec<String>>> = self
                        .uses_of(module)
                        .filter_map(|u| match u {
                            Use::Named(alias, path) if alias == first => {
                                Some(self.module_of(module, path, depth))
                            }
                            _ => None,
                        })
                        .collect();
                    match &named?[..] {
                        [one] => one.clone(),
                        _ => return None,
                    }
                }
            }
        };
        m.extend_from_slice(rest);
        self.modules.contains(&m).then_some(m)
    }

    /// The definition of the type `name` in scope in `module`: one defined there, else the one that a `use` binding names, else the
    /// one that exactly one glob import gives.
    fn lookup(&self, module: &[String], name: &str, depth: usize) -> Option<(Vec<String>, String)> {
        let depth = depth.checked_sub(1)?;
        let key = (module.to_vec(), name.to_string());
        if self.defined.contains(&key) {
            return Some(key);
        }
        let unique = |found: Vec<(Vec<String>, String)>| {
            let found: BTreeSet<_> = found.into_iter().collect();
            (found.len() == 1)
                .then(|| found.into_iter().next())
                .flatten()
        };
        // A named binding shadows every glob import, also when its target is outside this crate: an unresolved one gives no owner.
        let named: Vec<_> = self
            .uses_of(module)
            .filter_map(|u| match u {
                Use::Named(alias, path) if alias == name => Some(self.owner(module, path, depth)),
                _ => None,
            })
            .collect();
        if !named.is_empty() {
            return unique(named.into_iter().collect::<Option<_>>()?);
        }
        let globbed: Vec<_> = self
            .uses_of(module)
            .filter_map(|u| match u {
                Use::Glob(path) => self
                    .module_of(module, path, depth)
                    .and_then(|m| self.lookup(&m, name, depth)),
                _ => None,
            })
            .collect();
        unique(globbed)
    }

    fn uses_of(&self, module: &[String]) -> impl Iterator<Item = &Use> {
        self.uses.get(module).into_iter().flatten()
    }

    /// Reads a module file. `child_dir` is where its `mod x;` children live.
    fn read_module_file(&mut self, path: &Path, child_dir: &Path) -> Result<()> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
        let file = syn::parse_file(&text).with_context(|| format!("parse {}", path.display()))?;
        self.items_of(&file.items, Some(child_dir))
            .with_context(|| format!("index {}", path.display()))
    }

    fn child_module(&mut self, name: &str, dir: Option<&Path>) -> Result<()> {
        let Some(dir) = dir else {
            bail!("module file `{name}` outside a crate");
        };
        let flat = dir.join(format!("{name}.rs"));
        let nested = dir.join(name).join("mod.rs");
        let file = if flat.is_file() {
            flat
        } else if nested.is_file() {
            nested
        } else {
            bail!("no file for module `{name}` in {}", dir.display())
        };
        if !self.read.insert(file.clone()) {
            return Ok(());
        }
        self.module.push(name.to_string());
        let read = self.read_module_file(&file, &dir.join(name));
        self.module.pop();
        read
    }

    fn add(&mut self, name: String, def: Def) {
        let module = self.module.clone();
        self.items
            .entry(name)
            .or_default()
            .push(Located { module, def });
    }

    fn items_of(&mut self, items: &[syn::Item], dir: Option<&Path>) -> Result<()> {
        self.modules.insert(self.module.clone());
        for item in items {
            if is_test_only(item_attrs(item)) {
                continue;
            }
            let type_name = match item {
                syn::Item::Struct(i) => Some(&i.ident),
                syn::Item::Enum(i) => Some(&i.ident),
                syn::Item::Union(i) => Some(&i.ident),
                syn::Item::Type(i) => Some(&i.ident),
                syn::Item::Trait(i) => Some(&i.ident),
                _ => None,
            };
            if let Some(name) = type_name {
                self.defined.insert((self.module.clone(), name.to_string()));
            }
            if let syn::Item::Use(u) = item {
                let uses = self.uses.entry(self.module.clone()).or_default();
                use_bindings(&u.tree, &mut Vec::new(), uses);
            }
            if !is_public(item) {
                continue;
            }
            match item {
                syn::Item::Struct(s) => {
                    let fields = named_fields(&s.fields, false);
                    self.add(s.ident.to_string(), Def::Struct(fields));
                }
                syn::Item::Enum(e) => {
                    let variants = e
                        .variants
                        .iter()
                        .filter(|v| !is_test_only(&v.attrs))
                        .flat_map(|v| {
                            let fields = named_fields(&v.fields, true);
                            let wire = serde_rename(&v.attrs).map(|w| (w, fields.clone()));
                            std::iter::once((v.ident.to_string(), fields)).chain(wire)
                        })
                        .collect();
                    self.add(e.ident.to_string(), Def::Enum(variants));
                }
                syn::Item::Const(c) => self.add(c.ident.to_string(), Def::Plain),
                syn::Item::Static(s) => self.add(s.ident.to_string(), Def::Plain),
                syn::Item::Fn(f) => self.add(f.sig.ident.to_string(), Def::Plain),
                syn::Item::Type(t) => self.add(t.ident.to_string(), Def::Plain),
                syn::Item::Trait(t) => {
                    let members = t
                        .items
                        .iter()
                        .filter_map(|m| match m {
                            syn::TraitItem::Fn(f) if !is_test_only(&f.attrs) => {
                                Some(f.sig.ident.to_string())
                            }
                            syn::TraitItem::Const(c) if !is_test_only(&c.attrs) => {
                                Some(c.ident.to_string())
                            }
                            syn::TraitItem::Type(t) if !is_test_only(&t.attrs) => {
                                Some(t.ident.to_string())
                            }
                            _ => None,
                        })
                        .collect();
                    self.add(t.ident.to_string(), Def::Trait(members));
                }
                syn::Item::Union(u) => self.add(u.ident.to_string(), Def::Plain),
                syn::Item::Impl(i) => {
                    if let Some(path) = self_type_path(&i.self_ty) {
                        // An item of a trait impl is as public as the trait; an item of an inherent impl needs `pub`.
                        let open = |vis: &syn::Visibility| i.trait_.is_some() || is_pub(vis);
                        let members = i
                            .items
                            .iter()
                            .filter_map(|m| match m {
                                syn::ImplItem::Fn(f) if !is_test_only(&f.attrs) && open(&f.vis) => {
                                    Some(f.sig.ident.to_string())
                                }
                                syn::ImplItem::Const(c)
                                    if !is_test_only(&c.attrs) && open(&c.vis) =>
                                {
                                    Some(c.ident.to_string())
                                }
                                syn::ImplItem::Type(t)
                                    if !is_test_only(&t.attrs) && open(&t.vis) =>
                                {
                                    Some(t.ident.to_string())
                                }
                                _ => None,
                            })
                            .collect();
                        self.impls.push((self.module.clone(), path, members));
                    }
                }
                syn::Item::Mod(m) => match &m.content {
                    Some((_, inner)) => {
                        self.module.push(m.ident.to_string());
                        let read = self.items_of(inner, None);
                        self.module.pop();
                        read?;
                    }
                    None => self.child_module(&m.ident.to_string(), dir)?,
                },
                syn::Item::Macro(m) => {
                    // `macro_rules! name { .. }` defines a macro: its body is a pattern, not items. It is kept for the template rule.
                    if let Some(name) = &m.ident {
                        self.macros.insert(name.to_string(), m.mac.tokens.clone());
                        continue;
                    }
                    let name = m
                        .mac
                        .path
                        .segments
                        .last()
                        .map(|s| s.ident.to_string())
                        .unwrap_or_default();
                    item_macro(self, &name, m.mac.tokens.clone(), dir)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

/// The rule of each item-level macro of the contract crates. A macro with no rule is an error: the guard cannot see what it defines.
fn item_macro(
    index: &mut Index,
    name: &str,
    tokens: TokenStream,
    dir: Option<&Path>,
) -> Result<()> {
    match name {
        // hub: `open_enum! { pub enum X { A = "a", .. } }`, real enums with a lenient serde impl.
        "open_enum" => {
            let file: syn::File = syn::parse2(tokens).context("parse the body of open_enum!")?;
            index.items_of(&file.items, dir)
        }
        // core: `opaque_token! { /// doc  Name, /// doc  Name, }`: each name is a tuple struct `Name(pub u64)`, with no named field.
        "opaque_token" => {
            for ident in top_level_idents(&tokens) {
                index.add(ident, Def::Struct(BTreeSet::new()));
            }
            Ok(())
        }
        // plugin: `queued_admission!(/// doc  Name, KindTag)` and `const_str!(/// doc  Name, "value")`: one item each, written by the
        // macro's own transcriber. The rule expands that transcriber with the invocation's arguments and reads the result, so the
        // generated fields (`SafeWriteAdmission::kind`) resolve, and a field removed from the macro stops resolving.
        "queued_admission" | "const_str" => {
            let module = index.module.clone();
            index.pending.push((name.to_string(), tokens, module));
            Ok(())
        }
        // plugin: `limits! { counts { a: ..; } extras { b: ..; } optional { c: ..; } }` defines `PluginLimits` with one field per
        // entry, and `LIMIT_SPECS`.
        "limits" => {
            let mut fields = BTreeSet::new();
            for tree in tokens {
                if let TokenTree::Group(g) = tree {
                    if g.delimiter() == Delimiter::Brace {
                        fields.extend(entry_names(&g.stream()));
                    }
                }
            }
            index.add("PluginLimits".into(), Def::Struct(fields));
            index.add("LIMIT_SPECS".into(), Def::Plain);
            Ok(())
        }
        // core, codec, hub: `lenient! { Type => [..] }` writes a lenient Deserialize impl and defines no item.
        "lenient" => Ok(()),
        // plugin: `unstable_modules!(a, b, ..)` makes the modules public under the feature `unstable`. The plain `mod` items of
        // lib.rs already declare the same modules, so it defines nothing new.
        "unstable_modules" => Ok(()),
        other => bail!(
            "item macro `{other}!` has no crate-symbols rule (add one to xtask/src/crate_symbols.rs `item_macro`)"
        ),
    }
}

/// The idents at the top level of a macro body, skipping attribute groups (`#[..]`, doc comments) and every other group.
fn top_level_idents(tokens: &TokenStream) -> Vec<String> {
    tokens
        .clone()
        .into_iter()
        .filter_map(|t| match t {
            TokenTree::Ident(i) => Some(i.to_string()),
            _ => None,
        })
        .collect()
}

/// The first ident of each `;`-separated entry of a `name: ..;` list.
fn entry_names(tokens: &TokenStream) -> Vec<String> {
    let mut names = Vec::new();
    let mut at_start = true;
    for tree in tokens.clone() {
        match &tree {
            TokenTree::Punct(p) if p.as_char() == ';' => at_start = true,
            TokenTree::Ident(i) if at_start => {
                names.push(i.to_string());
                at_start = false;
            }
            _ => at_start = false,
        }
    }
    names
}

/// The named fields of a struct (`pub` ones only) or of an enum variant (`variant`: all, as the enum is).
fn named_fields(fields: &syn::Fields, variant: bool) -> BTreeSet<String> {
    match fields {
        syn::Fields::Named(n) => n
            .named
            .iter()
            .filter(|f| !is_test_only(&f.attrs) && (variant || is_pub(&f.vis)))
            .flat_map(|f| {
                let name = f.ident.as_ref().map(ToString::to_string);
                name.into_iter().chain(serde_rename(&f.attrs))
            })
            .collect(),
        _ => BTreeSet::new(),
    }
}

/// The wire name of `#[serde(rename = "x")]`. A member resolves by its Rust name and by this name.
fn serde_rename(attrs: &[syn::Attribute]) -> Option<String> {
    let mut wire = None;
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") && meta.input.peek(syn::Token![=]) {
                let lit: syn::LitStr = meta.value()?.parse()?;
                wire = Some(lit.value());
            } else if meta.input.peek(syn::Token![=]) {
                // Skip the value of another key, e.g. `default = "f"`; the keys after it are still read.
                let _: syn::Expr = meta.value()?.parse()?;
            } else if meta.input.peek(syn::token::Paren) {
                // Skip a list, e.g. `rename(serialize = "x")`: it gives no single wire name.
                let content;
                syn::parenthesized!(content in meta.input);
                let _: TokenStream = content.parse()?;
            }
            Ok(())
        });
    }
    wire
}

/// The path of an `impl` self type, without generic arguments (`rows::Row<T>` is `["rows", "Row"]`).
fn self_type_path(ty: &syn::Type) -> Option<Vec<String>> {
    match ty {
        syn::Type::Path(p) if p.qself.is_none() => Some(
            p.path
                .segments
                .iter()
                .map(|s| s.ident.to_string())
                .collect(),
        ),
        _ => None,
    }
}

/// `pub` with no restriction: `pub(crate)` and private items are not the crate's API.
fn is_pub(vis: &syn::Visibility) -> bool {
    matches!(vis, syn::Visibility::Public(_))
}

/// Whether an item is part of the crate's API. A module, an impl and a macro call are read: their items carry their own `pub`.
fn is_public(item: &syn::Item) -> bool {
    match item {
        syn::Item::Const(i) => is_pub(&i.vis),
        syn::Item::Enum(i) => is_pub(&i.vis),
        syn::Item::Fn(i) => is_pub(&i.vis),
        syn::Item::Static(i) => is_pub(&i.vis),
        syn::Item::Struct(i) => is_pub(&i.vis),
        syn::Item::Trait(i) => is_pub(&i.vis),
        syn::Item::Type(i) => is_pub(&i.vis),
        syn::Item::Union(i) => is_pub(&i.vis),
        _ => true,
    }
}

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
        _ => &[],
    }
}

/// A `#[cfg(..)]` whose predicate needs `test`: the item or member exists only in the crate's own tests. A predicate that this
/// function cannot read counts as test-only, so the guard fails closed (the member does not resolve).
fn is_test_only(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().filter(|a| a.path().is_ident("cfg")).any(|a| {
        a.parse_args::<syn::Meta>()
            .map_or(true, |m| cfg_needs_test(&m))
    })
}

/// Whether a cfg predicate can hold only when `test` is set. `test` does; `all(..)` does when one term does; `any(..)` when every
/// term does; `not(p)` never does when `p` is a plain name or `name = "value"`. Another form is not read: it counts as test-only.
fn cfg_needs_test(meta: &syn::Meta) -> bool {
    use syn::punctuated::Punctuated;
    match meta {
        syn::Meta::Path(p) => p.is_ident("test"),
        syn::Meta::NameValue(_) => false,
        syn::Meta::List(l) => {
            let Ok(terms) =
                l.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
            else {
                return true;
            };
            if l.path.is_ident("all") {
                terms.iter().any(cfg_needs_test)
            } else if l.path.is_ident("any") {
                !terms.is_empty() && terms.iter().all(cfg_needs_test)
            } else if l.path.is_ident("not") {
                !matches!(
                    terms.iter().collect::<Vec<_>>()[..],
                    [syn::Meta::Path(_) | syn::Meta::NameValue(_)]
                )
            } else {
                true
            }
        }
    }
}

/// Resolves `[module::]*Item[::Member[::Field]]` in one crate's index. `Err` is the reason it does not resolve.
///
/// The item is the first segment that names an item defined in a module whose path ends with the segments before it. A name with more
/// than one definition there is ambiguous and does not resolve: the declaration must name the module (`rows::WorktreeRow`).
pub fn resolve_path(index: &Index, parts: &[&str]) -> std::result::Result<(), String> {
    for k in 0..parts.len() {
        let (modules, rest) = parts.split_at(k);
        let item = rest[0];
        let Some(all) = index.items.get(item) else {
            continue;
        };
        let defs: Vec<&Located> = all
            .iter()
            .filter(|l| l.module.ends_with_path(modules))
            .collect();
        if defs.is_empty() {
            continue;
        }
        if defs.len() > 1 {
            let places: Vec<String> = defs.iter().map(|l| l.module.join("::")).collect();
            return Err(format!(
                "`{item}` is defined {} times (modules: {}); name the module",
                defs.len(),
                places.join(", ")
            ));
        }
        return resolve_member(index, item, defs[0], &rest[1..]);
    }
    Err(format!("no item `{}`", parts.join("::")))
}

trait EndsWithPath {
    fn ends_with_path(&self, tail: &[&str]) -> bool;
}

impl EndsWithPath for Vec<String> {
    fn ends_with_path(&self, tail: &[&str]) -> bool {
        tail.len() <= self.len()
            && self[self.len() - tail.len()..]
                .iter()
                .zip(tail)
                .all(|(a, b)| a == b)
    }
}

fn resolve_member(
    index: &Index,
    item: &str,
    located: &Located,
    rest: &[&str],
) -> std::result::Result<(), String> {
    let Some((member, rest)) = rest.split_first() else {
        return Ok(());
    };
    let def = &located.def;
    let assoc = index
        .assoc
        .get(&(located.module.clone(), item.to_string()))
        .is_some_and(|a| a.contains(*member));
    let found = match (def, rest) {
        (Def::Struct(fields), []) => fields.contains(*member) || assoc,
        (Def::Enum(variants), []) => variants.contains_key(*member) || assoc,
        (Def::Enum(variants), [field]) => variants
            .get(*member)
            .is_some_and(|fields| fields.contains(*field)),
        (Def::Trait(members), []) => members.contains(*member),
        (Def::Plain, []) => assoc,
        _ => false,
    };
    if found {
        return Ok(());
    }
    Err(match rest {
        [] => format!("`{item}` has no field, variant or associated item `{member}`"),
        [field] => format!("`{item}::{member}` is not a variant with a field `{field}`"),
        _ => "a path names at most a member and its field after the item".into(),
    })
}

/// Expands a one-arm `macro_rules!` whose pattern is `$(#[$m:meta])*` followed by comma-separated `$var:fragment`s, with the
/// arguments of one invocation. The attribute repetition is dropped (doc comments only), and each `$var` is replaced by its argument.
fn expand_template(definition: &TokenStream, invocation: &TokenStream) -> Result<TokenStream> {
    let arm: Vec<TokenTree> = definition.clone().into_iter().collect();
    let (Some(TokenTree::Group(pattern)), Some(TokenTree::Group(body))) = (arm.first(), arm.get(3))
    else {
        bail!("not a one-arm macro: `(pattern) => {{ body }}`");
    };
    // The metavariables of the pattern, in order, after the attribute repetition.
    let mut vars = Vec::new();
    let mut tokens = pattern.stream().into_iter().peekable();
    while let Some(t) = tokens.next() {
        if let TokenTree::Punct(p) = &t {
            if p.as_char() == '$' {
                match tokens.next() {
                    Some(TokenTree::Ident(v)) => vars.push(v.to_string()),
                    Some(TokenTree::Group(_)) => {
                        tokens.next(); // the repetition operator
                    }
                    _ => bail!("unexpected `$` in the pattern"),
                }
            }
        }
    }
    // The invocation's arguments: its tokens without attributes, split at top-level commas.
    let mut args: Vec<Vec<TokenTree>> = vec![Vec::new()];
    let mut tokens = invocation.clone().into_iter().peekable();
    while let Some(t) = tokens.next() {
        match &t {
            TokenTree::Punct(p) if p.as_char() == '#' => {
                tokens.next(); // the attribute's [..] group
            }
            TokenTree::Punct(p) if p.as_char() == ',' => args.push(Vec::new()),
            _ => args.last_mut().expect("one argument").push(t),
        }
    }
    args.retain(|a| !a.is_empty());
    if args.len() != vars.len() {
        bail!(
            "{} argument(s) for {} metavariable(s)",
            args.len(),
            vars.len()
        );
    }
    let map: BTreeMap<String, Vec<TokenTree>> = vars.into_iter().zip(args).collect();
    substitute(&body.stream(), &map)
}

fn substitute(stream: &TokenStream, map: &BTreeMap<String, Vec<TokenTree>>) -> Result<TokenStream> {
    let mut out = Vec::new();
    let mut tokens = stream.clone().into_iter().peekable();
    while let Some(t) = tokens.next() {
        match t {
            TokenTree::Punct(p) if p.as_char() == '$' => match tokens.next() {
                Some(TokenTree::Ident(v)) => out.extend(
                    map.get(&v.to_string())
                        .with_context(|| format!("unknown metavariable ${v}"))?
                        .iter()
                        .cloned(),
                ),
                Some(TokenTree::Group(_)) => {
                    tokens.next(); // a repetition (the attributes): dropped
                }
                _ => bail!("unexpected `$` in the transcriber"),
            },
            TokenTree::Group(g) => {
                let mut group =
                    proc_macro2::Group::new(g.delimiter(), substitute(&g.stream(), map)?);
                group.set_span(g.span());
                out.push(TokenTree::Group(group));
            }
            other => out.push(other),
        }
    }
    Ok(out.into_iter().collect())
}

// ------------------------------------------------------------------ registry data

/// Resolves `<scheme>:<arguments>` against the registries of the contract crates. `Err` is the reason it does not resolve.
pub fn resolve_data(symbol: &str) -> std::result::Result<(), String> {
    use botster_hub_contract::prelude as hub;
    use botster_plugin_contract::prelude as plugin;
    let (scheme, args) = symbol
        .split_once(':')
        .ok_or_else(|| format!("`{symbol}` is neither a path nor `<scheme>:<arguments>`"))?;
    // A name of the CLI spec (`cli_token`): a command, a flag, a key, a status field, a message or a path.
    if scheme.starts_with("cli.") {
        return cli_token(symbol)
            .map(|_| ())
            .ok_or_else(|| format!("`{symbol}` is not a name of the CLI crate"));
    }
    let a: Vec<&str> = args.split(':').collect();
    let found = match (scheme, a.as_slice()) {
        // An error code of HP `CODES`, under its kind.
        ("plugin.code", [kind, code]) => plugin::CODES
            .iter()
            .any(|c| c.kind == *kind && c.code == *code),
        // A call of HP Appendix A (`CALLS`), optionally with a failure kind and a code of that failure.
        ("plugin.call", [call]) => plugin::CALLS.iter().any(|c| c.name == *call),
        ("plugin.call", [call, kind]) => plugin::CALLS
            .iter()
            .any(|c| c.name == *call && c.failures.iter().any(|f| f.kind == *kind)),
        ("plugin.call", [call, kind, code]) => plugin::CALLS.iter().any(|c| {
            c.name == *call
                && c.failures
                    .iter()
                    .any(|f| f.kind == *kind && f.codes.contains(code))
        }),
        // A parameter of `PluginLimits` (`LIMIT_SPECS`), optionally with its `hub.toml` table and mode.
        ("plugin.limit", [name]) => plugin::LIMIT_SPECS.iter().any(|l| l.name == *name),
        ("plugin.limit", [name, table, mode]) => plugin::LIMIT_SPECS.iter().any(|l| {
            l.name == *name
                && l.toml.is_some_and(|(t, m)| {
                    t == *table
                        && matches!(
                            (m, *mode),
                            (plugin::KeyMode::Live, "live") | (plugin::KeyMode::Restart, "restart")
                        )
                })
        }),
        // An event of the HP event table (`EVENTS`), as `<owner>/<name>`.
        ("plugin.event", [event]) => event
            .split_once('/')
            .is_some_and(|(o, n)| plugin::EVENTS.iter().any(|e| e.owner == o && e.name == n)),
        // A `wrong_state` reason of a call (`WRONG_STATE_REASONS`).
        ("plugin.wrong_state", [call, reason]) => plugin::WRONG_STATE_REASONS
            .iter()
            .any(|(c, r)| c == call && r.contains(reason)),
        // A value of a closed string list of the plugin crate, named by its const (`plugin_list`).
        ("plugin.value", [list, value]) => plugin_list(list)
            .ok_or_else(|| format!("`{list}` is not a string list of the plugin crate"))?
            .contains(value),
        // A state of an operation kind's state table (`STATE_TABLES`, HP A2-4, SW-2).
        ("plugin.state", [kind, state]) => plugin::STATE_TABLES
            .iter()
            .any(|t| t.kind == *kind && t.states.contains(state)),
        // A failure reason of a Hub kind's verb or action, as `<kind>.<verb or action>` and `code:reason` or `code`.
        ("hub.reason", [target, rest @ ..]) if !rest.is_empty() => {
            let reason = rest.join(":");
            target.split_once('.').is_some_and(|(kind, name)| {
                hub::kind_spec(kind).is_some_and(|k| {
                    k.verbs
                        .iter()
                        .any(|v| v.verb == name && v.reasons.contains(&reason.as_str()))
                        || k.actions
                            .iter()
                            .any(|x| x.name == name && x.reasons.contains(&reason.as_str()))
                })
            })
        }
        // A Hub-owned kind of HC section 6 (`KINDS`).
        ("hub.kind", [kind]) => hub::kind_spec(kind).is_some(),
        // A verb or an action of a Hub kind.
        ("hub.action", [target]) => target.split_once('.').is_some_and(|(kind, name)| {
            hub::kind_spec(kind).is_some_and(|k| {
                k.verbs.iter().any(|v| v.verb == name) || k.actions.iter().any(|x| x.name == name)
            })
        }),
        // An operation kind of A.7b, optionally with a `failed` reason.
        ("hub.operation", [kind]) => hub::operation_kind(kind).is_some(),
        ("hub.operation", [kind, reason]) => {
            hub::operation_kind(kind).is_some_and(|k| k.failed_reasons.contains(reason))
        }
        // An operation kind whose `result` is typed.
        ("hub.operation_result", [kind]) => {
            hub::operation_kind(kind).is_some_and(|k| k.result.is_some())
        }
        // A core field of a Hub kind (`KindSpec::core_fields`).
        ("hub.core_field", [kind, field]) => {
            hub::kind_spec(kind).is_some_and(|k| k.core_fields.contains(field))
        }
        // An operation state name that this version knows (A.7b).
        ("hub.operation_state", [name]) => hub::OPERATION_STATE_NAMES.contains(name),
        // A state of one operation kind (`OPERATION_KINDS`, A.7b).
        ("hub.operation_state", [kind, state]) => {
            hub::operation_kind(kind).is_some_and(|k| k.states.contains(state))
        }
        // A `failed` code of the Hub's ready line.
        ("hub.ready_failure", [code]) => hub::READY_FAILURE_CODES.contains(code),
        // A route feature of `attached.features` (Codec A.4, TS-1).
        ("codec.route_feature", [name]) => {
            botster_route_codec::prelude::ROUTE_FEATURES.contains(name)
        }
        // A v1 binding name of `transport.binding` (HC TB-0).
        ("hub.binding", [name]) => hub::BINDINGS.contains(name),
        // A value of a closed string list of the hub crate, named by its const (`hub_list`). A value may hold `:`
        // (`COMMON_REASONS` holds `<code>:<reason>`).
        ("hub.value", [list, value @ ..]) if !value.is_empty() => {
            hub_list(list).is_some_and(|l| l.contains(&value.join(":").as_str()))
        }
        // A range filter that a Hub kind declares in `describe` (HC VB-1, `RANGE_FILTERS`).
        ("hub.range_filter", [kind, name]) => hub::range_filters(kind).contains(name),
        // A `NamedKey` name of the closed 0.1 set (Core 5.1A, Codec A.5).
        ("codec.named_key", [name]) => botster_route_codec::prelude::named_key_names()
            .iter()
            .any(|n| n == name),
        // A view feature that the Hub knows (UI-VR-4).
        ("ui.feature", [name]) => botster_ui_contract::prelude::AdmissionContext::default()
            .features
            .iter()
            .any(|f| f == name),
        // A renderer feature of `ui_capabilities.features` (UI-CF-5, UI-CF-6).
        // A role segment of a generated child id (UI-ID-2, `ROLE_WORDS`).
        ("ui.role", [word]) => botster_ui_contract::prelude::ROLE_WORDS.contains(word),
        ("ui.client_feature", [name]) => {
            use botster_ui_contract::realize as r;
            [
                r::FEATURE_DISPLAY_METRIC,
                r::FEATURE_DISPLAY_BADGE,
                r::FEATURE_EXTERNAL_OPEN,
                r::FEATURE_EMBED,
            ]
            .contains(name)
        }
        // A prop that a node kind allows (`node::prop_allowed`, the per-kind schema of UI-NV-1 and UI-RZ-2a), with the kind's wire name.
        ("ui.prop", [kind, prop]) => serde_json::from_value::<
            botster_ui_contract::prelude::UiNodeKind,
        >(serde_json::json!(kind))
        .is_ok_and(|k| botster_ui_contract::node::prop_allowed(k, prop)),
        // A slot that a node kind allows (`node::slot_allowed`): a slot holds nodes, not a prop value (UI-NV-9).
        ("ui.slot", [kind, slot]) => serde_json::from_value::<
            botster_ui_contract::prelude::UiNodeKind,
        >(serde_json::json!(kind))
        .is_ok_and(|k| botster_ui_contract::node::slot_allowed(k, slot)),
        // A copy key of UI-NV-6 (`COPY_KEYS`), and an arg of a copy key that takes args (`COPY_KEY_ARGS`).
        ("ui.copy_key", [key]) => botster_ui_contract::prelude::COPY_KEYS.contains(key),
        ("ui.copy_key_arg", [key, arg]) => botster_ui_contract::prelude::COPY_KEY_ARGS
            .iter()
            .any(|(k, args)| k == key && args.contains(arg)),
        // An `absent_reason` (UI-RF-1, UI-NV-2, `ABSENT_REASONS`), a local error code (UI-FM-4, UI-NV-2, `LOCAL_ERROR_CODES`) and a
        // diagnostic code (UI-AD-2, UI-NV-4, `DIAGNOSTIC_CODES`).
        ("ui.absent_reason", [reason]) => {
            botster_ui_contract::prelude::ABSENT_REASONS.contains(reason)
        }
        ("ui.local_error", [code]) => {
            botster_ui_contract::prelude::LOCAL_ERROR_CODES.contains(code)
        }
        ("ui.diagnostic", [code]) => botster_ui_contract::prelude::DIAGNOSTIC_CODES.contains(code),
        _ => return Err(format!("unknown scheme or arity in `{symbol}`")),
    };
    if found {
        Ok(())
    } else {
        Err(format!("`{symbol}` is not in its registry"))
    }
}

/// The closed string lists of the plugin crate that `plugin.value` names, by their const name. A list of pairs gives its first members.
/// `LUA_GLOBALS` is `lua_globals()`: the base functions, the libraries and the other globals of HP Appendix B.
fn hub_list(name: &str) -> Option<&'static [&'static str]> {
    use botster_hub_contract::prelude as hub;
    Some(match name {
        "COMMON_REASONS" => hub::COMMON_REASONS,
        "CHECK_UPDATE_FAILURE_REASONS" => hub::CHECK_UPDATE_FAILURE_REASONS,
        "PACKAGE_OUTCOME_STATES" => hub::PACKAGE_OUTCOME_STATES,
        _ => return None,
    })
}

fn plugin_list(name: &str) -> Option<Vec<&'static str>> {
    use botster_plugin_contract::prelude as plugin;
    if name == "LUA_GLOBALS" {
        return Some(plugin::lua_globals());
    }
    let plain: &[&str] = match name {
        "NOT_WRITTEN_REASONS" => plugin::NOT_WRITTEN_REASONS,
        "WRITE_RESULT_REASONS" => plugin::WRITE_RESULT_REASONS,
        "OPERATION_KINDS" => plugin::OPERATION_KINDS,
        "SPAWN_FAILED_REASONS" => plugin::SPAWN_FAILED_REASONS,
        "SAFE_WRITE_ABANDONED_REASONS" => plugin::SAFE_WRITE_ABANDONED_REASONS,
        "SAFE_WRITE_CANCELLED_REASONS" => plugin::SAFE_WRITE_CANCELLED_REASONS,
        "SAFE_WRITE_FAILED_REASONS" => plugin::SAFE_WRITE_FAILED_REASONS,
        "SAFE_WRITE_UNKNOWN_REASONS" => plugin::SAFE_WRITE_UNKNOWN_REASONS,
        "LUA_LOCAL_NAMES" => plugin::LUA_LOCAL_NAMES,
        "LUA_REGISTRATION_NAMES" => plugin::LUA_REGISTRATION_NAMES,
        "LUA_LOG_SUGAR" => plugin::LUA_LOG_SUGAR,
        "LUA_UNAVAILABLE" => plugin::LUA_UNAVAILABLE,
        "SAFE_WRITE_PROBE" => std::slice::from_ref(&plugin::SAFE_WRITE_PROBE),
        _ => {
            let pairs: &[(&str, &str)] = match name {
                "LOAD_FAILURE_CODES" => plugin::LOAD_FAILURE_CODES,
                "FAULT_CODES" => plugin::FAULT_CODES,
                "INGRESS_STATUSES" => plugin::INGRESS_STATUSES,
                "KIND_MEANINGS" => plugin::KIND_MEANINGS,
                _ => return None,
            };
            return Some(pairs.iter().map(|(first, _)| *first).collect());
        }
    };
    Some(plain.to_vec())
}

// ------------------------------------------------------------------ the command

/// Resolves one declared symbol with the crate indexes.
pub fn resolve(indexes: &BTreeMap<&str, Index>, symbol: &str) -> std::result::Result<(), String> {
    if let Some((krate, path)) = symbol.split_once("::") {
        let index = indexes.get(krate).ok_or_else(|| {
            format!("unknown crate `{krate}` (one of core, codec, hub, plugin, ui)")
        })?;
        let parts: Vec<&str> = path.split("::").collect();
        return resolve_path(index, &parts);
    }
    resolve_data(symbol)
}

/// The known conformance ids: the ledger and the withdrawn ids.
fn test_ids(root: &Path) -> Result<BTreeSet<String>> {
    #[derive(Deserialize)]
    struct Ledger {
        ids: Vec<LedgerId>,
    }
    #[derive(Deserialize)]
    struct LedgerId {
        id: String,
    }
    let path = root.join(crate::ledger::LEDGER_PATH);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let ledger: Ledger =
        serde_json::from_str(&text).with_context(|| format!("parse {}", path.display()))?;
    let mut ids: BTreeSet<String> = ledger.ids.into_iter().map(|i| i.id).collect();
    let path = root.join(crate::status::WITHDRAWN_PATH);
    let text =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    ids.extend(
        text.lines()
            .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            .filter_map(|l| l.split_whitespace().next())
            .map(str::to_string),
    );
    Ok(ids)
}

/// Candidate symbols for a word: each item, member and field of the crate indexes whose name equals the word (case and `_` ignored).
fn suggest(indexes: &BTreeMap<&str, Index>, word: &str) -> Vec<String> {
    let w = normal(word);
    let mut out = Vec::new();
    for (alias, index) in indexes {
        for (name, all) in &index.items {
            let qualify = all.len() > 1;
            for located in all {
                let item = if qualify && !located.module.is_empty() {
                    format!("{alias}::{}::{name}", located.module.join("::"))
                } else {
                    format!("{alias}::{name}")
                };
                if normal(name) == w {
                    out.push(item.clone());
                }
                let members: Vec<String> = match &located.def {
                    Def::Struct(fields) => fields.iter().cloned().collect(),
                    Def::Enum(variants) => variants
                        .iter()
                        .flat_map(|(v, fields)| {
                            std::iter::once(v.clone())
                                .chain(fields.iter().map(move |f| format!("{v}::{f}")))
                        })
                        .collect(),
                    Def::Trait(members) => members.iter().cloned().collect(),
                    Def::Plain => Vec::new(),
                };
                let assoc = index
                    .assoc
                    .get(&(located.module.clone(), name.clone()))
                    .into_iter()
                    .flatten()
                    .cloned();
                for member in members.into_iter().chain(assoc) {
                    if member
                        .rsplit("::")
                        .next()
                        .is_some_and(|last| normal(last) == w)
                    {
                        out.push(format!("{item}::{member}"));
                    }
                }
            }
        }
    }
    out.sort();
    out.dedup();
    out
}

/// Prints the missing rows of one file as TOML, each with its heading path and line, and candidate symbols for each word. The author
/// checks every row before the table takes it; the gate never runs this.
fn draft(
    root: &Path,
    config: &Config,
    indexes: &BTreeMap<&str, Index>,
    test_ids: &BTreeSet<String>,
    file: &str,
) -> Result<()> {
    let text = std::fs::read_to_string(root.join(&config.dir).join(file))
        .with_context(|| format!("read {file}"))?;
    let table = Table::load(root, file)?.unwrap_or_default();
    let mut line_seen = None;
    for ((line, span), place) in spans(&text) {
        let has = table
            .rows
            .get(&line.to_string())
            .is_some_and(|r| r.contains_key(&span));
        if has || !needs_row(&span, test_ids) {
            continue;
        }
        if line_seen != Some(line) {
            println!(
                "\n# {}\n# {}\n[rows.\"{line}\"]",
                place.headings, place.text
            );
            line_seen = Some(line);
        }
        let hints: Vec<String> = words(&span)
            .iter()
            .map(|w| format!("{w}: {}", suggest(indexes, w).join(", ")))
            .collect();
        println!(
            "{} = []  # {}",
            toml::Value::String(span.clone()),
            hints.join("; ")
        );
    }
    Ok(())
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    const USAGE: &str =
        "usage: cargo xtask crate-symbols [--check] [--base <git ref>] | --draft <file>";
    let mut base = None;
    let mut draft_file = None;
    let mut it = args.iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--check" => {}
            "--base" => base = Some(it.next().context(USAGE)?.clone()),
            "--draft" => draft_file = Some(it.next().context(USAGE)?.clone()),
            other => bail!("unknown argument '{other}'\n{USAGE}"),
        }
    }
    let config = Config::load(root)?;
    let mut indexes = BTreeMap::new();
    for (alias, dir) in CRATES {
        let src: PathBuf = root.join(dir);
        indexes.insert(
            *alias,
            Index::of_crate(&src).with_context(|| format!("index crate `{alias}`"))?,
        );
    }
    let test_ids = test_ids(root)?;
    if let Some(file) = draft_file {
        return draft(root, &config, &indexes, &test_ids, &file);
    }
    let mut listed = BTreeMap::new();
    for file in config.contracts.iter().flat_map(|c| &c.files) {
        let bytes = std::fs::read(root.join(&config.dir).join(file))
            .with_context(|| format!("read {file}"))?;
        let hash: String = Sha256::digest(&bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        listed.insert(file.clone(), hash);
    }
    let tables: BTreeSet<String> = listed
        .keys()
        .filter(|f| table_path(root, f).exists())
        .cloned()
        .collect();
    let mut orphans = Vec::new();
    if let Ok(dir) = std::fs::read_dir(root.join(TABLES)) {
        for entry in dir.flatten() {
            let name = entry.file_name().to_string_lossy().to_string();
            let md = format!("{}.md", name.strip_suffix(".toml").unwrap_or(&name));
            if !listed.contains_key(&md) {
                orphans.push(format!(
                    "{TABLES}/{name} is the table of no file in contracts.toml"
                ));
            }
        }
    }
    let allowed = match &base {
        None => None,
        Some(reference) => Some(
            allowed_pending(crate::fsutil::git_show(root, reference, SOURCE)?.as_deref())
                .with_context(|| format!("{reference}:{SOURCE}"))?,
        ),
    };
    let mut lines = check_set(&listed, &tables, PENDING, allowed.as_ref());
    lines.extend(orphans);
    let mut resolved = 0;
    let mut all_gaps = BTreeMap::new();
    for file in &tables {
        let text = std::fs::read_to_string(root.join(&config.dir).join(file))
            .with_context(|| format!("read {file}"))?;
        let table = Table::load(root, file)?.unwrap_or_default();
        let (problems, count) =
            check_table(file, &text, &table, &test_ids, &|s| resolve(&indexes, s));
        lines.extend(problems);
        resolved += count;
        let (keys, problems) = gaps(file, &table);
        all_gaps.extend(keys);
        lines.extend(problems);
    }
    let base_gaps = match &base {
        Some(reference) if crate::fsutil::git_show(root, reference, SOURCE)?.is_some() => {
            let mut keys = BTreeMap::new();
            for file in listed.keys() {
                let stem = file.strip_suffix(".md").unwrap_or(file);
                let path = format!("{TABLES}/{stem}.toml");
                if let Some(text) = crate::fsutil::git_show(root, reference, &path)? {
                    let table: Table = toml::from_str(&text)
                        .with_context(|| format!("parse {reference}:{path}"))?;
                    keys.extend(gaps(file, &table).0);
                }
            }
            Some(keys)
        }
        _ => None,
    };
    lines.extend(check_gaps(&all_gaps, base_gaps.as_ref()));
    let pending: Vec<&str> = PENDING.iter().map(|(f, _)| *f).collect();
    let summary = format!(
        "crate-symbols: {} table(s), {resolved} symbol(s) resolved{}\ncrate-symbols: warning: {} base file(s) have no table yet (PENDING): {}\ncrate-symbols: warning: {} crate gap(s) (GAP rows): {}",
        tables.len(),
        match &base {
            Some(b) => format!("; PENDING checked against {b}"),
            None => "; no base given".into(),
        },
        pending.len(),
        pending.join(", "),
        all_gaps.len(),
        GAP_CLOSERS
            .iter()
            .filter_map(|c| {
                let n = all_gaps.values().filter(|v| v == c).count();
                (n > 0).then(|| format!("{c} {n}"))
            })
            .collect::<Vec<_>>()
            .join(", ")
    );
    if !lines.is_empty() {
        let count = lines.len();
        return Err(crate::Problems::fail_after(
            lines,
            format!("{summary}\n"),
            format!("{count} problem(s) in {TABLES}/"),
        ));
    }
    println!("{summary}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE: &str = r#"
        /// `max_key_text_bytes` is named in this comment only.
        pub struct Limits { pub max_paste_bytes: u32 }
        pub struct Other { pub max_key_text_bytes: u32 }
        pub enum Op { Stop { session: u32 }, Start }
        impl Limits { pub fn check(&self) {} pub const MAX: u32 = 1; }
        pub const NAMES: &[&str] = &["max_key_text_bytes", "Ghost"];
        #[cfg(test)]
        pub struct TestOnly { pub x: u32 }
        #[cfg(test)]
        mod tests { pub struct Hidden; }
        mod inline { pub struct Nested { pub a: u8 } }
        macro_rules! open_enum { ($($t:tt)*) => {}; }
        macro_rules! make { () => { pub struct Phantom; }; }
        open_enum! { pub enum Effect { Read = "read", Write = "write" } }
        macro_rules! queued_admission {
            ($(#[$m:meta])* $name:ident, $kind:ident) => {
                $(#[$m])*
                pub struct $name { pub operation_id: String, pub kind: $kind, pub terminal: bool }
            };
        }
        queued_admission!(/// The answer.
            SafeWriteAdmission, KindSafeWrite);
        pub trait CoreApi { fn service_report(&self); fn pump(&self); const LIMIT: u32; type Out; }
        pub struct Fake;
        pub struct Wire { #[serde(default, rename = "type")] pub change: u8, #[serde(rename(serialize = "s"))] pub other: u8, #[serde(default, skip_serializing_if = "Option::is_none", rename = "ref", with = "x")] pub git_ref: u8 }
        pub enum WireEnum { #[serde(rename = "on-off")] OnOff { #[serde(rename = "v")] value: u8 } }
        impl Fake { pub fn terminal_identity(&self) {} fn private_fn(&self) {} pub(crate) fn crate_fn(&self) {} }
        struct Private { pub x: u8 }
        pub struct HalfOpen { pub open: u8, hidden: u8, pub(crate) krate: u8 }
        pub trait Shown { fn shown(&self); }
        impl Shown for HalfOpen { fn shown(&self) {} }
        pub mod rows { pub struct Row { pub status: u8 } impl Row { pub fn open(&self) {} } }
        pub mod frames { pub struct Row { pub kind: u8 } }
        pub mod third { impl crate::rows::Row { pub fn close(&self) {} pub type Item = u8; } impl Row { pub fn lost(&self) {} } }
        pub struct Gated {
            pub kept: u8,
            #[cfg(test)] pub field_t: u8,
            #[cfg(all(test, feature = "x"))] pub field_all: u8,
            #[cfg(any(test, feature = "x"))] pub field_any: u8,
            #[cfg(not(test))] pub field_not: u8,
            #[cfg(weird(test))] pub field_weird: u8,
        }
        pub enum GatedEnum { Kept, #[cfg(test)] VariantT, Shaped { #[cfg(test)] inner_t: u8, inner: u8 } }
        pub trait GatedTrait { fn kept(&self); #[cfg(test)] fn trait_t(&self); }
        impl Gated { pub fn kept_fn(&self) {} #[cfg(test)] pub fn impl_t(&self) {} #[cfg(test)] const IMPL_C: u8 = 0; }
        limits! { counts { frame_bytes: A, B, "x"; startup: A, B, "y"; } extras { lanes: T, A, B, "z"; } }
        lenient! { Op => ["Stop", "Start"] }
    "#;

    fn index() -> Index {
        Index::of_source(SOURCE).unwrap()
    }

    fn ok(path: &str) -> bool {
        resolve_path(&index(), &path.split("::").collect::<Vec<_>>()).is_ok()
    }

    #[test]
    fn items_fields_variants_and_associated_items_resolve() {
        for path in [
            "Limits",
            "Limits::max_paste_bytes",
            "Limits::check",
            "Limits::MAX",
            "Op::Stop",
            "Op::Stop::session",
            "Op::Start",
            "NAMES",
            "Nested::a",
        ] {
            assert!(ok(path), "{path} resolves");
        }
    }

    #[test]
    fn a_name_in_a_comment_a_string_or_another_type_does_not_resolve() {
        // The comment on `Limits`, the string in `NAMES` and the field of `Other` all name `max_key_text_bytes`.
        assert!(!ok("Limits::max_key_text_bytes"));
        assert!(ok("Other::max_key_text_bytes"));
        assert!(!ok("Ghost"));
        assert!(!ok("Op::Stop::other"));
        assert!(!ok("Op::Start::session"));
    }

    #[test]
    fn test_only_items_and_macro_rules_bodies_do_not_count() {
        assert!(!ok("TestOnly"));
        assert!(!ok("Hidden"));
        assert!(!ok("Phantom"));
    }

    #[test]
    fn the_item_macros_define_what_their_rules_say() {
        assert!(ok("Effect::Read"));
        assert!(ok("PluginLimits::frame_bytes"));
        assert!(ok("PluginLimits::lanes"));
        assert!(!ok("PluginLimits::A"));
    }

    #[test]
    fn a_template_macro_is_expanded_from_its_own_definition() {
        // The fields come from the transcriber of `queued_admission!`, with `$name` and `$kind` replaced.
        assert!(ok("SafeWriteAdmission::operation_id"));
        assert!(ok("SafeWriteAdmission::kind"));
        assert!(ok("SafeWriteAdmission::terminal"));
        assert!(
            !ok("SafeWriteAdmission::state"),
            "the test transcriber has no `state`"
        );
        assert!(
            !ok("KindSafeWrite"),
            "the second argument is a field type, not an item"
        );
        let without_field = SOURCE.replace("pub kind: $kind, ", "");
        let index = Index::of_source(&without_field).unwrap();
        assert!(resolve_path(&index, &["SafeWriteAdmission", "kind"]).is_err());
    }

    #[test]
    fn trait_members_belong_to_their_trait_only() {
        for path in [
            "CoreApi::service_report",
            "CoreApi::pump",
            "CoreApi::LIMIT",
            "CoreApi::Out",
        ] {
            assert!(ok(path), "{path} resolves");
        }
        // `terminal_identity` is a method of `Fake`, not of the trait.
        assert!(!ok("CoreApi::terminal_identity"));
        assert!(ok("Fake::terminal_identity"));
    }

    #[test]
    fn a_name_defined_twice_needs_its_module() {
        let err = resolve_path(&index(), &["Row", "status"]).unwrap_err();
        assert!(
            err.contains("defined 2 times") && err.contains("rows"),
            "{err}"
        );
        assert!(ok("rows::Row::status"));
        assert!(ok("frames::Row::kind"));
        assert!(!ok("frames::Row::status"));
        assert!(!ok("nowhere::Row"));
    }

    #[test]
    fn associated_items_belong_to_the_owner_in_its_module() {
        assert!(ok("rows::Row::open"));
        assert!(
            !ok("frames::Row::open"),
            "an impl of rows::Row gives frames::Row nothing"
        );
        // `impl crate::rows::Row` in another module: the absolute path names the owner, with its associated type.
        assert!(ok("rows::Row::close"));
        assert!(ok("rows::Row::Item"));
        assert!(!ok("frames::Row::close"));
        assert!(!ok("frames::Row::Item"));
        // `impl Row` in a module with no Row, and two Rows in the crate: the owner is ambiguous, so neither gets the member.
        assert!(!ok("rows::Row::lost"));
        assert!(!ok("frames::Row::lost"));
    }

    #[test]
    fn a_serde_wire_name_is_a_member_name() {
        assert!(ok("Wire::change"));
        assert!(ok("Wire::type"), "#[serde(rename = \"type\")]");
        assert!(!ok("Wire::s"), "only a plain rename gives a wire name");
        assert!(ok("Wire::ref"), "a rename after another key with a value");
        assert!(ok("WireEnum::OnOff::value"));
        assert!(ok("WireEnum::on-off::v"));
        assert!(!ok("Wire::kind"));
        assert_eq!(
            segments("plugin::Manifest::$schema"),
            ["manifest", "schema"]
        );
    }

    #[test]
    fn private_items_and_members_do_not_count() {
        assert!(ok("HalfOpen::open"));
        assert!(!ok("Private"), "a private struct");
        assert!(!ok("HalfOpen::hidden"), "a private field");
        assert!(!ok("HalfOpen::krate"), "a pub(crate) field");
        assert!(!ok("Fake::private_fn"), "a private associated fn");
        assert!(!ok("Fake::crate_fn"), "a pub(crate) associated fn");
        assert!(ok("HalfOpen::shown"), "an item of a trait impl");
        assert!(
            ok("Op::Stop::session"),
            "a variant field is as public as its enum"
        );
    }

    #[test]
    fn an_impl_belongs_to_the_type_its_path_names_in_scope() {
        let source = r#"
            pub struct Row;
            mod inner { struct Row; impl Row { pub fn leak(&self) {} } }
            pub mod a { pub struct T; }
            pub mod e { pub struct T; }
            mod b { use crate::a::T; impl T { pub fn via_use(&self) {} } }
            mod c { use super::a::*; impl T { pub fn via_glob(&self) {} } }
            mod d { use crate::a::{self as alias}; impl alias::T { pub fn via_module(&self) {} } }
            mod f { use crate::a::T as U; impl U { pub fn via_rename(&self) {} } }
            mod g { use crate::a::*; use crate::e::*; impl T { pub fn ambiguous(&self) {} } }
            mod h { impl T { pub fn nowhere(&self) {} } }
            pub trait Local { fn shadowed(&self); }
            mod i {
                use std::string::String as T;
                use crate::a::*;
                impl crate::Local for T { fn shadowed(&self) {} }
            }
        "#;
        let index = Index::of_source(source).unwrap();
        let ok = |p: &str| resolve_path(&index, &p.split("::").collect::<Vec<_>>()).is_ok();
        assert!(ok("Row"));
        assert!(
            !ok("Row::leak"),
            "the impl of a private type with the same name"
        );
        for member in ["via_use", "via_glob", "via_module", "via_rename"] {
            assert!(ok(&format!("a::T::{member}")), "{member}");
            assert!(!ok(&format!("e::T::{member}")), "{member}");
        }
        for path in [
            "a::T::ambiguous",
            "e::T::ambiguous",
            "a::T::nowhere",
            "e::T::nowhere",
            "a::T::shadowed",
        ] {
            assert!(!ok(path), "{path}: no provable owner");
        }
    }

    #[test]
    fn test_only_members_do_not_count() {
        assert!(ok("Gated::kept"));
        assert!(!ok("Gated::field_t"), "a #[cfg(test)] field");
        assert!(!ok("Gated::field_all"), "cfg(all(test, ..)) needs test");
        assert!(
            ok("Gated::field_any"),
            "cfg(any(test, feature)) holds without test"
        );
        assert!(
            ok("Gated::field_not"),
            "cfg(not(test)) is production source"
        );
        assert!(
            !ok("Gated::field_weird"),
            "an unread predicate fails closed"
        );
        assert!(ok("GatedEnum::Kept"));
        assert!(!ok("GatedEnum::VariantT"), "a #[cfg(test)] variant");
        assert!(ok("GatedEnum::Shaped::inner"));
        assert!(
            !ok("GatedEnum::Shaped::inner_t"),
            "a #[cfg(test)] variant field"
        );
        assert!(ok("GatedTrait::kept"));
        assert!(!ok("GatedTrait::trait_t"), "a #[cfg(test)] trait member");
        assert!(ok("Gated::kept_fn"));
        assert!(!ok("Gated::impl_t"), "a #[cfg(test)] impl fn");
        assert!(!ok("Gated::IMPL_C"), "a #[cfg(test)] impl const");
    }

    #[test]
    fn an_item_macro_without_a_rule_fails() {
        let err = Index::of_source("hidden_items! { pub struct X; }").unwrap_err();
        assert!(format!("{err:#}").contains("hidden_items!"), "{err:#}");
    }

    #[test]
    fn registry_data_resolves_by_typed_lookup() {
        for symbol in [
            "plugin.code:backpressured:write_pending_full",
            "plugin.call:sessions.safe_write:backpressured:write_pending_full",
            "plugin.limit:write_pending_bytes:sessions:live",
            "plugin.event:hub/config.changed",
            "plugin.wrong_state:sessions.restart:not_ready",
            "hub.reason:package.open_asset:not_found",
            "hub.reason:package.open_asset:wrong_state:assets_origin_unconfigured",
            "hub.reason:worktree.create:conflict:branch_checked_out",
            "hub.action:spawn_target.rescan_packages",
            "hub.operation:package.install:path_not_contained",
            "hub.operation_result:package.install",
            "hub.core_field:worktree:head",
            "hub.ready_failure:lock_held",
            "hub.operation_state:failed",
            "hub.operation_state:spawn:installed",
            "hub.operation_state:safe_write:delivered",
            "hub.kind:push_subscription",
            "ui.feature:ui_1_4",
            "ui.prop:list_item:selects",
            "ui.prop:embed:asset_id",
            "ui.prop:form_field:error",
            "ui.slot:dialog:body",
            "ui.copy_key:form_unavailable",
            "ui.copy_key:view_unavailable",
            "ui.copy_key_arg:view_unavailable:reason",
            "ui.absent_reason:remote_hub_unsupported",
            "ui.local_error:required_missing",
            "ui.diagnostic:stale",
            "ui.diagnostic:url_refused",
            "ui.client_feature:external_open",
            "ui.role:error",
            "plugin.value:NOT_WRITTEN_REASONS:stale",
            "plugin.value:SAFE_WRITE_UNKNOWN_REASONS:hub_restarted",
            "plugin.value:LOAD_FAILURE_CODES:host_not_trusted",
            "plugin.value:LUA_GLOBALS:require",
            "plugin.state:plugin_call:timed_out",
            "codec.named_key:page_up",
            "codec.named_key:f35",
            "codec.named_key:kp_9",
            "codec.route_feature:output_dropped",
            "codec.route_feature:output_deflate",
            "hub.binding:local-stream",
            "hub.binding:webrtc",
            "hub.binding:mcp",
            "hub.value:COMMON_REASONS:conflict:idempotency_key_reused",
            "hub.value:CHECK_UPDATE_FAILURE_REASONS:offline",
            "hub.value:PACKAGE_OUTCOME_STATES:succeeded",
            "hub.range_filter:plugin_log:after",
            "plugin.value:SAFE_WRITE_PROBE:zx",
        ] {
            assert_eq!(resolve_data(symbol), Ok(()), "{symbol}");
        }
        for symbol in [
            "plugin.code:failed:write_pending_full",
            "plugin.limit:write_pending_bytes:sessions:restart",
            "hub.reason:package.pin:not_found",
            "hub.operation:package.install:no_such_reason",
            "plugin.nothing:x",
            "plugin.code:only_one_arg",
            "ui.prop:badge:href",
            "hub.operation_state:exploded",
            "hub.operation_state:spawn:succeeded",
            "hub.operation_state:no_such_kind:failed",
            "hub.kind:no_such_kind",
            "ui.prop:no_such_kind:href",
            "ui.prop:dialog:body",
            "ui.slot:dialog:title",
            "ui.slot:empty_state:body",
            "ui.copy_key:required_missing",
            "ui.copy_key_arg:view_unavailable:code",
            "ui.copy_key_arg:cancel:reason",
            "ui.absent_reason:stale",
            "ui.absent_reason:not_found:x",
            "ui.local_error:not_found",
            "ui.diagnostic:option_unavailable",
            "ui.diagnostic",
            "ui.client_feature:ui_1_4",
            "ui.role:submit",
            "plugin.value:NOT_WRITTEN_REASONS:hub_restarted",
            "plugin.value:NO_SUCH_LIST:stale",
            "plugin.state:spawn:timed_out",
            "codec.named_key:f36",
            "codec.named_key:shift",
            "codec.route_feature:focus_report",
            "hub.binding:quic",
            "hub.value:COMMON_REASONS:idempotency_key_reused",
            "hub.value:COMMON_REASONS",
            "hub.value:NO_SUCH_LIST:offline",
            "hub.range_filter:session:after",
            "hub.range_filter:plugin_log:before",
            "plugin.value:SAFE_WRITE_PROBE:xz",
        ] {
            assert!(resolve_data(symbol).is_err(), "{symbol}");
        }
    }

    // ------------------------------------------------------------------ the tables

    fn ids() -> BTreeSet<String> {
        ["conf::a15_1_x".to_string()].into()
    }

    fn table(text: &str) -> Table {
        toml::from_str(text).unwrap()
    }

    /// A resolver that knows a few symbols.
    fn known(symbol: &str) -> std::result::Result<(), String> {
        let ok = [
            "core::ErrorCode::InvalidInput",
            "core::ErrorCode::InvalidInput::field",
            "hub.reason:hub.update:busy",
            "core::CoreLimits::max_key_text_bytes",
        ];
        if ok.contains(&symbol) {
            Ok(())
        } else {
            Err("unknown".into())
        }
    }

    const TEXT: &str = "# Core A7\n## A7-1\n\
        | Kind | Action | Failures |\n|---|---|---|\n\
        | `hub` | `update` | `busy` |\n\
        | `hub` | `shutdown` | `busy` |\n\
        Refused with `InvalidInput{field}`, see `conf::a15_1_x` and `SW-1`.\n\
        ```\n`max_key_text_bytes` in a fence\n```\n";

    #[test]
    fn spans_have_lines_and_heading_paths() {
        let spans = spans(TEXT);
        let place = &spans[&(5, "busy".to_string())];
        assert_eq!(place.headings, "Core A7 > A7-1");
        assert!(place.text.contains("`update`"));
        assert!(spans.contains_key(&(6, "busy".to_string())));
        assert!(spans.contains_key(&(7, "InvalidInput{field}".to_string())));
        assert!(
            !spans.keys().any(|(_, s)| s == "max_key_text_bytes"),
            "a fenced block has no spans"
        );
        assert_eq!(
            super::spans("a ``x `y` z`` b").keys().next().unwrap().1,
            "x `y` z"
        );
    }

    #[test]
    fn words_are_identifiers_with_a_lowercase_letter() {
        let w = words("InvalidInput{field: route_limits.max_frame_bytes} ESC SW-1 0x0F");
        let expected: BTreeSet<String> =
            ["InvalidInput", "field", "route_limits", "max_frame_bytes"]
                .map(String::from)
                .into();
        assert_eq!(w, expected);
    }

    #[test]
    fn a_rule_class_matches_a_whole_known_span_only() {
        assert_eq!(rule_of("conf::a15_1_x", &ids()), Some("conformance-id"));
        for span in [
            "conf::a15_1_x CoreLimits",
            "conf::a15_1_x max_key_text_bytes",
            "conf::unknown_id",
            "256 max_key_text_bytes",
            "SW-1 CoreLimits",
            "--max_key_text_bytes",
        ] {
            assert!(needs_row(span, &ids()), "{span}");
        }
        assert!(!needs_row("SW-1", &ids()));
        assert!(!needs_row("4,096", &ids()));
    }

    #[test]
    fn each_line_needs_its_own_row() {
        let t = table(
            r#"
            [rows."5"]
            hub = ["hub.reason:hub.update:busy"]
            update = ["hub.reason:hub.update:busy"]
            busy = ["hub.reason:hub.update:busy"]
            [rows."7"]
            "InvalidInput{field}" = ["core::ErrorCode::InvalidInput::field"]
            "#,
        );
        let (p, _) = check_table("f.md", TEXT, &t, &ids(), &known);
        let p = p.join("\n");
        assert!(p.contains("f.md:6: `busy` has no row"), "{p}");
        assert!(!p.contains("f.md:5: `busy`"), "{p}");
        assert!(!p.contains("conf::a15_1_x"), "{p}");
    }

    #[test]
    fn a_row_must_name_every_word_of_its_span() {
        let text = "Refused with `InvalidInput{field}`.\n";
        let only_code = table(
            r#"[rows."1"]
            "InvalidInput{field}" = ["core::ErrorCode::InvalidInput"]"#,
        );
        let (p, _) = check_table("f.md", text, &only_code, &ids(), &known);
        assert_eq!(p, ["f.md:1: `InvalidInput{field}`: no symbol names `field` (add one, or list it in `uncovered` with a reason)"]);
        let with_field = table(
            r#"[rows."1"]
            "InvalidInput{field}" = ["core::ErrorCode::InvalidInput::field"]"#,
        );
        assert!(check_table("f.md", text, &with_field, &ids(), &known)
            .0
            .is_empty());
        let excused = table(
            r#"[rows."1"]
            "InvalidInput{field}" = { symbols = ["core::ErrorCode::InvalidInput"], uncovered = { field = "why" } }"#,
        );
        assert!(check_table("f.md", text, &excused, &ids(), &known)
            .0
            .is_empty());
        let bad = table(
            r#"[rows."1"]
            "InvalidInput{field}" = { symbols = ["core::Gone"], uncovered = { field = "", other = "x" } }"#,
        );
        let p = check_table("f.md", text, &bad, &ids(), &known).0.join("\n");
        assert!(p.contains("core::Gone: unknown"), "{p}");
        assert!(p.contains("no symbol names `InvalidInput`"), "{p}");
        assert!(p.contains("gives no reason for `field`"), "{p}");
        assert!(
            p.contains("`other`, which is not a word of the span"),
            "{p}"
        );
    }

    #[test]
    fn cli_schemes_resolve_by_exact_membership() {
        for symbol in [
            "cli.binary:botster",
            "cli.command:COMMANDS:mcp-serve",
            "cli.command:COMMANDS:plugin pin",
            "cli.command:COMMANDS:plugin catalog refresh",
            "cli.command:GONE_COMMANDS:session",
            "cli.command:GONE_COMMANDS:reset",
            "cli.mcp_tool:botster_status",
            "cli.log_label:cli",
            "cli.dev_checkout_key:plugin_hosts",
            "cli.dev_checkout_key:hub",
            "cli.flag:DROPPED_FLAGS:delete-worktree:--force",
            "cli.confirm_answer:yes",
            "cli.flag:FLAGS:plugin pin:--unpin",
            "cli.flag:FLAGS:plugin remove:--yes",
            "cli.flag:FLAGS:botster:--version",
            "cli.context_key:CONTEXT_KEYS:branch_name",
            "cli.context_key:DROPPED_CONTEXT_KEYS:hub_manifest_path",
            "cli.config_key:CONFIG_KEYS:log_dir",
            "cli.config_key:DROPPED_CONFIG_KEYS:server_url",
            "cli.status_field:STATUS_FIELDS:uptime",
            "cli.status_field:DROPPED_STATUS_FIELDS:pid_file",
            "cli.message:CLI-G7:1",
            "cli.path:CLI-G3:7",
        ] {
            assert_eq!(resolve_data(symbol), Ok(()), "{symbol}");
        }
        for symbol in [
            // A gone or dropped name never resolves as a live one, and the reverse.
            "cli.command:COMMANDS:session",
            "cli.command:GONE_COMMANDS:start",
            "cli.context_key:CONTEXT_KEYS:hub_manifest_path",
            "cli.context_key:DROPPED_CONTEXT_KEYS:branch_name",
            "cli.config_key:CONFIG_KEYS:server_url",
            "cli.config_key:DROPPED_CONFIG_KEYS:log_dir",
            "cli.status_field:STATUS_FIELDS:pid_file",
            "cli.status_field:DROPPED_STATUS_FIELDS:uptime",
            // A part of a token, a flag of another command, and a wrong list, ordinal or arity.
            "cli.command:COMMANDS:mcp",
            "cli.command:COMMANDS:pin",
            "cli.flag:FLAGS:start:--unpin",
            "cli.flag:FLAGS:plugin pin:--unpin:x",
            "cli.flag:plugin pin:--unpin",
            "cli.flag:FLAGS:delete-worktree:--force",
            "cli.flag:DROPPED_FLAGS:plugin pin:--unpin",
            "cli.flag:NO_SUCH_LIST:plugin pin:--unpin",
            "cli.confirm_answer:no",
            "cli.command:NO_SUCH_LIST:start",
            "cli.command:start",
            "cli.message:CLI-G7:2",
            "cli.message:CLI-G7:01",
            "cli.message:CLI-G3:7",
            "cli.path:CLI-G7:1",
            "cli.nothing:x",
            "cli.binary:botster-hub",
            "cli.command:COMMANDS:reset",
            "cli.mcp_tool:botster",
            "cli.log_label:mcp-serve",
            "cli.log_label:start:x",
            "cli.dev_checkout_key:plugin-hosts",
            "cli.dev_checkout_key:session",
            "cli.dev_checkout_key:hub:x",
            "cli.dev_checkout_key",
            "cli.binary:bot",
            "cli.binary",
        ] {
            assert!(resolve_data(symbol).is_err(), "{symbol}");
            assert_eq!(cli_token(symbol), None, "{symbol}");
        }
        assert_eq!(
            cli_token("cli.flag:FLAGS:plugin pin:--unpin"),
            Some("--unpin")
        );
        assert_eq!(cli_token("hub.kind:session"), None);
    }

    #[test]
    fn a_cli_symbol_covers_its_whole_token_only() {
        let text = "Run `mcp-serve`, `mcp serve`, `plugin pin --unpin`, `--unpin-all` or `Hub busy; try again`.\n";
        let t = table(
            r#"[rows."1"]
            "mcp-serve" = ["cli.command:COMMANDS:mcp-serve"]
            "mcp serve" = ["cli.command:COMMANDS:mcp-serve"]
            "plugin pin --unpin" = ["cli.command:COMMANDS:plugin pin", "cli.flag:FLAGS:plugin pin:--unpin"]
            "--unpin-all" = ["cli.flag:FLAGS:plugin pin:--unpin"]
            "Hub busy; try again" = ["cli.message:CLI-G7:9"]"#,
        );
        let p = check_table("f.md", text, &t, &ids(), &|s| resolve_data(s)).0;
        assert_eq!(
            p,
            [
                "f.md:1: `--unpin-all`: no symbol names `all` (add one, or list it in `uncovered` with a reason)",
                "f.md:1: `--unpin-all`: no symbol names `unpin` (add one, or list it in `uncovered` with a reason)",
                "f.md:1: `mcp serve`: no symbol names `mcp` (add one, or list it in `uncovered` with a reason)",
                "f.md:1: `mcp serve`: no symbol names `serve` (add one, or list it in `uncovered` with a reason)",
            ]
        );
        // No segment of a cli symbol covers a word: the list name `COMMANDS` and the split parts do not.
        assert!(segments("cli.command:COMMANDS:mcp-serve").is_empty());
        let words = |symbol, span| {
            whole_token_words(symbol, span)
                .into_iter()
                .collect::<Vec<_>>()
        };
        assert_eq!(
            words("cli.command:COMMANDS:mcp-serve", "mcp-serve"),
            ["mcp", "serve"]
        );
        assert!(words("cli.command:COMMANDS:mcp-serve", "x-mcp-serve").is_empty());
        assert!(words("cli.command:COMMANDS:mcp-serve", "mcp-serve_x").is_empty());
        assert!(words("cli.command:COMMANDS:mcp-serve", "mcp-serve2").is_empty());
        assert!(words("cli.flag:FLAGS:botster:-h", "--help").is_empty());
        assert_eq!(words("cli.flag:FLAGS:botster:--help", "[--help]"), ["help"]);
        // A second occurrence can be whole when the first is not.
        assert_eq!(
            words("cli.command:COMMANDS:status", "statuses, status"),
            ["status"]
        );
        // A message template with leading spaces matches its trimmed text.
        assert_eq!(
            words("cli.message:CLI-ST6:18", "unadoptable: <session ref> ..."),
            ["ref", "session", "unadoptable"]
        );
        assert!(words("hub.kind:session", "session").is_empty());
        // The binary name covers `botster` alone, never a part of `botster-issue-<N>` or `botster-hub`.
        assert_eq!(
            words("cli.binary:botster", "botster context prompt"),
            ["botster"]
        );
        assert!(words("cli.binary:botster", "botster-issue-<N>").is_empty());
        assert!(words("cli.binary:botster", "run botster-hub up").is_empty());
        // A development checkout key covers its whole key only: `hub` alone, never a part of `hub_id` or `botster-hub`.
        assert_eq!(words("cli.dev_checkout_key:hub", "hub"), ["hub"]);
        assert!(words("cli.dev_checkout_key:hub", "hub_id").is_empty());
        assert!(words("cli.dev_checkout_key:hub", "botster-hub").is_empty());
        assert!(!words("cli.dev_checkout_key:plugin_hosts", "plugin_hosts").is_empty());
        assert!(words("cli.dev_checkout_key:plugin_hosts", "plugin_hosts_dir").is_empty());
    }

    #[test]
    fn stale_empty_and_unexplained_rows_fail() {
        let text = "`InvalidInput` and `x_y`\n";
        let t = table(
            r#"
            [rows."1"]
            InvalidInput = []
            x_y = "n/a"
            gone = ["core::ErrorCode::InvalidInput"]
            [rows."2"]
            InvalidInput = "n/a: moved"
            [rows.top]
            "#,
        );
        let p = check_table("f.md", text, &t, &ids(), &known).0.join("\n");
        assert!(
            p.contains("f.md:1: `InvalidInput`: the row lists no symbol"),
            "{p}"
        );
        assert!(
            p.contains("f.md:1: `x_y`: a text row is `n/a: <reason>`"),
            "{p}"
        );
        assert!(p.contains("f.md:1: `gone`: the row is stale"), "{p}");
        assert!(
            p.contains("f.md:2: `InvalidInput`: the row is stale"),
            "{p}"
        );
        assert!(p.contains("[rows.\"top\"] is not a line number"), "{p}");
    }

    #[test]
    fn the_removal_of_a_mapped_field_fails() {
        let source = "pub struct CoreLimits { pub max_key_text_bytes: u32 }";
        let index = Index::of_source(source).unwrap();
        let indexes: BTreeMap<&str, Index> = [("core", index)].into();
        let text = "`max_key_text_bytes`, default 256\n";
        let t = table(
            r#"[rows."1"]
            max_key_text_bytes = ["core::CoreLimits::max_key_text_bytes"]"#,
        );
        assert!(
            check_table("f.md", text, &t, &ids(), &|s| resolve(&indexes, s))
                .0
                .is_empty()
        );
        let reverted: BTreeMap<&str, Index> = [(
            "core",
            Index::of_source("pub struct CoreLimits { pub max_paste_bytes: u32 }").unwrap(),
        )]
        .into();
        let p = check_table("f.md", text, &t, &ids(), &|s| resolve(&reverted, s)).0;
        assert_eq!(p.len(), 1, "{p:?}");
        assert!(
            p[0].contains("has no field, variant or associated item `max_key_text_bytes`"),
            "{p:?}"
        );
    }

    /// The rows that the review of PR 1 corrected: each committed row needs its actual owner, and
    /// no longer holds the same-name symbol it held before. A resolver that removes one symbol
    /// stands for a crate change that deletes it. A row with no owner (`None`) is an `n/a` row:
    /// only the old symbol is checked.
    #[test]
    fn the_removal_of_a_corrected_owner_fails_its_row() {
        const REALIZE: &str = "ui-contract-v1.4-realize-completeness-candidate3.md";
        const UI13: &str = "ui-contract-v1.3-completeness-candidate9.md";
        const UI12: &str = "ui-contract-v1.2-embed-successor-candidate2.md";
        const HC2: &str = "hub-client-contract-v1-amendment-2-candidate6.md";
        const HP1: &str = "hub-plugin-contract-v1.1.amendment-1-candidate5.md";
        const HP2: &str = "hub-plugin-contract-v1.1.amendment-2-candidate7.md";
        const HP6: &str = "hub-plugin-contract-v1.1.amendment-6-candidate2.md";
        const HP: &str = "hub-plugin-contract-v1.1.joint-J4e.md";
        const CORE: &str = "core-contract-v1.17.md";
        const CODEC: &str = "route-codec-v1-extract-rev37.md";
        const HC: &str = "hub-client-contract-v1-candidate-parity45.md";
        const SNAPSHOTS: &str = "kind_snapshots: { <kind name>: { baseline: bool, hub_id?, rows: [row], error?: { message? } } }";
        // (file, line, span, owner, the same-name symbol that the row held before)
        let cases: &[(&str, usize, &str, Option<&str>, &str)] = &[
            // EN-7: each verb and action declares `effect` in `describe`, not the effect of an error.
            (
                HC,
                112,
                "effect",
                Some("hub::VerbDescriptor::effect"),
                "hub::Error::effect",
            ),
            (
                HC,
                112,
                "effect",
                Some("hub::ActionDescriptor::effect"),
                "hub::Error::effect",
            ),
            // VB-3: the declared effect of the create verb.
            (
                HC,
                165,
                "effect",
                Some("hub::VerbDescriptor::effect"),
                "hub::Error::effect",
            ),
            // VB-6: the declared effect of the invoked action.
            (
                HC,
                168,
                "effect",
                Some("hub::ActionDescriptor::effect"),
                "hub::Error::effect",
            ),
            // A.9 `write`: the HP RW-2 effect, unchanged.
            (
                HC,
                897,
                "committed",
                Some("plugin::Effect::Committed"),
                "hub::EffectClass::Committed",
            ),
            // EN-2: `watch_id` of the server watch frames, not of the request's receipt.
            (
                HC,
                101,
                "watch { watch_id, … }",
                Some("hub::WatchFrame::Upsert::watch_id"),
                "hub::WatchOpened::watch_id",
            ),
            // SF-1: the stable id of a view field, not an error target.
            (
                HC,
                340,
                "field_id",
                Some("ui::Field::id"),
                "hub::FieldError::field_id",
            ),
            // EV-2: the Hub event, not the semantic key input.
            (
                HC,
                378,
                "event { stream_id, owner, name, subject?, priority, source, payload, at }",
                Some("hub::Event"),
                "codec::Input::Key::event",
            ),
            // PR-2: the fields of the pairing bundle.
            (
                HC,
                452,
                "expires_at",
                Some("hub::PairingBundle::expires_at"),
                "hub::PairRequest::expires_at",
            ),
            (
                HC,
                452,
                "route",
                Some("hub::PairingBundle::route"),
                "hub::AuthChallenge::route",
            ),
            // A.9: the result shapes, not the request arguments.
            (
                HC,
                903,
                "{ entries: [{ name, path }] }",
                Some("hub::BrowseEntry::path"),
                "hub::BrowseArgs::path",
            ),
            // TB-0: `features[]` is the Hub's `welcome.features` (binding features), not the route's `attached.features`.
            (
                CODEC,
                196,
                "features[]",
                Some("hub::Welcome::features"),
                "codec::Attached::features",
            ),
            // AM-1: the `SessionState` events after `pump`, not the state enum.
            (
                CORE,
                363,
                "SessionState",
                Some("core::Event::SessionState"),
                "core::SessionState",
            ),
            (
                REALIZE,
                46,
                SNAPSHOTS,
                Some("ui::SnapshotError::message"),
                "ui::ErrorValue::message",
            ),
            (
                REALIZE,
                46,
                SNAPSHOTS,
                Some("ui::KindSnapshot::error"),
                "ui::Tone::Danger",
            ),
            (
                REALIZE,
                43,
                "{ message, tone? }",
                Some("ui::Confirm::message"),
                "ui::ErrorValue::message",
            ),
            (
                REALIZE,
                43,
                "{ navigate: { view, select? } }",
                Some("ui::Navigate::select"),
                "ui::UiNodeKind::Select",
            ),
            (
                REALIZE,
                46,
                r"overlays: [{ form: { button } } \| { confirm: { button } }]",
                Some("ui::Overlay::Confirm"),
                "ui::Action::confirm",
            ),
            (
                REALIZE,
                45,
                "confirm",
                Some("ui::Overlay::Confirm"),
                "ui::Action::confirm",
            ),
            (
                REALIZE,
                45,
                "form",
                Some("ui::Overlay::Form"),
                "ui::UiNodeKind::Form",
            ),
            (
                REALIZE,
                45,
                "stamped_errors: [{ draft_key, code }]",
                Some("ui::StampedError::code"),
                "ui::Diagnostic::code",
            ),
            (
                REALIZE,
                45,
                "stale",
                Some("ui::Diagnostic::code"),
                "plugin.value:NOT_WRITTEN_REASONS:stale",
            ),
            (
                REALIZE,
                48,
                "title",
                Some("ui.prop:dialog:title"),
                "plugin.event:terminal/title",
            ),
            (
                REALIZE,
                48,
                "label",
                Some("ui.prop:form_section:label"),
                "ui::Action::label",
            ),
            (REALIZE, 48, "confirm", None, "ui::Action::confirm"),
            (
                REALIZE,
                44,
                r#"child(<row>, "action")"#,
                Some("ui::child_id"),
                "ui::Operand::Row",
            ),
            (
                UI13,
                25,
                "invalid_request{detail.code: non_canonical_map_key}",
                Some("plugin::Detail::code"),
                "ui::Diagnostic::code",
            ),
            (
                UI13,
                17,
                "stale_descriptor",
                Some("hub::ErrorCode::StaleDescriptor"),
                "plugin.code:conflict:stale_descriptor",
            ),
            (
                UI13,
                16,
                "failed",
                Some("hub.operation_state:worktree.create:failed"),
                "plugin::ErrorKind::Failed",
            ),
            (
                UI12,
                25,
                "title",
                Some("ui::EmbedPart::title"),
                "plugin.event:terminal/title",
            ),
            (
                HC2,
                78,
                "restart",
                Some("hub.action:hub.restart"),
                "hub.action:session.restart",
            ),
            (
                HC2,
                147,
                "state",
                Some("hub::WorktreeRow::state"),
                "hub::OperationRow::state",
            ),
            (
                HP1,
                14,
                "path",
                Some("plugin::Detail::extra"),
                "plugin::PackageSource::Git::path",
            ),
            (
                HP2,
                31,
                "failed.reason",
                Some("plugin.state:safe_write:failed"),
                "plugin::ErrorKind::Failed",
            ),
            (
                HP2,
                34,
                "Ready",
                Some("plugin::Ready"),
                "plugin::WorktreeStatus::Ready",
            ),
            (
                HP2,
                11,
                r"not_ended \| not_restartable \| record_unavailable \| environment_not_retained \| not_ready",
                Some("plugin.wrong_state:sessions.restart:not_ready"),
                "plugin.code:unavailable:not_ready",
            ),
            (
                HP6,
                17,
                "accepted",
                Some("plugin::PublishStatus::Accepted"),
                "plugin.value:INGRESS_STATUSES:accepted",
            ),
            // HP J4e ER-1: the effect of a refusal, not of an interface export.
            (
                HP,
                527,
                "effect",
                Some("plugin::ErrorBody::effect"),
                "plugin::InterfaceExport::effect",
            ),
            // HP J4e ER-2: no HP call declares idempotent; the client action field is not its owner.
            (
                HP,
                528,
                "idempotent: true",
                None,
                "hub::ActionDescriptor::idempotent",
            ),
            // HP J4e SA-2: a wrong_state reason of sessions.restart.
            (
                HP,
                478,
                "not_ready",
                Some("plugin.wrong_state:sessions.restart:not_ready"),
                "plugin.code:unavailable:not_ready",
            ),
            // HP J4e The calling convention: the Result value, not the handler response frame.
            (
                HP,
                182,
                "Result",
                Some("plugin::PluginResult"),
                "plugin::Frame::Result",
            ),
            (
                HP,
                375,
                "Result",
                Some("plugin::PluginResult"),
                "plugin::Frame::Result",
            ),
            (
                HP,
                440,
                "Result",
                Some("plugin::PluginResult"),
                "plugin::Frame::Result",
            ),
            (
                HP,
                656,
                "Result",
                Some("plugin::PluginResult"),
                "plugin::Frame::Result",
            ),
            // HP J4e Appendix B: each frame row names the fields of its own frame.
            (
                HP,
                817,
                "plugin_key",
                Some("plugin::Hello::plugin_key"),
                "plugin::Caller::Plugin::plugin_key",
            ),
            (
                HP,
                821,
                "request_id",
                Some("plugin::Cancel::request_id"),
                "plugin::Invoke::request_id",
            ),
            (
                HP,
                822,
                "request_id",
                Some("plugin::ResultFrame::request_id"),
                "plugin::Invoke::request_id",
            ),
            (
                HP,
                824,
                "call_id",
                Some("plugin::HostResult::call_id"),
                "plugin::HostCall::call_id",
            ),
            (
                HP,
                825,
                "deadline_ms",
                Some("plugin::Shutdown::deadline_ms"),
                "plugin::Invoke::deadline_ms",
            ),
            // HP J4e LN-2: each Lua row names the args and result of its own call.
            (
                HP,
                861,
                "{watch_id}",
                Some("plugin::WatchId::watch_id"),
                "plugin::UnwatchArgs::watch_id",
            ),
            (
                HP,
                882,
                "{watch_id}",
                Some("plugin::WatchId::watch_id"),
                "plugin::UnwatchArgs::watch_id",
            ),
            (
                HP,
                877,
                "{name}",
                Some("plugin::SecretsGetArgs::name"),
                "plugin::AssetsLookupArgs::name",
            ),
            (
                HP,
                890,
                "{status}",
                Some("plugin::RelayDeliverResult::status"),
                "plugin::PublishResult::status",
            ),
            (
                HP,
                898,
                "{target_id}",
                Some("plugin::SessionTypesListArgs::target_id"),
                "plugin::TargetIdArgs::target_id",
            ),
            // HP J4e EV-1: the stamped event source instance, not the connection instance.
            (
                HP,
                734,
                "instance",
                Some("plugin::EventSourceRef::instance"),
                "plugin::Hello::instance",
            ),
            (
                HP,
                863,
                "instance",
                Some("plugin::EventSourceRef::instance"),
                "plugin::Hello::instance",
            ),
            // HP J4e PD-1, PD-2, PD-9: manifest keys.
            (
                HP,
                919,
                "description",
                Some("plugin::Manifest::description"),
                "plugin::ToolDecl::description",
            ),
            (
                HP,
                920,
                "botster_plugin_api",
                Some("plugin::Manifest::botster_plugin_api"),
                "plugin::no_such_symbol",
            ),
            (
                HP,
                928,
                "botster_plugin_api",
                Some("plugin::Requirements::botster_plugin_api"),
                "plugin::no_such_symbol",
            ),
            // HP J4e PD-3: the excluded catalog entry kind, not an error detail member.
            (HP, 921, "interface", None, "hub::Error::detail"),
            // HP J4e PD-3b: schema notation, not the store accepts list or a UI prop.
            (
                HP,
                922,
                "accepts(P, I)",
                None,
                "plugin::CollectionDecl::accepts",
            ),
            (HP, 922, "accepts(P.prop, I.prop)", None, "ui::UiNode::prop"),
            // HP J4e PD-8: the provider state and the plugin gap record.
            (
                HP,
                927,
                "available",
                Some("plugin::DependencyState::Available"),
                "hub::AvailableTag::Available",
            ),
            (
                HP,
                927,
                "state_read",
                Some("plugin::EventGapRecord::state_read"),
                "hub::EventGap::state_read",
            ),
            (
                HP,
                927,
                "unavailable",
                Some("plugin::DependencyState::Unavailable"),
                "plugin::no_such_symbol",
            ),
            (
                HP,
                927,
                "high",
                Some("plugin::Priority::name"),
                "hub::EventPriority::High",
            ),
            // HP J4e The sweep after the review: the HTTP Host header, the MCP http transport, a gap recovery, the operator hub_id.
            (HP, 11, "Host", None, "plugin::Ready::host"),
            (
                HP,
                11,
                "http",
                Some("plugin::McpServerDecl::Http"),
                "plugin.value:OPERATION_KINDS:http",
            ),
            (
                HP,
                11,
                "unavailable",
                Some("plugin::Recovery::Unavailable"),
                "plugin::ErrorKind::Unavailable",
            ),
            (
                HP,
                11,
                "hub_id",
                Some("plugin::Caller::Operator::hub_id"),
                "plugin::HubIdentity::hub_id",
            ),
            // HP J4e MN-2: the net allow list, not an iframe sandbox token.
            (
                HP,
                841,
                "allow",
                Some("plugin::Capability::allow"),
                "plugin::no_such_symbol",
            ),
            // HP J4e CA-18: the plugin interaction class, not the hub type.
            (
                HP,
                262,
                "interactive",
                Some("plugin::rows::Interaction::Interactive"),
                "hub::Interaction::Interactive",
            ),
            // HP J4e VW-1: the embed view part, not the client describe component.
            (
                HP,
                11,
                "embed",
                Some("ui::Part::Embed"),
                "hub::ViewComponent::Embed",
            ),
        ];
        let root = crate::fsutil::repo_root().unwrap();
        let config = Config::load(&root).unwrap();
        let test_ids = test_ids(&root).unwrap();
        let mut tables: BTreeMap<&str, Table> = BTreeMap::new();
        for &(file, line, span, owner, before) in cases {
            // Only the case's own line and row are checked, because a whole table is slow. Headings and fences stay, and line
            // numbers do not change. A span that the cut text loses fails the `before` check below.
            let text: String = std::fs::read_to_string(root.join(&config.dir).join(file))
                .unwrap()
                .lines()
                .enumerate()
                .map(|(i, l)| {
                    if i + 1 == line || l.starts_with('#') || l.starts_with("```") {
                        l
                    } else {
                        ""
                    }
                })
                .collect::<Vec<_>>()
                .join("\n");
            let full = tables
                .entry(file)
                .or_insert_with(|| Table::load(&root, file).unwrap().unwrap());
            let key = line.to_string();
            let table = Table {
                rows: full
                    .rows
                    .remove(&key)
                    .map(|r| (key.clone(), r))
                    .into_iter()
                    .collect(),
            };
            let at = format!("{file}:{line}: `{span}`");
            let without = |gone: &str| {
                check_table(file, &text, &table, &test_ids, &|s| {
                    if s == gone {
                        Err("removed".into())
                    } else {
                        Ok(())
                    }
                })
                .0
                .into_iter()
                .filter(|p| p.starts_with(&at))
                .collect::<Vec<_>>()
            };
            if let Some(owner) = owner {
                let p = without(owner);
                assert!(
                    p.iter().any(|p| p.contains("removed")),
                    "{at}: no problem without {owner}: {p:?}"
                );
            }
            // Also proves that the row exists: a missing row is a problem at `at`.
            let p = without(before);
            assert!(p.is_empty(), "{at} still needs {before}: {p:?}");
            full.rows.extend(table.rows);
        }
    }

    /// A span that names two owners at once fails its row when either owner is removed.
    #[test]
    fn a_span_with_two_owners_needs_both() {
        // Codec A.2c: `ended` is both the `state` and the `lifecycle_class` of the row.
        // HP J4e connection epoch and Appendix B: every frame carries the epoch.
        const EPOCHS: &[&str] = &[
            "plugin::Hello::epoch",
            "plugin::Ready::epoch",
            "plugin::Activated::epoch",
            "plugin::Invoke::epoch",
            "plugin::Cancel::epoch",
            "plugin::ResultFrame::epoch",
            "plugin::HostCall::epoch",
            "plugin::HostResult::epoch",
            "plugin::Shutdown::epoch",
            "plugin::Fault::epoch",
        ];
        let cases: &[(&str, usize, &str, &[&str])] = &[
            (
                "route-codec-v1-extract-rev37.md",
                250,
                "ended",
                &["codec::SessionState::Ended", "codec::LifecycleClass::Ended"],
            ),
            ("hub-plugin-contract-v1.1.joint-J4e.md", 45, "epoch", EPOCHS),
            (
                "hub-plugin-contract-v1.1.joint-J4e.md",
                813,
                "epoch",
                EPOCHS,
            ),
        ];
        let root = crate::fsutil::repo_root().unwrap();
        let config = Config::load(&root).unwrap();
        let test_ids = test_ids(&root).unwrap();
        for &(file, line, span, owners) in cases {
            let text = std::fs::read_to_string(root.join(&config.dir).join(file)).unwrap();
            let table = Table::load(&root, file).unwrap().unwrap();
            let at = format!("{file}:{line}: `{span}`");
            for owner in owners {
                let p: Vec<_> = check_table(file, &text, &table, &test_ids, &|s| {
                    if s == *owner {
                        Err("removed".into())
                    } else {
                        Ok(())
                    }
                })
                .0
                .into_iter()
                .filter(|p| p.starts_with(&at) && p.contains("removed"))
                .collect();
                assert!(!p.is_empty(), "{at}: no problem without {owner}");
            }
        }
    }

    const H: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    #[test]
    fn the_pending_list_binds_hashes_and_only_shrinks() {
        let listed: BTreeMap<String, String> = [
            ("base.md".to_string(), H.to_string()),
            ("done.md".to_string(), H.to_string()),
            ("a1.md".to_string(), H.to_string()),
            ("a2.md".to_string(), H.to_string()),
        ]
        .into();
        let tables: BTreeSet<String> = [
            "done.md".to_string(),
            "a1.md".to_string(),
            "x.md".to_string(),
        ]
        .into();
        let allowed: BTreeSet<String> = ["base.md".to_string(), "done.md".to_string()].into();
        let pending = [
            ("base.md", H),
            ("done.md", H),
            ("a2.md", "11"),
            ("gone.md", H),
        ];
        let p = check_set(&listed, &tables, &pending, Some(&allowed)).join("\n");
        assert!(
            p.contains("done.md has a table and is still pending"),
            "{p}"
        );
        assert!(
            p.contains("PENDING names a2.md, which the base's PENDING does not name"),
            "{p}"
        );
        assert!(p.contains("PENDING names a2.md with sha256 11"), "{p}");
        assert!(
            p.contains("PENDING names gone.md, which contracts.toml does not list"),
            "{p}"
        );
        assert!(
            p.contains("x.md, which contracts.toml does not list"),
            "{p}"
        );
        assert!(!p.contains("base.md"), "{p}");
        let p = check_set(&listed, &tables, &[("base.md", H)], None).join("\n");
        assert!(
            p.contains("a2.md is in contracts.toml and has no table"),
            "{p}"
        );
    }

    #[test]
    fn the_base_pending_list_is_read_or_fails() {
        let own = pending_names(include_str!("crate_symbols.rs")).unwrap();
        assert_eq!(own.len(), PENDING.len());
        // The list never grows: every entry was a base file when the check was adopted.
        assert!(PENDING.iter().all(|(f, _)| BOOTSTRAP.contains(f)));
        assert_eq!(allowed_pending(None).unwrap().len(), BOOTSTRAP.len());
        assert!(
            allowed_pending(Some("fn main() {}")).is_err(),
            "a base file without PENDING is not adoption"
        );
        let base = format!("const PENDING: &[(&str, &str)] = &[\n    (\"a.md\", \"{H}\"),\n];");
        assert_eq!(
            allowed_pending(Some(&base)).unwrap(),
            ["a.md".to_string()].into()
        );
    }

    #[test]
    fn gaps_name_their_closer_and_only_shrink() {
        let t = table(
            r#"
            [rows."3"]
            "a" = "n/a: GAP(hc): no type holds it"
            "b c" = { symbols = ["core::B"], uncovered = { c = "GAP(ui): a string literal" } }
            "d" = "n/a: an example value"
            [rows."4"]
            "e" = "n/a: no type holds it (GAP, reported to the lead)"
            "f" = "n/a: GAP(nobody): no type holds it"
            "#,
        );
        let (keys, problems) = gaps("x.md", &t);
        assert_eq!(
            keys,
            [
                ("x.md:3: `a`".to_string(), "hc".to_string()),
                ("x.md:3: `b c` c".to_string(), "ui".to_string()),
            ]
            .into()
        );
        // A reason that says GAP in another form, or names no closer, fails.
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].starts_with("x.md:4: `e`"), "{problems:?}");
        assert!(problems[1].starts_with("x.md:4: `f`"), "{problems:?}");
        // The adoption of the check allows every gap; after it, a gap that the base does not have fails.
        assert!(check_gaps(&keys, None).is_empty());
        assert!(check_gaps(&keys, Some(&keys)).is_empty());
        let mut fewer = keys.clone();
        fewer.remove("x.md:3: `a`");
        assert!(
            check_gaps(&fewer, Some(&keys)).is_empty(),
            "closing a gap passes"
        );
        let new = check_gaps(&keys, Some(&fewer));
        assert_eq!(new.len(), 1);
        assert!(
            new[0].starts_with("x.md:3: `a`: a new crate gap"),
            "{new:?}"
        );
    }
}
