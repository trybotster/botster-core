# Contracts #49: R-70 attach fence

## Round 1 — 2026-10-11

Head: `13fb5830f2824be98536c3dd6e96e7763b51751e`.
Source and PR base: `9d19b997fd044ca76c0844f8a32fa79d930e5ec4`.
Gate base: `5ff1b52be672bc107e80cd0d911110034745eda4`.
Tree: `05ec43f22b02846da222229f669410d8d99ac36f`.

Sol read the complete three-file delta, the complete live body, and the R-70 draft. Sol read 016c's complete local ACCEPT at `.review-verdicts/r70-ou9-attach-fence-13fb5830.md`. Its SHA256 is `bde9626554d6393f54fdc62d6bd8a2625fed393f7e35ea86e6ece99f906b8c29`. The lead supplied this accepted local record for this review. Sol claims no package verdict commit.

### Source and contract conditions

Both ou_9 transcripts append an OSC 2 sentinel to the earlier output. Each awaits that title before attach. The adopted case awaits on h before dropping h. No launch, adoption, or open interrupts either sentinel/wait pair.

Static JSON comparison confirms that all other fields and expectations equal the original transcripts. The two full scratch transcripts at `c511323252480d39fec4cc22e7da1f1a02043405` equal the final transcripts. Later exact live-output assertions remain unchanged. The body accounts for title events and preserves the original model expectations.

The lint extends the existing unread-output map. An attach checks unmet session output across handles. A later literal output assertion on that route produces one finding per attach. A prior wait that proves the output was read can meet the output. The new rule adds no exception. The existing exception list remains unchanged.

Package finding R1-1 MEDIUM closes. The predicate excludes variable byte references in all named forms. A mixed assertion with literal bytes still counts. The retained reversal fails the variable-reference case. The restored lint passes all 15 tests. Sol adds no finding.

### Supplied real evidence

Sol read the complete evidence script, the local delayed-read patch, both lint reversal logs, and all four before/after transcript logs. Sol ran none of them.

The Core proof head is historical `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`, on Darwin 27.0.0. The fixed run uses the local .36-plus-fence scratch tree through a temporary Cargo patch. The worker delay is local diagnosis code. It is absent from this PR.

For each ID, plain before and after runs pass five of five on each tier. With the delayed first PTY read, each real ID fails five of five before the fence. The baseline case fails at step 14 with `before` in live output. The adopted case fails at step 20 with its earlier output in live output. Both fixed cases pass five of five. The unfenced-transcript reversal makes the lint flag both IDs. These retained results support the identified missing-fence cause and the structural repair.

The local script retains focused failure details. It is not a whole-suite CI comparison and makes no CI seed claim. These proofs retain their historical Core and scratch heads. They do not claim execution after a future Core pin.

### Supplied exact-head gate

Gate: `gates/botster-contracts-p3-ou9-attach-fence-13fb5830-pool-20261011-085710-49335.log`.
SHA256: `3eb84d4b2ebc7d09c8d8562713c683081b7a1572f83bddc233b0b90a8e32c501`.

Sol read the head/base headers, lint test records, suite summaries, mutation record, and final exits. All ten steps pass in 513.5 seconds. The default suite has 2077 passes and zero skips. The slow suite has 33 passes and zero skips. All 15 fence-lint tests pass, including the full corpus and the new attach test. The mutation step selects no mutant. The job and wrapper exit zero after 589 seconds. Sol makes no individual fuzz-corpus inspection claim.

### Verdict and remaining propagation

The supplied package ACCEPT and exact-head evidence close the source review. The missing ou_9 fence has an identified cause and a reviewed structural repair. A later base import, its applicable check, and its exact-head gate remain separate. Core still needs the corrected tag and its own pin gate. The earlier FLAKY mutation records remain historical no-catch records. This review adds no exclusion, re-pend, or retrospective catch.

Sol changed no product code. Sol ran no builds, tests, gates, mutants, measurements, or reversal jobs. The steward owns the Contracts merge.

VERDICT: CLEAN (0 open)
