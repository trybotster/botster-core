//! The per-test times of a nextest JUnit file (moved from Core's xtask `test_budget`).

use regex::Regex;
use std::collections::BTreeMap;

/// The per-test times of a nextest JUnit file: `<binary id> <test name>` to seconds.
pub fn parse_junit(xml: &str) -> BTreeMap<String, f64> {
    let case = Regex::new(r"<testcase\s+([^>]*?)/?>").expect("regex");
    let attr = Regex::new(r#"(\w+)="([^"]*)""#).expect("regex");
    let mut times = BTreeMap::new();
    for caps in case.captures_iter(xml) {
        let attrs: BTreeMap<&str, &str> = attr
            .captures_iter(caps.get(1).map_or("", |m| m.as_str()))
            .map(|c| (c.get(1).unwrap().as_str(), c.get(2).unwrap().as_str()))
            .collect();
        let (Some(name), Some(time)) = (attrs.get("name"), attrs.get("time")) else {
            continue;
        };
        let Ok(secs) = time.parse::<f64>() else {
            continue;
        };
        let class = attrs.get("classname").copied().unwrap_or("");
        times.insert(unescape(&format!("{class} {name}")), secs);
    }
    times
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn junit_times_are_read_per_test() {
        let xml = r#"<testsuite><testcase name="a::b" classname="crate::bin/x" timestamp="t" time="0.250"><error type="e"/></testcase>
            <testcase name="c&amp;d" classname="y" time="1.5"/></testsuite>"#;
        let times = parse_junit(xml);
        assert_eq!(times["crate::bin/x a::b"], 0.25);
        assert_eq!(times["y c&d"], 1.5);
    }
}
