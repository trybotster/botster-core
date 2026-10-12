//! Each const of the crate against the frozen CLI spec, `cli-spec-v1-candidate34.md`.
//!
//! - Closed lists (the section 2 table, the CLI-CX3 alias table, the CLI-CF1 keys, the dropped fields of CLI-SS1): the const equals
//!   the parse of the text.
//! - Open lists (the `status` fields, MESSAGES, PATHS, the flags that only a clause names): each entry is a value of the cited
//!   text, so the crate invents no name.
//!
//! Each check has a negative case: the same check on a changed value or a changed text fails.

use botster_cli_contract::*;
use std::collections::BTreeSet;

const SPEC: &str = include_str!("../../../frozen/current/cli-spec-v1-candidate34.md");

fn set<'a>(items: impl IntoIterator<Item = &'a str>) -> BTreeSet<String> {
    items.into_iter().map(str::to_string).collect()
}

/// The code spans of `text`, in order.
fn code_spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// The code spans of `text` that are lowercase identifiers.
fn identifiers(text: &str) -> BTreeSet<String> {
    set(code_spans(text)
        .into_iter()
        .filter(|s| !s.is_empty() && s.chars().all(|c| c.is_ascii_lowercase() || c == '_')))
}

/// The row of `clause` in `spec`.
fn row<'a>(spec: &'a str, clause: &str) -> &'a str {
    let head = format!("| {clause} |");
    spec.lines()
        .find(|l| l.starts_with(&head))
        .unwrap_or_else(|| panic!("the {clause} row"))
}

/// The part of `text` from `start` up to `end`.
fn between<'a>(text: &'a str, start: &str, end: &str) -> &'a str {
    let rest = &text[text.find(start).unwrap_or_else(|| panic!("{start:?}")) + start.len()..];
    &rest[..rest.find(end).unwrap_or_else(|| panic!("{end:?}"))]
}

/// The rows of the markdown table that follows the line that starts with `heading`.
fn table_rows<'a>(spec: &'a str, heading: &str) -> Vec<&'a str> {
    spec.lines()
        .skip_while(|l| !l.starts_with(heading))
        .skip(1)
        .skip_while(|l| !l.starts_with('|'))
        .take_while(|l| l.starts_with('|'))
        .skip(2)
        .collect()
}

/// The cells of a table row. A `\|` inside a cell is not a cell border.
fn cells(row: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = row.trim().trim_start_matches('|').chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&'|') => {
                out.last_mut().unwrap().push('|');
                chars.next();
            }
            '|' => out.push(String::new()),
            _ => out.last_mut().unwrap().push(c),
        }
    }
    out.pop();
    out.iter().map(|c| c.trim().to_string()).collect()
}

fn is_word(token: &str) -> bool {
    token.starts_with(|c: char| c.is_ascii_lowercase())
        && token
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
}

/// The flag names in a usage token: `[--swap`, `-y|--yes]`, `<name>...|--all` and `--bind` give `--swap`, `-y` and `--yes`,
/// `--all`, `--bind`.
fn flags_in(token: &str) -> Vec<String> {
    token
        .split(['[', ']', '|'])
        .filter(|p| {
            p.starts_with('-')
                && p.trim_start_matches('-')
                    .starts_with(|c: char| c.is_ascii_alphabetic())
        })
        .map(|p| {
            let end = p
                .char_indices()
                .find(|&(i, c)| i > 0 && !(c.is_ascii_alphanumeric() || c == '-'))
                .map_or(p.len(), |(i, _)| i);
            p[..end].to_string()
        })
        .collect()
}

/// The parse of the section 2 table of `spec`: (v1 commands, gone commands, (command, flag) pairs).
///
/// A code span of the command cell is a usage: the leading lowercase words are the command path, and an alternative
/// `add|remove|...` of the last word gives one path for each. A span of a row whose first path has two words, and which does not
/// start with that row's first word, is relative to it (`update <name>` in the `plugin install` row is `plugin update`). The
/// groups (`plugin catalog`) are commands too.
type Section2 = (
    BTreeSet<String>,
    BTreeSet<String>,
    BTreeSet<(String, String)>,
);

fn section_2(spec: &str) -> Section2 {
    let (mut live, mut gone, mut flags) = (BTreeSet::new(), BTreeSet::new(), BTreeSet::new());
    for line in table_rows(spec, "## 2. Commands at a glance") {
        let cells = cells(line);
        let is_gone = cells[2].starts_with("GONE");
        let mut group: Option<String> = None;
        for usage in code_spans(&cells[0]) {
            let tokens: Vec<&str> = usage
                .trim_start_matches("botster ")
                .split_whitespace()
                .collect();
            let mut path: Vec<String> = vec![];
            let mut alternatives: Vec<String> = vec![];
            for token in &tokens {
                if !alternatives.is_empty() || !token.split('|').all(is_word) {
                    break;
                }
                if token.contains('|') {
                    alternatives = token.split('|').map(str::to_string).collect();
                } else {
                    path.push(token.to_string());
                }
            }
            if let (Some(g), Some(first)) = (&group, path.first()) {
                if first != g {
                    path.insert(0, g.clone());
                }
            } else if path.len() > 1 {
                group = Some(path[0].clone());
            }
            let paths: Vec<String> = if alternatives.is_empty() {
                vec![path.join(" ")]
            } else {
                alternatives
                    .iter()
                    .map(|a| [path.clone(), vec![a.clone()]].concat().join(" "))
                    .collect()
            };
            for p in paths {
                // A usage with no command path (`botster --version`, `-V`) gives the global flags, owned by the binary.
                let owner = if p.is_empty() {
                    "botster".to_string()
                } else {
                    p.clone()
                };
                for f in tokens.iter().flat_map(|t| flags_in(t)) {
                    flags.insert((owner.clone(), f));
                }
                if p.is_empty() {
                    continue;
                }
                let words: Vec<&str> = p.split(' ').collect();
                for n in 1..=words.len() {
                    let prefix = words[..n].join(" ");
                    if is_gone {
                        gone.insert(prefix)
                    } else {
                        live.insert(prefix)
                    };
                }
            }
        }
    }
    (live, gone, flags)
}

/// The binary name that CLI-G1 of `spec` gives.
fn binary(spec: &str) -> &str {
    between(row(spec, "CLI-G1"), "The binary is `", "`")
}

#[test]
fn binary_is_the_cli_g1_name() {
    assert_eq!(binary(SPEC), BINARY);
    // A negative case: another name in CLI-G1.
    let spec = SPEC.replacen("The binary is `botster`", "The binary is `botster-cli`", 1);
    assert_ne!(binary(&spec), BINARY);
}

#[test]
fn commands_and_gone_commands_equal_the_section_2_table() {
    let (live, gone, _) = section_2(SPEC);
    assert_eq!(live, set(COMMANDS.iter().copied()));
    let all_gone = set(GONE_COMMANDS.iter().map(|g| g.command));
    assert!(gone.is_subset(&all_gone), "{gone:?}");
    assert!(live.is_disjoint(&all_gone));
    // A negative case: a table row with one more command changes the parse.
    let spec = SPEC.replacen("| `config` |", "| `config`, `config show` |", 1);
    assert_ne!(section_2(&spec).0, set(COMMANDS.iter().copied()));
}

/// The table row of `spec` whose first cell starts with `clause` (`CL-118..120 \`reset\``).
fn row_starting<'a>(spec: &'a str, clause: &str) -> Option<&'a str> {
    let head = format!("| {clause} ");
    spec.lines().find(|l| l.starts_with(&head))
}

/// Whether the row of `clause` declares `command` gone: it names the command (alone or after `botster`) and says "gone".
fn declares_gone(spec: &str, clause: &str, command: &str) -> bool {
    row_starting(spec, clause).is_some_and(|r| {
        let named = code_spans(r)
            .iter()
            .any(|s| *s == command || s.strip_prefix("botster ") == Some(command));
        named && r.to_lowercase().contains("gone")
    })
}

#[test]
fn each_gone_command_is_declared_gone_by_its_rows() {
    for g in GONE_COMMANDS {
        assert!(!g.clauses.is_empty(), "{g:?}");
        for c in g.clauses {
            assert!(declares_gone(SPEC, c, g.command), "{g:?} {c}");
        }
    }
    // Negative cases: D-10 names `botster session` but does not say "gone"; CL-131 does not name `reset`; no row starts `CL-999`.
    assert!(!declares_gone(SPEC, "D-10", "session"));
    assert!(!declares_gone(SPEC, "CL-131", "reset"));
    assert!(!declares_gone(SPEC, "CL-999", "reset"));
}

#[test]
fn a_dropped_flag_is_named_by_its_clause_and_is_not_live() {
    assert!(!DROPPED_FLAGS.is_empty());
    for f in DROPPED_FLAGS {
        assert!(COMMANDS.contains(&f.command), "{f:?}");
        assert!(names_flag(row(SPEC, f.source), f.flag), "{f:?}");
        assert!(
            !FLAGS
                .iter()
                .any(|l| l.command == f.command && l.flag == f.flag),
            "{f:?} is also live"
        );
    }
    // A negative case: D-18 names `--force`, not `--forced`.
    assert!(!names_flag(row(SPEC, "D-18"), "--forced"));
}

/// The confirmation answers that CLI-PL8 of `spec` accepts.
fn confirm_answers(spec: &str) -> BTreeSet<String> {
    identifiers(between(row(spec, "CLI-PL8"), "on anything but", "prints"))
}

#[test]
fn confirm_answers_equal_cli_pl8() {
    assert_eq!(confirm_answers(SPEC), set(CONFIRM_ANSWERS.iter().copied()));
    // A negative case: one more answer.
    let spec = SPEC.replacen(
        "but `y` or `yes` prints",
        "but `y`, `ok` or `yes` prints",
        1,
    );
    assert_ne!(confirm_answers(&spec), set(CONFIRM_ANSWERS.iter().copied()));
}

/// The MCP tool that CLI-MS4 of `spec` names.
fn mcp_tools(spec: &str) -> BTreeSet<String> {
    set([between(row(spec, "CLI-MS4"), "offers one tool, `", "`")])
}

#[test]
fn mcp_tools_equal_cli_ms4() {
    assert_eq!(mcp_tools(SPEC), set(MCP_TOOLS.iter().copied()));
    // A negative case: another tool name.
    let spec = SPEC.replacen("one tool, `botster_status`", "one tool, `botster_info`", 1);
    assert_ne!(mcp_tools(&spec), set(MCP_TOOLS.iter().copied()));
}

/// The log labels that CLI-G3 of `spec` lists.
fn log_labels(spec: &str) -> BTreeSet<String> {
    identifiers(between(row(spec, "CLI-G3"), "and `<label>` is", "."))
}

#[test]
fn log_labels_equal_cli_g3() {
    assert_eq!(log_labels(SPEC), set(LOG_LABELS.iter().copied()));
    // A negative case: one more label.
    let spec = SPEC.replacen("`attach` or `cli`.", "`attach`, `mcp` or `cli`.", 1);
    assert_ne!(log_labels(&spec), set(LOG_LABELS.iter().copied()));
}

/// The keys of `dev-checkouts.toml` that CLI-DV4 of `spec` lists.
fn dev_checkout_keys(spec: &str) -> BTreeSet<String> {
    identifiers(between(
        row(spec, "CLI-DV4"),
        "to an **absolute path**:",
        ". Only a development build",
    ))
}

#[test]
fn dev_checkout_keys_equal_cli_dv4() {
    assert_eq!(
        dev_checkout_keys(SPEC),
        set(DEV_CHECKOUT_KEYS.iter().copied())
    );
    // A negative case: one more key.
    let spec = SPEC.replacen(
        "`plugin_hosts` and `mcp_bridge`.",
        "`plugin_hosts`, `ui` and `mcp_bridge`.",
        1,
    );
    assert_ne!(
        dev_checkout_keys(&spec),
        set(DEV_CHECKOUT_KEYS.iter().copied())
    );
}

#[test]
fn section_2_flags_equal_the_table_flags() {
    let (_, _, parsed) = section_2(SPEC);
    let listed: BTreeSet<(String, String)> = FLAGS
        .iter()
        .filter(|f| f.source == "2")
        .map(|f| (f.command.to_string(), f.flag.to_string()))
        .collect();
    assert_eq!(parsed, listed);
    // A negative case: one more flag in the table.
    let spec = SPEC.replacen("`status` |", "`status [--verbose]` |", 1);
    assert_ne!(section_2(&spec).2, listed);
}

/// Whether `flag` is a whole token of a code span of `text`.
fn names_flag(text: &str, flag: &str) -> bool {
    code_spans(text)
        .iter()
        .flat_map(|s| s.split_whitespace())
        .any(|t| flags_in(t).iter().any(|f| f == flag))
}

#[test]
fn a_clause_flag_is_named_by_its_clause() {
    let others: Vec<&Flag> = FLAGS.iter().filter(|f| f.source != "2").collect();
    assert!(!others.is_empty());
    for f in others {
        assert!(COMMANDS.contains(&f.command), "{f:?}");
        assert!(names_flag(row(SPEC, f.source), f.flag), "{f:?}");
    }
    // A negative case: `-y` is a part of `-y|--yes`, but `--ye` is no whole token of it.
    assert!(!names_flag(row(SPEC, "CLI-PL8"), "--ye"));
}

#[test]
fn flags_are_unique_and_name_a_command() {
    let mut seen = BTreeSet::new();
    for f in FLAGS {
        assert!(
            f.command == BINARY || COMMANDS.contains(&f.command),
            "{f:?}"
        );
        assert!(seen.insert((f.command, f.flag)), "{f:?} twice");
    }
}

/// The CLI-CX3 alias table of `spec`: (live keys, dropped keys).
fn alias_table(spec: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let (mut live, mut dropped) = (BTreeSet::new(), BTreeSet::new());
    for line in table_rows(spec, "**Alias table (CLI-CX3).**") {
        let cells = cells(line);
        let key = code_spans(&cells[0])[0].to_string();
        if cells[1] == "(dropped)" {
            dropped.insert(key)
        } else {
            live.insert(key)
        };
    }
    (live, dropped)
}

#[test]
fn context_keys_equal_the_alias_table() {
    let (live, dropped) = alias_table(SPEC);
    assert_eq!(live, set(CONTEXT_KEYS.iter().copied()));
    assert_eq!(dropped, set(DROPPED_CONTEXT_KEYS.iter().copied()));
    assert!(live.is_disjoint(&dropped));
    // A negative case: a key that the table drops is not live.
    let spec = SPEC.replacen("| `repo` | `repo` |", "| `repo` | (dropped) |", 1);
    assert_ne!(alias_table(&spec).0, set(CONTEXT_KEYS.iter().copied()));
}

/// The CLI-CF1 keys of `spec`: (printed, not printed).
fn config_keys(spec: &str) -> (BTreeSet<String>, BTreeSet<String>) {
    let cf1 = row(spec, "CLI-CF1");
    (
        identifiers(between(cf1, "pretty JSON:", "The legacy keys")),
        identifiers(between(cf1, "The legacy keys", "are not printed")),
    )
}

#[test]
fn config_keys_equal_cli_cf1() {
    let (printed, legacy) = config_keys(SPEC);
    assert_eq!(printed, set(CONFIG_KEYS.iter().copied()));
    assert_eq!(legacy, set(DROPPED_CONFIG_KEYS.iter().copied()));
    assert!(printed.is_disjoint(&legacy));
    // A negative case: one more printed key.
    let spec = SPEC.replacen("and `log_dir`", "`log_dir` and `log_level`", 1);
    assert_ne!(config_keys(&spec).0, set(CONFIG_KEYS.iter().copied()));
}

/// The dropped fields of CLI-SS1 in `spec`.
fn dropped_status_fields(spec: &str) -> BTreeSet<String> {
    identifiers(between(
        row(spec, "CLI-SS1"),
        "**Dropped from the legacy block (ruling UD-5, D-17):**",
        "are Hub internals",
    ))
}

#[test]
fn status_fields_are_named_by_cli_ss1() {
    // CLI-SS1 gives no closed field list (its code spans also hold values such as `yes` and Hub fields such as `uptime_seconds`),
    // so each live field must be a code span of the block text, and the dropped fields equal the dropped list.
    let block = identifiers(between(row(SPEC, "CLI-SS1"), "Always:", "**Dropped"));
    let live = set(STATUS_FIELDS.iter().copied());
    assert!(live.is_subset(&block), "{:?}", live.difference(&block));
    let dropped = dropped_status_fields(SPEC);
    assert_eq!(dropped, set(DROPPED_STATUS_FIELDS.iter().copied()));
    assert!(live.is_disjoint(&dropped));
    // A negative case: a field name that the block does not name.
    assert!(!block.contains("uptime_ms"));
    let spec = SPEC.replacen("and `lock_file` are", "are", 1);
    assert_ne!(dropped_status_fields(&spec), dropped);
}

/// The problems of `texts` against `spec`: each text must be code span number `ordinal` of its clause row.
fn text_problems(spec: &str, texts: &[SpecText]) -> Vec<String> {
    let mut out = vec![];
    let mut seen = BTreeSet::new();
    for t in texts {
        let spans = code_spans(row(spec, t.clause));
        if spans.get(t.ordinal.wrapping_sub(1)) != Some(&t.text) {
            out.push(format!(
                "{t:?}: the row has {:?} there",
                spans.get(t.ordinal.wrapping_sub(1))
            ));
        }
        if !seen.insert(t.text) {
            out.push(format!("{t:?}: the text is listed twice"));
        }
    }
    out
}

#[test]
fn messages_and_paths_are_code_spans_of_their_rows() {
    assert_eq!(text_problems(SPEC, MESSAGES), Vec::<String>::new());
    assert_eq!(text_problems(SPEC, PATHS), Vec::<String>::new());
    let messages = set(MESSAGES.iter().map(|t| t.text));
    assert!(messages.is_disjoint(&set(PATHS.iter().map(|t| t.text))));
    // Negative cases: a wrong ordinal, a changed text and a repeated text each fail.
    let m = MESSAGES[0];
    let wrong = [
        SpecText {
            ordinal: m.ordinal + 1,
            ..m
        },
        SpecText {
            text: "botster <version>",
            ..m
        },
        SpecText { ordinal: 0, ..m },
    ];
    for w in wrong {
        assert_eq!(text_problems(SPEC, &[w]).len(), 1, "{w:?}");
    }
    assert_eq!(text_problems(SPEC, &[m, m]).len(), 1);
}
