//! `cargo xtask gate-decisions`: no mutation exclusion covers a gate decision (lead, 2026-10-08; plan r22 review PR1). A
//! decision is a tested pure function; an exclusion may cover only the I/O shell that calls it, and the entry names that
//! decision function (the `mutants_job` entry of #167 is the model).
//!
//! The check lists the mutants of the xtask as cargo-mutants generates them (`cargo mutants --list --json --no-config`,
//! which builds nothing) and applies every exclusion to them: each `exclude_re` entry of `.cargo/mutants.toml` with the
//! comment above it as its reason, each regex of `ci::OFF_MACOS_EXCLUSIONS`, and each `exclude_globs` entry.
//!
//! An exclusion that covers a mutant of xtask function F passes only when both of these hold (#181 B5):
//! - the mutant replaces the whole body of F (genre `FnValue`): an operator or a match-arm mutant is a decision mutant;
//! - its reason cites a proof (plan section 8, r23d): a function D of the xtask, other than F, that F calls, that a test
//!   calls and that no exclusion covers, written `D (proof, ..)`; mutants-cited checks that each proof is a test that a
//!   gate tier runs. A decision named only in free text does not count.
//!
//! The check does not decide from the source whether F does I/O (plan section 8, 23g, "No automatic I/O
//! classification"). That F is an I/O shell rests on the reason of its exclusion and on review: every change to
//! `.cargo/mutants.toml` is HIGH (`ci/high-tier-paths.txt`), so the package reviewer and the integration reviewer each
//! read every new or changed exclusion.
//!
//! The check reads the calls from the syntax of the xtask, by name: a call `f(..)`, `m::f(..)` or `x.f(..)` is a call of
//! each xtask function named `f`. A test is a function with a `#[test]` attribute, or a function in a `#[cfg(test)]`
//! module. The check reads the arguments of an `ARGUMENT_MACROS` macro as code of the caller (a macro is one by its path
//! as written: the listed path, or its last segment alone). It reads no token of any other macro, so a call there does
//! not count, and the check fails closed.
//!
//! The check defends against honest drift and mistakes, not deliberate evasion (plan section 8, 23f). A declaration that
//! introduces a reserved name (`is_reserved`: the name and the crate of each `ARGUMENT_MACROS` macro) fails, and the
//! check names the form and the file: a module, a struct, an enum, a union, a trait, a type alias, a constant, a static,
//! a `macro_rules!` definition, an `extern crate` rename, and a `use` unless its path ends in the name and is the listed
//! path, the crate or a `std` or `core` path.
//!
//! Every other exclusion of an xtask mutant fails, a glob or an `OFF_MACOS_EXCLUSIONS` regex included (neither has a
//! reason here).

use crate::mutants_cited::MUTANTS_FILE;
use crate::tools::{cargo, require_cargo_tool};
use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::visit::Visit;

/// A mutant as `cargo mutants --list --json` gives it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mutant {
    pub file: String,
    /// The function as cargo-mutants names it (`f`, `Type::f`, `<impl Trait for Type>::f`); empty outside a function.
    pub function: String,
    pub name: String,
    /// Whether the mutant replaces the whole body of its function (genre `FnValue`).
    pub whole_body: bool,
}

impl Mutant {
    /// The function's own name, the last segment.
    fn short(&self) -> &str {
        self.function.rsplit("::").next().unwrap_or_default()
    }
}

/// An exclusion and its reason (empty when it has none).
#[derive(Clone, Debug)]
pub struct Exclusion {
    pub pattern: String,
    pub reason: String,
}

/// The macros whose arguments are expressions of the caller: each expands to an expression and binds or imports no name
/// of the caller, so the check reads its arguments as code of the caller (#181 B5 round 5). A macro is one of these by
/// its path as written: the listed path, or its last segment alone (`bail!` is `anyhow::bail!`). The reserved names
/// (`is_reserved`) keep a declaration from giving the name to another macro.
const ARGUMENT_MACROS: [&[&str]; 22] = [
    &["assert"],
    &["assert_eq"],
    &["assert_ne"],
    &["debug_assert"],
    &["debug_assert_eq"],
    &["debug_assert_ne"],
    &["eprint"],
    &["eprintln"],
    &["format"],
    &["panic"],
    &["print"],
    &["println"],
    &["todo"],
    &["unimplemented"],
    &["unreachable"],
    &["vec"],
    &["write"],
    &["writeln"],
    &["anyhow", "anyhow"],
    &["anyhow", "bail"],
    &["anyhow", "ensure"],
    &["format_args"],
];

/// Whether `name` is a reserved name of the check (plan section 8, 23f): a name that it resolves by text. These are the
/// name and the crate of each `ARGUMENT_MACROS` macro. A declaration that introduces one fails (`Index::visit_item`,
/// `Index::visit_item_use`).
fn is_reserved(name: &str) -> bool {
    ARGUMENT_MACROS
        .iter()
        .flat_map(|path| [path[0], path[path.len() - 1]])
        .any(|reserved| reserved == name)
}

/// Whether the macro `path` is an `ARGUMENT_MACROS` macro: a listed path, or the last segment of one alone (`bail` is
/// `anyhow::bail`).
fn is_argument_macro(path: &syn::Path) -> bool {
    let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
    ARGUMENT_MACROS.iter().any(|want| {
        is_path(&segments, want)
            || (segments.len() == 1 && want.last() == Some(&segments[0].as_str()))
    })
}

/// Whether `path` is `want`, segment by segment.
fn is_path(path: &[String], want: &[&str]) -> bool {
    path.len() == want.len() && path.iter().zip(want).all(|(segment, want)| segment == want)
}

/// The calls of the xtask, from its syntax: what each function calls, by file and name, and what the tests call.
#[derive(Default, Debug)]
pub struct Calls {
    by_function: BTreeMap<(String, String), BTreeSet<String>>,
    tested: BTreeSet<String>,
}

impl Calls {
    /// The calls of the Rust files `(path, text)`.
    ///
    /// # Errors
    /// A file does not parse, or it declares a reserved name (each such declaration is reported).
    pub fn of(files: &[(String, String)]) -> Result<Calls> {
        let mut calls = Calls::default();
        let mut rejected = Vec::new();
        for (file, text) in files {
            let parsed = syn::parse_file(text)
                .map_err(|error| anyhow::anyhow!("{file}: does not parse: {error}"))?;
            let mut index = Index {
                file,
                calls: &mut calls,
                function: Vec::new(),
                test: false,
                rejected: Vec::new(),
            };
            index.visit_file(&parsed);
            rejected.extend(index.rejected.iter().map(|at| format!("{file}:{at}")));
        }
        if !rejected.is_empty() {
            bail!("{}", rejected.join("\n"));
        }
        Ok(calls)
    }

    fn calls(&self, file: &str, function: &str) -> Option<&BTreeSet<String>> {
        self.by_function
            .get(&(file.to_string(), function.to_string()))
    }
}

struct Index<'a> {
    file: &'a str,
    calls: &'a mut Calls,
    /// The function being read, innermost last.
    function: Vec<String>,
    test: bool,
    /// The declarations of reserved names, each as `line:column: what`, in the order found.
    rejected: Vec<String>,
}

/// The arguments of `mac` that the check reads as code of the caller: those of an `ARGUMENT_MACROS` macro, when they parse
/// as expressions separated by commas.
fn macro_arguments(mac: &syn::Macro) -> Vec<syn::Expr> {
    if !is_argument_macro(&mac.path) {
        return Vec::new();
    }
    mac.parse_body_with(syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        .map(|exprs| exprs.into_iter().collect())
        .unwrap_or_default()
}

fn is_test(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        (path.is_ident("cfg")
            && attr
                .parse_args::<syn::Ident>()
                .is_ok_and(|ident| ident == "test"))
            || path
                .segments
                .last()
                .is_some_and(|last| last.ident == "test")
    })
}

impl Index<'_> {
    fn called(&mut self, name: String) {
        if self.test {
            self.calls.tested.insert(name.clone());
        }
        if let Some(function) = self.function.last() {
            self.calls
                .by_function
                .entry((self.file.to_string(), function.clone()))
                .or_default()
                .insert(name);
        }
    }

    fn function(
        &mut self,
        ident: &syn::Ident,
        attrs: &[syn::Attribute],
        visit: impl FnOnce(&mut Self),
    ) {
        let name = ident.to_string();
        let was = self.test;
        self.test |= is_test(attrs);
        self.calls
            .by_function
            .entry((self.file.to_string(), name.clone()))
            .or_default();
        self.function.push(name);
        visit(self);
        self.function.pop();
        self.test = was;
    }
}

impl<'ast> Visit<'ast> for Index<'_> {
    /// A declaration that introduces a reserved name (`is_reserved`) fails (plan section 8, 23f; #181 R6-1): the check
    /// reads a macro by the text of its path, so a `macro_rules! assert`, a `mod anyhow` would take it.
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let declared = match item {
            syn::Item::Const(item) => Some((&item.ident, "constant")),
            syn::Item::Enum(item) => Some((&item.ident, "enum")),
            // `extern crate x;` names the crate x itself; only a rename introduces a name.
            syn::Item::ExternCrate(syn::ItemExternCrate {
                rename: Some((_, rename)),
                ..
            }) => Some((rename, "extern crate")),
            syn::Item::Macro(syn::ItemMacro {
                ident: Some(ident), ..
            }) => Some((ident, "macro")),
            syn::Item::Mod(item) => Some((&item.ident, "module")),
            syn::Item::Static(item) => Some((&item.ident, "static")),
            syn::Item::Struct(item) => Some((&item.ident, "struct")),
            syn::Item::Trait(item) => Some((&item.ident, "trait")),
            syn::Item::Type(item) => Some((&item.ident, "type alias")),
            syn::Item::Union(item) => Some((&item.ident, "union")),
            _ => None,
        };
        if let Some((ident, kind)) = declared.filter(|(ident, _)| is_reserved(&ident.to_string())) {
            let at = ident.span().start();
            self.rejected.push(format!(
                "{}:{}: the {kind} `{ident}` declares a reserved name of gate-decisions (plan section 8): the check \
                 resolves the name by text; rename it",
                at.line,
                at.column + 1
            ));
        }
        syn::visit::visit_item(self, item);
    }

    /// A `use` that introduces a reserved name fails unless its path ends in that name and is a listed macro, the crate
    /// itself, or a `std` or `core` path (`use anyhow::bail;`, `use anyhow;`, `use std::fs::write;`). A rename (`use syn::parse_quote as bail;`) fails, and so no local
    /// module exports a reserved name to a glob (plan 23f, #181 R6-1).
    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        let uses = crate::process_check::Uses::of([&syn::Item::Use(item.clone())], false);
        for (name, target) in uses.bindings() {
            let listed = ARGUMENT_MACROS.iter().any(|want| is_path(target, want));
            let own = target.last() == Some(name)
                && (listed || target.len() == 1 || matches!(target[0].as_str(), "std" | "core"));
            if is_reserved(name) && !own {
                let at = item.use_token.span.start();
                self.rejected.push(format!(
                    "{}:{}: the `use` introduces the reserved name `{name}` of gate-decisions as `{}` (plan section 8): \
                     only its listed path, its crate or a `std` or `core` path may introduce it",
                    at.line,
                    at.column + 1,
                    target.join("::")
                ));
            }
        }
        syn::visit::visit_item_use(self, item);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let was = self.test;
        self.test |= is_test(&item.attrs);
        syn::visit::visit_item_mod(self, item);
        self.test = was;
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.function(&item.sig.ident, &item.attrs, |index| {
            syn::visit::visit_item_fn(index, item)
        });
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.function(&item.sig.ident, &item.attrs, |index| {
            syn::visit::visit_impl_item_fn(index, item)
        });
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            if let Some(last) = path.path.segments.last() {
                self.called(last.ident.to_string());
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.called(call.method.to_string());
        syn::visit::visit_expr_method_call(self, call);
    }

    /// The check reads the arguments of an `ARGUMENT_MACROS` macro as code of the caller, and no token of any other macro.
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        for expr in &macro_arguments(mac) {
            self.visit_expr(expr);
        }
    }
}

/// The mutants of a `cargo mutants --list --json` document. A mutant outside a function (a constant) has no function.
///
/// # Errors
/// The document is not the list of mutants.
pub fn parse_mutants(json: &str) -> Result<Vec<Mutant>> {
    let list: serde_json::Value =
        serde_json::from_str(json).context("the mutant list is not JSON")?;
    let mut mutants = Vec::new();
    for item in list.as_array().context("the mutant list is not an array")? {
        let text = |value: &serde_json::Value, what: &str| -> Result<String> {
            value
                .as_str()
                .map(str::to_string)
                .with_context(|| format!("a mutant has no {what}"))
        };
        mutants.push(Mutant {
            file: text(&item["file"], "file")?,
            function: item["function"]["function_name"]
                .as_str()
                .unwrap_or_default()
                .to_string(),
            name: text(&item["name"], "name")?,
            whole_body: text(&item["genre"], "genre")? == "FnValue",
        });
    }
    Ok(mutants)
}

/// The `exclude_re` entries of the mutants file, each with the comment block above it (or above its group of entries).
///
/// # Errors
/// An entry line is not a single-quoted string.
pub fn exclusions_of(toml: &str) -> Result<Vec<Exclusion>> {
    let mut entries = Vec::new();
    let mut inside = false;
    let mut reason: Vec<&str> = Vec::new();
    let mut last_was_comment = false;
    for line in toml.lines() {
        let line = line.trim();
        if !inside {
            inside = line.starts_with("exclude_re");
            continue;
        }
        if line == "]" {
            break;
        }
        if let Some(comment) = line.strip_prefix('#') {
            if !last_was_comment {
                reason.clear();
            }
            reason.push(comment.trim());
            last_was_comment = true;
        } else if !line.is_empty() {
            let Some(pattern) = line.strip_prefix('\'').and_then(|l| l.strip_suffix("',")) else {
                bail!("{MUTANTS_FILE}: an exclude_re entry that is not a single-quoted string: {line}");
            };
            entries.push(Exclusion {
                pattern: pattern.to_string(),
                reason: reason.join(" "),
            });
            last_was_comment = false;
        }
    }
    Ok(entries)
}

/// A glob of `exclude_globs` as a regex over a path: `**/` is any directories, `**` and `*` any text (`*` within one
/// component), `?` one character. A glob with no `/` matches the file name in any directory.
fn glob_regex(glob: &str) -> Regex {
    let mut pattern = String::from(if glob.contains('/') { "^" } else { "(^|/)" });
    let mut rest = glob;
    while let Some(c) = rest.chars().next() {
        let (piece, used) = if rest.starts_with("**/") {
            ("(.*/)?".to_string(), 3)
        } else if rest.starts_with("**") {
            (".*".to_string(), 2)
        } else if c == '*' {
            ("[^/]*".to_string(), 1)
        } else if c == '?' {
            ("[^/]".to_string(), 1)
        } else {
            (regex::escape(&c.to_string()), c.len_utf8())
        };
        pattern.push_str(&piece);
        rest = &rest[used..];
    }
    pattern.push('$');
    Regex::new(&pattern).expect("an escaped glob")
}

/// Whether an exclusion covers a mutant.
type Covers = Box<dyn Fn(&Mutant) -> bool>;

/// The violations: each exclusion that covers a decision mutant, or a mutant of a function whose reason cites no tested
/// decision function that the function calls and that no exclusion covers. One violation per exclusion, function and kind of mutant (whole body or not).
///
/// # Errors
/// An exclusion is not a valid regex.
pub fn check(
    mutants: &[Mutant],
    exclusions: &[Exclusion],
    globs: &[String],
    calls: &Calls,
) -> Result<Vec<String>> {
    let mut all: Vec<(Exclusion, Covers)> = Vec::new();
    for exclusion in exclusions {
        let regex = Regex::new(&exclusion.pattern)
            .with_context(|| format!("the exclusion `{}`", exclusion.pattern))?;
        all.push((
            exclusion.clone(),
            Box::new(move |m: &Mutant| regex.is_match(&m.name)),
        ));
    }
    for glob in globs {
        let regex = glob_regex(glob);
        all.push((
            Exclusion {
                pattern: glob.clone(),
                reason: String::new(),
            },
            Box::new(move |m: &Mutant| regex.is_match(&m.file)),
        ));
    }
    let covered: BTreeSet<&str> = mutants
        .iter()
        .filter(|m| all.iter().any(|(_, covers)| covers(m)))
        .map(Mutant::short)
        .collect();
    let mut seen = BTreeSet::new();
    let mut violations = Vec::new();
    for mutant in mutants {
        for (exclusion, covers) in &all {
            if !covers(mutant)
                || !seen.insert((
                    exclusion.pattern.clone(),
                    mutant.file.clone(),
                    mutant.function.clone(),
                    mutant.whole_body,
                ))
            {
                continue;
            }
            let short = mutant.short();
            let callees = calls.calls(&mutant.file, short);
            // Plan section 8 (r23d): only a proof citation `decision (proof, ..)` names a decision; a name in free text
            // does not. mutants-cited checks that each proof is a test that a gate tier runs.
            let named = crate::mutants_cited::citations(&exclusion.reason)
                .iter()
                .map(|citation| citation.decision.as_str())
                .any(|d| {
                    d != short
                        && callees.is_some_and(|c| c.contains(d))
                        && calls.tested.contains(d)
                        && !covered.contains(d)
                        && calls.by_function.keys().any(|(_, f)| f == d)
                });
            let problem = if !mutant.whole_body {
                "it is a decision mutant: an exclusion covers only the whole-body replacement of an I/O shell"
            } else if !named {
                "its reason cites no tested decision function that the shell calls and that no exclusion covers, as \
                 `decision (proof, ..)`"
            } else {
                continue;
            };
            violations.push(format!(
                "`{}` excludes {} in {} (`{}`): {problem}",
                exclusion.pattern,
                if mutant.function.is_empty() {
                    "code outside a function"
                } else {
                    &mutant.function
                },
                mutant.file,
                mutant.name
            ));
        }
    }
    Ok(violations)
}

/// What the check reads from a repository: the `exclude_re` entries of the mutants file with their reasons and the
/// off-macOS exclusions of the gate, the `exclude_globs`, and the calls of the xtask's sources.
struct Inputs {
    exclusions: Vec<Exclusion>,
    globs: Vec<String>,
    calls: Calls,
}

/// The inputs of the check in `root`.
///
/// # Errors
/// The mutants file cannot be read or parsed, an `exclude_re` entry has no reason block, or a source cannot be read or
/// parsed.
fn inputs(root: &Path) -> Result<Inputs> {
    let text = std::fs::read_to_string(root.join(MUTANTS_FILE))
        .with_context(|| format!("read {MUTANTS_FILE}"))?;
    let config: toml::Table = text
        .parse()
        .with_context(|| format!("parse {MUTANTS_FILE}"))?;
    let globs: Vec<String> = config
        .get("exclude_globs")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect();
    let mut exclusions = exclusions_of(&text)?;
    let configured = config
        .get("exclude_re")
        .and_then(toml::Value::as_array)
        .map_or(0, Vec::len);
    if exclusions.len() != configured {
        bail!("{MUTANTS_FILE}: {} exclude_re entries read with their reasons, {configured} configured", exclusions.len());
    }
    exclusions.extend(crate::ci::OFF_MACOS_EXCLUSIONS.iter().map(|re| Exclusion {
        pattern: (*re).to_string(),
        reason: String::new(),
    }));
    let mut files = Vec::new();
    for file in crate::fsutil::tracked_files(root)? {
        if file.starts_with("xtask/src/") && file.ends_with(".rs") {
            files.push((file.clone(), std::fs::read_to_string(root.join(&file))?));
        }
    }
    Ok(Inputs {
        exclusions,
        globs,
        calls: Calls::of(&files)?,
    })
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    require_cargo_tool(
        root,
        &["mutants", "--version"],
        "cargo install cargo-mutants --version 27.1.0 --locked",
    )?;
    let output = cargo(root)
        .args(["mutants", "--list", "--json", "--no-config", "-p", "xtask"])
        .output()
        .context("run cargo mutants --list")?;
    let mutants = parse_mutants(&crate::tools::stdout_of(&output, "cargo mutants --list")?)?;
    let inputs = inputs(root)?;
    println!(
        "gate-decisions: {} mutants of the xtask, {} exclude_re entries, {} exclude_globs",
        mutants.len(),
        inputs.exclusions.len(),
        inputs.globs.len()
    );
    crate::tools::verdict(&check(
        &mutants,
        &inputs.exclusions,
        &inputs.globs,
        &inputs.calls,
    )?)
}

#[cfg(test)]
mod tests;
