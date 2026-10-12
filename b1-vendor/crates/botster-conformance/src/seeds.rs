//! Seed sets and test selection (design 6.1).
//!
//! A run has a seed set. The default local set is seeds 0 to 7. CI uses seeds 0 to 31 (`BOTSTER_SEEDS=0-31`).
//! A fake receives the seed at construction. A real implementation ignores it.

use crate::transcript::Transcript;
use rand_chacha::ChaCha8Rng;
use rand_core::SeedableRng;

/// The seeds of one run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeedSet {
    pub seeds: Vec<u64>,
}

impl SeedSet {
    /// Seeds 0 to 7, the local default.
    pub fn local() -> Self {
        SeedSet {
            seeds: (0..8).collect(),
        }
    }

    /// Seeds 0 to 31, the CI set.
    pub fn ci() -> Self {
        SeedSet {
            seeds: (0..32).collect(),
        }
    }

    /// Parses `3`, `0-31` or a comma list such as `1,4-6`. `Err` carries the bad piece.
    pub fn parse(text: &str) -> Result<Self, String> {
        let mut seeds = Vec::new();
        for piece in text.split(',') {
            let piece = piece.trim();
            let bad = || format!("bad seed range '{piece}'");
            match piece.split_once('-') {
                Some((from, to)) => {
                    let (from, to): (u64, u64) = (
                        from.trim().parse().map_err(|_| bad())?,
                        to.trim().parse().map_err(|_| bad())?,
                    );
                    if from > to {
                        return Err(bad());
                    }
                    seeds.extend(from..=to);
                }
                None => seeds.push(piece.parse().map_err(|_| bad())?),
            }
        }
        if seeds.is_empty() {
            return Err("empty seed set".to_string());
        }
        seeds.sort_unstable();
        seeds.dedup();
        Ok(SeedSet { seeds })
    }

    /// Reads `BOTSTER_SEEDS`. An unset variable gives [`SeedSet::local`]. A bad value panics, so a typo never narrows a run silently.
    pub fn from_env() -> Self {
        match std::env::var("BOTSTER_SEEDS") {
            Ok(text) => Self::parse(&text).unwrap_or_else(|e| panic!("BOTSTER_SEEDS: {e}")),
            Err(_) => Self::local(),
        }
    }
}

/// A ChaCha8 generator for runner-side choices, derived from the run seed and the transcript id (design 7.2, rule 2).
pub fn driver_rng(seed: u64, transcript_id: &str) -> ChaCha8Rng {
    // FNV-1a of the id, so that two transcripts under one seed draw different streams.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in transcript_id.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    ChaCha8Rng::seed_from_u64(seed ^ hash)
}

/// Which transcripts and seeds to run: `--only <id or prefix>`, `--clause <id>`, `--seed <n>` (design 6.1).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Selection {
    pub only: Option<String>,
    pub clause: Option<String>,
    pub seed: Option<u64>,
}

impl Selection {
    /// Parses runner arguments. `Err` carries the bad argument.
    pub fn from_args<I: IntoIterator<Item = String>>(args: I) -> Result<Self, String> {
        let mut sel = Selection::default();
        let mut it = args.into_iter();
        while let Some(arg) = it.next() {
            let mut value = |name: &str| it.next().ok_or_else(|| format!("{name} needs a value"));
            match arg.as_str() {
                "--only" => sel.only = Some(value("--only")?),
                "--clause" => sel.clause = Some(value("--clause")?),
                "--seed" => {
                    sel.seed = Some(
                        value("--seed")?
                            .parse()
                            .map_err(|_| "--seed needs a number".to_string())?,
                    )
                }
                other => return Err(format!("unknown argument '{other}'")),
            }
        }
        Ok(sel)
    }

    /// Reads `BOTSTER_ONLY`, `BOTSTER_CLAUSE` and `BOTSTER_SEED`, for `cargo test`.
    pub fn from_env() -> Self {
        Selection {
            only: std::env::var("BOTSTER_ONLY").ok(),
            clause: std::env::var("BOTSTER_CLAUSE").ok(),
            seed: std::env::var("BOTSTER_SEED").ok().map(|s| {
                s.parse()
                    .unwrap_or_else(|_| panic!("BOTSTER_SEED needs a number"))
            }),
        }
    }

    /// True when the transcript is selected. `only` matches an id or an id prefix.
    pub fn selects(&self, t: &Transcript) -> bool {
        self.only.as_deref().is_none_or(|p| t.id.starts_with(p))
            && self.clause.as_deref().is_none_or(|c| t.clause == c)
    }

    /// The seeds to run: the selected seed, or the whole set.
    pub fn seeds(&self, base: &SeedSet) -> SeedSet {
        match self.seed {
            Some(seed) => SeedSet { seeds: vec![seed] },
            None => base.clone(),
        }
    }
}
