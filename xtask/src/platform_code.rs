//! Platform-only code (plan section 8, revision 22 "Platform-only code"): cargo-mutants walks the module tree by syntax
//! and ignores `cfg`, so a gate lists the mutants of code that its OS does not compile, and each one shows as MISSED. This
//! module derives that code from the `cfg` attributes, and the mutation step excludes exactly its mutants (by file and
//! line) on that OS. The run on the OS that compiles the code tests them: the Linux gate for Linux-only code, a focused Mac
//! mutation run for macOS-only code.
//!
//! A `cfg` predicate is decided only from the target OS (`target_os`, `target_family`, `unix`, `windows`, and `not`, `all`
//! and `any` of them). Any other predicate (a feature, `test`, an architecture) can be true, so code under it counts as
//! compiled and keeps its mutants. Code that the derivation cannot place fails it: a gated `mod` whose file is not found,
//! and a `cfg_if!` macro (its branches are tokens, not items).

use proc_macro2::{Delimiter, TokenTree};
use quote::ToTokens;
use std::collections::{BTreeMap, BTreeSet};
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::visit::Visit;

/// Whether `os` compiles code under the predicate `meta`: `Some(false)` only when the OS alone decides that it does not.
pub fn compiled(meta: &syn::Meta, os: &str) -> Option<bool> {
    let unix = matches!(os, "linux" | "macos");
    match meta {
        syn::Meta::Path(path) if path.is_ident("unix") => Some(unix),
        syn::Meta::Path(path) if path.is_ident("windows") => Some(os == "windows"),
        syn::Meta::NameValue(pair) => {
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) = &pair.value
            else {
                return None;
            };
            if pair.path.is_ident("target_os") {
                Some(value.value() == os)
            } else if pair.path.is_ident("target_family") {
                Some(match value.value().as_str() {
                    "unix" => unix,
                    "windows" => os == "windows",
                    _ => false,
                })
            } else {
                None
            }
        }
        syn::Meta::List(list) => {
            let inner = list
                .parse_args_with(Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
                .ok()?;
            let values: Vec<Option<bool>> = inner.iter().map(|m| compiled(m, os)).collect();
            if list.path.is_ident("not") {
                match values.as_slice() {
                    [value] => value.map(|v| !v),
                    _ => None,
                }
            } else if list.path.is_ident("all") {
                if values.contains(&Some(false)) {
                    Some(false)
                } else if values.iter().all(Option::is_some) {
                    Some(true)
                } else {
                    None
                }
            } else if list.path.is_ident("any") {
                if values.contains(&Some(true)) {
                    Some(true)
                } else if values.iter().all(|v| *v == Some(false)) {
                    Some(false)
                } else {
                    None
                }
            } else {
                None
            }
        }
        syn::Meta::Path(_) => None,
    }
}

/// Whether `os` cannot compile `node`: one of its outer `cfg` predicates is false there. The outer attributes are the first
/// tokens of every node (a doc comment is a `#[doc]` attribute), so one reader serves items, statements and match arms.
fn gated(node: &impl ToTokens, os: &str) -> bool {
    let mut tokens = node.to_token_stream().into_iter();
    while let (Some(TokenTree::Punct(hash)), Some(TokenTree::Group(attr))) =
        (tokens.next(), tokens.next())
    {
        if hash.as_char() != '#' || attr.delimiter() != Delimiter::Bracket {
            return false;
        }
        let Ok(syn::Meta::List(list)) = syn::parse2::<syn::Meta>(attr.stream()) else {
            continue;
        };
        if list.path.is_ident("cfg")
            && list
                .parse_args::<syn::Meta>()
                .is_ok_and(|predicate| compiled(&predicate, os) == Some(false))
        {
            return true;
        }
    }
    false
}

/// The directory of `file`, with its final `/` (empty for a file at the root of the run).
fn parent_dir(file: &str) -> &str {
    &file[..file.rfind('/').map_or(0, |slash| slash + 1)]
}

/// The directory that holds the files of the modules that `file` declares (`a/b.rs` declares `a/b/c.rs`; `lib.rs`,
/// `main.rs` and `mod.rs` declare beside themselves).
fn module_dir(file: &str) -> String {
    let dir = parent_dir(file);
    let stem = file[dir.len()..].trim_end_matches(".rs");
    if matches!(stem, "lib" | "main" | "mod") {
        dir.to_string()
    } else {
        format!("{dir}{stem}/")
    }
}

/// The value of a `#[path = "..."]` attribute.
fn path_attr(attrs: &[syn::Attribute]) -> Option<String> {
    attrs.iter().find_map(|attr| match &attr.meta {
        syn::Meta::NameValue(pair) if pair.path.is_ident("path") => match &pair.value {
            syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) => Some(value.value()),
            _ => None,
        },
        _ => None,
    })
}

/// What one file contributes: the lines that `os` does not compile, the module files and directories that it does not
/// compile at all, and the errors.
struct Scan<'a> {
    file: &'a str,
    os: &'a str,
    files: &'a BTreeSet<String>,
    /// The module directory of the current inline module.
    dir: String,
    lines: BTreeSet<usize>,
    modules: Vec<String>,
    errors: Vec<String>,
}

impl Scan<'_> {
    /// Records the lines of `node` when `os` does not compile it, and returns whether it does not.
    fn gate(&mut self, node: &impl ToTokens) -> bool {
        let gated = gated(node, self.os);
        if gated {
            let span = node.span();
            self.lines.extend(span.start().line..=span.end().line);
        }
        gated
    }

    /// The file (and the directory of its own modules) of the gated module `item`, declared without a body.
    fn gate_module(&mut self, item: &syn::ItemMod) {
        let name = item.ident.to_string();
        let candidates = match path_attr(&item.attrs) {
            Some(path) => vec![format!("{}{path}", parent_dir(self.file))],
            None => vec![
                format!("{}{name}.rs", self.dir),
                format!("{}{name}/mod.rs", self.dir),
            ],
        };
        match candidates.into_iter().find(|c| self.files.contains(c)) {
            Some(found) => {
                self.modules.push(module_dir(&found));
                self.modules.push(found);
            }
            None => self.errors.push(format!(
                "{}:{}: the file of the platform-gated module `{name}` is not found",
                self.file,
                item.span().start().line
            )),
        }
    }
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item(&mut self, item: &'ast syn::Item) {
        match item {
            syn::Item::Mod(module) if module.content.is_none() => {
                if gated(module, self.os) {
                    self.gate_module(module);
                }
            }
            _ if self.gate(item) => {}
            _ => syn::visit::visit_item(self, item),
        }
    }

    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        let outer = self.dir.clone();
        self.dir = format!("{outer}{}/", module.ident);
        syn::visit::visit_item_mod(self, module);
        self.dir = outer;
    }

    fn visit_impl_item(&mut self, item: &'ast syn::ImplItem) {
        if !self.gate(item) {
            syn::visit::visit_impl_item(self, item);
        }
    }

    fn visit_trait_item(&mut self, item: &'ast syn::TraitItem) {
        if !self.gate(item) {
            syn::visit::visit_trait_item(self, item);
        }
    }

    fn visit_stmt(&mut self, stmt: &'ast syn::Stmt) {
        if !self.gate(stmt) {
            syn::visit::visit_stmt(self, stmt);
        }
    }

    fn visit_arm(&mut self, arm: &'ast syn::Arm) {
        if !self.gate(arm) {
            syn::visit::visit_arm(self, arm);
        }
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        if mac
            .path
            .segments
            .last()
            .is_some_and(|s| s.ident == "cfg_if")
        {
            self.errors.push(format!(
                "{}:{}: cfg_if! hides its platform branches from the derivation; write #[cfg] items",
                self.file,
                mac.span().start().line
            ));
        }
        syn::visit::visit_macro(self, mac);
    }
}

/// The `--exclude-re` patterns of a mutation run on `os`: one per file with code that `os` does not compile, matching the
/// mutants of that file (a whole gated module) or of its gated lines. `files` holds each Rust source file as (path relative
/// to the root of the run, text).
///
/// # Errors
/// A file does not parse, a gated module's file is not found, or a `cfg_if!` hides its branches.
pub fn exclusions(files: &[(String, String)], os: &str) -> Result<Vec<String>, Vec<String>> {
    let paths: BTreeSet<String> = files.iter().map(|(path, _)| path.clone()).collect();
    let mut lines: BTreeMap<&str, BTreeSet<usize>> = BTreeMap::new();
    let mut modules = Vec::new();
    let mut errors = Vec::new();
    for (path, text) in files {
        let parsed = match syn::parse_file(text) {
            Ok(parsed) => parsed,
            Err(e) => {
                errors.push(format!(
                    "{path}:{}: does not parse: {e}",
                    e.span().start().line
                ));
                continue;
            }
        };
        let mut scan = Scan {
            file: path,
            os,
            files: &paths,
            dir: module_dir(path),
            lines: BTreeSet::new(),
            modules: Vec::new(),
            errors: Vec::new(),
        };
        scan.visit_file(&parsed);
        if !scan.lines.is_empty() {
            lines.insert(path, scan.lines);
        }
        modules.extend(scan.modules);
        errors.extend(scan.errors);
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    let whole = |path: &str| {
        modules
            .iter()
            .any(|m| path == m || (m.ends_with('/') && path.starts_with(m.as_str())))
    };
    let mut patterns: Vec<String> = paths
        .iter()
        .filter(|path| whole(path))
        .map(|path| format!("^{}:", regex::escape(path)))
        .collect();
    for (path, gated) in lines {
        if whole(path) {
            continue;
        }
        let numbers: Vec<String> = gated.iter().map(usize::to_string).collect();
        patterns.push(format!("^{}:({}):", regex::escape(path), numbers.join("|")));
    }
    Ok(patterns)
}

#[cfg(test)]
mod tests;
