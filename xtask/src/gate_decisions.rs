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
//! - F does I/O, so F is an I/O shell: F performs a listed I/O operation, or calls an xtask function that does I/O;
//! - its reason cites a proof (plan section 8, r23d): a function D of the xtask that F calls, that a test calls and that no
//!   exclusion covers, written `D (proof, ..)`; mutants-cited checks that each proof is a test that a gate tier runs. A
//!   decision named only in free text does not count.
//!
//! The forms that the check resolves (plan section 8, "Source-reading checks accept a closed set of forms"):
//! - a call path, through the `use` declarations of its file, inline module and block (`process_check::resolve`);
//! - the module of a function from its file path: `xtask/src/main.rs` is the crate root, `crate::`, `super::` and a child
//!   or root module `m::f` (`Calls::callees`); a plain `f` is the `f` of its file, else each `f` of the xtask;
//! - the I/O operations: a free function of `std::fs` or `std::env` (`IO_MODULES`; not a type's function such as the
//!   builder `OpenOptions::new`), a signal or `run_to_completion` (`IO_FUNCTIONS`), and a `status`, `output` or `spawn`
//!   call on a process command: a method chain that begins at `Command::new(..)` or at a call of an xtask function
//!   declared to return `Command`, a parameter typed `Command` (also by reference), or a `let` bound to such a chain.
//!   `Command::new` alone is a builder and starts nothing.
//!
//! A `#[path]` module in the xtask is not a listed form, so the check fails on it. Any other way of doing I/O (a start on
//! a field, a closure that holds a command) is not recognized: a shell that does only that is no shell for the check, so
//! its exclusion fails until the code takes a listed form or a reviewed change extends the list.
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

/// The modules whose free functions do I/O: the file system and the environment of the process. A free function is one
/// lowercase segment after the module (`std::fs::write`, `std::env::var`); a function of a type (`std::fs::OpenOptions::new`,
/// a builder) is not one. A call does I/O by the full path that it resolves to through the `use` declarations in its scope;
/// a name alone (a parameter `write`, a method `status`) does not (#181 B5 round 2).
const IO_MODULES: [&[&str]; 2] = [&["std", "fs"], &["std", "env"]];

/// The functions that do I/O, by their full path: a signal and the bounded run of a tool.
const IO_FUNCTIONS: [&[&str]; 4] = [
    &["botster_core_sys", "signal", "signal_group"],
    &["botster_core_sys", "signal", "signal_process"],
    &["botster_core_sys", "signal", "signal_own_group"],
    &["botster_test_process", "run_to_completion"],
];

/// The constructor of a process command. It is a builder and starts nothing (#181 B5 round 3): only a `SPAWN_METHODS`
/// call on it starts a process.
const COMMAND_NEW: [&str; 4] = ["std", "process", "Command", "new"];

/// The type of a process command, as a function's declared return type.
const COMMAND: [&str; 3] = ["std", "process", "Command"];

/// The methods of a process command that start the process.
const SPAWN_METHODS: [&str; 3] = ["status", "output", "spawn"];

/// Whether `path` is `want`, segment by segment.
fn is_path(path: &[String], want: &[&str]) -> bool {
    path.len() == want.len() && path.iter().zip(want).all(|(segment, want)| segment == want)
}

/// Whether a call of the resolved path `path` does I/O: a free function of an `IO_MODULES` module, or an `IO_FUNCTIONS`
/// function.
fn io_path(path: &[String]) -> bool {
    IO_FUNCTIONS.iter().any(|function| is_path(path, function))
        || IO_MODULES.iter().any(|module| {
            path.split_last().is_some_and(|(name, prefix)| {
                is_path(prefix, module) && name.starts_with(|c: char| c.is_ascii_lowercase())
            })
        })
}

/// The calls of the xtask, from its syntax: what each function calls, by file and name, what the tests call, and which
/// functions do I/O themselves.
#[derive(Default, Debug)]
pub struct Calls {
    by_function: BTreeMap<(String, String), BTreeSet<String>>,
    tested: BTreeSet<String>,
    /// The path calls of each function, by file and name, each resolved through the `use` declarations in its scope. A
    /// call of a local binding (a parameter, a closure) is not among them.
    paths: BTreeMap<(String, String), BTreeSet<Vec<String>>>,
    /// The functions, by file and name, that do I/O: a call that resolves to an I/O path (`io_path`), a process start, or
    /// a call of an xtask function that does I/O (`callees`).
    io: BTreeSet<(String, String)>,
    /// The functions, by file and name, whose declared return type is `std::process::Command`.
    commands: BTreeSet<(String, String)>,
    /// The process starts of each function, by file and name, whose receiver chain begins at a call of the resolved path
    /// (other than `Command::new`): a start when the path names an xtask function of `commands`.
    starts: BTreeMap<(String, String), BTreeSet<Vec<String>>>,
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
            let mut paths = PathModules::default();
            paths.visit_file(&parsed);
            if let Some((line, column, name)) = paths.0.first() {
                bail!(
                    "{file}:{line}:{column}: `#[path]` on the module `{name}` is not a form that gate-decisions resolves \
                     (plan section 8): it reads the module of a function from its file path"
                );
            }
            let mut index = Index {
                file,
                calls: &mut calls,
                function: Vec::new(),
                locals: Vec::new(),
                scopes: vec![crate::process_check::Uses::of(&parsed.items, true)],
                test: false,
                command_locals: Vec::new(),
            };
            index.visit_file(&parsed);
        }
        let started: Vec<(String, String)> = calls
            .starts
            .iter()
            .filter(|(function, roots)| {
                roots.iter().any(|root| {
                    calls
                        .callees(&function.0, root)
                        .iter()
                        .any(|callee| calls.commands.contains(callee))
                })
            })
            .map(|(function, _)| function.clone())
            .collect();
        calls.io.extend(started);
        // Each round adds a function or ends the search, so there are at most as many rounds as functions.
        for _ in 0..=calls.paths.len() {
            let before = calls.io.len();
            let found: Vec<(String, String)> = calls
                .paths
                .iter()
                .filter(|(function, paths)| {
                    !calls.io.contains(*function)
                        && paths.iter().any(|path| {
                            calls
                                .callees(&function.0, path)
                                .iter()
                                .any(|callee| calls.io.contains(callee))
                        })
                })
                .map(|(function, _)| function.clone())
                .collect();
            calls.io.extend(found);
            if calls.io.len() == before {
                break;
            }
        }
        Ok(calls)
    }

    /// The xtask functions that a resolved path call in `file` names. A module path names the file of the module:
    /// `[]` is `xtask/src/main.rs` (the crate root), `[a, b]` is `xtask/src/a/b.rs` or `xtask/src/a/b/mod.rs`.
    /// `crate::m::f` names the `f` of the module `m`, each leading `super` the parent of the module of `file`, and `m::f`
    /// the `f` of the child `m` of the module of `file`, else of the module `m`. A leading `self` (and a `super` inside an
    /// inline module) never comes here: `process_check::resolve` removes it. `Self::f`
    /// names the `f` of `file`, and a plain `f` the `f` of `file`, else each `f` of the xtask (a glob import). A path of
    /// another crate, or a `super` above the crate root, names none.
    fn callees(&self, file: &str, path: &[String]) -> Vec<(String, String)> {
        let Some((name, modules)) = path.split_last() else {
            return Vec::new();
        };
        let known = |file: &str| {
            let key = (file.to_string(), name.clone());
            self.by_function.contains_key(&key).then_some(key)
        };
        let in_module = |module: &[String]| -> Vec<(String, String)> {
            let files = if module.is_empty() {
                vec!["xtask/src/main.rs".to_string()]
            } else {
                let dir = module.join("/");
                vec![
                    format!("xtask/src/{dir}.rs"),
                    format!("xtask/src/{dir}/mod.rs"),
                ]
            };
            files.iter().filter_map(|file| known(file)).collect()
        };
        let mut here = module_path(file);
        match modules {
            [] => known(file).map_or_else(
                || {
                    self.by_function
                        .keys()
                        .filter(|(_, function)| function == name)
                        .cloned()
                        .collect()
                },
                |key| vec![key],
            ),
            [only] if only == "Self" => known(file).into_iter().collect(),
            [first, rest @ ..] if first == "crate" => in_module(rest),
            [first, ..] if first == "super" => {
                let supers = modules.iter().take_while(|m| *m == "super").count();
                if supers > here.len() {
                    return Vec::new();
                }
                here.truncate(here.len() - supers);
                here.extend_from_slice(&modules[supers..]);
                in_module(&here)
            }
            _ => {
                here.extend_from_slice(modules);
                let child = in_module(&here);
                if child.is_empty() {
                    in_module(modules)
                } else {
                    child
                }
            }
        }
    }

    fn calls(&self, file: &str, function: &str) -> Option<&BTreeSet<String>> {
        self.by_function
            .get(&(file.to_string(), function.to_string()))
    }
}

/// The module path of an xtask file: `xtask/src/main.rs` is the crate root (`[]`), `xtask/src/a.rs` and
/// `xtask/src/a/mod.rs` are `[a]`, and `xtask/src/a/b.rs` is `[a, b]`.
fn module_path(file: &str) -> Vec<String> {
    let path = file.strip_prefix("xtask/src/").unwrap_or(file);
    let path = path.strip_suffix(".rs").unwrap_or(path);
    let path = path.strip_suffix("/mod").unwrap_or(path);
    if path == "main" {
        return Vec::new();
    }
    path.split('/').map(str::to_string).collect()
}

struct Index<'a> {
    file: &'a str,
    calls: &'a mut Calls,
    /// The function being read, innermost last.
    function: Vec<String>,
    /// The names that the function being read binds (parameters, `let`, patterns, closure parameters), innermost last.
    locals: Vec<BTreeSet<String>>,
    /// The `use` scopes around the current code, the file's own first.
    scopes: Vec<crate::process_check::Uses>,
    test: bool,
    /// The bindings of the function being read that hold a process command, innermost last: a parameter typed `Command`
    /// (`None`), or a `let` whose initializer begins at a call of the resolved path (`Some`, kept for `Calls::of` unless it
    /// is `Command::new`).
    command_locals: Vec<BTreeMap<String, Option<Vec<String>>>>,
}

/// The syntax of the bindings of a function that may hold a process command: each typed parameter with its type's path
/// (a reference stripped), and each `let` with the path of the call that begins its initializer's method chain.
#[derive(Default)]
struct CommandBindings {
    typed: Vec<(String, Vec<String>)>,
    started: Vec<(String, Vec<String>)>,
}

impl<'ast> Visit<'ast> for CommandBindings {
    fn visit_pat_type(&mut self, pat: &'ast syn::PatType) {
        let mut ty = &*pat.ty;
        while let syn::Type::Reference(inner) = ty {
            ty = &inner.elem;
        }
        if let (syn::Pat::Ident(name), syn::Type::Path(ty)) = (&*pat.pat, ty) {
            self.typed.push((
                name.ident.to_string(),
                ty.path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect(),
            ));
        }
        syn::visit::visit_pat_type(self, pat);
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let (syn::Pat::Ident(name), Some(init)) = (&local.pat, &local.init) {
            if let Some(path) = chain_root(&init.expr) {
                self.started.push((name.ident.to_string(), path));
            }
        }
        syn::visit::visit_local(self, local);
    }
}

/// The path of the call that begins the method chain `expr` (`Command::new("git").arg(x)` gives `Command::new`).
fn chain_root(expr: &syn::Expr) -> Option<Vec<String>> {
    let mut root = expr;
    while let syn::Expr::MethodCall(inner) = root {
        root = &inner.receiver;
    }
    let syn::Expr::Call(syn::ExprCall { func, .. }) = root else {
        return None;
    };
    let syn::Expr::Path(path) = &**func else {
        return None;
    };
    Some(
        path.path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect(),
    )
}

/// The modules with a `#[path]` attribute, as line, column and name. gate-decisions resolves `crate::`, `super::` and
/// `m::f` from the file path of a function (`module_path`), so a `#[path]` module is a form that it does not resolve.
#[derive(Default)]
struct PathModules(Vec<(usize, usize, String)>);

impl<'ast> Visit<'ast> for PathModules {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if let Some(attr) = module
            .attrs
            .iter()
            .find(|attr| attr.path().is_ident("path"))
        {
            let at = attr.pound_token.span.start();
            self.0
                .push((at.line, at.column + 1, module.ident.to_string()));
        }
        syn::visit::visit_item_mod(self, module);
    }
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

    /// Records a path call of the current function, resolved through the `use` scopes: an I/O call marks the function,
    /// another call is kept for `callees`. A call of a local binding (a parameter `write`, a closure) is neither.
    fn path_call(&mut self, path: &syn::Path) {
        let Some(resolved) = self.resolved(path) else {
            return;
        };
        if io_path(&resolved) {
            self.io();
        } else if let Some(function) = self.function.last() {
            self.calls
                .paths
                .entry((self.file.to_string(), function.clone()))
                .or_default()
                .insert(resolved);
        }
    }

    /// The full path of a called `path` through the `use` scopes; `None` for a call of a local binding (a parameter `write`,
    /// a closure).
    fn resolved(&self, path: &syn::Path) -> Option<Vec<String>> {
        let segments: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
        if let [name] = &segments[..] {
            if self.locals.last().is_some_and(|l| l.contains(name)) {
                return None;
            }
        }
        Some(crate::process_check::resolve(&self.scopes, &segments))
    }

    /// Records a process start of the current function: a `SPAWN_METHODS` call whose receiver chain (method calls) begins
    /// at a call of `Command::new`, or of a function that may return a command (kept for `Calls::of`). A start on any
    /// other receiver (a local binding, a field) is not recognized, so a shell that does only that does no I/O for the
    /// check (plan section 8: the form is not listed).
    fn process_start(&mut self, call: &syn::ExprMethodCall) {
        if !SPAWN_METHODS.contains(&call.method.to_string().as_str()) {
            return;
        }
        let mut receiver = &*call.receiver;
        while let syn::Expr::MethodCall(inner) = receiver {
            receiver = &inner.receiver;
        }
        let resolved = match receiver {
            syn::Expr::Call(syn::ExprCall { func, .. }) => match &**func {
                syn::Expr::Path(path) => self.resolved(&path.path),
                _ => None,
            },
            syn::Expr::Path(path) => path.path.get_ident().and_then(|name| {
                self.command_locals
                    .last()
                    .and_then(|locals| locals.get(&name.to_string()))
                    .map(|held| {
                        held.clone()
                            .unwrap_or_else(|| COMMAND_NEW.map(str::to_string).to_vec())
                    })
            }),
            _ => None,
        };
        let Some(resolved) = resolved else {
            return;
        };
        if is_path(&resolved, &COMMAND_NEW) {
            self.io();
        } else if let Some(function) = self.function.last() {
            self.calls
                .starts
                .entry((self.file.to_string(), function.clone()))
                .or_default()
                .insert(resolved);
        }
    }

    /// Records that the function `name` returns a process command, when its declared return type resolves to `COMMAND`.
    fn returns(&mut self, name: &str, output: &syn::ReturnType) {
        let syn::ReturnType::Type(_, ty) = output else {
            return;
        };
        let syn::Type::Path(ty) = &**ty else {
            return;
        };
        let segments: Vec<String> = ty
            .path
            .segments
            .iter()
            .map(|s| s.ident.to_string())
            .collect();
        if is_path(
            &crate::process_check::resolve(&self.scopes, &segments),
            &COMMAND,
        ) {
            self.calls
                .commands
                .insert((self.file.to_string(), name.to_string()));
        }
    }

    /// Visits code inside the `use` scope of `items`.
    fn with_uses<'a>(
        &mut self,
        items: impl IntoIterator<Item = &'a syn::Item>,
        module: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        self.scopes
            .push(crate::process_check::Uses::of(items, module));
        visit(self);
        self.scopes.pop();
    }

    /// The command bindings (`command_locals`) of a function from the syntax of its bindings.
    fn command_locals(&self, bindings: CommandBindings) -> BTreeMap<String, Option<Vec<String>>> {
        let mut held = BTreeMap::new();
        for (name, ty) in bindings.typed {
            if is_path(&crate::process_check::resolve(&self.scopes, &ty), &COMMAND) {
                held.insert(name, None);
            }
        }
        for (name, root) in bindings.started {
            held.insert(
                name,
                Some(crate::process_check::resolve(&self.scopes, &root)),
            );
        }
        held
    }

    fn function(
        &mut self,
        name: String,
        attrs: &[syn::Attribute],
        (locals, commands): (Bindings, CommandBindings),
        visit: impl FnOnce(&mut Self),
    ) {
        let commands = self.command_locals(commands);
        self.command_locals.push(commands);
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
        self.command_locals.pop();
        self.function.pop();
        self.test = was;
    }
}

impl<'ast> Visit<'ast> for Index<'_> {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        let was = self.test;
        self.test |= is_test(&item.attrs);
        match &item.content {
            Some((_, items)) => self.with_uses(items, true, |index| {
                syn::visit::visit_item_mod(index, item);
            }),
            None => syn::visit::visit_item_mod(self, item),
        }
        self.test = was;
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let items = block.stmts.iter().filter_map(|stmt| match stmt {
            syn::Stmt::Item(item) => Some(item),
            _ => None,
        });
        self.with_uses(items, false, |index| syn::visit::visit_block(index, block));
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        self.returns(&item.sig.ident.to_string(), &item.sig.output);
        let mut locals = Bindings::default();
        locals.visit_item_fn(item);
        let mut commands = CommandBindings::default();
        commands.visit_item_fn(item);
        self.function(
            item.sig.ident.to_string(),
            &item.attrs,
            (locals, commands),
            |index| syn::visit::visit_item_fn(index, item),
        );
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.returns(&item.sig.ident.to_string(), &item.sig.output);
        let mut locals = Bindings::default();
        locals.visit_impl_item_fn(item);
        let mut commands = CommandBindings::default();
        commands.visit_impl_item_fn(item);
        self.function(
            item.sig.ident.to_string(),
            &item.attrs,
            (locals, commands),
            |index| syn::visit::visit_impl_item_fn(index, item),
        );
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if let syn::Expr::Path(path) = &*call.func {
            if let Some(last) = path.path.segments.last() {
                self.called(last.ident.to_string());
                self.path_call(&path.path);
            }
        }
        syn::visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.called(call.method.to_string());
        self.process_start(call);
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
            } else if !io {
                "the function does no process, file or signal I/O itself, so it is a decision, which is never excluded"
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
