//! The whole-word match of the old-world names over the files of a repository (moved from Core's xtask `taint`; BUILD.md
//! "Starting fresh" rule 5). A repository passes its denylist tokens and its files. `docs/` is not scanned: it names the old
//! mechanisms only to forbid them.

const SKIPPED_PREFIXES: [&str; 1] = ["docs/"];

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// The tokens of a denylist text: `#` comments and blank lines skipped.
pub fn parse_denylist(text: &str) -> Vec<String> {
    text.lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string)
        .collect()
}

/// The tokens that occur in `line` as whole words.
pub fn hits_in_line<'a>(line: &str, tokens: &'a [String]) -> Vec<&'a str> {
    let mut found = Vec::new();
    for token in tokens {
        let mut from = 0;
        while let Some(at) = line[from..].find(token.as_str()) {
            let start = from + at;
            let end = start + token.len();
            let before_ok = !line[..start].chars().next_back().is_some_and(is_word);
            let after_ok = !line[end..].chars().next().is_some_and(is_word);
            if before_ok && after_ok {
                found.push(token.as_str());
                break;
            }
            from = start + line[start..].chars().next().map_or(1, char::len_utf8);
        }
    }
    found
}

/// A banned name in a file: `(file, 1-based line, token)`.
pub type Hit = (String, usize, String);

/// The hits of whole-word tokens over files given as `(path, text)`. A path under a skipped prefix is not scanned.
pub fn scan_files(files: &[(String, String)], tokens: &[String]) -> (usize, Vec<Hit>) {
    let mut scanned = 0;
    let mut hits = Vec::new();
    for (file, text) in files {
        if SKIPPED_PREFIXES.iter().any(|p| file.starts_with(p)) {
            continue;
        }
        scanned += 1;
        for (index, line) in text.lines().enumerate() {
            for token in hits_in_line(line, tokens) {
                hits.push((file.clone(), index + 1, token.to_string()));
            }
        }
    }
    (scanned, hits)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens() -> Vec<String> {
        parse_denylist("# comment\n\nold_name\nthing\n")
    }

    #[test]
    fn matches_whole_words_only() {
        let t = tokens();
        assert_eq!(hits_in_line("use old_name here", &t), vec!["old_name"]);
        assert_eq!(hits_in_line("(thing)", &t), vec!["thing"]);
        assert!(hits_in_line("something", &t).is_empty());
        assert!(hits_in_line("my_old_name2", &t).is_empty());
    }

    #[test]
    fn a_later_whole_word_after_a_partial_one_is_found() {
        assert_eq!(hits_in_line("something thing", &tokens()), vec!["thing"]);
    }

    #[test]
    fn denylist_skips_comments_and_blank_lines() {
        assert_eq!(tokens(), vec!["old_name", "thing"]);
    }

    fn file(path: &str, text: &str) -> (String, String) {
        (path.to_string(), text.to_string())
    }

    #[test]
    fn hits_carry_file_line_and_token_and_files_are_counted() {
        let files = [
            file("a.rs", "ok\nuse old_name;\n"),
            file("b.rs", "thing\n"),
            file("c.rs", "clean\n"),
        ];
        let (scanned, hits) = scan_files(&files, &tokens());
        assert_eq!(scanned, 3);
        assert_eq!(
            hits,
            [
                ("a.rs".to_string(), 2, "old_name".to_string()),
                ("b.rs".to_string(), 1, "thing".to_string())
            ]
        );
    }

    #[test]
    fn docs_are_not_scanned_and_not_counted() {
        let files = [file("docs/x.md", "old_name\n"), file("a.rs", "ok\n")];
        let (scanned, hits) = scan_files(&files, &tokens());
        assert_eq!((scanned, hits.len()), (1, 0));
    }
}
