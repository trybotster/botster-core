//! Source guard for Core route-close call sites.
//!
//! Every call that ends a terminal route or closes an adapter must name its
//! [`botster_core::contract::terminal_adapter::TerminalRouteCloseReason`].
//! [`route_close_sites`] lists each such call in a source file with its
//! enclosing function and the reason it passes, so a test can compare the
//! list against an exhaustive table and fail on any new, unlisted site.

/// Calls that end a route or close an adapter.
const CLOSE_CALLS: [&str; 5] = [
    "hard_stop_key(",
    "hard_stop_owner(",
    "teardown_session(",
    ".close(",
    "hard_stop(",
];

/// One route-close call: its enclosing function and the reason it passes.
///
/// `reason` is the variant name, or the argument text when the call passes
/// a reason through from its caller.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CloseSite {
    /// Name of the function that contains the call.
    pub function: String,
    /// Variant name such as `Replaced`, or a pass-through argument name.
    pub reason: String,
}

impl CloseSite {
    /// A site in `function` that passes `reason`.
    #[must_use]
    pub fn new(function: &str, reason: &str) -> Self {
        Self {
            function: function.to_string(),
            reason: reason.to_string(),
        }
    }
}

/// Every route-close call in the production part of `source`, sorted.
///
/// The production part ends at the first `#[cfg(test)]` module. Function
/// definitions named like a close call are skipped. A call that passes no
/// reason is listed with reason `<none>`.
#[must_use]
pub fn route_close_sites(source: &str) -> Vec<CloseSite> {
    let production = source
        .find("#[cfg(test)]\nmod ")
        .map_or(source, |end| &source[..end]);
    let mut sites = Vec::new();
    for call in CLOSE_CALLS {
        let mut from = 0;
        while let Some(offset) = production[from..].find(call) {
            let at = from + offset;
            from = at + call.len();
            if is_definition(production, at) || !is_call_start(production, at, call) {
                continue;
            }
            let arguments = call_arguments(production, at + call.len());
            if call == ".close(" && arguments.trim().is_empty() {
                // A close with no reason is not the adapter contract.
                sites.push(CloseSite::new(
                    &enclosing_function(production, at),
                    "<none>",
                ));
                continue;
            }
            sites.push(CloseSite::new(
                &enclosing_function(production, at),
                &reason_of(arguments),
            ));
        }
    }
    sites.sort();
    sites
}

fn is_definition(source: &str, at: usize) -> bool {
    source[..at].trim_end().ends_with("fn")
}

fn is_call_start(source: &str, at: usize, call: &str) -> bool {
    if call.starts_with('.') {
        return true;
    }
    // `hard_stop(` must not match the tail of `self.hard_stop(`-like names
    // such as `hard_stop_key(`; the needle ends at `(`, so only an
    // identifier character before it can join a longer name.
    !source[..at]
        .chars()
        .next_back()
        .is_some_and(|before| before.is_alphanumeric() || before == '_')
}

fn call_arguments(source: &str, open: usize) -> &str {
    let mut depth = 1usize;
    for (index, character) in source[open..].char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return &source[open..open + index];
                }
            }
            _ => {}
        }
    }
    &source[open..]
}

fn reason_of(arguments: &str) -> String {
    const PREFIX: &str = "TerminalRouteCloseReason::";
    if let Some(start) = arguments.find(PREFIX) {
        return arguments[start + PREFIX.len()..]
            .chars()
            .take_while(|character| character.is_alphanumeric() || *character == '_')
            .collect();
    }
    // A pass-through: the last argument, such as `reason`.
    arguments
        .rsplit(',')
        .map(str::trim)
        .find(|argument| !argument.is_empty())
        .unwrap_or("<none>")
        .to_string()
}

fn enclosing_function(source: &str, at: usize) -> String {
    source[..at]
        .lines()
        .rev()
        .find_map(|line| {
            let trimmed = line.trim_start();
            let rest = ["pub(crate) fn ", "pub fn ", "fn "]
                .iter()
                .find_map(|prefix| trimmed.strip_prefix(prefix))?;
            Some(
                rest.chars()
                    .take_while(|character| character.is_alphanumeric() || *character == '_')
                    .collect(),
            )
        })
        .unwrap_or_else(|| "<top>".to_string())
}

#[cfg(test)]
mod tests {
    use super::{route_close_sites, CloseSite};

    #[test]
    fn lists_each_call_with_its_function_and_reason() {
        let source = "fn one(&mut self) {\n    self.hard_stop_key(&key, TerminalRouteCloseReason::Stalled);\n}\n\
fn two(&mut self, reason: TerminalRouteCloseReason) {\n    adapter.close(reason);\n    adapter.close();\n}\n\
fn hard_stop_key(&mut self) {}\n#[cfg(test)]\nmod tests {\n    fn t() { x.close(); }\n}\n";
        assert_eq!(
            route_close_sites(source),
            vec![
                CloseSite::new("one", "Stalled"),
                CloseSite::new("two", "<none>"),
                CloseSite::new("two", "reason"),
            ]
        );
    }
}
