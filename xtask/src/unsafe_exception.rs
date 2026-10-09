//! The one exception to the workspace's `unsafe_code = "forbid"` (lead ruling 2026-10-09 on #171 TP3, (A)).
//!
//! botster-test-process copies the workspace's lint table with exactly one difference, `unsafe_code = "deny"`, so that one
//! function of its anchor binary, `close_inherited`, can allow the lint. The check fails:
//! - on any other difference between that crate's lint table and the workspace's;
//! - on any key or string that names the lint in a manifest or a Cargo configuration (`rustflags` included), but the lint
//!   entries of the workspace, of that crate and of the libghostty-vt binding (the earlier exception: that crate calls a C
//!   ABI, and its manifest allows `unsafe_code` for the whole crate). TOML is parsed, so an escaped key is read as Cargo reads
//!   it; `-` is read as `_`, as rustc reads a lint name;
//! - on any `unsafe_code` identifier in a Rust source, other than the one in the `#[allow(unsafe_code)]` of the top-level
//!   `fn close_inherited` of the anchor binary. Sources are read as Rust tokens: comments and string literals are not
//!   identifiers, and an attribute on several lines, a `cfg_attr`, an `expect` or a macro body is read as a whole (#171
//!   round 2, TP7).

use crate::fsutil::tracked_files;
use anyhow::{bail, Context, Result};
use proc_macro2::{Delimiter, LineColumn, TokenStream, TokenTree};
use std::path::Path;

/// The crate with the exception.
const CRATE: &str = "crates/botster-test-process/Cargo.toml";
/// The C ABI binding of libghostty-vt, which allows `unsafe_code` in its own lint table (eddd5e75; not this ruling).
const FFI_CRATE: &str = "crates/botster-terminal-ghostty/Cargo.toml";
/// The lint entries that may name `unsafe_code`: each manifest and the path of its entry.
const LINT_ENTRIES: [(&str, &str); 3] = [
    ("Cargo.toml", "workspace.lints.rust.unsafe_code"),
    (CRATE, "lints.rust.unsafe_code"),
    (FFI_CRATE, "lints.rust.unsafe_code"),
];
/// The file and the function that may allow `unsafe_code`.
const ALLOWED_FILE: &str = "crates/botster-test-process/src/bin/botster-test-anchor.rs";
const ALLOWED_FN: &str = "close_inherited";

/// Whether the crate's lint table is the workspace's with `unsafe_code = "deny"` in place of `"forbid"`, and nothing else.
///
/// # Errors
/// A manifest does not parse, the workspace does not forbid `unsafe_code`, or the tables differ otherwise.
pub fn lint_drift(workspace: &str, krate: &str) -> Result<(), String> {
    let workspace: toml::Table = workspace
        .parse()
        .map_err(|e| format!("the workspace manifest: {e}"))?;
    let krate: toml::Table = krate.parse().map_err(|e| format!("{CRATE}: {e}"))?;
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
        .ok_or(format!("{CRATE} has no lint table"))?;
    if actual != &expected {
        return Err(format!(
            "the lint table of {CRATE} must be the workspace's with only unsafe_code = \"deny\": it is {actual}, expected {expected}"
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
            let allowed = LINT_ENTRIES.iter().any(|&(manifest, entry)| {
                file == manifest
                    && (path == entry
                        || path
                            .strip_prefix(entry)
                            .is_some_and(|v| v.starts_with(" = ")))
            });
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
/// item, after any other outer attributes (doc comments included), is `fn close_inherited`.
fn the_exception(tokens: TokenStream) -> Option<LineColumn> {
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
        matches!(item, [TokenTree::Ident(keyword), TokenTree::Ident(name), ..]
            if keyword == "fn" && name == ALLOWED_FN)
        .then_some(position)
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
        let allowed = if file == ALLOWED_FILE {
            the_exception(tokens.clone())
        } else {
            None
        };
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
pub fn verdict(workspace: &str, krate: &str, files: &[(String, String)]) -> Result<String, String> {
    let (tables, sources): (Vec<_>, Vec<_>) =
        files.iter().cloned().partition(|(file, _)| is_table(file));
    let mut problems: Vec<String> = lint_drift(workspace, krate).err().into_iter().collect();
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
    let summary =
        verdict(&read("Cargo.toml")?, &read(CRATE)?, &files).map_err(anyhow::Error::msg)?;
    println!("{summary}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const WORKSPACE: &str = include_str!("../../Cargo.toml");
    const KRATE: &str = include_str!("../../crates/botster-test-process/Cargo.toml");
    const FFI: &str = include_str!("../../crates/botster-terminal-ghostty/Cargo.toml");
    const CONFIG: &str = include_str!("../../.cargo/config.toml");
    const ANCHOR: &str =
        include_str!("../../crates/botster-test-process/src/bin/botster-test-anchor.rs");

    #[test]
    fn the_crate_differs_from_the_workspace_only_by_deny() {
        assert_eq!(lint_drift(WORKSPACE, KRATE), Ok(()));
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
            assert!(lint_drift(WORKSPACE, &changed).is_err(), "{changed}");
        }
        let unforbidden = WORKSPACE.replace("unsafe_code = \"forbid\"", "unsafe_code = \"deny\"");
        assert_eq!(
            lint_drift(&unforbidden, KRATE),
            Err("the workspace does not forbid unsafe_code".into())
        );
        assert!(lint_drift("[workspace]\n", KRATE).is_err());
    }

    #[test]
    fn the_verdict_counts_the_files_or_reports_every_problem() {
        let files = vec![
            ("Cargo.toml".to_string(), WORKSPACE.to_string()),
            (CRATE.to_string(), KRATE.to_string()),
            (".cargo/config.toml".to_string(), CONFIG.to_string()),
            (ALLOWED_FILE.to_string(), ANCHOR.to_string()),
        ];
        assert_eq!(
            verdict(WORKSPACE, KRATE, &files),
            Ok("unsafe-code: 3 manifests and configurations and 1 sources checked".into())
        );
        // A source that names the lint is still a source, and a manifest is never read as a source.
        let bad = vec![
            ("a.rs".to_string(), "#![allow(unsafe_code)]\n".to_string()),
            ("x/Cargo.toml".to_string(), "[lints\n".to_string()),
        ];
        let report = verdict(WORKSPACE, KRATE, &bad).unwrap_err();
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
        let drift = verdict(WORKSPACE, "[lints]\nworkspace = true\n", &[]).unwrap_err();
        assert!(
            drift.ends_with("\n1 problem(s) with the unsafe_code exception"),
            "{drift}"
        );
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

    /// Red on revert (#171 round 2, TP7): the manifests are parsed, so only the three lint entries name the lint, whatever
    /// their spelling; an escaped key, a dashed name or a flag elsewhere is found, in any manifest or configuration.
    #[test]
    fn only_the_three_lint_entries_name_unsafe_code() {
        for (file, text) in [
            ("Cargo.toml", WORKSPACE),
            (CRATE, KRATE),
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
}
