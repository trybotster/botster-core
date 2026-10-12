//! `cargo xtask built-in`: every workspace crate takes part in the build-root check of the test binaries (bug catalog B13;
//! design `design-gs-b13-stale-binary.md`, Core lead 2026-10-11: "today showed that review misses mechanical rules").
//!
//! - A crate with a library exports `pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");` from its library root.
//! - Every crate has the unit test `this_test_binary_was_built_in_this_worktree` at the root of its library, or of its binary
//!   when it has no library: a `#[test]` function that the default tier runs on Linux and on macOS (the test index and tier
//!   rules of `mutants-cited`, so an `#[ignore]`, a `cfg` that never holds, a missing `#[test]` or a commented-out function
//!   is no test), and whose body is one call of `built_in::assert_built_here` with the compile-time package name and root
//!   (parsed and compared token for token with the supported forms, so a call in a comment or a run-time root is no call).
//! - The `LINKED` list of `tests/suite/mod.rs` (the conformance binaries) names the binary itself and each workspace library
//!   that `botster-core` links into them: its dependencies and dev-dependencies, and their dependencies, transitively.

use crate::mutants_cited::{self, Filter};
use anyhow::{bail, Context, Result};
use quote::ToTokens;
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

/// The token text of each supported body statement of the root function `TEST_NAME` of `package`. `lib`: the root file is
/// `src/lib.rs`, which has `BUILT_IN` (`CONST_LINE`); a binary root passes `env!("CARGO_MANIFEST_DIR")` itself.
///
/// - The path is the full path of the function: through `botster_test_process` or the testkit re-export, or, in
///   `botster-test-process` itself, through its own module. A function of another crate whose path ends in the same
///   names is not the call.
/// - The first argument is the compile-time package name, and the second is the compile-time root of this crate, so a
///   test that passes the run-time root (which equals the run-time root it is compared with) is refused (017c).
/// - The forms differ only in the `;` and in the comma that rustfmt adds when it wraps the arguments.
fn supported_calls(package: &str, lib: bool) -> BTreeSet<String> {
    let mut paths = vec![
        "botster_test_process::built_in::assert_built_here",
        "botster_core_testkit::built_in::assert_built_here",
    ];
    if package == "botster-test-process" {
        paths.extend([
            "built_in::assert_built_here",
            "crate::built_in::assert_built_here",
        ]);
    }
    let mut roots = vec!["env!(\"CARGO_MANIFEST_DIR\")"];
    if lib {
        roots.push("BUILT_IN");
    }
    let mut calls = BTreeSet::new();
    for path in &paths {
        for root in &roots {
            for comma in ["", ","] {
                for semi in ["", ";"] {
                    // A body, as the root function has it: a call without `;` is the tail expression of a block.
                    let text =
                        format!("{{ {path}(env!(\"CARGO_PKG_NAME\"), {root}{comma}){semi} }}");
                    let block: syn::Block = syn::parse_str(&text).expect("a supported call parses");
                    calls.extend(
                        block
                            .stmts
                            .iter()
                            .map(|stmt| stmt.to_token_stream().to_string()),
                    );
                }
            }
        }
    }
    calls
}

/// Whether the root function `TEST_NAME` of `text` has a supported form: its body is exactly one statement, and that
/// statement is, token for token, one of [`supported_calls`]. Every other body is refused: a reference to the function, a
/// call inside a nested function or closure, a call after an early return, an attribute on the call (`#[cfg(any())]` can
/// remove it, Sol-B), or another argument. An inner attribute of the body applies to the function, so the body has none.
/// A file that does not parse has no such function.
fn root_test_calls(text: &str, package: &str, lib: bool) -> bool {
    let Ok(file) = syn::parse_file(text) else {
        return false;
    };
    let supported = supported_calls(package, lib);
    file.items.iter().any(|item| match item {
        syn::Item::Fn(function) if function.sig.ident == TEST_NAME => {
            let inner = function
                .attrs
                .iter()
                .any(|attr| matches!(attr.style, syn::AttrStyle::Inner(_)));
            match function.block.stmts.as_slice() {
                [stmt] if !inner => supported.contains(&stmt.to_token_stream().to_string()),
                _ => false,
            }
        }
        _ => false,
    })
}

/// The tiers that run `TEST_NAME` at the root of each package's library (or binary), by package, from the test index and
/// tier rules of `mutants-cited`. `files` holds `(package, root file relative to root, kind)`. Only the default tier counts,
/// so each package is given no slow tier.
fn root_test_tiers(
    files: &[(String, String, String)],
    read: &dyn Fn(&str) -> Option<String>,
) -> Result<BTreeMap<String, Vec<String>>> {
    let packages: Vec<mutants_cited::Package> = files
        .iter()
        .map(|(name, root, kind)| mutants_cited::Package {
            name: name.clone(),
            slow: false,
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
    let test_calls = match (&lib, &main) {
        (Some(text), _) => root_test_calls(text, &name, true),
        (None, Some(text)) => root_test_calls(text, &name, false),
        (None, None) => false,
    };
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
        let call = "botster_test_process::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), BUILT_IN);";
        let body = |stmt: &str| format!("#[test]\nfn {TEST_NAME}() {{\n    {stmt}\n}}\n");
        // A library root, with `BUILT_IN` or with the macro, and wrapped by rustfmt.
        assert!(root_test_calls(&body(call), "kit", true));
        let wrapped = "botster_test_process::built_in::assert_built_here(\n        env!(\"CARGO_PKG_NAME\"),\n        env!(\"CARGO_MANIFEST_DIR\"),\n    );";
        assert!(root_test_calls(&body(wrapped), "kit", true));
        // A binary root has no `BUILT_IN`, so it passes the macro.
        assert!(root_test_calls(&body(wrapped), "worker", false));
        assert!(!root_test_calls(&body(call), "worker", false));
        // The re-export path of the testkit.
        let testkit = "botster_core_testkit::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), BUILT_IN);";
        assert!(root_test_calls(&body(testkit), "kit", true));
        // Its own module, only in `botster-test-process`: elsewhere it is another function with the same names.
        let own = "built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), BUILT_IN);";
        assert!(root_test_calls(&body(own), "botster-test-process", true));
        assert!(!root_test_calls(&body(own), "kit", true));
        for negative in [
            // The run-time root equals the run-time root that it is compared with, so a stale binary passes (017c,
            // Sol-B, round 4). Also a literal root, a literal package name, and an early return in an argument.
            body("botster_test_process::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), &std::env::var(\"CARGO_MANIFEST_DIR\").unwrap());"),
            body("botster_test_process::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), \"/root\");"),
            body("botster_test_process::built_in::assert_built_here(\"x\", BUILT_IN);"),
            body("botster_test_process::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), return);"),
            body("botster_test_process::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"));"),
            // Another function whose path ends in the same names.
            body("other::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), BUILT_IN);"),
            body("crate::built_in::assert_built_here(env!(\"CARGO_PKG_NAME\"), BUILT_IN);"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    // {call}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{}}\nfn other() {{\n    {call}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    botster_test_process::built_in::check_here(&[]).unwrap();\n}}\n"),
            "not rust {".to_string(),
            // A reference to the function, not a call (Sol-B, round 2).
            format!("#[test]\nfn {TEST_NAME}() {{\n    let _ = botster_test_process::built_in::assert_built_here;\n}}\n"),
            // A call inside a nested function that nothing calls (Sol-B, round 2), and inside a closure.
            format!("#[test]\nfn {TEST_NAME}() {{\n    fn never() {{\n        {call}\n    }}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    let _never = || {{\n        {call}\n    }};\n}}\n"),
            // A call after an early return, and a call with another statement before it.
            format!("#[test]\nfn {TEST_NAME}() {{\n    return;\n    {call}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    if false {{\n        {call}\n    }}\n}}\n"),
            // An attribute can remove the call (Sol-B, round 3): on the statement, and as an inner attribute of the body.
            format!("#[test]\nfn {TEST_NAME}() {{\n    #[cfg(any())]\n    {call}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    #[allow(unused)]\n    {call}\n}}\n"),
            format!("#[test]\nfn {TEST_NAME}() {{\n    #![cfg(any())]\n    {call}\n}}\n"),
            // A method or macro of that name is not the call.
            format!("#[test]\nfn {TEST_NAME}() {{\n    x.assert_built_here(\"x\", BUILT_IN);\n}}\n"),
        ] {
            assert!(!root_test_calls(&negative, "kit", true), "{negative}");
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
