# #247 — sha2 profile optimization

## Round 1 — 2026-10-11

Exact head: `14cde79e46da90b83079b15ce463d833364a1ef6`.
Base: `f6778ad85c01266ac49e2ededd97487da689e920`.
Receipt: `msg_plugin-w_1791706121_ab989b`.
Package CLEAN: `c7c538a1d15873d82cec866d88976cf6f38a8c6e`, `verdicts/gs-gate-speed.md`.
I read the complete exact-head package artifact after `msg_plugin-w_1791706259_28e730`.
Authority: approved plan 23ak at `4eceef06434d643bf31e965a35a75d4252d27d19`, BUILD.md at `56bd0a5`, and the GS brief.
The live plan pin has SHA256 `db7ba8e5c0748aa53257fb53a5e6f8923a9549c6c9871127a7d2ef2c97000813`.
The lead's ~03:15 ruling approves this lever and its conditions. The ~06:10 entry assigns this exact-head HIGH review.

### Scope and package interfaces

The complete own diff adds eleven lines to root Cargo.toml. It changes no other file against the base.
The sha2 dependency builds at dev opt-level 3, with debug assertions and overflow checks explicitly enabled.
The override names one dependency. No Core workspace crate receives an override.
The existing #223 overrides remain unchanged. The dependency version and Cargo.lock remain unchanged.
The comment identifies the production scope: botster-core-link also uses sha2 for its handshake proof.
I read that proof code, Candidate::locate, their relevant public tests, and the prebuild hash writer.
The xtask also uses sha2 for prebuild manifests and other hashes. Those dev builds receive the same dependency override.
The proof layout, roles, verification bytes, mismatch refusal, manifest entries, and comparison remain unchanged.
Every harness still verifies all three candidate binaries. This PR adds no cache and removes no verification.
The existing mismatch proof checks refusal after each binary changes. The retained gate passes that proof and the link proof tests.

The change preserves every test name, selector, timeout, mutation selector, mutation verdict, and candidate build rule.
It changes no runtime owner, cleanup, conformance ID, transcript, contract pin, or mutation exclusion.
The dependency's pure hash computation changes no behavior that the operating system decides.
This own delta requires no additional focused Mac real-process run.

The head imports current v1 without a conflict. Its automatic source merge tree equals its actual tree:
`dde1e700c989514053eff58d7cbfda8a10521627`.
The accepted #244 correction remains present: its internal constant assertion is absent, and its public repository proofs remain.
The exact-head diff check passes. GitHub names the reviewed head and base branch v1.

### Prior art and retained comparison

The PR compares OLD CORE at binding `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c`.
I read its root Cargo.toml and search its tracked source for this candidate verification and sha2 profile override.
It has neither corresponding mechanism. This PR reuses current #223's profile form and adds no custom hash code.

I read the evidence-only stamp patch, retained log headers, outcomes, summary lines, and timing records.
The measurement jobs use earlier base `7e8e07d6`, candidate head `d1ccfe9b`, and temporary stamp snapshots.
The d1ccfe9b own Cargo.toml change matches this PR's eleven-line delta. The measurement patch does not enter this PR.
Those jobs are measurements, not merge gates.

L0 log: `~/botster-sessions/gates/botster-core-gs-l0-trace-7e8e07d6-evidence.log`.
Static parsing finds 258 completed candidate intervals, totalling 360617 ms.
The eighteen test-process stamp spans total 363791 ms. Candidate verification accounts for 99.1 percent of that measured time.

Comparison logs:
- `~/botster-sessions/gates/botster-core-gs-sha2-before-7e8e07d6-evidence.log`.
- `~/botster-sessions/gates/botster-core-gs-sha2-after-d1ccfe9b-evidence.log`.
- `~/botster-sessions/gates/botster-core-gs-sha2-after-trace-d1ccfe9b-evidence.log`.

I independently compare the complete PASS-name multisets of both first slow-suite runs. They match, with 412 names each.
The wrapper reports slow wall time 109.0 seconds before and 29.3 seconds after.
The 125 real-conformance test durations total 340.226 seconds before and 27.313 seconds after.
Both trace samples contain 258 completed candidate intervals, with no unmatched start.
The before mean is 1191.5 ms, range 1054–2240 ms. The after mean is 42.3 ms, range 37–55 ms.
The before slow and trace commands, after slow command, and after trace command each report exit zero.
These results support the measured improvement. They do not prove an old-versus-new mutant outcome comparison.

### Retained exact-head gate and limits

Log: `~/botster-sessions/gates/botster-core-stage1-gs-sha2-opt-14cde79e-pool-20261011-010427-15764.log`.
I read its exact-head header, conformance reports, relevant verification tests, summary, and mutation and fuzz decisions.
All ten stages pass in 209.4 seconds. Default: 1610 passed, 484 skipped, in 11.585 seconds.
Slow: 427 passed, 1993 skipped, in 29.165 seconds. The complete slow step takes 53.2 seconds.
Conformance passes 206 tests. Strict pending runs 373 / 394, with 21 real-only IDs not run and two held IDs.
Testkit minimum: 55 / 69. Pending-real: 77 IDs, zero newly passing IDs.
The gate reports real-passing 36 / 68 and real-accepted 37 / 69.
Those are historical gate counts. R-59 counts the current real minimum as 35 / 68 plus one unfenced AD-7 pass.
The mutation step reports zero mutants because the own diff changes no Rust source. It runs no cargo-mutants job.
Fuzz reports no changed decoder harness. The job exits zero after 218 seconds, with zero queue wait.
The wrapper exits zero after 219 seconds. This gate covers the exact reviewed head.

The package artifact notes the existing prebuilt-worker gap: mutation runs do not rebuild target/candidate for each mutant.
This PR changes no candidate build rule and grants no exclusion or acceptance for that gap.
This verdict covers only the sha2 profile. It accepts no cache, in-place mutation run, baseline reuse, or later GS lever.
The GS brief requires a final gate after both CLEANs. This verdict claims only the retained pre-review gate result.
No integration finding remains. I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open)
