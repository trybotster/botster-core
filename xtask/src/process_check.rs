//! `cargo xtask process-check`: real-process test code has one owner, `botster-test-process` (brief-p6-test-process, lead
//! ruling 2026-10-08: a pattern finding is fixed together with the mechanical check that finds all its instances).
//!
//! Outside that crate, test code must not wait for a child, read a line without a deadline, sleep, or start a shell sleep
//! or busy loop: the crate's owned child, guard, bounded reads and blocked fixture do each of these with a deadline and an
//! owner. The check parses every Rust file with `syn`, so a `use` rename, a glob import, a path call (`Child::wait`), a
//! function reference and a macro body are found as well as a plain method call. Test code is a file under a `tests`
//! directory (or a `tests.rs` or `*_test.rs` file), an item with `#[test]` or `#[cfg(test)]`, and the testkit library. The
//! xtask's tests are test code too (lead ruling on #170 G2); its commands, which run the gate's own tools, are not.
//!
//! An exception is allowed per call site only: one entry per site in `.config/process-check-allow.txt`, `<file> | <item> |
//! <rule>`, under a comment that gives the reason (the style of the `exclude_re` entries). An entry that matches no site
//! fails the check too, so the list shrinks as each migration lands.

use crate::fsutil::tracked_files;
use anyhow::{bail, Result};
use proc_macro2::{Delimiter, Span, TokenStream, TokenTree};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use syn::punctuated::Punctuated;
use syn::visit::Visit;

/// The owner of real-process test code: the check does not apply inside it.
const OWNER: &str = "crates/botster-test-process/";
/// Library code that only tests use: all of it is test code.
const TEST_SUPPORT: [&str; 1] = ["crates/botster-core-testkit/src/"];
/// The allowlist, relative to the repository root.
pub const ALLOW_FILE: &str = ".config/process-check-allow.txt";

/// What a finding breaks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rule {
    /// `Child::wait`, `try_wait` or `wait_with_output`: an unbounded wait, a polling probe, or a reap that production may own.
    ChildWait,
    /// `Command::status` or `Command::output`: a spawn and an unbounded wait.
    CommandWait,
    /// A raw spawn: the child has no owner that ends it on every path.
    Spawn,
    /// A blocking read with no deadline on a pipe, a socket or a channel: `read_line`, `read_to_end`, `read_to_string`,
    /// `read_exact`, `lines()` of a `BufReader`, `accept`, `incoming`, a channel's `recv()`, a socket's `recv(buf)` (lead ruling 2026-10-08, after #169 F19). A
    /// file is not a pipe: a read of a `File` is allowed.
    BlockingRead,
    /// `std::thread::sleep`.
    Sleep,
    /// A shell `sleep` or `while :`/`while true` loop in a string.
    ShellLoop,
}

impl Rule {
    pub fn name(self) -> &'static str {
        match self {
            Rule::ChildWait => "child-wait",
            Rule::CommandWait => "command-wait",
            Rule::Spawn => "spawn",
            Rule::BlockingRead => "blocking-read",
            Rule::Sleep => "sleep",
            Rule::ShellLoop => "shell-loop",
        }
    }

    fn parse(name: &str) -> Option<Rule> {
        [
            Rule::ChildWait,
            Rule::CommandWait,
            Rule::Spawn,
            Rule::BlockingRead,
            Rule::Sleep,
            Rule::ShellLoop,
        ]
        .into_iter()
        .find(|rule| rule.name() == name)
    }

    fn advice(self) -> &'static str {
        match self {
            Rule::ChildWait => "a raw wait for a child: use botster_test_process::OwnedChild (status, exited_within)",
            Rule::CommandWait => "Command::status/output waits without a bound: use OwnedChild::spawn(..).status()",
            Rule::Spawn => "a raw spawn: start the child with OwnedChild::spawn or spawn_group, or through a Guard wrapper",
            Rule::BlockingRead => {
                "a blocking read with no deadline: use botster_test_process::Bounded (line, to_eof) or first_line, or \
                 recv_timeout with a Deadline"
            }
            Rule::Sleep => "a sleep in test code: wait on the real event with a Deadline",
            Rule::ShellLoop => "a shell sleep or busy loop: use botster_test_process::Blocker (a FIFO-blocked /bin/cat)",
        }
    }
}

/// One banned site: the file, its 1-based line and column (two calls on one line are two sites), the innermost enclosing
/// item (a function, a constant or a static; `-` outside any), and the rule.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub file: String,
    pub line: usize,
    pub column: usize,
    pub item: String,
    pub rule: Rule,
}

/// Which code of a file is test code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scope {
    /// Not checked: not Rust, the owner crate, or an xtask fixture (its own workspace, the code that a fixture test mutates).
    Skip,
    /// The whole file is test code.
    All,
    /// Only `#[test]` and `#[cfg(test)]` items are test code.
    TestItems,
}

fn scope(file: &str) -> Scope {
    if !file.ends_with(".rs") || skipped(file) {
        return Scope::Skip;
    }
    if TEST_SUPPORT.iter().any(|prefix| file.starts_with(prefix)) {
        return Scope::All;
    }
    let stem = file
        .rsplit('/')
        .next()
        .and_then(|name| name.strip_suffix(".rs"))
        .unwrap_or(file);
    let test_dir = file
        .split('/')
        .rev()
        .skip(1)
        .any(|part| part == "tests" || part == "test");
    let test_file =
        matches!(stem, "tests" | "test") || stem.ends_with("_test") || stem.ends_with("_tests");
    if test_dir || test_file {
        Scope::All
    } else {
        Scope::TestItems
    }
}

/// The `use` declarations of one scope (a file, an inline module or a block): each local name with the path that it names,
/// and the glob prefixes. A `use` is visible in its own scope and in the scopes inside it.
#[derive(Default, Debug)]
pub(crate) struct Uses {
    names: BTreeMap<String, Vec<String>>,
    globs: Vec<Vec<String>>,
    /// Whether the scope is a module (a file or an inline module), which `self` and `super` name.
    module: bool,
}

impl Uses {
    /// The `use` declarations among `items`, the direct items of one scope.
    pub(crate) fn of<'a>(items: impl IntoIterator<Item = &'a syn::Item>, module: bool) -> Uses {
        let mut uses = Uses {
            module,
            ..Uses::default()
        };
        for item in items {
            if let syn::Item::Use(declaration) = item {
                add_use_tree(&mut uses, &mut Vec::new(), &declaration.tree);
            }
        }
        uses
    }
}

/// The full path of `segments`, seen from the innermost of `scopes`: the first segment is expanded through its nearest
/// binding, and the result again from the scope of that binding, until no binding applies (a chain such as `use std::thread
/// as th; use th::sleep as nap;`). A leading `self` names the nearest module, and a leading `super` the module around it. A
/// single unknown name is tried against each visible glob prefix, and kept as it is when no target rule matches the result.
pub(crate) fn resolve(scopes: &[Uses], segments: &[String]) -> Vec<String> {
    resolve_from(scopes, segments.to_vec(), &mut BTreeSet::new())
}

/// `resolve` within `scopes`; `seen` holds each binding already expanded (its scope and name), so a cycle ends.
fn resolve_from(
    mut scopes: &[Uses],
    mut path: Vec<String>,
    seen: &mut BTreeSet<(usize, String)>,
) -> Vec<String> {
    let module = |scopes: &[Uses]| scopes.iter().rposition(|scope| scope.module);
    loop {
        let Some(first) = path.first().cloned() else {
            return path;
        };
        let visible = match first.as_str() {
            "self" => module(scopes).map(|at| at + 1),
            "super" => module(scopes)
                .and_then(|at| module(&scopes[..at]))
                .map(|at| at + 1),
            _ => None,
        };
        if let Some(visible) = visible {
            scopes = &scopes[..visible];
            path.remove(0);
            continue;
        }
        let binding = scopes
            .iter()
            .enumerate()
            .rev()
            .find_map(|(at, scope)| scope.names.get(&first).map(|full| (at, full)));
        let Some((at, full)) = binding else {
            break;
        };
        if !seen.insert((at, first)) {
            return path;
        }
        let mut expanded = full.clone();
        expanded.extend_from_slice(&path[1..]);
        path = expanded;
        scopes = &scopes[..=at];
    }
    if let [name] = &path[..] {
        for (at, scope) in scopes.iter().enumerate().rev() {
            for (index, glob) in scope.globs.iter().enumerate() {
                if !seen.insert((at, format!("*{index}"))) {
                    continue;
                }
                let mut candidate = glob.clone();
                candidate.push(name.clone());
                let candidate = resolve_from(&scopes[..=at], candidate, seen);
                if path_rule(&candidate).is_some() {
                    return candidate;
                }
            }
        }
    }
    path
}

fn add_use_tree(uses: &mut Uses, prefix: &mut Vec<String>, tree: &syn::UseTree) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            add_use_tree(uses, prefix, &path.tree);
            prefix.pop();
        }
        // `{self}` needs no entry of its own: a path through the module name ends in the same two segments, which are all
        // that the rules read.
        syn::UseTree::Name(name) => {
            let ident = name.ident.to_string();
            let mut full = prefix.clone();
            full.push(ident.clone());
            uses.names.insert(ident, full);
        }
        syn::UseTree::Rename(rename) => {
            let mut full = prefix.clone();
            if rename.ident != "self" {
                full.push(rename.ident.to_string());
            }
            uses.names.insert(rename.rename.to_string(), full);
        }
        syn::UseTree::Glob(_) => uses.globs.push(prefix.clone()),
        syn::UseTree::Group(group) => {
            for tree in &group.items {
                add_use_tree(uses, prefix, tree);
            }
        }
    }
}

/// The rule that a resolved path breaks when it is called or referenced, if any.
fn path_rule(path: &[String]) -> Option<Rule> {
    let name = |i: usize| path.len().checked_sub(i).map(|at| path[at].as_str());
    match (name(2), name(1)?) {
        (Some("thread"), "sleep") => Some(Rule::Sleep),
        (Some("Child"), "wait" | "try_wait" | "wait_with_output") => Some(Rule::ChildWait),
        (Some(_), "read_line") => Some(Rule::BlockingRead),
        (Some("Read" | "BufRead"), "read_to_end" | "read_to_string" | "read_exact" | "lines") => {
            Some(Rule::BlockingRead)
        }
        (Some("io"), "read_to_string") => Some(Rule::BlockingRead),
        (Some("Command"), "spawn") => Some(Rule::Spawn),
        (Some("Command"), "status" | "output") => Some(Rule::CommandWait),
        _ => None,
    }
}

/// The rule that a method call breaks, if any; `receiver` is what the scan knows of its receiver.
fn method_rule(method: &str, args: usize, receiver: Option<Kind>) -> Option<Rule> {
    let command = receiver == Some(Kind::Command);
    match (method, args) {
        ("wait" | "try_wait" | "wait_with_output", 0) => Some(Rule::ChildWait),
        ("read_line" | "read_to_end" | "read_to_string" | "read_exact", 1)
            if receiver != Some(Kind::File) =>
        {
            Some(Rule::BlockingRead)
        }
        ("lines", 0) if receiver == Some(Kind::Reader) => Some(Rule::BlockingRead),
        ("accept" | "incoming" | "recv", 0) => Some(Rule::BlockingRead),
        ("recv" | "recv_from", 1) if receiver == Some(Kind::Socket) => Some(Rule::BlockingRead),
        ("spawn", 0) => Some(Rule::Spawn),
        ("spawn", _) if command => Some(Rule::Spawn),
        ("status" | "output", 0) if command => Some(Rule::CommandWait),
        _ => None,
    }
}

/// Whether a string holds a shell sleep (`sleep 1`, `/bin/sleep 0.5`) or a shell loop that never ends by itself (`while :`,
/// `while true`).
fn shell_loop(text: &str) -> bool {
    static SHELL: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    SHELL
        .get_or_init(|| {
            Regex::new(
                r"(?:^|[\s;&|(`])(?:/usr)?(?:/bin/)?sleep\s+[0-9.]|\bwhile\s+(?::|true)(?:\s|;|$)",
            )
            .expect("regex")
        })
        .is_match(text)
}

/// Whether a `cfg` predicate holds only under `test`: `test`, or `all(..)` with such a member, or `any(..)` whose members all
/// are.
fn cfg_requires_test(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::List(list) => {
            let Ok(members) =
                list.parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
            else {
                return false;
            };
            if list.path.is_ident("all") {
                members.iter().any(cfg_requires_test)
            } else if list.path.is_ident("any") {
                !members.is_empty() && members.iter().all(cfg_requires_test)
            } else {
                false
            }
        }
        syn::Meta::NameValue(_) => false,
    }
}

/// Whether attributes make an item test code: `#[test]` (any path ending in `test`, such as `tokio::test`) or a `cfg` that
/// holds only under `test`.
fn is_test_item(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        let path = attr.path();
        if path.is_ident("cfg") {
            attr.parse_args::<syn::Meta>()
                .is_ok_and(|meta| cfg_requires_test(&meta))
        } else {
            path.segments
                .last()
                .is_some_and(|last| last.ident == "test")
        }
    })
}

/// The attributes of an item that can hold code. A function's own visit reads its attributes (`visit_item_fn`); a `use`, an
/// `extern crate`, an `extern` block and a trait alias hold no expression.
fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        _ => &[],
    }
}

/// The segments of a path expression, with a qualified self type first (`<Child>::wait` is `Child::wait`).
fn expr_path_segments(expr: &syn::ExprPath) -> Vec<String> {
    let mut segments = Vec::new();
    let mut skip = 0;
    if let Some(qself) = &expr.qself {
        if let syn::Type::Path(ty) = &*qself.ty {
            segments.extend(ty.path.segments.iter().map(|s| s.ident.to_string()));
        }
        skip = qself.position;
    }
    segments.extend(
        expr.path
            .segments
            .iter()
            .skip(skip)
            .map(|s| s.ident.to_string()),
    );
    segments
}

/// What the scan knows of a receiver: what built it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    /// `Command::new(..)`.
    Command,
    /// `File::open(..)`, `File::create(..)` or `OpenOptions::new()`: a file, whose reads do not block.
    File,
    /// `BufReader::new(..)`: a reader whose `lines()` blocks.
    Reader,
    /// A socket of the standard library (`UnixStream::connect`, `UnixDatagram::bind`, `UdpSocket::bind`, ...): its
    /// `recv(buf)` blocks. Another type's `recv(buf)`, such as the testkit's in-memory link, does not.
    Socket,
}

/// The kind of value that the constructor path `full` (after the `use` names) builds.
fn constructor_kind(full: &[String]) -> Option<Kind> {
    let name = |i: usize| full.len().checked_sub(i).map(|at| full[at].as_str());
    match (name(2)?, name(1)?) {
        ("Command", "new") => Some(Kind::Command),
        ("File", "open" | "create") | ("OpenOptions", "new") => Some(Kind::File),
        ("BufReader", "new") => Some(Kind::Reader),
        ("UnixStream" | "TcpStream", "connect")
        | ("UnixDatagram", "bind" | "unbound")
        | ("UdpSocket", "bind") => Some(Kind::Socket),
        _ => None,
    }
}

struct Scan<'a> {
    file: &'a str,
    /// The `use` scopes around the current code, the file's own first.
    scopes: Vec<Uses>,
    in_test: bool,
    items: Vec<String>,
    /// Per function: the local names bound to a value of a known kind.
    locals: Vec<Vec<(String, Kind)>>,
    findings: Vec<Finding>,
}

impl Scan<'_> {
    fn report(&mut self, at: Span, rule: Rule) {
        if !self.in_test {
            return;
        }
        let start = at.start();
        self.findings.push(Finding {
            file: self.file.to_string(),
            line: start.line,
            column: start.column + 1,
            item: self.items.last().cloned().unwrap_or_else(|| "-".into()),
            rule,
        });
    }

    /// Visits code inside the `use` scope of `items`.
    fn with_uses<'a>(
        &mut self,
        items: impl IntoIterator<Item = &'a syn::Item>,
        module: bool,
        visit: impl FnOnce(&mut Self),
    ) {
        self.scopes.push(Uses::of(items, module));
        visit(self);
        self.scopes.pop();
    }

    fn with_item(&mut self, name: String, test: bool, visit: impl FnOnce(&mut Self)) {
        let was = self.in_test;
        self.in_test |= test;
        self.items.push(name);
        self.locals.push(Vec::new());
        visit(self);
        self.locals.pop();
        self.items.pop();
        self.in_test = was;
    }

    /// What built `expr`: a chain of method calls (`?` and `&` included) on a known constructor (after the `use` names), or a
    /// local name bound to one.
    fn kind(&self, expr: &syn::Expr) -> Option<Kind> {
        match expr {
            syn::Expr::MethodCall(call) => self.kind(&call.receiver),
            syn::Expr::Paren(paren) => self.kind(&paren.expr),
            syn::Expr::Reference(reference) => self.kind(&reference.expr),
            syn::Expr::Try(attempt) => self.kind(&attempt.expr),
            syn::Expr::Call(call) => match &*call.func {
                syn::Expr::Path(path) => {
                    constructor_kind(&resolve(&self.scopes, &expr_path_segments(path)))
                }
                _ => None,
            },
            syn::Expr::Path(path) => {
                let ident = path.path.get_ident()?.to_string();
                self.locals
                    .last()?
                    .iter()
                    .rev()
                    .find(|(name, _)| *name == ident)
                    .map(|(_, kind)| *kind)
            }
            _ => None,
        }
    }

    /// A macro body: as expressions when it parses as a comma-separated list, otherwise token by token.
    fn scan_macro(&mut self, mac: &syn::Macro) {
        if !self.in_test {
            return;
        }
        if let Ok(exprs) =
            mac.parse_body_with(Punctuated::<syn::Expr, syn::Token![,]>::parse_terminated)
        {
            for expr in &exprs {
                self.visit_expr(expr);
            }
            return;
        }
        self.scan_tokens(mac.tokens.clone());
    }

    /// Tokens that do not parse as expressions: string literals, method calls (`. name (..)`) and path calls (`a :: b
    /// (..)`), recursively into groups.
    fn scan_tokens(&mut self, tokens: TokenStream) {
        let tokens: Vec<TokenTree> = tokens.into_iter().collect();
        for (i, token) in tokens.iter().enumerate() {
            match token {
                TokenTree::Group(group) => self.scan_tokens(group.stream()),
                TokenTree::Literal(literal) => {
                    if let Ok(text) =
                        syn::parse2::<syn::LitStr>(TokenTree::Literal(literal.clone()).into())
                    {
                        if shell_loop(&text.value()) {
                            self.report(literal.span(), Rule::ShellLoop);
                        }
                    }
                }
                TokenTree::Ident(ident) => {
                    let Some(TokenTree::Group(args)) = tokens.get(i + 1) else {
                        continue;
                    };
                    if args.delimiter() != Delimiter::Parenthesis {
                        continue;
                    }
                    let arg_count = count_args(args.stream());
                    let after_dot = i > 0
                        && matches!(&tokens[i - 1], TokenTree::Punct(p) if p.as_char() == '.');
                    if after_dot {
                        if let Some(rule) = method_rule(&ident.to_string(), arg_count, None) {
                            self.report(ident.span(), rule);
                        }
                        continue;
                    }
                    let segments = path_before(&tokens, i);
                    if let Some(rule) = path_rule(&resolve(&self.scopes, &segments)) {
                        self.report(ident.span(), rule);
                    }
                }
                TokenTree::Punct(_) => {}
            }
        }
    }
}

/// The number of comma-separated arguments in a parenthesized token stream.
fn count_args(stream: TokenStream) -> usize {
    let mut count = 0;
    let mut pending = false;
    for token in stream {
        match token {
            TokenTree::Punct(p) if p.as_char() == ',' => {
                count += usize::from(pending);
                pending = false;
            }
            _ => pending = true,
        }
    }
    count + usize::from(pending)
}

/// The path that ends at the identifier `tokens[end]`: the identifiers joined by `::` before it.
fn path_before(tokens: &[TokenTree], end: usize) -> Vec<String> {
    let mut segments = vec![tokens[end].to_string()];
    let mut at = end;
    while at >= 3 {
        let colons = matches!((&tokens[at - 1], &tokens[at - 2]),
            (TokenTree::Punct(a), TokenTree::Punct(b)) if a.as_char() == ':' && b.as_char() == ':');
        match (&tokens[at - 3], colons) {
            (TokenTree::Ident(ident), true) => {
                segments.insert(0, ident.to_string());
                at -= 3;
            }
            _ => break,
        }
    }
    segments
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        let test = is_test_item(item_attrs(item));
        let was = self.in_test;
        self.in_test |= test;
        syn::visit::visit_item(self, item);
        self.in_test = was;
    }

    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        let test = is_test_item(&item.attrs);
        self.with_item(item.sig.ident.to_string(), test, |scan| {
            syn::visit::visit_item_fn(scan, item);
        });
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        let test = is_test_item(&item.attrs);
        self.with_item(item.sig.ident.to_string(), test, |scan| {
            syn::visit::visit_impl_item_fn(scan, item);
        });
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        let test = is_test_item(&item.attrs);
        self.with_item(item.sig.ident.to_string(), test, |scan| {
            syn::visit::visit_trait_item_fn(scan, item);
        });
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.with_item(item.ident.to_string(), false, |scan| {
            syn::visit::visit_item_const(scan, item);
        });
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.with_item(item.ident.to_string(), false, |scan| {
            syn::visit::visit_item_static(scan, item);
        });
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        if let (syn::Pat::Ident(pat), Some(init)) = (&local.pat, &local.init) {
            if let Some(kind) = self.kind(&init.expr) {
                if let Some(names) = self.locals.last_mut() {
                    names.push((pat.ident.to_string(), kind));
                }
            }
        }
        syn::visit::visit_local(self, local);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        let receiver = self.kind(&call.receiver);
        if let Some(rule) = method_rule(&call.method.to_string(), call.args.len(), receiver) {
            self.report(call.method.span(), rule);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        let rule = path_rule(&resolve(&self.scopes, &expr_path_segments(path)));
        if let (Some(rule), Some(last)) = (rule, path.path.segments.last()) {
            self.report(last.ident.span(), rule);
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        match &module.content {
            Some((_, items)) => self.with_uses(items, true, |scan| {
                syn::visit::visit_item_mod(scan, module);
            }),
            None => syn::visit::visit_item_mod(self, module),
        }
    }

    fn visit_block(&mut self, block: &'ast syn::Block) {
        let items = block.stmts.iter().filter_map(|stmt| match stmt {
            syn::Stmt::Item(item) => Some(item),
            _ => None,
        });
        self.with_uses(items, false, |scan| syn::visit::visit_block(scan, block));
    }

    fn visit_lit_str(&mut self, lit: &'ast syn::LitStr) {
        if shell_loop(&lit.value()) {
            self.report(lit.span(), Rule::ShellLoop);
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.scan_macro(mac);
    }
}

/// The findings of one file, or why it could not be parsed (a file that does not parse cannot be proved clean). Only the
/// file's own path and attributes tell its test code: `scan_files`, which the check runs, adds the test modules that other
/// files declare.
///
/// # Errors
/// The file does not parse.
#[cfg(test)]
fn scan(file: &str, text: &str) -> Result<Vec<Finding>, String> {
    let scope = scope(file);
    if scope == Scope::Skip {
        return Ok(Vec::new());
    }
    Ok(scan_parsed(file, &parse(file, text)?, scope == Scope::All))
}

#[cfg(test)]
fn parse(file: &str, text: &str) -> Result<syn::File, String> {
    syn::parse_file(text).map_err(|e| parse_error(file, &e))
}

/// Why `file` cannot be proved clean: it does not parse, at the error's position.
fn parse_error(file: &str, error: &syn::Error) -> String {
    let at = error.span().start();
    format!(
        "{file}:{}:{}: does not parse: {error}",
        at.line,
        at.column + 1
    )
}

/// The findings of a parsed file; with `whole`, all of it is test code.
fn scan_parsed(file: &str, parsed: &syn::File, whole: bool) -> Vec<Finding> {
    let mut scan = Scan {
        file,
        scopes: vec![Uses::of(&parsed.items, true)],
        in_test: whole,
        items: Vec::new(),
        locals: vec![Vec::new()],
        findings: Vec::new(),
    };
    scan.visit_file(parsed);
    let mut findings = scan.findings;
    findings.sort();
    findings
}

/// A module that a file declares without a body (`mod name;`).
pub(crate) struct Declared {
    name: String,
    line: usize,
    /// Whether the declaration is test code: a test attribute on it or on an inline module around it.
    test: bool,
    /// The file of the module, when it is among the files of the run.
    file: Option<String>,
}

/// The modules that `items` declare without a body, recursively through inline modules. `dir` holds the files of the
/// modules of `items`; `inline` tells whether `items` are inside an inline module; `test` tells whether `items` are test
/// code. A `#[path]` is relative to the directory of `file` at the top of the file, and relative to `dir` inside an inline
/// module (the Rust reference, "The path attribute"). Without `#[path]`, the file is `<dir><name>.rs` or
/// `<dir><name>/mod.rs`, and then, for a crate root such as `tests/a.rs`, the same beside `file`.
fn declared_modules(
    file: &str,
    items: &[syn::Item],
    (dir, inline): (&str, bool),
    test: bool,
    files: &BTreeSet<&str>,
    declared: &mut Vec<Declared>,
) {
    use crate::platform_code::{parent_dir, path_attr};
    for item in items {
        let syn::Item::Mod(module) = item else {
            continue;
        };
        let name = module.ident.to_string();
        let test = test || is_test_item(&module.attrs);
        if let Some((_, inner)) = &module.content {
            let inner_dir = format!("{dir}{name}/");
            declared_modules(file, inner, (&inner_dir, true), test, files, declared);
            continue;
        }
        let beside = parent_dir(file);
        let candidates = match path_attr(&module.attrs) {
            Some(path) => {
                let base = if inline { dir } else { beside };
                vec![normalize(&format!("{base}{path}"))]
            }
            None => vec![
                format!("{dir}{name}.rs"),
                format!("{dir}{name}/mod.rs"),
                format!("{beside}{name}.rs"),
                format!("{beside}{name}/mod.rs"),
            ],
        };
        declared.push(Declared {
            file: candidates.into_iter().find(|c| files.contains(c.as_str())),
            line: module.ident.span().start().line,
            name,
            test,
        });
    }
}

/// `path` without its `.` segments, and with each `..` segment taken out together with the segment before it.
pub(crate) fn normalize(path: &str) -> String {
    let mut segments: Vec<&str> = Vec::new();
    for segment in path.split('/') {
        match segment {
            "." => {}
            ".." if segments.last().is_some_and(|last| *last != "..") => {
                segments.pop();
            }
            _ => segments.push(segment),
        }
    }
    segments.join("/")
}

/// The module declarations of the parsed `file`; `files` are the files of the run.
pub(crate) fn declarations(file: &str, tree: &syn::File, files: &BTreeSet<&str>) -> Vec<Declared> {
    let mut declared = Vec::new();
    let dir = crate::platform_code::module_dir(file);
    declared_modules(
        file,
        &tree.items,
        (&dir, false),
        false,
        files,
        &mut declared,
    );
    declared
}

/// The parsed module tree of a run: each file that `include` takes, and each file that a parsed file declares as a module
/// (`mod x;`, `#[path]` included, whatever its extension), unless `skip` takes it. A file that does not parse is an
/// error.
pub(crate) struct Tree<'a> {
    pub(crate) parsed: BTreeMap<&'a str, syn::File>,
    pub(crate) declared: BTreeMap<&'a str, Vec<Declared>>,
    pub(crate) errors: Vec<(&'a str, syn::Error)>,
}

/// The module tree of `sources` (each file of the run with its text). See `Tree`.
pub(crate) fn module_tree<'a>(
    sources: &'a BTreeMap<String, String>,
    include: impl Fn(&str) -> bool,
    skip: impl Fn(&str) -> bool,
) -> Tree<'a> {
    let files: BTreeSet<&str> = sources.keys().map(String::as_str).collect();
    let mut tree = Tree {
        parsed: BTreeMap::new(),
        declared: BTreeMap::new(),
        errors: Vec::new(),
    };
    let mut pending: Vec<&str> = files.iter().copied().filter(|file| include(file)).collect();
    let mut seen: BTreeSet<&str> = pending.iter().copied().collect();
    while let Some(file) = pending.pop() {
        match syn::parse_file(&sources[file]) {
            Ok(parsed) => {
                let declared = declarations(file, &parsed, &files);
                for module in &declared {
                    let reached = module.file.as_deref().and_then(|child| files.get(child));
                    if let Some(child) = reached.filter(|child| !skip(child)) {
                        if seen.insert(child) {
                            pending.push(child);
                        }
                    }
                }
                tree.declared.insert(file, declared);
                tree.parsed.insert(file, parsed);
            }
            Err(error) => tree.errors.push((file, error)),
        }
    }
    tree.errors.sort_by(|a, b| a.0.cmp(b.0));
    tree
}

/// The files of `declared` (each parsed file with its module declarations) that are test code as a whole: those of
/// `whole`, and, to a fixed point through the module tree, each module file that a test declaration (`#[cfg(test)] mod
/// helpers;`, `#[path]` included) or a file that is test code as a whole declares. A declared file that is not parsed
/// (one that the run skips) stays out.
///
/// # Errors
/// The file of a module that is test code is not among the files of the run.
pub(crate) fn whole_test_files<'a>(
    declared: &BTreeMap<&'a str, Vec<Declared>>,
    mut whole: BTreeSet<&'a str>,
) -> Result<BTreeSet<&'a str>, String> {
    // Each round adds a file or ends the search, so there are at most as many rounds as files.
    for _ in 0..=declared.len() {
        let mut added = Vec::new();
        for (file, modules) in declared {
            let all = whole.contains(file);
            for module in modules.iter().filter(|module| all || module.test) {
                let Some(child) = &module.file else {
                    return Err(format!(
                        "{file}:{}: the file of the test module `{}` is not found",
                        module.line, module.name
                    ));
                };
                added.extend(declared.get_key_value(child.as_str()).map(|(key, _)| *key));
            }
        }
        let before = whole.len();
        whole.extend(added);
        if whole.len() == before {
            break;
        }
    }
    Ok(whole)
}

/// The number of files scanned and the findings of `sources` (each file of the run with its text): its Rust files, and
/// every module file that they reach, whatever its extension. A module file is test code as a whole as
/// `whole_test_files` tells.
///
/// # Errors
/// A file does not parse, or the file of a module that is test code is not among `sources`.
pub fn scan_files(sources: &BTreeMap<String, String>) -> Result<(usize, Vec<Finding>), String> {
    let tree = module_tree(sources, |file| scope(file) != Scope::Skip, skipped);
    if let Some((file, error)) = tree.errors.first() {
        return Err(parse_error(file, error));
    }
    let all = tree
        .parsed
        .keys()
        .copied()
        .filter(|file| scope(file) == Scope::All)
        .collect();
    let whole = whole_test_files(&tree.declared, all)?;
    let mut findings = Vec::new();
    for (file, parsed) in &tree.parsed {
        findings.extend(scan_parsed(file, parsed, whole.contains(file)));
    }
    Ok((tree.parsed.len(), findings))
}

/// Whether the check never reads `file`, not even as a module that test code declares: the owner crate and the xtask
/// fixtures.
fn skipped(file: &str) -> bool {
    file.starts_with(OWNER) || file.starts_with("xtask/fixtures/")
}

/// One allowlist entry: the site's key and the 1-based line of the entry.
#[derive(Debug, PartialEq, Eq)]
pub struct Allowed {
    pub key: (String, String, Rule),
    pub line: usize,
}

/// The allowlist: `<file> | <item> | <rule>` entries, each under a comment block that gives the reason (a block covers the
/// entries that follow it up to a blank line).
///
/// # Errors
/// An entry is malformed, names an unknown rule, or has no reason.
pub fn parse_allowlist(text: &str) -> Result<Vec<Allowed>, String> {
    let mut entries = Vec::new();
    let mut reason = false;
    for (index, raw) in text.lines().enumerate() {
        let line = raw.trim();
        if line.is_empty() {
            reason = false;
            continue;
        }
        if let Some(comment) = line.strip_prefix('#') {
            reason |= !comment.trim().is_empty();
            continue;
        }
        let at = index + 1;
        let parts: Vec<&str> = line.split('|').map(str::trim).collect();
        let [file, item, rule] = parts[..] else {
            return Err(format!(
                "{ALLOW_FILE}:{at}: an entry is `<file> | <item> | <rule>`"
            ));
        };
        let rule =
            Rule::parse(rule).ok_or_else(|| format!("{ALLOW_FILE}:{at}: unknown rule `{rule}`"))?;
        if !reason {
            return Err(format!(
                "{ALLOW_FILE}:{at}: an entry needs its reason in a comment above it"
            ));
        }
        entries.push(Allowed {
            key: (file.to_string(), item.to_string(), rule),
            line: at,
        });
    }
    Ok(entries)
}

/// The violations: each finding that no entry allows (one entry allows one site), and each entry that allows no site.
pub fn judge(findings: &[Finding], allowed: &[Allowed]) -> Vec<String> {
    let mut sites: BTreeMap<(String, String, Rule), Vec<&Finding>> = BTreeMap::new();
    for finding in findings {
        sites
            .entry((finding.file.clone(), finding.item.clone(), finding.rule))
            .or_default()
            .push(finding);
    }
    let mut entries: BTreeMap<&(String, String, Rule), Vec<usize>> = BTreeMap::new();
    for entry in allowed {
        entries.entry(&entry.key).or_default().push(entry.line);
    }
    let mut unallowed: Vec<&Finding> = Vec::new();
    for (key, found) in &sites {
        let allowed = entries.get(key).map_or(0, Vec::len);
        unallowed.extend(found.iter().skip(allowed));
    }
    unallowed.sort();
    let mut violations: Vec<String> = unallowed
        .iter()
        .map(|finding| {
            format!(
                "{}:{}:{}: [{}] in `{}`: {}",
                finding.file,
                finding.line,
                finding.column,
                finding.rule.name(),
                finding.item,
                finding.rule.advice()
            )
        })
        .collect();
    for (key, lines) in &entries {
        let found = sites.get(*key).map_or(0, Vec::len);
        for line in lines.iter().skip(found) {
            violations.push(format!(
                "{ALLOW_FILE}:{line}: the entry `{} | {} | {}` allows no site: remove it",
                key.0,
                key.1,
                key.2.name()
            ));
        }
    }
    violations
}

/// What the check found in a repository.
#[derive(Debug, PartialEq, Eq)]
pub struct Report {
    /// The Rust files whose test code was scanned.
    pub scanned: usize,
    /// The entries of the allowlist.
    pub allowed: usize,
    pub violations: Vec<String>,
}

/// The check of the tracked files of `root` against its allowlist.
///
/// # Errors
/// The files cannot be listed, the allowlist is malformed, or a file does not parse.
pub fn check(root: &Path) -> Result<Report> {
    let allow_text = std::fs::read_to_string(root.join(ALLOW_FILE)).unwrap_or_default();
    let allowed = parse_allowlist(&allow_text).map_err(anyhow::Error::msg)?;
    // Every tracked text file: a test module may have any extension (`#[path = "gen.inc"]`).
    let mut sources = BTreeMap::new();
    for file in tracked_files(root)? {
        if skipped(&file) {
            continue;
        }
        if let Ok(text) = std::fs::read_to_string(root.join(&file)) {
            sources.insert(file, text);
        }
    }
    let (scanned, findings) = scan_files(&sources).map_err(anyhow::Error::msg)?;
    Ok(Report {
        scanned,
        allowed: allowed.len(),
        violations: judge(&findings, &allowed),
    })
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let report = check(root)?;
    println!(
        "process-check: {} Rust files scanned, {} allowed sites",
        report.scanned, report.allowed
    );
    crate::tools::verdict(&report.violations)
}

#[cfg(test)]
mod tests;
