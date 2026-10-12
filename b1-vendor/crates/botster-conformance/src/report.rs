//! The report printer (design 6.1): one line per failure, a summary, and a reproduce command.

use crate::result::Outcome;
use std::io::{self, Write};

/// The test function name of an id: `conf::en_8_x` becomes `conf_en_8_x`.
pub fn test_name(id: &str) -> String {
    crate::testgen::test_name(id)
}

/// The command that reproduces one result, or an empty string for a result without a seed.
pub fn reproduce(crate_name: &str, test: &str, outcome: &Outcome) -> String {
    let seed = match outcome {
        Outcome::Failed { seed, .. } | Outcome::Inconclusive { seed, .. } => Some(*seed),
        _ => None,
    };
    match seed {
        Some(seed) => {
            format!("reproduce: BOTSTER_SEEDS={seed} cargo test -p {crate_name} -- --exact {test}")
        }
        None => format!("reproduce: cargo test -p {crate_name} -- --exact {test}"),
    }
}

/// A one-line description of a result that is not a pass.
pub fn describe(id: &str, outcome: &Outcome) -> String {
    match outcome {
        Outcome::Passed => format!("PASS {id}"),
        Outcome::Failed {
            step,
            expected,
            received,
            seed,
        } => {
            format!(
                "FAIL {id} seed={seed} step={step}\n  expected: {expected}\n  received: {received}"
            )
        }
        Outcome::Inconclusive { step, limit, seed } => {
            format!("INCONCLUSIVE {id} seed={seed} step={step} limit={limit}")
        }
        Outcome::NotApplicable { feature } => format!("NOT_APPLICABLE {id} feature={feature}"),
        Outcome::UnsupportedControl { op } => format!("UNSUPPORTED_CONTROL {id} op={op}"),
    }
}

/// The results of a run.
pub struct Report {
    crate_name: String,
    entries: Vec<(String, Outcome)>,
}

impl Report {
    pub fn new(crate_name: &str) -> Self {
        Report {
            crate_name: crate_name.to_string(),
            entries: Vec::new(),
        }
    }

    pub fn push(&mut self, id: &str, outcome: Outcome) {
        self.entries.push((id.to_string(), outcome));
    }

    /// True when every result is `passed`. Phase acceptance needs this (design 6.1).
    pub fn is_clean(&self) -> bool {
        self.entries.iter().all(|(_, o)| o.is_pass())
    }

    fn count(&self, pick: fn(&Outcome) -> bool) -> usize {
        self.entries.iter().filter(|(_, o)| pick(o)).count()
    }

    /// The summary line.
    pub fn summary(&self) -> String {
        format!(
            "passed {}, failed {}, inconclusive {}, not_applicable {}, unsupported_control {}",
            self.count(Outcome::is_pass),
            self.count(|o| matches!(o, Outcome::Failed { .. })),
            self.count(|o| matches!(o, Outcome::Inconclusive { .. })),
            self.count(|o| matches!(o, Outcome::NotApplicable { .. })),
            self.count(|o| matches!(o, Outcome::UnsupportedControl { .. })),
        )
    }

    /// Prints one block per result that is not a pass, then the summary.
    pub fn print<W: Write>(&self, out: &mut W) -> io::Result<()> {
        for (id, outcome) in self.entries.iter().filter(|(_, o)| !o.is_pass()) {
            writeln!(out, "{}", describe(id, outcome))?;
            writeln!(
                out,
                "  {}",
                reproduce(&self.crate_name, &test_name(id), outcome)
            )?;
        }
        writeln!(out, "{}", self.summary())
    }
}
