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
use proc_macro2::{Delimiter, TokenStream, TokenTree};
use regex::Regex;
use std::collections::BTreeMap;
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

/// One banned site: the file, its 1-based line, the innermost enclosing item (a function, a constant or a static; `-`
/// outside any), and the rule.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Finding {
    pub file: String,
    pub line: usize,
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
    if !file.ends_with(".rs") || file.starts_with(OWNER) || file.starts_with("xtask/fixtures/") {
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

/// The `use` declarations of a file: each local name with the full path that it names, and the glob prefixes.
#[derive(Default, Debug)]
struct Uses {
    names: BTreeMap<String, Vec<String>>,
    globs: Vec<Vec<String>>,
}

impl Uses {
    fn collect(file: &syn::File) -> Uses {
        struct Collector(Uses);
        impl<'ast> Visit<'ast> for Collector {
            fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
                add_use_tree(&mut self.0, &mut Vec::new(), &item.tree);
            }
        }
        let mut collector = Collector(Uses::default());
        collector.visit_file(file);
        collector.0
    }

    /// The full path of `segments`: the first segment expanded through the `use` names. A single unknown name is tried
    /// against each glob prefix, and kept as it is when no target rule matches the result.
    fn resolve(&self, segments: &[String]) -> Vec<String> {
        let Some((first, rest)) = segments.split_first() else {
            return Vec::new();
        };
        if let Some(full) = self.names.get(first) {
            let mut path = full.clone();
            path.extend_from_slice(rest);
            return path;
        }
        if rest.is_empty() {
            for glob in &self.globs {
                let mut path = glob.clone();
                path.push(first.clone());
                if path_rule(&path).is_some() {
                    return path;
                }
            }
        }
        segments.to_vec()
    }
}

fn add_use_tree(uses: &mut Uses, prefix: &mut Vec<String>, tree: &syn::UseTree) {
    match tree {
        syn::UseTree::Path(path) => {
            prefix.push(path.ident.to_string());
            add_use_tree(uses, prefix, &path.tree);
            prefix.pop();
        }
        syn::UseTree::Name(name) => {
            let ident = name.ident.to_string();
            if ident == "self" {
                if let Some(last) = prefix.last() {
                    uses.names.insert(last.clone(), prefix.clone());
                }
            } else {
                let mut full = prefix.clone();
                full.push(ident.clone());
                uses.names.insert(ident, full);
            }
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

fn item_attrs(item: &syn::Item) -> &[syn::Attribute] {
    match item {
        syn::Item::Const(i) => &i.attrs,
        syn::Item::Enum(i) => &i.attrs,
        syn::Item::ExternCrate(i) => &i.attrs,
        syn::Item::Fn(i) => &i.attrs,
        syn::Item::ForeignMod(i) => &i.attrs,
        syn::Item::Impl(i) => &i.attrs,
        syn::Item::Macro(i) => &i.attrs,
        syn::Item::Mod(i) => &i.attrs,
        syn::Item::Static(i) => &i.attrs,
        syn::Item::Struct(i) => &i.attrs,
        syn::Item::Trait(i) => &i.attrs,
        syn::Item::TraitAlias(i) => &i.attrs,
        syn::Item::Type(i) => &i.attrs,
        syn::Item::Union(i) => &i.attrs,
        syn::Item::Use(i) => &i.attrs,
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
    uses: &'a Uses,
    in_test: bool,
    items: Vec<String>,
    /// Per function: the local names bound to a value of a known kind.
    locals: Vec<Vec<(String, Kind)>>,
    findings: Vec<Finding>,
}

impl Scan<'_> {
    fn report(&mut self, line: usize, rule: Rule) {
        if !self.in_test {
            return;
        }
        self.findings.push(Finding {
            file: self.file.to_string(),
            line,
            item: self.items.last().cloned().unwrap_or_else(|| "-".into()),
            rule,
        });
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
                    constructor_kind(&self.uses.resolve(&expr_path_segments(path)))
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
                            self.report(literal.span().start().line, Rule::ShellLoop);
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
                    let line = ident.span().start().line;
                    let arg_count = count_args(args.stream());
                    let after_dot = i > 0
                        && matches!(&tokens[i - 1], TokenTree::Punct(p) if p.as_char() == '.');
                    if after_dot {
                        if let Some(rule) = method_rule(&ident.to_string(), arg_count, None) {
                            self.report(line, rule);
                        }
                        continue;
                    }
                    let segments = path_before(&tokens, i);
                    if let Some(rule) = path_rule(&self.uses.resolve(&segments)) {
                        self.report(line, rule);
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
            self.report(call.method.span().start().line, rule);
        }
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
        if let Some(rule) = path_rule(&self.uses.resolve(&expr_path_segments(path))) {
            let line = path
                .path
                .segments
                .last()
                .map_or(0, |last| last.ident.span().start().line);
            self.report(line, rule);
        }
        syn::visit::visit_expr_path(self, path);
    }

    fn visit_lit_str(&mut self, lit: &'ast syn::LitStr) {
        if shell_loop(&lit.value()) {
            self.report(lit.span().start().line, Rule::ShellLoop);
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.scan_macro(mac);
    }
}

/// The findings of one file, or why it could not be parsed (a file that does not parse cannot be proved clean).
///
/// # Errors
/// The file does not parse.
pub fn scan(file: &str, text: &str) -> Result<Vec<Finding>, String> {
    let scope = scope(file);
    if scope == Scope::Skip {
        return Ok(Vec::new());
    }
    let parsed = syn::parse_file(text).map_err(|e| {
        let at = e.span().start();
        format!("{file}:{}:{}: does not parse: {e}", at.line, at.column + 1)
    })?;
    let uses = Uses::collect(&parsed);
    let mut scan = Scan {
        file,
        uses: &uses,
        in_test: scope == Scope::All,
        items: Vec::new(),
        locals: vec![Vec::new()],
        findings: Vec::new(),
    };
    scan.visit_file(&parsed);
    let mut findings = scan.findings;
    findings.sort();
    findings.dedup();
    Ok(findings)
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
                "{}:{}: [{}] in `{}`: {}",
                finding.file,
                finding.line,
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

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let allow_text = std::fs::read_to_string(root.join(ALLOW_FILE)).unwrap_or_default();
    let allowed = parse_allowlist(&allow_text).map_err(anyhow::Error::msg)?;
    let mut findings = Vec::new();
    let mut scanned = 0;
    for file in tracked_files(root)? {
        if scope(&file) == Scope::Skip {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(root.join(&file)) else {
            continue;
        };
        scanned += 1;
        findings.extend(scan(&file, &text).map_err(anyhow::Error::msg)?);
    }
    let violations = judge(&findings, &allowed);
    for violation in &violations {
        eprintln!("{violation}");
    }
    println!(
        "process-check: {scanned} Rust files scanned, {} allowed sites",
        allowed.len()
    );
    if !violations.is_empty() {
        bail!("{} violation(s)", violations.len());
    }
    Ok(())
}

#[cfg(test)]
mod tests;
