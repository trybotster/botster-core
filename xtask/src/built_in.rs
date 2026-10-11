//! `cargo xtask built-in`: every workspace crate takes part in the build-root check of the test binaries (bug catalog B13;
//! design `design-gs-b13-stale-binary.md`, Core lead 2026-10-11: "today showed that review misses mechanical rules").
//!
//! - A crate with a library exports `pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");` from its library root.
//! - Every crate has the unit test `this_test_binary_was_built_in_this_worktree` at the root of its library, or of its binary
//!   when it has no library: a `#[test]` function that the default tier runs on Linux and on macOS (the test index and tier
//!   rules of `mutants-cited`, so an `#[ignore]`, a `cfg` that never holds, a missing `#[test]` or a commented-out function
//!   is no test), and whose body calls `built_in::assert_built_here` (parsed, so a call in a comment is no call).
//! - The `LINKED` list of `tests/suite/mod.rs` (the conformance binaries) names the binary itself and each workspace library
//!   that `botster-core` links into them: its dependencies and dev-dependencies, and their dependencies, transitively.

use crate::mutants_cited::{self, Filter};
use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The line that each library root has.
pub const CONST_LINE: &str = "pub const BUILT_IN: &str = env!(\"CARGO_MANIFEST_DIR\");";
/// The unit test that each crate has.
pub const TEST_NAME: &str = "this_test_binary_was_built_in_this_worktree";
/// The function that the unit test calls (the last two segments of its path).
pub const TEST_CALL: [&str; 2] = ["built_in", "assert_built_here"];
/// The tiers that must run the unit test.
pub const TEST_TIERS: [&str; 2] = ["default on linux", "default on macos"];
/// The suite of the conformance binaries.
pub const SUITE: &str = "tests/suite/mod.rs";
/// The package of the conformance binaries.
pub const CORE: &str = "botster-core";
/// The entry of `LINKED` for the binary itself.
pub const SELF_ENTRY: &str = "(\"this test binary\", env!(\"CARGO_MANIFEST_DIR\")),";

/// A workspace package, as its files show it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Package {
    pub name: String,
    /// The text of `src/lib.rs`, when the package has a library.
    pub lib: Option<String>,
    /// The text of `src/main.rs`, when the package has a binary there.
    pub main: Option<String>,
    /// The names of its dependencies (every `dependencies` table, target tables included).
    pub dependencies: BTreeSet<String>,
    /// The names of its dev-dependencies (every `dev-dependencies` table).
    pub dev_dependencies: BTreeSet<String>,
    /// The tiers that run the unit test `TEST_NAME` at the root of the library (or binary): empty when there is no such
    /// test.
    pub test_tiers: Vec<String>,
    /// Whether the body of the root function `TEST_NAME` calls `built_in::assert_built_here`.
    pub test_calls: bool,
}

/// The `LINKED` entry of a library: `("<name>", <name with _>::BUILT_IN),`.
fn entry(name: &str) -> String {
    format!("(\"{name}\", {}::BUILT_IN),", name.replace('-', "_"))
}

/// Whether `text` has `needle` as a whole line (spaces at the ends ignored).
fn has_line(text: &str, needle: &str) -> bool {
    text.lines().any(|line| line.trim() == needle)
}

/// The text without `//` comments, without white space and without the comma that rustfmt adds before a closing
/// parenthesis when it wraps a tuple, so that an entry matches however rustfmt wraps it, and a commented-out entry does
/// not match.
fn compact(text: &str) -> String {
    let code: String = text
        .lines()
        .map(|line| line.split_once("//").map_or(line, |(code, _)| code))
        .flat_map(str::chars)
        .filter(|c| !c.is_whitespace())
        .collect();
    code.replace(",)", ")")
}

/// The workspace libraries in the conformance binaries: `botster-core`, and every workspace library that it reaches through
/// its dependencies and dev-dependencies, then through their dependencies.
fn linked(packages: &BTreeMap<String, Package>) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let Some(core) = packages.get(CORE) else {
        return found;
    };
    found.insert(CORE.to_string());
    let mut next: Vec<&String> = core
        .dependencies
        .iter()
        .chain(&core.dev_dependencies)
        .collect();
    while let Some(name) = next.pop() {
        let Some(package) = packages.get(name) else {
            continue;
        };
        if package.lib.is_some() && found.insert(name.clone()) {
            next.extend(&package.dependencies);
        }
    }
    found
}

/// The problems, one line each. No problem: an empty list.
pub fn verdict(packages: &BTreeMap<String, Package>, suite: &str) -> Vec<String> {
    let mut problems = Vec::new();
    for package in packages.values() {
        let name = &package.name;
        if let Some(lib) = &package.lib {
            if !has_line(lib, CONST_LINE) {
                problems.push(format!("{name}: src/lib.rs has no line `{CONST_LINE}`"));
            }
        }
        if package.lib.is_none() && package.main.is_none() {
            problems.push(format!("{name}: no src/lib.rs and no src/main.rs"));
            continue;
        }
        let missing: Vec<&str> = TEST_TIERS
            .into_iter()
            .filter(|tier| !package.test_tiers.iter().any(|t| t == tier))
            .collect();
        if !missing.is_empty() {
            problems.push(format!(
                "{name}: the {} tier does not run the test `{TEST_NAME}` at the root of its library (or binary)",
                missing.join(" and the ")
            ));
        }
        if !package.test_calls {
            problems.push(format!(
                "{name}: the root function `{TEST_NAME}` does not call `{}`",
                TEST_CALL.join("::")
            ));
        }
    }
    let suite = compact(suite);
    if !suite.contains(&compact(SELF_ENTRY)) {
        problems.push(format!("{SUITE}: LINKED has no entry `{SELF_ENTRY}`"));
    }
    for name in linked(packages) {
        if !suite.contains(&compact(&entry(&name))) {
            problems.push(format!("{SUITE}: LINKED has no entry `{}`", entry(&name)));
        }
    }
    problems
}

/// Whether the root function `TEST_NAME` of `text` calls `built_in::assert_built_here` (a path that ends in those two
/// segments). A file that does not parse has no such call.
fn root_test_calls(text: &str) -> bool {
    struct Calls(bool);
    impl<'ast> syn::visit::Visit<'ast> for Calls {
        fn visit_path(&mut self, path: &'ast syn::Path) {
            let names: Vec<String> = path.segments.iter().map(|s| s.ident.to_string()).collect();
            self.0 |= names.ends_with(&TEST_CALL.map(str::to_string));
            syn::visit::visit_path(self, path);
        }
    }
    let Ok(file) = syn::parse_file(text) else {
        return false;
    };
    file.items.iter().any(|item| match item {
        syn::Item::Fn(function) if function.sig.ident == TEST_NAME => {
            let mut calls = Calls(false);
            syn::visit::Visit::visit_block(&mut calls, &function.block);
            calls.0
        }
        _ => false,
    })
}

/// The tiers that run `TEST_NAME` at the root of each package's library (or binary), by package, from the test index and
/// tier rules of `mutants-cited`. `files` holds `(package, has a slow feature, root file relative to root, kind)`.
fn root_test_tiers(
    files: &[(String, bool, String, String)],
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<BTreeMap<String, Vec<String>>> {
    let packages: Vec<mutants_cited::Package> = files
        .iter()
        .map(|(name, slow, root, kind)| mutants_cited::Package {
            name: name.clone(),
            slow: *slow,
            targets: vec![mutants_cited::Target {
                kind: kind.clone(),
                name: name.replace('-', "_"),
                root: root.clone(),
                required_features: Vec::new(),
            }],
        })
        .collect();
    let slow = Filter::parse(crate::test_budget::SLOW_FILTER)?;
    let mut tiers = BTreeMap::new();
    for test in mutants_cited::tests(&packages, read)? {
        if test.name == TEST_NAME && test.path == TEST_NAME {
            tiers.insert(
                test.package.clone(),
                mutants_cited::tiers_of(&test, &packages, &slow)?,
            );
        }
    }
    Ok(tiers)
}

/// The dependency names of every table named `table` in a manifest: the top-level one and each `[target.*.<table>]`.
fn table_names(manifest: &toml::Table, table: &str) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    let mut add = |value: Option<&toml::Value>| {
        if let Some(deps) = value.and_then(toml::Value::as_table) {
            names.extend(deps.keys().cloned());
        }
    };
    add(manifest.get(table));
    if let Some(targets) = manifest.get("target").and_then(toml::Value::as_table) {
        for platform in targets.values() {
            add(platform.get(table));
        }
    }
    names
}

/// The package of the directory `dir`, from its `Cargo.toml` and its `src/lib.rs` and `src/main.rs`.
fn package(dir: &Path) -> Result<Package> {
    let text = std::fs::read_to_string(dir.join("Cargo.toml"))
        .with_context(|| format!("read {}/Cargo.toml", dir.display()))?;
    let manifest: toml::Table = text
        .parse()
        .with_context(|| format!("parse {}/Cargo.toml", dir.display()))?;
    let name = manifest
        .get("package")
        .and_then(|package| package.get("name"))
        .and_then(toml::Value::as_str)
        .with_context(|| format!("{}/Cargo.toml has no package name", dir.display()))?
        .to_string();
    let read = |file: &str| std::fs::read_to_string(dir.join(file)).ok();
    let lib = read("src/lib.rs");
    let main = read("src/main.rs");
    let test_calls = lib
        .as_ref()
        .or(main.as_ref())
        .is_some_and(|text| root_test_calls(text));
    Ok(Package {
        name,
        lib,
        main,
        dependencies: table_names(&manifest, "dependencies"),
        dev_dependencies: table_names(&manifest, "dev-dependencies"),
        test_tiers: Vec::new(),
        test_calls,
    })
}

/// Whether the manifest of `dir` declares a `slow` feature.
fn has_slow_feature(dir: &Path) -> bool {
    std::fs::read_to_string(dir.join("Cargo.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .is_some_and(|manifest| {
            manifest
                .get("features")
                .and_then(|f| f.get("slow"))
                .is_some()
        })
}

/// `cargo xtask built-in`: the I/O shell of [`verdict`] over `crates/*` and `xtask`.
///
/// # Errors
/// A manifest cannot be read or parsed, or a crate has a problem.
pub fn command(root: &Path, _args: &[String]) -> Result<()> {
    let mut dirs = vec![root.join("xtask")];
    for entry in std::fs::read_dir(root.join("crates")).context("read crates/")? {
        let path = entry?.path();
        if path.join("Cargo.toml").is_file() {
            dirs.push(path);
        }
    }
    let mut packages = BTreeMap::new();
    let mut files = Vec::new();
    for dir in dirs {
        let package = package(&dir)?;
        let relative = dir
            .strip_prefix(root)
            .unwrap_or(&dir)
            .to_string_lossy()
            .into_owned();
        let (file, kind) = if package.lib.is_some() {
            ("src/lib.rs", "lib")
        } else {
            ("src/main.rs", "bin")
        };
        files.push((
            package.name.clone(),
            has_slow_feature(&dir),
            format!("{relative}/{file}"),
            kind.to_string(),
        ));
        packages.insert(package.name.clone(), package);
    }
    let read = |path: &str| std::fs::read_to_string(root.join(path)).ok();
    for (name, tiers) in root_test_tiers(&files, &read)? {
        if let Some(package) = packages.get_mut(&name) {
            package.test_tiers = tiers;
        }
    }
    let suite =
        std::fs::read_to_string(root.join(SUITE)).with_context(|| format!("read {SUITE}"))?;
    let problems = verdict(&packages, &suite);
    if !problems.is_empty() {
        bail!(
            "the build-root check has gaps (bug catalog B13):\n{}",
            problems.join("\n")
        );
    }
    println!(
        "built-in: {} crates have the build-root test; LINKED names {} libraries",
        packages.len(),
        linked(&packages).len()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LIB: &str = "pub const BUILT_IN: &str = env!(\"CARGO_MANIFEST_DIR\");\n#[test]\nfn this_test_binary_was_built_in_this_worktree() {\n    botster_test_process::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), BUILT_IN);\n}\n";
    const MAIN: &str = "#[test]\nfn this_test_binary_was_built_in_this_worktree() {\n    botster_test_process::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), env!(\"CARGO_MANIFEST_DIR\"));\n}\n";

    fn names(list: &[&str]) -> BTreeSet<String> {
        list.iter().map(|name| name.to_string()).collect()
    }

    fn lib(name: &str, dependencies: &[&str], dev: &[&str]) -> Package {
        Package {
            name: name.to_string(),
            lib: Some(LIB.to_string()),
            main: None,
            dependencies: names(dependencies),
            dev_dependencies: names(dev),
            test_tiers: TEST_TIERS.map(str::to_string).to_vec(),
            test_calls: true,
        }
    }

    fn workspace(list: Vec<Package>) -> BTreeMap<String, Package> {
        list.into_iter()
            .map(|package| (package.name.clone(), package))
            .collect()
    }

    /// botster-core reaches `edges` directly, `kit` as a dev-dependency, and `sys` through `kit`. `guardian` and the
    /// binary-only `xtask` are not linked; a dev-dependency of `kit` (`fuzz`) is not linked either.
    fn good() -> (BTreeMap<String, Package>, String) {
        let mut xtask = lib("xtask", &["kit"], &[]);
        xtask.lib = None;
        xtask.main = Some(MAIN.to_string());
        let packages = workspace(vec![
            lib(CORE, &["edges", "serde"], &["kit"]),
            lib("edges", &[], &["kit"]),
            lib("kit", &["sys"], &["fuzz"]),
            lib("sys", &["edges"], &[]),
            lib("fuzz", &[], &[]),
            lib("guardian", &["edges"], &[]),
            xtask,
        ]);
        let suite = format!(
            "const LINKED: [(&str, &str); 5] = [\n    {SELF_ENTRY}\n    {}\n    {}\n    {}\n    {}\n];\n",
            entry(CORE),
            entry("edges"),
            entry("kit"),
            entry("sys")
        );
        (packages, suite)
    }

    #[test]
    fn a_workspace_where_every_crate_takes_part_passes() {
        let (packages, suite) = good();
        assert_eq!(verdict(&packages, &suite), Vec::<String>::new());
        assert_eq!(linked(&packages), names(&[CORE, "edges", "kit", "sys"]));
    }

    #[test]
    fn a_library_without_the_const_or_the_test_is_named() {
        let (mut packages, suite) = good();
        packages.get_mut("edges").unwrap().lib =
            Some(LIB.replace(CONST_LINE, "pub const OTHER: u8 = 0;"));
        packages.get_mut("guardian").unwrap().test_tiers = Vec::new();
        packages.get_mut("fuzz").unwrap().test_tiers = vec!["default on linux".to_string()];
        packages.get_mut("kit").unwrap().test_calls = false;
        assert_eq!(
            verdict(&packages, &suite),
            vec![
                format!("edges: src/lib.rs has no line `{CONST_LINE}`"),
                format!("fuzz: the default on macos tier does not run the test `{TEST_NAME}` at the root of its library (or binary)"),
                format!("guardian: the default on linux and the default on macos tier does not run the test `{TEST_NAME}` at the root of its library (or binary)"),
                format!("kit: the root function `{TEST_NAME}` does not call `built_in::assert_built_here`"),
            ]
        );
    }

    #[test]
    fn a_crate_without_a_root_file_is_named() {
        let (mut packages, suite) = good();
        let xtask = packages.get_mut("xtask").unwrap();
        xtask.main = None;
        assert_eq!(
            verdict(&packages, &suite),
            vec!["xtask: no src/lib.rs and no src/main.rs".to_string()]
        );
    }

    /// The test index of `mutants-cited` decides what a test is and which tiers run it: each negative form gives no
    /// default-tier run, and only the root function counts.
    #[test]
    fn only_a_registered_root_test_that_the_default_tier_runs_counts() {
        let body =
            "{\n    botster_test_process::built_in::assert_built_here(\"x\", BUILT_IN);\n}\n";
        let tiers_of = |text: &str| {
            let files = vec![(
                "x".to_string(),
                true,
                "crates/x/src/lib.rs".to_string(),
                "lib".to_string(),
            )];
            let text = text.to_string();
            let read = move |path: &str| (path == "crates/x/src/lib.rs").then(|| text.clone());
            root_test_tiers(&files, &read)
                .unwrap()
                .remove("x")
                .unwrap_or_default()
        };
        let all = TEST_TIERS.map(str::to_string).to_vec();
        assert_eq!(
            tiers_of(&format!("#[cfg(test)]\n#[test]\nfn {TEST_NAME}() {body}")),
            all
        );
        for negative in [
            // No `#[test]`.
            format!("#[cfg(test)]\nfn {TEST_NAME}() {body}"),
            // Commented out.
            format!("// #[test]\n// fn {TEST_NAME}() {{}}\n"),
            format!("/*\n#[test]\nfn {TEST_NAME}() {body}*/\n"),
            // Ignored, or under a cfg that never holds, or only with the slow feature.
            format!("#[test]\n#[ignore]\nfn {TEST_NAME}() {body}"),
            format!("#[cfg(any())]\n#[test]\nfn {TEST_NAME}() {body}"),
            format!("#[cfg(feature = \"slow\")]\n#[test]\nfn {TEST_NAME}() {body}"),
            // Not at the root: inside a module.
            format!("#[cfg(test)]\nmod tests {{\n    #[test]\n    fn {TEST_NAME}() {body}}}\n"),
        ] {
            assert!(
                !tiers_of(&negative).iter().any(|t| t == "default on linux"),
                "{negative}"
            );
        }
        // Only on macOS.
        let mac = tiers_of(&format!(
            "#[cfg(target_os = \"macos\")]\n#[test]\nfn {TEST_NAME}() {body}"
        ));
        assert_eq!(mac, vec!["default on macos".to_string()]);
    }

    #[test]
    fn only_a_parsed_call_in_the_root_function_counts() {
        let call = "botster_test_process::built_in::assert_built_here(\"x\", BUILT_IN);";
        assert!(root_test_calls(&format!(
            "#[test]\nfn {TEST_NAME}() {{\n    {call}\n}}\n"
        )));
        // The re-export path of the testkit, and a `use` of the module.
        assert!(root_test_calls(&format!(
            "#[test]\nfn {TEST_NAME}() {{\n    botster_core_testkit::built_in::assert_built_here(\"x\", BUILT_IN);\n}}\n"
        )));
        for negative in [
            format!("#[test]\nfn {TEST_NAME}() {{\n    // {call}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{}}\nfn other() {{\n    {call}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    botster_test_process::built_in::check_here(&[]).unwrap();\n}}\n"),
            "not rust {".to_string(),
        ] {
            assert!(!root_test_calls(&negative), "{negative}");
        }
    }

    #[test]
    fn a_linked_library_missing_from_the_suite_list_is_named() {
        let (packages, suite) = good();
        // `sys` is reached only through `kit`.
        let without = suite.replace(&entry("sys"), "");
        assert_eq!(
            verdict(&packages, &without),
            vec![format!("{SUITE}: LINKED has no entry `{}`", entry("sys"))]
        );
        let without_self = suite.replace(SELF_ENTRY, "");
        assert_eq!(
            verdict(&packages, &without_self),
            vec![format!("{SUITE}: LINKED has no entry `{SELF_ENTRY}`")]
        );
        // An entry that rustfmt wraps over lines is an entry.
        let wrapped = suite.replace(
            &entry("edges"),
            "(\n        \"edges\",\n        edges::BUILT_IN,\n    ),",
        );
        assert_eq!(verdict(&packages, &wrapped), Vec::<String>::new());
        // A commented-out entry is not an entry.
        let commented = suite.replace(&entry("edges"), &format!("// {}", entry("edges")));
        assert_eq!(
            verdict(&packages, &commented),
            vec![format!("{SUITE}: LINKED has no entry `{}`", entry("edges"))]
        );
    }

    #[test]
    fn a_workspace_without_botster_core_links_nothing() {
        let (mut packages, _) = good();
        packages.remove(CORE);
        assert!(linked(&packages).is_empty());
    }

    #[test]
    fn the_dependency_tables_include_the_target_tables() {
        let manifest: toml::Table = "[dependencies]\na.workspace = true\n[dev-dependencies]\nd = \"1\"\n[target.'cfg(unix)'.dependencies]\nb = \"1\"\n[target.'cfg(unix)'.dev-dependencies]\ne = \"1\"\n"
            .parse()
            .unwrap();
        assert_eq!(table_names(&manifest, "dependencies"), names(&["a", "b"]));
        assert_eq!(
            table_names(&manifest, "dev-dependencies"),
            names(&["d", "e"])
        );
    }

    /// The command reads `crates/*`, `xtask` and the suite, and fails on a gap: a library of `crates/` without the const.
    #[test]
    fn the_command_fails_on_a_crate_without_the_const() {
        let root = botster_test_support::tempdir::TempRoot::new().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write("xtask/Cargo.toml", "[package]\nname = \"xtask\"\n");
        write("xtask/src/main.rs", MAIN);
        write(
            "crates/botster-core/Cargo.toml",
            "[package]\nname = \"botster-core\"\n",
        );
        write("crates/botster-core/src/lib.rs", LIB);
        write(SUITE, &format!("{SELF_ENTRY}\n{}\n", entry(CORE)));
        command(root.path(), &[]).unwrap();
        write(
            "crates/botster-core/src/lib.rs",
            &LIB.replace(CONST_LINE, ""),
        );
        let error = command(root.path(), &[]).unwrap_err().to_string();
        assert!(
            error.contains(&format!(
                "botster-core: src/lib.rs has no line `{CONST_LINE}`"
            )),
            "{error}"
        );
    }
}
