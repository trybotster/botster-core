//! `cargo xtask mutants-cited`: every name that a reason in `.cargo/mutants.toml` cites is real, and every cited test runs in
//! some gate tier (lead, 2026-10-08: an exclusion reason named slow tests that no tier selected, the `slow_tests` modules of
//! a library behind `cfg(feature = "slow")` under the filter `binary(/^slow/)`).
//!
//! The check builds the test list as cargo and nextest do: from each target's root file (`cargo metadata`), it follows every
//! `mod` declaration, and it records each `#[test]` function with its binary, its module path and the `cfg` predicates
//! around it. A test runs in a tier when its predicates hold in the tier's configuration (the `slow` feature on or off,
//! Linux or macOS), it is not `#[ignore]`, and the tier's filter selects it. The default tier runs every test of the
//! workspace; the slow tier runs the tests of the packages that have a `slow` feature, with that feature, that
//! `test_budget::SLOW_FILTER` selects.
//!
//! A cited name is a snake_case word with at least three parts in a comment of the file. It must be a test that some tier
//! runs, a test target with such a test, an item of a tracked Rust file that is not a test (a function, a constant, a type,
//! a field, a module), or a word of a vendored file (a file of a git submodule). A word that names nothing else fails the
//! check: a renamed or deleted test leaves its old name behind in the reason, and a mention in a document, a comment or a
//! string does not keep it alive (#181 B6). A test with `#[ignore]` runs in no tier, and a test with `#[cfg_attr(<predicate>,
//! ignore)]` runs in no tier where the predicate holds; a predicate that the check does not know fails the check.

use anyhow::{bail, Context, Result};
use regex::Regex;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::process::Command;
use syn::visit::Visit;

/// The file whose reasons the check reads, relative to the repository root.
pub const MUTANTS_FILE: &str = ".cargo/mutants.toml";

/// The operating systems of the gate tiers.
const SYSTEMS: [&str; 2] = ["linux", "macos"];

/// A compiled target of a workspace package, as `cargo metadata` gives it.
#[derive(Clone, Debug)]
pub struct Target {
    /// `lib`, `bin` or `test`; other kinds hold no test that a tier runs.
    pub kind: String,
    /// The target name, which nextest calls the binary name.
    pub name: String,
    /// The root file, relative to the repository root.
    pub root: String,
    /// The features that the target requires.
    pub required_features: Vec<String>,
}

/// A workspace package.
#[derive(Clone, Debug)]
pub struct Package {
    pub name: String,
    /// Whether it declares a `slow` feature: the slow tier runs it.
    pub slow: bool,
    pub targets: Vec<Target>,
}

/// A `#[test]` function as nextest lists it.
#[derive(Clone)]
pub struct TestFn {
    pub package: String,
    pub binary: String,
    /// The module path in the binary and the function name, joined by `::` (nextest's test name).
    pub path: String,
    pub name: String,
    /// The `cfg` predicates of the target, the files, the modules and the function.
    cfgs: Vec<syn::Meta>,
    /// The conditions under which the test is ignored: `None` for `#[ignore]`, the predicate for `#[cfg_attr(<predicate>,
    /// ignore)]`.
    ignores: Vec<Option<syn::Meta>>,
}

/// A tier's configuration and filter.
struct Tier<'a> {
    name: &'static str,
    slow: bool,
    filter: Option<&'a Filter>,
}

/// A nextest filter of the forms that the slow tier uses: `binary(/re/)` and `test(/re/)` terms joined by `|` or `or`.
pub struct Filter {
    terms: Vec<(bool, Regex)>,
}

impl Filter {
    /// # Errors
    /// The expression has another form: the check must learn it before the gate uses it.
    pub fn parse(expression: &str) -> Result<Filter> {
        // A term ends at the first `/)`. After it comes the end, or a separator and the next term.
        let term = Regex::new(r"^\s*(binary|test)\(/(.*?)/\)\s*").expect("regex");
        let separator = Regex::new(r"^(?:\||or\s)").expect("regex");
        let mut terms = Vec::new();
        let mut rest = expression;
        loop {
            let Some(found) = term.captures(rest) else {
                bail!(
                    "the filter `{expression}` has a term that the check does not know: `{}`",
                    rest.trim()
                );
            };
            let regex =
                Regex::new(&found[2]).with_context(|| format!("the filter `{expression}`"))?;
            terms.push((&found[1] == "binary", regex));
            rest = &rest[found[0].len()..];
            if rest.is_empty() {
                return Ok(Filter { terms });
            }
            let Some(joined) = separator.find(rest) else {
                bail!(
                    "the filter `{expression}` has a term that the check does not know: `{}`",
                    rest.trim()
                );
            };
            rest = &rest[joined.end()..];
        }
    }

    fn selects(&self, test: &TestFn) -> bool {
        self.terms
            .iter()
            .any(|(binary, regex)| regex.is_match(if *binary { &test.binary } else { &test.path }))
    }
}

/// Whether `meta` holds with the `slow` feature on or off, on `system`.
fn holds(meta: &syn::Meta, slow: bool, system: &str) -> Result<bool, String> {
    let list = |meta: &syn::Meta| -> Result<Vec<syn::Meta>, String> {
        let syn::Meta::List(list) = meta else {
            return Err(format!("`{}` takes a list", path_name(meta.path())));
        };
        list.parse_args_with(
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
        )
        .map(|items| items.into_iter().collect())
        .map_err(|error| error.to_string())
    };
    let name = path_name(meta.path());
    match (name.as_str(), meta) {
        ("all", _) => list(meta)?
            .iter()
            .try_fold(true, |all, m| Ok(all && holds(m, slow, system)?)),
        ("any", _) => list(meta)?
            .iter()
            .try_fold(false, |any, m| Ok(any || holds(m, slow, system)?)),
        ("not", _) => match list(meta)?.as_slice() {
            [one] => Ok(!holds(one, slow, system)?),
            _ => Err("`not` takes one predicate".to_string()),
        },
        ("test" | "unix" | "debug_assertions", syn::Meta::Path(_)) => Ok(true),
        ("windows", syn::Meta::Path(_)) => Ok(false),
        (key @ ("feature" | "target_os" | "target_family"), syn::Meta::NameValue(pair)) => {
            let syn::Expr::Lit(syn::ExprLit {
                lit: syn::Lit::Str(value),
                ..
            }) = &pair.value
            else {
                return Err(format!("`{key}` takes a string"));
            };
            let value = value.value();
            Ok(match key {
                "feature" => slow && value == "slow",
                "target_os" => value == system,
                _ => value == "unix",
            })
        }
        _ => Err(format!("the check does not know the predicate `{name}`")),
    }
}

fn path_name(path: &syn::Path) -> String {
    path.segments
        .iter()
        .map(|s| s.ident.to_string())
        .collect::<Vec<_>>()
        .join("::")
}

/// What the attributes of an item say for the test list: its `cfg` predicates (into `cfgs`), whether they mark a test, and
/// when they ignore it.
fn read_attrs(
    attrs: &[syn::Attribute],
    cfgs: &mut Vec<syn::Meta>,
) -> Result<(bool, Vec<Option<syn::Meta>>), String> {
    let mut test = false;
    let mut ignores = Vec::new();
    for attr in attrs {
        let path = attr.path();
        if path.is_ident("cfg") {
            cfgs.push(
                attr.parse_args::<syn::Meta>()
                    .map_err(|error| error.to_string())?,
            );
        } else if path.is_ident("cfg_attr") {
            conditional(&attr.meta, None, cfgs, &mut test, &mut ignores)?;
        } else if path.is_ident("ignore") {
            ignores.push(None);
        } else if is_test_path(path) {
            test = true;
        }
    }
    Ok((test, ignores))
}

/// Whether an attribute path marks a test: `test` or a path that ends in it (`tokio::test`).
fn is_test_path(path: &syn::Path) -> bool {
    path.segments
        .last()
        .is_some_and(|last| last.ident == "test")
}

/// A `cfg_attr(<predicate>, <attribute>, ..)` under the condition `outer`: an `ignore` ignores the test where both hold, a
/// test attribute makes a test that exists only where both hold, and a nested `cfg_attr` adds its predicate.
fn conditional(
    meta: &syn::Meta,
    outer: Option<&syn::Meta>,
    cfgs: &mut Vec<syn::Meta>,
    test: &mut bool,
    ignores: &mut Vec<Option<syn::Meta>>,
) -> Result<(), String> {
    let syn::Meta::List(list) = meta else {
        return Err("`cfg_attr` takes a list".to_string());
    };
    let mut items = list
        .parse_args_with(syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated)
        .map_err(|error| error.to_string())?
        .into_iter();
    let Some(predicate) = items.next() else {
        return Err("`cfg_attr` takes a predicate".to_string());
    };
    let condition: syn::Meta = match outer {
        Some(outer) => syn::parse_quote!(all(#outer, #predicate)),
        None => predicate,
    };
    for item in items {
        let path = item.path();
        if path.is_ident("cfg_attr") {
            conditional(&item, Some(&condition), cfgs, test, ignores)?;
        } else if path.is_ident("ignore") {
            ignores.push(Some(condition.clone()));
        } else if is_test_path(path) {
            *test = true;
            cfgs.push(condition.clone());
        }
    }
    Ok(())
}

/// The value of a `#[path = "..."]` attribute.
fn path_attr(attrs: &[syn::Attribute]) -> Option<String> {
    attrs
        .iter()
        .find(|attr| attr.path().is_ident("path"))
        .and_then(|attr| match &attr.meta {
            syn::Meta::NameValue(syn::MetaNameValue {
                value:
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(value),
                        ..
                    }),
                ..
            }) => Some(value.value()),
            _ => None,
        })
}

/// The walk of one target's module tree.
struct Walk<'a> {
    package: &'a str,
    binary: &'a str,
    read: &'a dyn Fn(&str) -> Option<String>,
    found: Vec<TestFn>,
}

impl Walk<'_> {
    /// Walks `file`, whose child modules are in `dir`.
    fn file(&mut self, file: &str, dir: &str, module: &[String], cfgs: &[syn::Meta]) -> Result<()> {
        let text = (self.read)(file).with_context(|| format!("{file} cannot be read"))?;
        let parsed = syn::parse_file(&text).map_err(|error| {
            anyhow::anyhow!(
                "{file}:{}: does not parse: {error}",
                error.span().start().line
            )
        })?;
        let mut cfgs = cfgs.to_vec();
        read_attrs(&parsed.attrs, &mut cfgs).map_err(|error| anyhow::anyhow!("{file}: {error}"))?;
        self.items(&parsed.items, file, dir, module, &cfgs)
    }

    fn items(
        &mut self,
        items: &[syn::Item],
        file: &str,
        dir: &str,
        module: &[String],
        cfgs: &[syn::Meta],
    ) -> Result<()> {
        for item in items {
            match item {
                syn::Item::Fn(function) => {
                    let mut own = cfgs.to_vec();
                    let (test, ignores) = read_attrs(&function.attrs, &mut own)
                        .map_err(|error| anyhow::anyhow!("{file}: {error}"))?;
                    if test {
                        let name = function.sig.ident.to_string();
                        let mut path = module.to_vec();
                        path.push(name.clone());
                        self.found.push(TestFn {
                            package: self.package.to_string(),
                            binary: self.binary.to_string(),
                            path: path.join("::"),
                            name,
                            cfgs: own,
                            ignores,
                        });
                    }
                }
                syn::Item::Mod(declared) => {
                    let mut own = cfgs.to_vec();
                    read_attrs(&declared.attrs, &mut own)
                        .map_err(|error| anyhow::anyhow!("{file}: {error}"))?;
                    let name = declared.ident.to_string();
                    let mut inner = module.to_vec();
                    inner.push(name.clone());
                    let child_dir = format!("{dir}/{name}");
                    if let Some((_, items)) = &declared.content {
                        self.items(items, file, &child_dir, &inner, &own)?;
                        continue;
                    }
                    let candidates = match path_attr(&declared.attrs) {
                        Some(path) => {
                            vec![(format!("{dir}/{path}"), parent(&format!("{dir}/{path}")))]
                        }
                        None => vec![
                            (format!("{dir}/{name}.rs"), child_dir.clone()),
                            (format!("{dir}/{name}/mod.rs"), child_dir.clone()),
                        ],
                    };
                    let Some((path, child)) = candidates
                        .into_iter()
                        .find(|(path, _)| (self.read)(path).is_some())
                    else {
                        bail!("{file}: `mod {name};` has no file");
                    };
                    self.file(&path, &child, &inner, &own)?;
                }
                _ => {}
            }
        }
        Ok(())
    }
}

fn parent(path: &str) -> String {
    path.rsplit_once('/')
        .map_or_else(String::new, |(dir, _)| dir.to_string())
}

/// Every `#[test]` function of the workspace's lib, bin and test targets.
///
/// # Errors
/// A file cannot be read or parsed, or a `mod` declaration has no file.
pub fn tests(packages: &[Package], read: &dyn Fn(&str) -> Option<String>) -> Result<Vec<TestFn>> {
    let mut found = Vec::new();
    for package in packages {
        for target in &package.targets {
            if !matches!(target.kind.as_str(), "lib" | "bin" | "test") {
                continue;
            }
            let mut walk = Walk {
                package: &package.name,
                binary: &target.name,
                read,
                found: Vec::new(),
            };
            let cfgs: Vec<syn::Meta> = target
                .required_features
                .iter()
                .map(|feature| {
                    syn::parse_str(&format!("feature = {feature:?}")).expect("a feature predicate")
                })
                .collect();
            walk.file(&target.root, &parent(&target.root), &[], &cfgs)?;
            found.extend(walk.found);
        }
    }
    Ok(found)
}

/// The tiers that run `test`, as `tier on system` names; empty when none does.
///
/// # Errors
/// A predicate that the check does not know.
fn tiers_of(test: &TestFn, packages: &[Package], slow: &Filter) -> Result<Vec<String>> {
    let has_slow = packages.iter().any(|p| p.name == test.package && p.slow);
    let tiers = [
        Tier {
            name: "default",
            slow: false,
            filter: None,
        },
        Tier {
            name: "slow",
            slow: true,
            filter: Some(slow),
        },
    ];
    let mut runs = Vec::new();
    for tier in tiers.iter().filter(|tier| !tier.slow || has_slow) {
        if tier.filter.is_some_and(|filter| !filter.selects(test)) {
            continue;
        }
        for system in SYSTEMS {
            let known = |error| anyhow::anyhow!("{} ({}): {error}", test.path, test.binary);
            let mut all = true;
            for cfg in &test.cfgs {
                all &= holds(cfg, tier.slow, system).map_err(known)?;
            }
            for ignore in &test.ignores {
                all &= match ignore {
                    Some(condition) => !holds(condition, tier.slow, system).map_err(known)?,
                    None => false,
                };
            }
            if all {
                runs.push(format!("{} on {system}", tier.name));
            }
        }
    }
    Ok(runs)
}

/// The cited names of the file and the line of each one's first citation.
pub fn cited(toml: &str) -> BTreeMap<String, usize> {
    let word = Regex::new(r"\b[a-z][a-z0-9]*(?:_[a-z0-9]+){2,}\b").expect("regex");
    let mut names = BTreeMap::new();
    for (index, line) in toml.lines().enumerate() {
        let line = line.trim_start();
        if let Some(comment) = line.strip_prefix('#') {
            for found in word.find_iter(comment) {
                names.entry(found.as_str().to_string()).or_insert(index + 1);
            }
        }
    }
    names
}

/// The violations: each cited name that is not a test that some tier runs, a test target with such a test, an item of a
/// Rust file in `paths` that is not a test, or a word of a vendored file in `paths` (under a `path` of `.gitmodules`).
///
/// # Errors
/// A source file cannot be walked or parsed, or a predicate is unknown.
pub fn check(
    toml: &str,
    packages: &[Package],
    read: &dyn Fn(&str) -> Option<String>,
    paths: &[String],
    slow: &Filter,
) -> Result<Vec<String>> {
    let tests = tests(packages, read)?;
    let mut violations = Vec::new();
    let mut words = BTreeMap::new();
    for (name, line) in cited(toml) {
        let named: Vec<&TestFn> = tests.iter().filter(|t| t.name == name).collect();
        if !named.is_empty() {
            let mut runs = 0;
            for test in &named {
                runs += tiers_of(test, packages, slow)?.len();
            }
            if runs == 0 {
                let places: Vec<String> = named
                    .iter()
                    .map(|t| format!("{} in {}", t.path, t.binary))
                    .collect();
                violations.push(format!(
                    "{MUTANTS_FILE}:{line}: cites the test `{name}`, which no gate tier runs ({})",
                    places.join(", ")
                ));
            }
            continue;
        }
        let target: Vec<&TestFn> = tests.iter().filter(|t| t.binary == name).collect();
        if packages
            .iter()
            .any(|p| p.targets.iter().any(|t| t.kind == "test" && t.name == name))
        {
            let mut runs = 0;
            for test in &target {
                runs += tiers_of(test, packages, slow)?.len();
            }
            if runs == 0 {
                violations.push(format!(
                    "{MUTANTS_FILE}:{line}: cites the test target `{name}`, which no gate tier runs a test of"
                ));
            }
            continue;
        }
        words.insert(name, line);
    }
    if !words.is_empty() {
        let defined = defined_names(read, paths)?;
        words.retain(|name, _| !defined.contains(name));
    }
    for (name, line) in words {
        violations.push(format!(
            "{MUTANTS_FILE}:{line}: cites `{name}`, which names no test, no test target, no item and no vendored word"
        ));
    }
    violations.sort();
    Ok(violations)
}

/// The names that a citation may name besides a test: the items of the Rust files of `paths` that are not tests, and the
/// words of the vendored files of `paths` (under a `path` of `.gitmodules`). The mutants file itself is not read.
///
/// # Errors
/// A Rust file outside the vendored files does not parse.
fn defined_names(
    read: &dyn Fn(&str) -> Option<String>,
    paths: &[String],
) -> Result<BTreeSet<String>> {
    let vendored: Vec<String> = read(".gitmodules")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.trim().strip_prefix("path = "))
        .map(|path| format!("{}/", path.trim()))
        .collect();
    let word = Regex::new(r"\b[a-z][a-z0-9_]*\b").expect("regex");
    let mut items = Items::default();
    for path in paths.iter().filter(|path| path.as_str() != MUTANTS_FILE) {
        let Some(text) = read(path) else {
            continue;
        };
        if vendored.iter().any(|dir| path.starts_with(dir.as_str())) {
            items
                .0
                .extend(word.find_iter(&text).map(|w| w.as_str().to_string()));
        } else if path.ends_with(".rs") {
            let parsed = syn::parse_file(&text).map_err(|error| {
                anyhow::anyhow!(
                    "{path}:{}: does not parse: {error}",
                    error.span().start().line
                )
            })?;
            items.visit_file(&parsed);
        }
    }
    Ok(items.0)
}

/// The names of the items of a Rust file that are not tests: functions, methods, constants, statics, types, traits,
/// modules, macros, variants and fields.
#[derive(Default)]
struct Items(BTreeSet<String>);

impl<'ast> Visit<'ast> for Items {
    fn visit_item_fn(&mut self, item: &'ast syn::ItemFn) {
        if !item.attrs.iter().any(|attr| is_test_path(attr.path())) {
            self.0.insert(item.sig.ident.to_string());
        }
        syn::visit::visit_item_fn(self, item);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast syn::ImplItemFn) {
        self.0.insert(item.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, item);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast syn::TraitItemFn) {
        self.0.insert(item.sig.ident.to_string());
        syn::visit::visit_trait_item_fn(self, item);
    }

    fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_const(self, item);
    }

    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_static(self, item);
    }

    fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_struct(self, item);
    }

    fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_enum(self, item);
    }

    fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_type(self, item);
    }

    fn visit_item_trait(&mut self, item: &'ast syn::ItemTrait) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_trait(self, item);
    }

    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        self.0.insert(item.ident.to_string());
        syn::visit::visit_item_mod(self, item);
    }

    fn visit_item_macro(&mut self, item: &'ast syn::ItemMacro) {
        self.0.extend(item.ident.as_ref().map(ToString::to_string));
        syn::visit::visit_item_macro(self, item);
    }

    fn visit_variant(&mut self, variant: &'ast syn::Variant) {
        self.0.insert(variant.ident.to_string());
        syn::visit::visit_variant(self, variant);
    }

    fn visit_field(&mut self, field: &'ast syn::Field) {
        self.0.extend(field.ident.as_ref().map(ToString::to_string));
        syn::visit::visit_field(self, field);
    }
}

/// The workspace packages and their targets, from `cargo metadata`, with root files relative to `root`.
fn packages(root: &Path) -> Result<Vec<Package>> {
    let out = crate::tools::cargo(root)
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .output()
        .context("run cargo metadata")?;
    let json: serde_json::Value =
        serde_json::from_str(&crate::tools::stdout_of(&out, "cargo metadata")?)?;
    let root = std::fs::canonicalize(root)?;
    let strings = |value: &serde_json::Value| -> Vec<String> {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    };
    let mut packages = Vec::new();
    for package in json["packages"]
        .as_array()
        .context("no packages in cargo metadata")?
    {
        let mut targets = Vec::new();
        for target in package["targets"].as_array().into_iter().flatten() {
            let source = Path::new(target["src_path"].as_str().unwrap_or_default());
            let Ok(relative) = source.strip_prefix(&root) else {
                continue;
            };
            targets.push(Target {
                kind: strings(&target["kind"])
                    .into_iter()
                    .next()
                    .unwrap_or_default(),
                name: target["name"].as_str().unwrap_or_default().to_string(),
                root: relative.to_string_lossy().into_owned(),
                required_features: strings(&target["required-features"]),
            });
        }
        packages.push(Package {
            name: package["name"].as_str().unwrap_or_default().to_string(),
            slow: package["features"].get("slow").is_some(),
            targets,
        });
    }
    Ok(packages)
}

/// The tracked files, with the submodules' files: a reason may cite a word of a vendored library.
fn tracked_with_submodules(root: &Path) -> Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["ls-files", "-z", "--recurse-submodules"])
        .output()
        .context("run git ls-files")?;
    Ok(crate::tools::stdout_of(&output, "git ls-files")?
        .split('\0')
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect())
}

pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let toml = std::fs::read_to_string(root.join(MUTANTS_FILE))
        .with_context(|| format!("read {MUTANTS_FILE}"))?;
    let packages = packages(root)?;
    let paths = tracked_with_submodules(root)?;
    let slow = Filter::parse(crate::test_budget::SLOW_FILTER)?;
    let read = |path: &str| std::fs::read_to_string(root.join(path)).ok();
    let violations = check(&toml, &packages, &read, &paths, &slow)?;
    println!("mutants-cited: {} cited names checked", cited(&toml).len());
    crate::tools::verdict(&violations)
}

#[cfg(test)]
mod tests;
