//! Source readers shared by the tests that check what `realize` emits (`copy_keys.rs`, `codes.rs`).

/// The arguments of each call `name(...)` in `src`, split at the top-level commas. The definition `fn name(` is skipped.
pub fn calls<'a>(src: &'a str, name: &str) -> Vec<Vec<&'a str>> {
    let open = format!("{name}(");
    let mut out = vec![];
    let mut from = 0;
    while let Some(i) = src[from..].find(&open) {
        let at = from + i;
        from = at + open.len();
        let before = &src[..at];
        let word_char = |c: char| c.is_alphanumeric() || c == '_';
        if before.ends_with("fn ") || before.chars().last().is_some_and(word_char) {
            continue;
        }
        let (mut depth, mut start, mut args) = (1, from, vec![]);
        for (j, c) in src[from..].char_indices() {
            match c {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => {
                    depth -= 1;
                    if depth == 0 {
                        args.push(src[start..from + j].trim());
                        break;
                    }
                }
                ',' if depth == 1 => {
                    args.push(src[start..from + j].trim());
                    start = from + j + 1;
                }
                _ => {}
            }
        }
        out.push(args.into_iter().filter(|a| !a.is_empty()).collect());
    }
    out
}
