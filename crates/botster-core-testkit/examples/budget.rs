//! The R4 budget measurement of P6 M0b: `cargo run -p botster-core-testkit --example budget --release`.
//!
//! `run_transcript` stops at the first seed that does not pass, so this runs every Core transcript once per seed, for seeds 0 to
//! 31 explicitly, and counts the driver constructions and the outcomes. It measures the cost of the harness path of the ids
//! that run. While `open` reports that no Core exists, every run stops at its first failing step, and the numbers say nothing
//! about passing transcripts.

use botster_conformance::{load_dir, run_transcript, Limits, SeedSet};
use botster_core_conformance::{driver_for, CoreSchemas, CORE_TRANSCRIPTS};
use botster_core_testkit::TestkitHarness;
use std::cell::Cell;
use std::time::{Duration, Instant};

fn main() {
    let transcripts = load_dir(&CORE_TRANSCRIPTS).expect("the Core transcripts load");
    let constructions = Cell::new(0usize);
    let (mut passed, mut not_passed) = (0usize, 0usize);
    let mut worst = (Duration::ZERO, String::new());
    let start = Instant::now();
    for transcript in &transcripts {
        for seed in 0..32u64 {
            let began = Instant::now();
            let outcome = run_transcript(
                transcript,
                &|seed| {
                    constructions.set(constructions.get() + 1);
                    driver_for(Box::new(TestkitHarness::new(seed)))
                },
                &SeedSet { seeds: vec![seed] },
                &CoreSchemas,
                &Limits::default(),
            );
            if outcome.is_pass() {
                passed += 1;
            } else {
                not_passed += 1;
            }
            let took = began.elapsed();
            if took > worst.0 {
                worst = (took, format!("{} seed {seed}", transcript.id));
            }
        }
    }
    let runs = transcripts.len() * 32;
    let total = start.elapsed();
    println!(
        "transcripts {}, seeds 0-31, runs {runs}, driver constructions {}, passed {passed}, not passed {not_passed}",
        transcripts.len(),
        constructions.get()
    );
    println!(
        "total {total:?}, per transcript (32 seeds) {:?}, per run {:?}, worst run {:?} ({})",
        total / transcripts.len() as u32,
        total / runs as u32,
        worst.0,
        worst.1
    );
}
