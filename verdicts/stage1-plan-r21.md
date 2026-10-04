# Stage 1 plan revision 21 review

Exact reviewed head: `ad03636ffbd68048f9f07faf176986f004d5e516`.
Review scope: `27ffaa4..71b765e`, `docs/stage1-plan.md` only, and consistency of that delta with the plan and BUILD.md.

VERDICT: CLEAN (0 open). R21-F1 closes at the current reviewed head.

## Review inputs and accepted facts

- Read-only pin: `~/botster-sessions/pins/stage1-plan.0e6dc165.md`.
  The file and committed plan both have SHA-256 `0e6dc1658496c9861597533d2f4c80bc8b11df8d2ed660e6085d718b6a9d3818`.
- Contracts sync record: `docs/ghostty/upstream-sync-20261002.md` at `8ee672a`, with its evidence directory.
- Merged audit revision 10: `38e6989:docs/stage1/libghostty-audit.md`.
- Published P2 verdict: `f7dadfb9cbc21a412335b16e0132bf008bcc0758`.
- PR #134 metadata and description, read through `gh pr view` after the GitHub MCP tool failed.
- BUILD.md, including terminal semantics, the terminal oracle rule, and Ghostty fork policy.

The delta changes the two pin rows, the Q1 row, and the revision 21 history row only.
The following facts match the evidence:

- Fork branch `botster/upstream-sync-20261002`, head `3f8eb6810bb673aa782b047de21783ac81fb1121`, and upstream base `f523504ea5c9f41d150d1eb93cc7a748b90f9361`.
- The stack contains 22 commits in patch groups 0–13. The range-diff preserves all patches.
  Only the macOS Swift file differs between the old and new stack heads. No patch was dropped.
- Patch groups 9–13 and their clauses agree with the audit and published review.
- P2 is CLEAN at `ff603fe3603275c4c02c173605a817aa9949960b`, with green Mac and Linux gates on that exact head.
- PR #134 is MERGED into `v1`, with that head and merge commit `38e6989be8166ff773c8ad02ac511aa4247448c6`.
  The merge's second parent is the reviewed P2 head.
- Zig remains 0.16.0. The package list has seven hashes; the audit explains the former duplicate count of nine.
- `ci/remote/fetch-public.sh` arrived through PR #131, merge `41ebcc05efe7c1c5a6e7209d35f91825453caac8`.
- The terminfo is unchanged by the sync. This Ghostty move requires no related terminfo or contract tag change.
- Q1 and the revision 21 row name the same pin as section 0.
- The P35 text preserves the lead ruling. Botster assertions retain the native oracle requirement.
  Fork unit tests may define the oracle with literals. A test derived from its own encoder table is not independent.
- The sync record and retained patch list follow the fork policy. The delta grants no permission to contact upstream.

The reviewer ran no tests, builds, or gates.
The plan review does not claim that pending Stage 1 conformance IDs pass.

## R21-F1 — LOW — Section 8 still names the private fetch step for build-time sources

**Evidence:** the revised section 0 Zig row correctly sends the Ghostty submodule and packages through `ci/remote/fetch-public.sh`.
The unchanged section 8 gate paragraph at line 459 still says that build-time fetches go to `ci/remote/fetch.sh`.
These instructions conflict for the sources described in the revised pin row.
The merged gate separates private Cargo dependency resolution from public source builds and package fetches.
`fetch.sh` uses the private dependency context; `fetch-public.sh` runs afterward without the token.

**Required change:** update the section 8 gate paragraph to distinguish the two fetch steps.
State that `fetch.sh` resolves Cargo dependencies and `fetch-public.sh` fetches the public submodule and Zig packages without the token.
Retain the gate container's network denial and the infra engineer's ownership.
Send the exact corrected plan head and hash-verified pin for delta review.

VERDICT: NOT CLEAN (1 open: R21-F1) at plan head `71b765e09f1795fd284644a20ce7fde60fa9ac80`.


## R21-F1 correction review — CLEAN

Exact corrected head: `ad03636ffbd68048f9f07faf176986f004d5e516`.
The reviewer read the complete delta from `71b765e09f1795fd284644a20ce7fde60fa9ac80`.
Only the section 8 gate paragraph and revision 21 history row change.

Corrected read-only pin: `~/botster-sessions/pins/stage1-plan.bdda2359.md`.
The pin and committed plan both have SHA-256 `bdda23593c29e3cc654acf0a5a98eee43f218b119ff2cd530cbd31d6b2cff9ac`.

**R21-F1 closes.** Section 8 now distinguishes Cargo dependency resolution with the token from public source fetching without the token.
It assigns the Ghostty submodule and Zig packages to `fetch-public.sh` and states that Mac runs this step inside the exclusive job.
It retains the gate container's network denial and the infra engineer's ownership of both scripts.
The revision 21 row records the correction.
Section 8 now agrees with the revised section 0 Zig row and the merged gate scripts.
All earlier accepted facts remain unchanged and valid.

The reviewer ran no tests, builds, or gates for this document correction.
Every finding, including the LOW finding, is closed within the assigned revision 21 review scope.

VERDICT: CLEAN (0 open) at plan head `ad03636ffbd68048f9f07faf176986f004d5e516`.
