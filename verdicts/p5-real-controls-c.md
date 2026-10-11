# P5 real controls C review — #239

## Round 1 — 2026-10-10

Head: `a24c47e9613a6943a40d69183797580ac68b6187`.
Tree: `4817880cb0efc392f3f2014a77c7e7e3731a61a6`.
Parents: `5c4b7a7083c58e59804b61fbeccb716a3a083152` and `cf3bcaa93faa240b418fa2248884a96581fd82c3`.
PR and gate base: `cf3bcaa93faa240b418fa2248884a96581fd82c3`, contained in the head.
PR: <https://github.com/trybotster/botster-core/pull/239>.
READY: `msg_plugin-w_1791693885_7e165c`.

Authority: approved plan 23ag, `~/botster-sessions/pins/stage1-plan.cbbdb74e.md`.
Verified SHA256: `b7c41291e83b89b8224cc7b18f383912440555e999f729c1cfcb1111ce922bcb`.
BUILD.md at contracts `56bd0a5` applies.
Contracts pin: v0.1.27, `b161db87ca82873b4f6b21d8433a69f086abb7cf`.
Risk: HIGH under rule 3, because this PR changes the shared `botster-core-testkit` crate.

### Findings

**RC-C-F1 LOW — CLOSED: the PR body omitted the risk tier and deciding rule.**
The corrected body states HIGH and BUILD.md rule 3.
The reviewer read the published correction on the unchanged head.

**Integration SI-1 LOW — CLOSED: the PR body omitted the Prior art note.**
The corrected body compares old Core at `72b2e3354ffc291e39f9a5d7eb2f9c5fcbb5e79c` and explains each retained or rejected approach.
It explains the frame inspection, stored-row read, and reuse of `RefusalLayer` and shared argument checks.
The reviewer checked the named old fake methods and registry test through read-only Git commands.
The body also corrects the claim that the tap keeps no byte copy.
Core retains and sends the held bytes; the decoders keep copies for observation.
The tap has no separate buffer that sends held bytes after release.
The reviewer read both published corrections on the unchanged head.

**RC-C-F2 MEDIUM — OPEN: an armed hold lets a valid large Launch frame pass.**

At `crates/botster-core-testkit/src/edge_tap.rs:95`, the outbound decoder uses `DEFAULT_MAX_PAYLOAD`, which is 1 MiB.
At line 421, `sendable` returns `bytes.len()` after a decoder error.
The tap therefore forwards the complete send to the inner edge.
The subsequent `sent` call disables that decoder after the same error.

Core accepts a larger frame bound.
`crates/botster-core-host/src/engine.rs:458-462` derives the bound from the configured paste and snapshot limits.
The default 16 MiB snapshot limit gives a 65 MiB link bound.
`admit.rs:80-103` imposes no length limit on non-NUL argv or environment strings.
`run.rs` copies those values into the Launch specification.
`driver.rs:410-419` encodes the complete frame with Core's actual bound before flushing it.

A Launch with seventeen non-NUL argv strings of 64 KiB exceeds the tap's 1 MiB bound and fits Core's default bound.
The tap encounters `TooLarge` at the frame header and permits the send.
Thus the first Launch byte reaches the worker while `hold_start_at(payload)` is armed.
This violates R-49 Q2 condition 1, which requires a hold from the first byte through every later byte until release.
The finding concerns Core's accepted outbound frame; it does not require a successful worker exec of those arguments.

Required correction:

- Inspect every frame that Core can send without a smaller payload bound.
- Do not release a legal frame silently after a decoder error.
- Add a boundary proof above 1 MiB and below Core's bound.
- Prove that no Launch byte passes before release and that all bytes pass unchanged after release.

The reviewer sent RC-C-F2 directly to P5 and Sol integration.
P5 accepted the finding in `msg_plugin-w_1791694286_5db075`.
P5 reports local correction `c6f6c9b120899cd81433b3a65a55cb28849a54e3` and a running gate.
That unpublished correction is outside this round.
Its replacement READY and exact-head evidence remain required.

### Source review

The reviewer read the complete six-file delta: 585 insertions and 43 deletions.
The changed files are the real pending list, `edge_tap.rs`, its tests, `real.rs`, `refusal.rs`, and `start_controls.rs`.
No Core or worker product source changes.
No new mutation exclusion or platform-only code exists.
`git diff --check` passes.

The small-frame path holds the Launch prefix and all later sends on the selected link.
Core retains the unsent bytes in its outbound buffer.
Release restores Core's last requested write interest.
The reviewer checked partial sends, prior-frame prefixes, descriptor sends, other links, and unheld starts.
The six new boundary tests assert public bytes and edge behavior.
Those tests do not cover RC-C-F2's large-frame condition.

`registry_row` reads the actual stored bytes through the inner storage edge and uses `Row::decode`.
The start controls reuse the shared session checks and require the Created row before arming.
The real harness wraps its driver with the existing synchronous `RefusalLayer`.
The wrapper returns the selected code before it calls real Core.
Shared `arm_refusal` checks the synchronous column, occurrence, and armed-call conflict.
The plain real harness retains unsupported control results.

The reviewer read R-49 and all three removed transcripts at the exact contracts pin.
The real proof checks stored Starting state, durable identity, quiet observation, and identity-checked `payload_alive` before release.
It checks Running and a live payload after release.
It also checks refusal before Core, unchanged Created state, and a later successful Start.
These checks cover the supplied small Launch; they do not resolve RC-C-F2.

### Exact-head evidence and lists

Gate: `~/botster-sessions/gates/botster-core-stage1-p5-real-controls-c-a24c47e9-pool-20261010-211218-74506.log`.
The header names the exact head and current base.
The command is the plain Linux gate with `CARGO_BUILD_JOBS=4` and `NEXTEST_TEST_THREADS=4`.
All ten stages pass; CI takes 1202.5 seconds.
Default: 1591 passed, 485 skipped, in 10.597 seconds.
Slow: 412 passed, 1980 skipped, in 97.948 seconds.
The new real proof passes in 1.182 seconds.
Testkit conformance has 205 actual passes; real conformance has 125 actual passes.

Real pending decreases from 83 to 80, with three removals and no additions:

- `conf::a5_3_only_sync_column_codes_scripted`.
- `conf::a5_3_sync_refusal_has_no_op_event_slot_or_state`.
- `conf::ad_7_payload_launches_after_durable_identity` — a minimum ID.

Each removed ID has an actual real-tier PASS at this exact head.
Minimum results: testkit 54 / 69, real-passing 36 / 68, real-accepted 36 / 69.
Pending-real reports 80 IDs and zero newly passing IDs.
Every other conformance file equals the current base bytes.
Ledger: 690; Core pending: 465; deferred: two; withdrawn: 18.

Stage 1 tests 57 mutants: 36 caught, 19 missed, two unviable, zero timeout, in 164.8 seconds.
The slow mutation baseline passes 412 tests, in 124.7 seconds including its build.
Stage 2 tests exactly those 19 miss identities once each.
It catches 16 and finds three unviable, with zero missed or timeout, in 725.9 seconds.
The reviewer checked every printed outcome and the exact identity correspondence between stages.
Final result: 52 caught, five unviable, zero missed or timeout.
The mutation job takes 1017.5 seconds.
The job and gate exit zero after 1913 seconds on msa1: 701 queued seconds and 1212 running seconds.
The unchanged per-tag probe cache is valid under the lead's #235 ruling.

The green gate does not test the large-frame hold condition.
RC-C-F2 remains open on this exact head.
The reviewer changed only this verdict file and ran no builds, tests, gates, mutation jobs, measurements, or base-merge checks.

VERDICT: NOT CLEAN
