//! `cargo xtask built-in`: every workspace crate takes part in the build-root check of the test binaries (bug catalog B13;
//! design `design-gs-b13-stale-binary.md`, Core lead 2026-10-11: "today showed that review misses mechanical rules").
//!
//! - A crate with a library exports `pub const BUILT_IN: &str = env!("CARGO_MANIFEST_DIR");` from its library root.
//! - Every crate has the unit test `this_test_binary_was_built_in_this_worktree` in its library root, or in its binary root
//!   when it has no library, which calls `built_in::assert_built_here`.
//! - The `LINKED` list of `tests/suite/mod.rs` (the conformance binaries) names the binary itself and each workspace library
//!   that `botster-core` links into them: its dependencies and dev-dependencies, and their dependencies, transitively.

use anyhow::{bail, Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// The line that each library root has.
pub const CONST_LINE: &str = "pub const BUILT_IN: &str = env!(\"CARGO_MANIFEST_DIR\");";
/// The unit test that each crate has.
pub const TEST_FN: &str = "fn this_test_binary_was_built_in_this_worktree()";
/// The call that the unit test makes.
pub const TEST_CALL: &str = "built_in::assert_built_here(";
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
        match package.lib.as_ref().or(package.main.as_ref()) {
            Some(root) if root.contains(TEST_FN) && root.contains(TEST_CALL) => {}
            Some(_) => problems.push(format!(
                "{name}: its library root (or its binary root, with no library) has no `{TEST_FN}` that calls `{TEST_CALL}..)`"
            )),
            None => problems.push(format!("{name}: no src/lib.rs and no src/main.rs")),
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
    Ok(Package {
        name,
        lib: read("src/lib.rs"),
        main: read("src/main.rs"),
        dependencies: table_names(&manifest, "dependencies"),
        dev_dependencies: table_names(&manifest, "dev-dependencies"),
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
    for dir in dirs {
        let package = package(&dir)?;
        packages.insert(package.name.clone(), package);
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
        packages.get_mut("guardian").unwrap().lib = Some(LIB.replace(TEST_FN, "fn other()"));
        packages.get_mut("fuzz").unwrap().lib = Some(LIB.replace(TEST_CALL, "assert!("));
        assert_eq!(
            verdict(&packages, &suite),
            vec![
                format!("edges: src/lib.rs has no line `{CONST_LINE}`"),
                format!("fuzz: its library root (or its binary root, with no library) has no `{TEST_FN}` that calls `{TEST_CALL}..)`"),
                format!("guardian: its library root (or its binary root, with no library) has no `{TEST_FN}` that calls `{TEST_CALL}..)`"),
            ]
        );
    }

    #[test]
    fn a_binary_crate_needs_the_test_in_its_binary_root() {
        let (mut packages, suite) = good();
        packages.get_mut("xtask").unwrap().main = Some("fn main() {}\n".to_string());
        assert_eq!(
            verdict(&packages, &suite),
            vec![format!("xtask: its library root (or its binary root, with no library) has no `{TEST_FN}` that calls `{TEST_CALL}..)`")]
        );
        packages.get_mut("xtask").unwrap().main = None;
        assert_eq!(
            verdict(&packages, &suite),
            vec!["xtask: no src/lib.rs and no src/main.rs".to_string()]
        );
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

    /// The workspace itself passes: the gate's taint step runs the command too, but this test fails first, locally.
    #[test]
    fn this_workspace_passes() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
        command(root, &[]).unwrap();
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
