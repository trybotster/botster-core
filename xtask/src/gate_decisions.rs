//! `cargo xtask gate-decisions`: no mutation exclusion covers a gate decision (lead, 2026-10-08; plan r22 review PR1). A
//! decision is a tested pure function; an exclusion may cover only the I/O shell that calls it, and the entry names that
//! decision function (the `mutants_job` entry of #167 is the model).
//!
//! The check lists the mutants of the xtask as cargo-mutants generates them (`cargo mutants --list --json --no-config`,
//! which builds nothing) and applies every exclusion to them: each `exclude_re` entry of `.cargo/mutants.toml` with the
//! comment above it as its reason, each regex of `ci::OFF_MACOS_EXCLUSIONS`, and each `exclude_globs` entry.
//!
//! An exclusion that covers a mutant of xtask function F passes only when all of these hold (#181 B5):
//! - the mutant replaces the whole body of F (genre `FnValue`): an operator or a match-arm mutant is a decision mutant;
//! - F does process, file or signal I/O itself (it starts a process, reads or writes a file, sends a signal, or runs an
//!   xtask command), so F is an I/O shell: a function without I/O is a decision, also when it only forwards to another;
//! - its reason names a function D of the xtask that F calls, that a test calls, and that no exclusion covers.
//!
//! The check reads the calls from the syntax of the xtask. Every other exclusion of an xtask mutant fails, a glob or an
//! `OFF_MACOS_EXCLUSIONS` regex included (neither has a reason here).

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

/// The path calls that do process, file or signal I/O (`std::fs::write(..)`, `signal_group(..)`), by their last segment.
const IO_CALLS: [&str; 11] = [
    "read_to_string",
    "write",
    "create_dir_all",
    "remove_file",
    "remove_dir_all",
    "read_dir",
    "copy",
    "rename",
    "signal_group",
    "signal_process",
    "run_to_completion",
];

/// The method calls that start a process (`Command::status`, `output`, `spawn`).
const IO_METHODS: [&str; 3] = ["status", "output", "spawn"];

/// The calls of the xtask, from its syntax: what each function calls, by file and name, what the tests call, and which
/// functions do I/O themselves.
#[derive(Default, Debug)]
pub struct Calls {
    by_function: BTreeMap<(String, String), BTreeSet<String>>,
    tested: BTreeSet<String>,
    /// The path calls of each function, by file and name, as (the module segment before the name, if any; the name). A
    /// call of a local binding (a parameter, a closure) is not among them.
    paths: BTreeMap<(String, String), BTreeSet<(Option<String>, String)>>,
    /// The functions, by file and name, that do I/O: an I/O call (`IO_CALLS`, `IO_METHODS`), an xtask command run by its
    /// module path (`taint::command(..)`), or a path call of an xtask function that does I/O (`tools::run(..)`).
    io: BTreeSet<(String, String)>,
}

impl Calls {
    /// The calls of the Rust files `(path, text)`.
    ///
    /// # Errors
    /// A file does not parse.
    pub fn of(files: &[(String, String)]) -> Result<Calls> {
        let mut calls = Calls::default();
        for (file, text) in files {
            let parsed = syn::parse_file(text)
                .map_err(|error| anyhow::anyhow!("{file}: does not parse: {error}"))?;
            let mut index = Index {
                file,
                calls: &mut calls,
                function: Vec::new(),
                locals: Vec::new(),
                test: false,
            };
            index.visit_file(&parsed);
        }
        loop {
            let before = calls.io.len();
            let found: Vec<(String, String)> = calls
                .paths
                .iter()
                .filter(|(function, paths)| {
                    !calls.io.contains(*function)
                        && paths.iter().any(|(module, name)| {
                            calls
                                .callees(&function.0, module.as_deref(), name)
                                .iter()
                                .any(|callee| calls.io.contains(callee))
                        })
                })
                .map(|(function, _)| function.clone())
                .collect();
            calls.io.extend(found);
            if calls.io.len() == before {
                return Ok(calls);
            }
        }
    }

    /// The xtask functions that a path call in `file` names: with a module segment `m`, the function of `xtask/src/m.rs`
    /// or `xtask/src/m/mod.rs` (`self`, `Self`: of `file`); without one (or with `crate` or `super`), the function of
    /// `file` with that name, else each function of the xtask with that name.
    fn callees(&self, file: &str, module: Option<&str>, name: &str) -> Vec<(String, String)> {
        let known = |file: &str| {
            let key = (file.to_string(), name.to_string());
            self.by_function.contains_key(&key).then_some(key)
        };
        match module {
            Some("self" | "Self") => known(file).into_iter().collect(),
            Some(module) if module != "crate" && module != "super" => [
                format!("xtask/src/{module}.rs"),
                format!("xtask/src/{module}/mod.rs"),
            ]
            .iter()
            .filter_map(|file| known(file))
            .collect(),
            _ => known(file).map_or_else(
                || {
                    self.by_function
                        .keys()
                        .filter(|(_, function)| function == name)
                        .cloned()
                        .collect()
                },
                |key| vec![key],
            ),
        }
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
    /// The names that the function being read binds (parameters, `let`, patterns, closure parameters), innermost last.
    locals: Vec<BTreeSet<String>>,
    test: bool,
}

/// The names that a pattern binds, anywhere in a function.
#[derive(Default)]
struct Bindings(BTreeSet<String>);

impl<'ast> Visit<'ast> for Bindings {
    fn visit_pat_ident(&mut self, pat: &'ast syn::PatIdent) {
        self.0.insert(pat.ident.to_string());
        syn::visit::visit_pat_ident(self, pat);
    }
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

    /// Records that the current function does I/O.
    fn io(&mut self) {
        if let Some(function) = self.function.last() {
            self.calls
                .io
                .insert((self.file.to_string(), function.clone()));
        }
    }

    /// Records a path call of the current function, unless it calls a local binding.
    fn path_call(&mut self, path: &syn::Path) {
        let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        let Some((name, before)) = segments.split_last() else {
            return;
        };
        let local = before.is_empty() && self.locals.last().is_some_and(|l| l.contains(name));
        if let (Some(function), false) = (self.function.last(), local) {
            self.calls
                .paths
                .entry((self.file.to_string(), function.clone()))
                .or_default()
                .insert((before.last().cloned(), name.clone()));
        }
    }

    fn function(
        &mut self,
        name: String,
        attrs: &[syn::Attribute],
        locals: Bindings,
        visit: impl FnOnce(&mut Self),
    ) {
        let was = self.test;
        self.test |= is_test(attrs);
        self.calls
            .by_function
            .entry((self.file.to_string(), name.clone()))
            .or_default();
        self.function.push(name);
        self.locals.push(locals.0);
        visit(self);
        self.locals.pop();
        self.function.pop();
        self.test = was;
    }
}

impl<'ast> Visit<'ast> for Index<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let was = self.test;
        self.test |= is_test(&item.attrs);
        syn::visit::visit_item_mod(self, item);
        self.test = was;
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let mut locals = Bindings::default();
        locals.visit_item_fn(item);
        self.function(item.sig.ident.to_string(), &item.attrs, locals, |index| {
            syn::visit::visit_item_fn(index, item)
        });
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let mut locals = Bindings::default();
        locals.visit_impl_item_fn(item);
        self.function(item.sig.ident.to_string(), &item.attrs, locals, |index| {
            syn::visit::visit_impl_item_fn(index, item)
        });
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            if let Some(last) = path.path.segments.last() {
                let name = last.ident.to_string();
                let command = name == "command" && path.path.segments.len() == 2;
                if command || IO_CALLS.contains(&name.as_str()) {
                    self.io();
                }
                self.called(name);
                self.path_call(&path.path);
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if IO_METHODS.contains(&call.method.to_string().as_str()) {
            self.io();
        }
        self.called(call.method.to_string());
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        // A call inside `assert!`, `format!` and the like: the macro's arguments, when they are expressions.
        if let Ok(exprs) = mac.parse_body_with(
            syn::punctuated::Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated,
        ) {
            for expr in &exprs {
                self.visit_expr(expr);
            }
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

/// The violations: each exclusion that covers a decision mutant, a mutant of a function that does no I/O itself, or a
/// mutant of a function whose reason names no tested decision function that the function calls and that no exclusion
/// covers. One violation per exclusion, function and kind of mutant (whole body or not).
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
    let word = Regex::new(r"[A-Za-z_][A-Za-z0-9_]*").expect("regex");
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
            let io = calls.io.contains(&(mutant.file.clone(), short.to_string()));
            let callees = calls.calls(&mutant.file, short);
            let named = word
                .find_iter(&exclusion.reason)
                .map(|w| w.as_str())
                .any(|d| {
                    d != short
                        && callees.is_some_and(|c| c.contains(d))
                        && calls.tested.contains(d)
                        && !covered.contains(d)
                        && calls.by_function.keys().any(|(_, f)| f == d)
                });
            let problem = if !mutant.whole_body {
                "it is a decision mutant: an exclusion covers only the whole-body replacement of an I/O shell"
            } else if !io {
                "the function does no process, file or signal I/O itself, so it is a decision, which is never excluded"
            } else if !named {
                "its reason names no tested decision function that the shell calls and that no exclusion covers"
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
