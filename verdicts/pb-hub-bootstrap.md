# Hub bootstrap: package review

Date: 2026-10-12. PR: trybotster/botster-hub #212. Round: 1. Tier: HIGH.
Head: `e9f344923fba060c4d88626e7864582b7c904ced`.
Base: `c5f088efa0b9dacf5af4f16f46ec6c74d3668f51`.

VERDICT: NOT CLEAN

The reviewer read the source, the PR body, the Stage 2 brief, the approved repo plan,
the minimum-Hub selection and additions, BUILD.md, and the referenced gate-kit code.
The reviewer built nothing locally and ran no execution job.

## Open findings

### HUBB-1 — HIGH: a rejected mutation report can pass the gate

Location: `xtask/src/mutants.rs:126-138`.
The command ignores an unreadable or missing `outcomes.json`. It prints a `stage_report` error and continues.
Only the cargo-mutants exit code decides the step's result.

The pinned gate-kit rejects a retried unmutated baseline, a caught mutant with no named catching test,
an unknown nextest status, and a missing or malformed log. These errors are verdict errors, not optional output.
For example, the mutation profile permits one retry. A baseline test that fails and then passes can leave
cargo-mutants with exit 0. Gate-kit rejects that baseline, but this command still returns success.
This breaks BUILD.md Testing rule 9: a retry must never turn a failure into a pass for any verdict.

Fail the step when the outcomes file cannot be read or the report fails. Keep the cargo exit check too.
Test the combined decision for a valid report, a rejected baseline, a missing report, and a report parse error.
Keep each catching test in the output.

### HUBB-2 — HIGH: mutation exclusions remove gate decisions

Location: `.cargo/mutants.toml` `.* in command$` exclusions; `xtask/src/ci.rs:87-91`.
The exclusions remove every mutation inside each command, not only whole-function replacements of I/O code.
Several excluded bodies still choose pass or fail. For example, deleting `!` from `if !failed.is_empty()`
changes the final CI verdict. The summary test only checks output text; it does not check this decision.
The excluded taint body also decides whether hits fail the step. The excluded mutation body contains HUBB-1.

BUILD.md's HIGH tier requires decision mutants to be caught or given a valid individual reason.
The stated reason that each excluded shell has tested pure decisions does not hold for these branches.
Move the remaining admission and result decisions into tested helpers. Narrow the exclusions to the I/O
operations or whole-body replacements that need them. Review every exclusion in this file for the same problem.
Do not add a new check family.

### HUBB-3 — MEDIUM: taint skips tracked files that cannot be read

Location: `xtask/src/taint.rs:25-29`.
`if let Ok(bytes)` discards each read error. The scanner can then report success over an incomplete file set.
A tracked source with denied read access is omitted even if it contains a banned name.
The command's own error contract says that an unreadable file fails. BUILD.md Starting fresh rule 5 requires
the taint check over code and generated artifacts; a skipped source does not prove that requirement.

Propagate each file read error with its path. Keep only the explicit skips of the shared taint policy.
Prove that a read failure cannot produce a successful taint verdict.

## Checks and evidence

The empty root, three initial crates, and nine-step gate match repo plan section 5 and the lead's bootstrap scope.
The placeholder programs do not claim to implement a selected conformance ID.
The minimum list has exactly the approved 125 IDs plus the two recorded HC additions.
All 127 remain pending. The single HP hello-ID ledger wait is explicit and is reported as waiting.
`ci/remote/job-common.sh` is byte-identical to contracts tag `contracts-v0.1.50`.
The workspace forbids unsafe code. The gate uses the shared gate-kit dependency rather than a copied Core gate.
The provisional gate-kit revision must move to the reviewed Core v1 delivery revision before final delivery.

Pool log: `~/botster-sessions/gates/botster-hub-stage2-gs-hub-bootstrap-e9f34492-pool-20261012-171623-39635.log`.
SHA256: `8a8c85ec60ed0debd6e42051697c01f41d3a5d984a301a0609322c052bed7f71`.
The log names the exact head and base above. All nine steps pass; the wrapper and job exit 0.
It reports 91 mutants: 87 caught, four unviable, zero missed. The printed catches name their tests.
This run does not exercise the rejected-report or unreadable-file paths, and its mutation exclusions omit
the decisions in HUBB-2. A green log therefore does not close these findings.

The integration reviewer also reports incomplete descendant cleanup at a tier deadline in
`xtask/src/budget.rs:162-164`. That finding is tracked in the integration verdict.
