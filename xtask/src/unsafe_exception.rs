//! The exceptions to the workspace's `unsafe_code = "forbid"`, each of one item in one file of one crate:
//! - botster-test-process, the function `close_inherited` of its anchor binary (lead ruling 2026-10-09 on #171 TP3, (A));
//! - botster-core-testkit, `unsafe impl GlobalAlloc for PeakAllocator`, the allocator of the `alloc_window` and
//!   `alloc_peak` controls (lead ruling 2026-10-10, (A); steward rulings R-37, R-52). Its methods only call the system
//!   allocator.
//!
//! Each of these crates copies the workspace's lint table with exactly one difference, `unsafe_code = "deny"`, so that its
//! one item can allow the lint. The check fails:
//! - on any other difference between such a crate's lint table and the workspace's;
//! - on any key or string that names the lint in a manifest or a Cargo configuration (`rustflags` included), but the lint
//!   entries of the workspace, of those crates and of the libghostty-vt binding (the earlier exception: that crate calls a C
//!   ABI, and its manifest allows `unsafe_code` for the whole crate). TOML is parsed, so an escaped key is read as Cargo reads
//!   it; `-` is read as `_`, as rustc reads a lint name;
//! - on any `unsafe_code` identifier in a Rust source, other than the one in the `#[allow(unsafe_code)]` of an exception's
//!   top-level item in its own file. Sources are read as Rust tokens: comments and string literals are not identifiers, and
//!   an attribute on several lines, a `cfg_attr`, an `expect` or a macro body is read as a whole (#171 round 2, TP7).

use crate::fsutil::tracked_files;
use anyhow::{bail, Context, Result};
use proc_macro2::{Delimiter, LineColumn, TokenStream, TokenTree};
use std::path::Path;

/// One exception: the crate's manifest, the file, and the identifiers that start the item (after its attributes).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Exception {
    pub manifest: &'static str,
    pub file: &'static str,
    pub item: &'static [&'static str],
}

/// The exceptions (see the module's text).
pub const EXCEPTIONS: [Exception; 2] = [
    Exception {
        manifest: "crates/botster-test-process/Cargo.toml",
        file: "crates/botster-test-process/src/bin/botster-test-anchor.rs",
        item: &["fn", "close_inherited"],
    },
    Exception {
        manifest: "crates/botster-core-testkit/Cargo.toml",
        file: "crates/botster-core-testkit/src/alloc_controls.rs",
        item: &["unsafe", "impl", "GlobalAlloc", "for", "PeakAllocator"],
    },
];
/// The C ABI binding of libghostty-vt, which allows `unsafe_code` in its own lint table (eddd5e75; not this ruling).
const FFI_CRATE: &str = "crates/botster-terminal-ghostty/Cargo.toml";

/// Whether `(file, entry)` is a lint entry that may name `unsafe_code`: the workspace's, an exception crate's or the FFI
/// crate's.
fn lint_entry(file: &str, entry: &str) -> bool {
    let crate_entry = entry == "lints.rust.unsafe_code"
        && (file == FFI_CRATE || EXCEPTIONS.iter().any(|e| e.manifest == file));
    crate_entry || (file == "Cargo.toml" && entry == "workspace.lints.rust.unsafe_code")
}

/// Whether the crate's lint table is the workspace's with `unsafe_code = "deny"` in place of `"forbid"`, and nothing else.
///
/// # Errors
/// A manifest does not parse, the workspace does not forbid `unsafe_code`, or the tables differ otherwise.
pub fn lint_drift(workspace: &str, name: &str, krate: &str) -> Result<(), String> {
    let workspace: toml::Table = workspace
        .parse()
        .map_err(|e| format!("the workspace manifest: {e}"))?;
    let krate: toml::Table = krate.parse().map_err(|e| format!("{name}: {e}"))?;
    let mut expected = workspace
        .get("workspace")
        .and_then(|w| w.get("lints"))
        .cloned()
        .ok_or("the workspace has no lint table")?;
    let rust = expected
        .get_mut("rust")
        .and_then(toml::Value::as_table_mut)
        .ok_or("the workspace has no rust lint table")?;
    if rust.get("unsafe_code").and_then(toml::Value::as_str) != Some("forbid") {
        return Err("the workspace does not forbid unsafe_code".into());
    }
    rust.insert("unsafe_code".into(), "deny".into());
    let actual = krate
        .get("lints")
        .ok_or(format!("{name} has no lint table"))?;
    if actual != &expected {
        return Err(format!(
            "the lint table of {name} must be the workspace's with only unsafe_code = \"deny\": it is {actual}, expected {expected}"
        ));
    }
    Ok(())
}

/// Whether `text` names the lint, with `-` read as `_` (rustc accepts `-A unsafe-code`).
fn names_lint(text: &str) -> bool {
    text.replace('-', "_").contains("unsafe_code")
}

/// The dotted paths of the keys and strings in `value` that name the lint.
fn lint_paths(value: &toml::Value, path: &str, found: &mut Vec<String>) {
    match value {
        toml::Value::String(text) if names_lint(text) => found.push(format!("{path} = {text:?}")),
        toml::Value::Table(table) => {
            for (key, value) in table {
                let path = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                if names_lint(key) {
                    found.push(path.clone());
                }
                lint_paths(value, &path, found);
            }
        }
        toml::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                lint_paths(item, &format!("{path}[{index}]"), found);
            }
        }
        _ => {}
    }
}

/// Every key or string that names `unsafe_code` in the manifests and Cargo configurations, but the allowed lint entries
/// (with their values), and every such file that does not parse.
pub fn other_manifests(tables: &[(String, String)]) -> Vec<String> {
    let mut found = Vec::new();
    for (file, text) in tables {
        let table: toml::Value = match text.parse::<toml::Table>() {
            Ok(table) => toml::Value::Table(table),
            Err(error) => {
                found.push(format!("{file}: does not parse: {error}"));
                continue;
            }
        };
        let mut paths = Vec::new();
        lint_paths(&table, "", &mut paths);
        for path in paths {
            let entry = path
                .split_once(" = ")
                .map_or(path.as_str(), |(entry, _)| entry);
            let allowed = lint_entry(file, entry);
            if !allowed {
                found.push(format!("{file}: {path}"));
            }
        }
    }
    found
}

/// The position of each `unsafe_code` identifier in `tokens`, groups included (a raw identifier too).
fn lint_idents(tokens: TokenStream, found: &mut Vec<LineColumn>) {
    for tree in tokens {
        match tree {
            TokenTree::Group(group) => lint_idents(group.stream(), found),
            TokenTree::Ident(ident)
                if ident.to_string().trim_start_matches("r#") == "unsafe_code" =>
            {
                found.push(ident.span().start());
            }
            _ => {}
        }
    }
}

/// The position of the `unsafe_code` identifier of the attribute `#[allow(unsafe_code)]` at the start of `tokens`.
fn allow_attribute(tokens: &[TokenTree]) -> Option<LineColumn> {
    let [TokenTree::Punct(hash), TokenTree::Group(attribute), ..] = tokens else {
        return None;
    };
    if hash.as_char() != '#' || attribute.delimiter() != Delimiter::Bracket {
        return None;
    }
    let attribute: Vec<TokenTree> = attribute.stream().into_iter().collect();
    let [TokenTree::Ident(allow), TokenTree::Group(list)] = attribute.as_slice() else {
        return None;
    };
    if allow != "allow" || list.delimiter() != Delimiter::Parenthesis {
        return None;
    }
    match list.stream().into_iter().collect::<Vec<_>>().as_slice() {
        [TokenTree::Ident(lint)] if lint == "unsafe_code" => Some(lint.span().start()),
        _ => None,
    }
}

/// The position of the one allowed identifier: in the first `#[allow(unsafe_code)]` at the top level of the file whose
/// item, after any other outer attributes (doc comments included), starts with the identifiers `item`.
fn the_exception(tokens: TokenStream, item_words: &[&str]) -> Option<LineColumn> {
    let tokens: Vec<TokenTree> = tokens.into_iter().collect();
    (0..tokens.len()).find_map(|index| {
        let position = allow_attribute(&tokens[index..])?;
        let mut item = &tokens[index + 2..];
        while let [TokenTree::Punct(hash), TokenTree::Group(attribute), rest @ ..] = item {
            if hash.as_char() != '#' || attribute.delimiter() != Delimiter::Bracket {
                break;
            }
            item = rest;
        }
        let starts = item_words.len() <= item.len()
            && item_words
                .iter()
                .zip(item)
                .all(|(word, tree)| matches!(tree, TokenTree::Ident(ident) if ident == word));
        starts.then_some(position)
    })
}

/// Every `unsafe_code` identifier in the Rust `sources`, other than the one of the exception, with its line; and every
/// source that does not lex.
pub fn other_attributes(sources: &[(String, String)]) -> Vec<String> {
    let mut found = Vec::new();
    for (file, text) in sources {
        let tokens: TokenStream = match text.parse() {
            Ok(tokens) => tokens,
            Err(error) => {
                found.push(format!("{file}: does not lex: {error}"));
                continue;
            }
        };
        let allowed = EXCEPTIONS
            .iter()
            .find(|e| e.file == file)
            .and_then(|e| the_exception(tokens.clone(), e.item));
        let mut idents = Vec::new();
        lint_idents(tokens, &mut idents);
        for position in idents.into_iter().filter(|&p| Some(p) != allowed) {
            let line = text.lines().nth(position.line - 1).unwrap_or_default();
            found.push(format!("{file}:{}: {}", position.line, line.trim()));
        }
    }
    found
}

/// Whether the check reads `file`: a manifest, a Cargo configuration or a Rust source.
pub fn checked(file: &str) -> bool {
    is_table(file) || file.ends_with(".rs")
}

/// A manifest or a Cargo configuration.
fn is_table(file: &str) -> bool {
    let name = file.rsplit('/').next().unwrap_or(file);
    let parent = file.strip_suffix(name).unwrap_or_default();
    name == "Cargo.toml"
        || ((name == "config.toml" || name == "config")
            && (parent == ".cargo/" || parent.ends_with("/.cargo/")))
}

/// The verdict of the check over the checked `files` (path and text): a summary line, or every problem with their count.
///
/// # Errors
/// The report of the problems that the check found.
pub fn verdict(
    workspace: &str,
    krates: &[(&str, String)],
    files: &[(String, String)],
) -> Result<String, String> {
    let (tables, sources): (Vec<_>, Vec<_>) =
        files.iter().cloned().partition(|(file, _)| is_table(file));
    let mut problems: Vec<String> = krates
        .iter()
        .filter_map(|(name, krate)| lint_drift(workspace, name, krate).err())
        .collect();
    problems.extend(other_manifests(&tables));
    problems.extend(other_attributes(&sources));
    if problems.is_empty() {
        Ok(format!(
            "unsafe-code: {} manifests and configurations and {} sources checked",
            tables.len(),
            sources.len()
        ))
    } else {
        Err(format!(
            "{}\n{} problem(s) with the unsafe_code exception",
            problems.join("\n"),
            problems.len()
        ))
    }
}

/// The check over the repository's tracked files.
///
/// # Errors
/// A file could not be read, or the check found a problem.
pub fn command(root: &Path, args: &[String]) -> Result<()> {
    if let Some(arg) = args.first() {
        bail!("unknown argument '{arg}'");
    }
    let read = |file: &str| {
        std::fs::read_to_string(root.join(file)).with_context(|| format!("read {file}"))
    };
    let mut files = Vec::new();
    for file in tracked_files(root)?
        .into_iter()
        .filter(|file| checked(file))
    {
        let text = read(&file)?;
        files.push((file, text));
    }
    let mut krates = Vec::new();
    for exception in EXCEPTIONS {
        krates.push((exception.manifest, read(exception.manifest)?));
    }
    let summary = verdict(&read("Cargo.toml")?, &krates, &files).map_err(anyhow::Error::msg)?;
    println!("{summary}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE: &str = include_str!("../../Cargo.toml");
    const KRATE: &str = include_str!("../../crates/botster-test-process/Cargo.toml");
    const TESTKIT: &str = include_str!("../../crates/botster-core-testkit/Cargo.toml");
    const ALLOC: &str = include_str!("../../crates/botster-core-testkit/src/alloc_controls.rs");
    const CRATE: &str = EXCEPTIONS[0].manifest;
    const ALLOWED_FILE: &str = EXCEPTIONS[0].file;
    const TESTKIT_CRATE: &str = EXCEPTIONS[1].manifest;
    const ALLOC_FILE: &str = EXCEPTIONS[1].file;

    fn krates() -> Vec<(&'static str, String)> {
        vec![
            (CRATE, KRATE.to_string()),
            (TESTKIT_CRATE, TESTKIT.to_string()),
        ]
    }
    const FFI: &str = include_str!("../../crates/botster-terminal-ghostty/Cargo.toml");
    const CONFIG: &str = include_str!("../../.cargo/config.toml");
    const ANCHOR: &str =
        include_str!("../../crates/botster-test-process/src/bin/botster-test-anchor.rs");

    #[test]
    fn each_exception_crate_differs_from_the_workspace_only_by_deny() {
        assert_eq!(lint_drift(WORKSPACE, CRATE, KRATE), Ok(()));
        assert_eq!(lint_drift(WORKSPACE, TESTKIT_CRATE, TESTKIT), Ok(()));
    }

    /// Red on revert: each other difference of the crate's table fails the check, and so does a workspace without `forbid`.
    #[test]
    fn any_other_lint_difference_fails() {
        for changed in [
            KRATE.replace("unsafe_code = \"deny\"", "unsafe_code = \"allow\""),
            KRATE.replace("unsafe_code = \"deny\"", "unsafe_code = \"forbid\""),
            KRATE.replace(
                "unsafe_code = \"deny\"",
                "unsafe_code = \"deny\"\nunused = \"allow\"",
            ),
            KRATE.replace(
                "[lints.rust]\nunsafe_code = \"deny\"",
                "[lints]\nworkspace = true",
            ),
            KRATE.replace("[lints.rust]\nunsafe_code = \"deny\"\n", ""),
        ] {
            assert_ne!(changed, KRATE, "the replacement applies");
            assert!(lint_drift(WORKSPACE, CRATE, &changed).is_err(), "{changed}");
        }
        let unforbidden = WORKSPACE.replace("unsafe_code = \"forbid\"", "unsafe_code = \"deny\"");
        assert_eq!(
            lint_drift(&unforbidden, CRATE, KRATE),
            Err("the workspace does not forbid unsafe_code".into())
        );
        assert!(lint_drift("[workspace]\n", CRATE, KRATE).is_err());
        // The testkit back on the workspace's table fails too.
        let workspace_only = TESTKIT.replace(
            "[lints.rust]\nunsafe_code = \"deny\"",
            "[lints]\nworkspace = true",
        );
        assert_ne!(workspace_only, TESTKIT, "the replacement applies");
        assert!(lint_drift(WORKSPACE, TESTKIT_CRATE, &workspace_only).is_err());
    }

    #[test]
    fn the_verdict_counts_the_files_or_reports_every_problem() {
        let files = vec![
            ("Cargo.toml".to_string(), WORKSPACE.to_string()),
            (CRATE.to_string(), KRATE.to_string()),
            (".cargo/config.toml".to_string(), CONFIG.to_string()),
            (TESTKIT_CRATE.to_string(), TESTKIT.to_string()),
            (ALLOWED_FILE.to_string(), ANCHOR.to_string()),
            (ALLOC_FILE.to_string(), ALLOC.to_string()),
        ];
        assert_eq!(
            verdict(WORKSPACE, &krates(), &files),
            Ok("unsafe-code: 4 manifests and configurations and 2 sources checked".into())
        );
        // A source that names the lint is still a source, and a manifest is never read as a source.
        let bad = vec![
            ("a.rs".to_string(), "#![allow(unsafe_code)]\n".to_string()),
            ("x/Cargo.toml".to_string(), "[lints\n".to_string()),
        ];
        let report = verdict(WORKSPACE, &krates(), &bad).unwrap_err();
        assert!(
            report.starts_with("x/Cargo.toml: does not parse: "),
            "{report}"
        );
        assert!(
            report.ends_with(
                "\na.rs:1: #![allow(unsafe_code)]\n2 problem(s) with the unsafe_code exception"
            ),
            "{report}"
        );
        // Each exception crate's table is checked: the anchor's crate, and the testkit.
        for name in [CRATE, TESTKIT_CRATE] {
            let krates: Vec<(&str, String)> = krates()
                .into_iter()
                .map(|(n, text)| {
                    if n == name {
                        (n, "[lints]\nworkspace = true\n".to_string())
                    } else {
                        (n, text)
                    }
                })
                .collect();
            let drift = verdict(WORKSPACE, &krates, &[]).unwrap_err();
            assert!(drift.contains(name), "{drift}");
            assert!(
                drift.ends_with("\n1 problem(s) with the unsafe_code exception"),
                "{drift}"
            );
        }
    }

    #[test]
    fn the_check_reads_manifests_configurations_and_rust_sources_only() {
        for file in [
            "Cargo.toml",
            "crates/x/Cargo.toml",
            ".cargo/config.toml",
            ".cargo/config",
            "crates/x/.cargo/config.toml",
            "crates/x/.cargo/config",
            "xtask/src/main.rs",
        ] {
            assert!(checked(file), "{file}");
        }
        for file in [
            "crates/x/Cargo.lock",
            "docs/notCargo.toml",
            "a.rs.txt",
            ".cargo/mutants.toml",
            "config.toml",
            "x.cargo/config.toml",
            "crates/x/config",
        ] {
            assert!(!checked(file), "{file}");
        }
    }

    fn tables(file: &str, text: &str) -> Vec<String> {
        other_manifests(&[(file.to_string(), text.to_string())])
    }

    /// Red on revert (#171 round 2, TP7): the manifests are parsed, so only the four lint entries name the lint, whatever
    /// their spelling; an escaped key, a dashed name or a flag elsewhere is found, in any manifest or configuration.
    #[test]
    fn only_the_four_lint_entries_name_unsafe_code() {
        for (file, text) in [
            ("Cargo.toml", WORKSPACE),
            (CRATE, KRATE),
            (TESTKIT_CRATE, TESTKIT),
            (FFI_CRATE, FFI),
            (".cargo/config.toml", CONFIG),
        ] {
            assert_eq!(tables(file, text), Vec::<String>::new(), "{file}");
        }
        assert!(tables("crates/x/Cargo.toml", "[lints]\nworkspace = true\n").is_empty());
        let escaped = "[lints.rust]\n\"unsafe_\\u0063ode\" = \"allow\"\n";
        assert_eq!(
            tables("crates/x/Cargo.toml", escaped),
            ["crates/x/Cargo.toml: lints.rust.unsafe_code"]
        );
        assert_eq!(
            tables(
                "crates/x/Cargo.toml",
                "[lints.rust]\nunsafe-code = \"allow\"\n"
            ),
            ["crates/x/Cargo.toml: lints.rust.unsafe-code"]
        );
        // An allowed manifest names the lint only in its entry.
        assert_eq!(
            tables(
                CRATE,
                &format!("{KRATE}\n[package.metadata]\nx = \"-Aunsafe-code\"\n")
            ),
            [format!("{CRATE}: package.metadata.x = \"-Aunsafe-code\"")]
        );
        assert_eq!(
            tables(
                FFI_CRATE,
                "[workspace.lints.rust]\nunsafe_code = \"allow\"\n"
            ),
            [format!("{FFI_CRATE}: workspace.lints.rust.unsafe_code")]
        );
        let flags = "[build]\nrustflags = [\"-A\", \"unsafe_code\"]\n[env]\nRUSTFLAGS = \"--allow=unsafe-code\"\n";
        assert_eq!(
            tables(".cargo/config.toml", flags),
            [
                ".cargo/config.toml: build.rustflags[1] = \"unsafe_code\"",
                ".cargo/config.toml: env.RUSTFLAGS = \"--allow=unsafe-code\"",
            ]
        );
    }

    fn attributes(file: &str, text: &str) -> Vec<String> {
        other_attributes(&[(file.to_string(), text.to_string())])
    }

    /// Red on revert (#171 round 2, TP7): sources are read as tokens. The anchor binary has exactly the one allowed
    /// identifier; any other `unsafe_code` identifier fails the check, whatever its attribute's layout.
    #[test]
    fn only_close_inherited_allows_unsafe_code() {
        assert_eq!(attributes(ALLOWED_FILE, ANCHOR), Vec::<String>::new());
        let elsewhere = "#[allow(unsafe_code)]\nfn close_inherited() {}\n";
        assert_eq!(
            attributes("crates/x/src/lib.rs", elsewhere),
            ["crates/x/src/lib.rs:1: #[allow(unsafe_code)]"]
        );
        let between =
            "#[allow(unsafe_code)]\n// note\n/// doc\n#[inline]\nfn close_inherited() {}\n";
        assert!(attributes(ALLOWED_FILE, between).is_empty());
        for (text, line) in [
            (
                "/// doc\n#[allow(unsafe_code)]\n#[inline]\nfn other() {}\n",
                2,
            ),
            (
                "#[allow(\n    unsafe_code\n)]\nfn another() { unsafe { libc::close(3); } }\n",
                2,
            ),
            (
                "#[allow(\n    clippy::x,\n    unsafe_code\n)]\nfn close_inherited() {}\n",
                3,
            ),
            (
                "#[expect(\n    unsafe_code,\n)]\nfn close_inherited() {}\n",
                2,
            ),
            (
                "#[cfg_attr(all(), allow(unsafe_code))]\nfn close_inherited() {}\n",
                1,
            ),
            ("#![allow(unsafe_code)]\n", 1),
            (
                "mod inner {\n    #[allow(unsafe_code)]\n    fn close_inherited() {}\n}\n",
                2,
            ),
            (
                "fn f() {\n    #[allow(unsafe_code)]\n    fn close_inherited() {}\n}\n",
                2,
            ),
            ("#[allow(unsafe_code)]\npub fn close_inherited() {}\n", 1),
            (
                "macro_rules! m { () => { #[allow(unsafe_code)] fn g() {} } }\n",
                1,
            ),
            ("#[allow(r#unsafe_code)]\nfn close_inherited() {}\n", 1),
            // Tokens that are not the attribute (in a macro body, for example), or another lint level.
            ("#(allow(unsafe_code))\nfn close_inherited() {}\n", 1),
            ("![allow(unsafe_code)]\nfn close_inherited() {}\n", 1),
            ("#[deny(unsafe_code)]\nfn close_inherited() {}\n", 1),
            ("#[allow[unsafe_code]]\nfn close_inherited() {}\n", 1),
            (
                "#[allow(unsafe_code)]\n#(inline)\nfn close_inherited() {}\n",
                1,
            ),
            (
                "#[allow(unsafe_code)]\n$[inline]\nfn close_inherited() {}\n",
                1,
            ),
        ] {
            let found = attributes(ALLOWED_FILE, text);
            assert_eq!(found.len(), 1, "{text}: {found:?}");
            assert!(
                found[0].starts_with(&format!("{ALLOWED_FILE}:{line}: ")),
                "{text}: {found:?}"
            );
        }
        let twice = format!("{ANCHOR}\n#[allow(unsafe_code)]\nfn close_inherited() {{}}\n");
        assert_eq!(attributes(ALLOWED_FILE, &twice).len(), 1);
        // Comments and string literals are not identifiers.
        let text =
            "// #[allow(unsafe_code)]\n/* unsafe_code */\nconst A: &str = \"unsafe_code\";\n";
        assert!(attributes("crates/x/src/lib.rs", text).is_empty());
        let unlexed = attributes("crates/x/src/lib.rs", "fn f() {\n");
        assert_eq!(unlexed.len(), 1);
        assert!(
            unlexed[0].starts_with("crates/x/src/lib.rs: does not lex: "),
            "{unlexed:?}"
        );
    }

    /// Red on revert (lead ruling 2026-10-10): the testkit's one exception is the allocator's `impl` in its own file. The
    /// file has exactly that identifier; a second unsafe item in the crate fails, and so does the same item in another file.
    #[test]
    fn only_the_peak_allocator_impl_allows_unsafe_code_in_the_testkit() {
        assert_eq!(attributes(ALLOC_FILE, ALLOC), Vec::<String>::new());
        let second = format!("{ALLOC}\n#[allow(unsafe_code)]\nfn read_raw() {{}}\n");
        let found = attributes(ALLOC_FILE, &second);
        assert_eq!(found.len(), 1, "{found:?}");
        let other_file = "crates/botster-core-testkit/src/harness.rs";
        let moved = "#[allow(unsafe_code)]\nunsafe impl GlobalAlloc for PeakAllocator {}\n";
        assert_eq!(
            attributes(other_file, moved),
            [format!("{other_file}:1: #[allow(unsafe_code)]")]
        );
        assert!(attributes(ALLOC_FILE, moved).is_empty());
        // The whole item: another trait or another type is not the exception.
        for text in [
            "#[allow(unsafe_code)]\nunsafe impl GlobalAlloc for Other {}\n",
            "#[allow(unsafe_code)]\nunsafe impl Send for PeakAllocator {}\n",
            "#[allow(unsafe_code)]\nunsafe impl GlobalAlloc {}\n",
            "#[allow(unsafe_code)]\nfn close_inherited() {}\n",
        ] {
            assert_eq!(attributes(ALLOC_FILE, text).len(), 1, "{text}");
        }
        // The anchor's item is not allowed in the allocator's file, nor the reverse.
        assert_eq!(
            attributes(ALLOWED_FILE, moved).len(),
            1,
            "the allocator's item in the anchor binary"
        );
    }
}
