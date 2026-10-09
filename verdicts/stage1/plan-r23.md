# Integration review: plan revision 23 (branch stage1/plan)

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate. This is a doc change, so no gate applies.

## Round 1 — NOT CLEAN on head 1cdb24e2

Reviewed head: `1cdb24e28f2093cb4ee60e4e17e16c93e82f66c2`, diff from `eeb3108728e7973960498a91340fc49f47a5a3f6`
(`docs/stage1-plan.md`, `docs/stage1-clauses/minimum-core.txt`, `docs/stage1-minimum-core.md`).

### Checked, no finding

- **The minimum list against the ledger.** `minimum-core.txt` has 70 unique ids. Each is in the ledger at
  `contracts-v0.1.19` with the same clause, and each is in the clause file of the owner that the list gives. The counts
  per package are P0 2, P1 31, P2 1, P3 2, P4a 18, P4b 1, P5 15, as in the plan and in section 1 of the minimum-core doc.
  The six steps of the doc add up to 70 (15, 12, 16, 3, 10, 14).
- **The clause files.** The package files hold 673 ids (2, 126, 6, 167, 152, 68, 15, 55, 15, 67), which matches the doc's
  table. The core id set is the same at `contracts-v0.1.17` and `contracts-v0.1.19` (675), so "no change from revision 22"
  is right.
- **Pending on v1.** At `aaac0c0d`, `conformance/core-pending.txt` lists 671 ids outside comments, and all 70 minimum ids
  are among them. So "0 / 70" is right.
- **The missing transcripts.** Exactly two minimum ids have no transcript at `contracts-v0.1.19`:
  `dp_3_frame_limit_checked_before_allocation` and `ou_9_attach_is_sync_baseline_in_pump`.
- **The contracts facts.** `contracts-v0.1.19` = `636bc1ba`, `contracts-v0.1.18` = `ae4abc9b`, `contracts-v0.1.17` =
  `1725abf1`. The last ruling is R-36. No file under `frozen/` changes between v0.1.17 and v0.1.19, and the manifest stays
  final35. The risk tiers and the round limit are in BUILD.md at `main` `56bd0a5`.
- **The pins.** v1 is at `aaac0c0d`. #161 moved it to v0.1.18 and #182 to v0.1.19. The Ghostty pin is `39a68e82` on
  upstream `9d479dcb`, by #179 (`1832866c`), and its test counts match the #179 record.

### P23-1 LOW — the contracts CI guard does not exist

Section 0 says "a contracts CI guard now fails any symbol of the contract text that the crates lack". No such check is at
`contracts-v0.1.19`: no xtask, `.github` or `ci` file changes between v0.1.17 and v0.1.19, and no xtask source checks
symbols. Contracts `main` after the tag does not add one either. The steward handoff (v0.1.19 `docs/handoffs/steward.md:74`)
says that the crate catch-up "adds a CI check", which is planned work. Fix: say that the steward plans the guard, or name
the commit that adds it.

### P23-2 LOW — the list of merges since revision 22 is not complete

Section 11 says "Merged since revision 22" and names #171, #177, #178, #179, #180, #161, #182 and #183. Revision 22 read v1
at `ee7dd16c`. `git log --merges ee7dd16c..aaac0c0d` also has #170 (base-merge-check, `59ce1268`), #169 (the P5 host audit
fixes, `a0f78fe4`), #175 (the P6 guard reset, `0f789572`) and #174 (adoption part 1, `0b684a19`). Fix: add the four, or say
that the list is a selection.

### P23-3 LOW — a stale sentence about #161

The section 0 contracts row still says, in its v0.1.13 to v0.1.17 part, "`v1` moves to this tag by PR #161, which waits for
#163 part B". #161 moved v1 to v0.1.18, not to v0.1.17 (the same row says so earlier). Fix: delete the sentence, or put it
in the past with the real tag.

VERDICT: NOT CLEAN at 1cdb24e28f2093cb4ee60e4e17e16c93e82f66c2 (3 open: P23-1, P23-2, P23-3, all LOW)

## Round 2 — CLEAN on head dd656249

Reviewed head: `dd6562490d6b14255f6312d9b6c8e2d68caf5058`, one commit on `1cdb24e2` (`docs/stage1-plan.md`, +3 -3).

- **P23-1 closed.** The guard is now "planned by the steward (not yet in the contracts repo at `contracts-v0.1.19` or
  `main`)".
- **P23-2 closed.** The merged list adds #170, #169, #175 and #174. "The payload-guard ECONNRESET fix" matches #175's
  commit (`0f789572`: "a reset of the payload member's connection is its end of file").
- **P23-3 closed.** The v0.1.17 note now says that v1 moved past that tag: #161 went to v0.1.18, and #182 to v0.1.19.
- Revision row 23 records the round. No other text changed.

VERDICT: CLEAN (0 open) at dd6562490d6b14255f6312d9b6c8e2d68caf5058

## Round 3 — CLEAN on head 48c14ab8 (revision 23a)

Reviewed head: `48c14ab8dd40341295b004b1dfc679eb27207a52`, one commit on `dd656249` (the lead's ruling of 2026-10-09, raised
on #185).

- **Section 1** now has two counts: testkit-passing (out of `core-pending.txt`, the `TestkitHarness` trial passes) and
  real-passing (the `RealCoreHarness` trial or a named real-process test passes).
- **Section 5** lets an id leave the pending list when it passes on the `TestkitHarness`. A real-only id (4.2b: A5-3, or
  a replacement-map `slow` row) leaves only with its passing real proof. When the `RealCoreHarness` lands, it runs every
  non-pending id, and a failure is a finding. This agrees with 4.2b, which is unchanged: at acceptance, every id passes on
  both harnesses, and the real-only ids pass on the real harness or as a named real-process test.
- **Revision row 23a** records the ruling and the tier text.

Observation (not counted): row 23a makes a removal-only pending change STANDARD. The real-only exception is then checked by
the package review alone. The replacement map (`conformance/replacement-map.json`, `proof` = `slow:*`) makes the check
mechanical: `cargo xtask lists` could refuse a non-pending `slow:*` id unless it names its real test. This is the plan's own
rule "a rule that a gate can check is checked by the gate". It is the lead's call.

VERDICT: CLEAN (0 open) at 48c14ab8dd40341295b004b1dfc679eb27207a52

## Round 4 — revisions 23b and 23c (48c14ab8..def1d25e) — NOT CLEAN

Reviewed: `stage1/plan` `def1d25e375c7c7615c329573986fe4692ef3c18`, two commits on `48c14ab8` (`e637ccc2` 23b, `def1d25e`
23c). The lead asked for a case in which a merge-base comparison lets an addition through on the merging gate. The code
read is `xtask/src/lists.rs` at v1 `1f157c29`.

### P23b-1 MEDIUM — the merging-gate claim depends on a rule that the plan does not state and no step checks

23b says: "The merge rule requires the head to contain the current `v1`, so on the merging gate the merge base is the
base revision." The plan has no such rule. The Merge bullet (section 8) needs only the CLEANs and a green gate on the exact
head. `lists` does not check that `BOTSTER_CI_BASE_REF` is an ancestor of `HEAD`, and the gate's base ref can be older than
the v1 tip at the time of the merge.

When the head does not contain the base, an addition gets through. Example:
1. The merge base `M` lists `X`.
2. v1 removes `X` after `M` (a flip).
3. The PR moves the line of `X` to another place in the file, for example under another `#` comment group. The file is not
   sorted: `lists` rejects only duplicates (`lists.rs:51-60`).
4. The PR's gate compares with `M`: `X` is in `M` and in `HEAD`, so `X` is no gain, and the step passes. Under the
   revision 23a rule (compare with the base tip), the same head fails.
5. A merge of that head into v1 applies the PR's delete hunk (v1 deletes the same line) and its add hunk (clean). So `X`
   is pending again on v1.

The same gap applies to `core-deferred.toml` (any edit that v1's removal does not conflict with).

When the head contains the base, `git merge-base <base> HEAD` is the base, and the check is the 23a check. So the fix
is to make that condition real:
- Add the rule to the Merge bullet: the lead merges only a head that contains the current v1 tip, and only on a gate
  whose recorded base is that tip. The merge is then a fast-forward or a merge with no change to the tree.
- Make it mechanical: `lists` prints the base, the merge base and whether the base is an ancestor of `HEAD`, and the
  lead's merge step (or `base-merge-check`) refuses a head that does not contain the base.

### P23b-2 LOW — the merge base is read at two places where it can mislead

- With criss-cross history, `git merge-base` prints one of several bases. When the base is an ancestor of `HEAD`, there
  is only one. Otherwise, `lists` should use `--all` and fail with "merge v1" when it gets more than one.
- `lists` takes the initialization branch when the base has no pending file (`lists.rs:140`, `:283`). At a merge base
  older than the file, the check is initialization, which allows any ledger id back. Keep that decision on the base tip,
  which always has the file now, or remove the initialization branch.

Neither case reaches the merging gate if P23b-1 is fixed.

### P23c-1 LOW — the source-reading rule names the checks incompletely, and a proof can be outside the parentheses

- The rule names `process_check`, `platform_code` and `mutants_cited` "and later ones". `gate_decisions` (B5's check)
  and the timer-marker check (`timers`) also read Rust source and are in #181 now. Name every source-reading check, or
  say "every check that parses Rust source".
- "Any other word is a source reference": a shell entry whose reason names its tests in text, without parentheses, is not
  checked as a proof. Section 8 says the check "accepts a shell entry only when the shell's decision function is named in
  the entry's reason and has its tests". State that this entry must use the `decision (proof, …)` form with at least one
  proof, and that the check fails otherwise.

### P23c-2 LOW — Mac text that 23c did not change contradicts its new rule

- THE GATE bullet (`:492`) still says "A green log counts as CI green on either machine", and it still names `botsterq` on
  the Mac. 23c says that a Mac run covers only macOS-only code, so a Mac log is not a full gate and is not the merging gate.
- The "Heavy work" bullet (`:494`) still says `botsterq`. The pool bullet (`:497`) overrides it ("where the bullets above
  name them, read the pool gate"), but the 23c changelog says the `botsterq` text is removed.

Correct the first line, so that only a Linux pool gate is the merging gate. Then remove or point the others to `:497`.

### Checked, no finding

- 23b covers the four base files that `lists` reads (`core-pending.txt`, `core-deferred.toml`, `core-ledger-ids.txt`,
  the `Cargo.toml` pin). The pin exemption (`new_in_ledger`) and rule 2 (`tag_moved`) are then judged at the same point.
- 23c (1) is the right root fix for #181: a closed set of forms with a fail-closed refusal ends the chain of
  inference findings. `#[path]` on an inline module is refused as an unlisted form.
- 23c (2): Mac runs through `botster-gate --on mac`, only for macOS-only code (R8), agree with the pool bullet.

VERDICT: NOT CLEAN at def1d25e375c7c7615c329573986fe4692ef3c18 (4 open: P23b-1 MEDIUM; P23b-2, P23c-1, P23c-2 LOW)

## Round 5 — revision 23d (def1d25e..9b7bc5a0) — CLEAN

Reviewed: `stage1/plan` `9b7bc5a065c62943aaa03f6bb8860f2cfb6b4c9f`, two commits on `def1d25e` (`c7c39437`, and `9b7bc5a0`, which
only removes a pool time-limit claim from mutation step 8).

- **P23b-1 closed.** The Merge bullet now says: the head contains the current v1 tip; the gate's `BOTSTER_CI_BASE_REF`
  is that tip; `lists` prints the base, the merge base and whether the base is an ancestor of `HEAD`; the lead refuses a
  merge when the base is not an ancestor or v1 moved after the gate started. With the base an ancestor of `HEAD`, the
  merge base is the base, so the round 4 line-move case cannot reach v1.
- **P23b-2 closed.** `git merge-base --all <base> HEAD`, and the check fails on more than one commit. The initialization
  decision stays at the base revision.
- **P23c-1 closed.** The rule names every check that parses Rust source (`process_check`, `platform_code`,
  `mutants_cited`, `gate_decisions`, `timers` and every later one). An I/O shell exclusion cites at least one proof in the
  `decision (proof, ...)` form, and a proof named only in free text fails.
- **P23c-2 closed.** THE GATE bullet: only a green Linux pool gate is the merging gate, and a Mac run adds evidence for
  macOS-only code only. `botsterq` remains only in notes that say it is retired (`:409`, `:472`, `:493`, `:497`).

VERDICT: CLEAN (0 open) at 9b7bc5a065c62943aaa03f6bb8860f2cfb6b4c9f

## Round 6 — revision 23e — CLEAN

Reviewer: integration reviewer (Astra), `sess-1791575341-0172-bec01ec06119f11c948f14371195fa92`.

Reviewed head: `7a6adf84102a6bfcd5f69c631a16fe40e916257c`.
The complete revision changes `docs/stage1-plan.md` from revision 23d at
`9b7bc5a065c62943aaa03f6bb8860f2cfb6b4c9f`. The lead assigned this review on 2026-10-09.

- Section 6.3 assigns the integration review to Astra (`agent_name: codex`).
  The assignment agrees with the user's instruction and the lead's introduction.
- The trial states its failure condition: an in-scope finding that a package reviewer later catches moves the seat to Sol.
- Package reviewers remain Sol at high effort. HIGH changes still require both reviews.
- Revision row 23e records the staffing decision. The revision changes no product code, gate rule, pin, or package boundary.
- The full diff against the verified revision 23d pin contains only the staffing paragraph and revision row.
  `git diff --check` passes. This staffing-only plan review needs no gate, as recorded for this verdict file.

VERDICT: CLEAN (0 open) at 7a6adf84102a6bfcd5f69c631a16fe40e916257c

## Round 7 — revision 23f — CLEAN

Reviewed head: `ae89651169810dce96a1c54ab862db11447283a7`.
Previous reviewed head: `7a6adf84102a6bfcd5f69c631a16fe40e916257c`.
The lead assigned this plan review on 2026-10-09.
The complete delta adds the section 8 threat model and the revision 23f row.
The reviewer used the orchestrate-delivery skill to check the changed premise against the existing acceptance rules.
The reviewer changed no product code and ran no tests, builds, mutants, or gates.

### Acceptance rules

The plan explicitly limits the source-reading checks to honest drift and mistakes in reviewed code.
It does not require compiler-level resistance to deliberate evasion.
The existing closed-form rule still requires a named failure for unsupported forms.
The rule against excluding gate decisions remains in force.

The reserved-name rule provides a mechanical refusal for the reported crate, type, and macro shadowing cases.
R6-1's local `anyhow` module and internal macro glob fall under that rule.
R6-2's local `Command` type falls under that rule.
R6-3's quoted binding requires the shared-list rule: binding collection must skip the same opaque tokens as the expression index.
The opaque-token case does not require an actual runtime declaration of a reserved name outside the quotation.
Its closure therefore depends on consistent opaque handling, not merely on a reserved-name scan.

The plan requires rejection fixtures and shared lists within each check.
Those requirements are implementation acceptance conditions, not evidence that the current #181 code satisfies them.
The #181 verdict at `60a80f21d9e0da84deb47f471faddd4944b00ed7` remains NOT CLEAN pending a replacement review.

### Ordinary mistakes remain in scope

The reviewer checked an ordinary refactor case that declares no reserved name:

```rust
fn forwarded(code: Option<i32>) -> Result<()> {
    let _load = || std::fs::read("config");
    mutation_verdict(code)
}
```

The closure is never called, so its construction performs no file I/O.
The current #181 visitor enters its body under `forwarded` and can treat the function as an I/O shell.
With the existing tested decision citation, that classification can accept the pure forwarding function's exclusion.
Moving an immediate read into an uncalled loader is an ordinary refactor mistake, without reserved-name shadowing.
Revision 23f's good-faith exception therefore keeps this case within blocking review scope.
It does not invalidate the plan's stated boundary; the replacement implementation must respect that boundary.
Rejecting an unsupported deferred body with the form and file is sufficient under the existing closed-form rule.

The reviewer sent this source observation directly to P6 and its reviewer for the replacement READY.
The reviewer did not execute a fixture or issue a new #181 verdict before that READY.

### Compatibility and verification

The reviewer searched current v1 `a6555ebaf221042ca7b777ca2f2425e4a63dd960` for declarations using the relevant reserved names.
The search covered the current gate-decision crate roots, `Command`, argument and opaque macro names, and process-check type names.
No required declaration conflict was found in those forms.
Ordinary canonical imports remain distinct from the reserved rename and replacement declarations described in the plan.
The implementation must publish its actual lists and pass the full tree; this source survey does not replace that gate.

The remote plan ref still names the reviewed head. `git diff --check` passes.
The delta changes no product source, contracts pin, pending list, package owner, or gate command.
No gate was run for this documentation-only review.
The next #181 review will apply revision 23f to the actual correction and its evidence.

VERDICT: CLEAN (0 open) at ae89651169810dce96a1c54ab862db11447283a7

## Round 8 — revision 23g — CLEAN

Reviewed head: `6dece66f00144bd36e627dac739699c4579e0dd0`.
Previous reviewed head: `ae89651169810dce96a1c54ab862db11447283a7`.
The lead assigned this review and reported the user's approval of the changed policy.
The complete delta adds one section 8 paragraph and the revision 23g row.
The reviewer applied orchestrate-delivery to the changed acceptance premise.
The reviewer changed no product code and ran no tests, builds, mutants, or gates.

### Changed responsibility

The plan removes automatic classification of whether a function performs I/O.
Review now establishes that an excluded whole-body function is an I/O shell and forwards to the cited decision.
The strict `decision (proof, ...)` citation remains required, with at least one proof that a gate tier runs.
Every `.cargo/mutants.toml` change becomes HIGH and requires both package and integration review.

This replaces the failed classification mechanism with a defined review responsibility.
It does not allow a gate decision to be excluded.
The existing prohibition on excluding gate decisions and the remaining decision checks stay in force.
The remaining source-reading checks retain the closed-form and reserved-name rules for the names they resolve.
The explicit 23g paragraph supersedes the earlier 23f claim that the check detects a function that stops doing I/O.

### Implementation acceptance

R7-1 and B5 can close by removal of the mechanism that produces their false I/O evidence.
Their round-7 verdict remains an accurate record for `7826ba0adab5962089a7e3279f78cd5f4a8425c9`.
This plan verdict does not close them on that implementation head or grant #181 CLEAN.

The next #181 review must verify the actual removal and its callers.
It must verify that whole-body exclusions still require strict proof citations and cannot cover gate decisions.
The remaining checks must still reject unsupported forms under 23f.
The next #184 review must verify that `ci/high-tier-paths.txt` includes `.cargo/mutants.toml`.
The #181/#184 union review remains required for the second PR to merge.
Until that path rule lands, the explicit plan rule still requires HIGH review for every change to the file.

The plan changes no contract pin, pending id, package owner, gate command, or exact-head merge requirement.
The remote plan ref names the reviewed head. `git diff --check` passes.
No gate was run for this documentation-only review.

VERDICT: CLEAN (0 open) at 6dece66f00144bd36e627dac739699c4579e0dd0
