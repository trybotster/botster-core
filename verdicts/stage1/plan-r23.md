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

## Round 9 — revision 23h — CLEAN

Reviewed head: `93fd39704e27fdf596127d0ab655ef6b88da1ead`.
Previous reviewed head: `6dece66f00144bd36e627dac739699c4579e0dd0`.
The review covers both `676d4cc2` and `93fd3970`, as the lead requested.
The complete delta changes section 6.3, section 11 step 3, and the revision table.
The reviewer applied orchestrate-delivery to the changed staffing and dependency order.
The reviewer changed no product code and ran no tests, builds, mutants, or gates.

The P3 pair completes `th_1`, then takes P4a's minimum work from current `v1`.
This assignment uses an existing pair and preserves the three-pair cap.
The P3 handoff records the probe that found `th_1` independent of P4a and real-process work.
It also records the route dependency for the additional event, output, timing, and attach ids.
The plan names P4a's 18 minimum ids and six additional dependent minimum ids as the reason for the move.
This dependency count does not claim that those ids already pass.

The second commit replaces the first commit's automatic assignment of a fourth pair to non-minimum work.
P3's non-minimum queue remains on hold until minimum Core is complete.
The fourth pair requires a named minimum id that lacks staff, and the lead must name that id to the orchestrator.
The plan records the shared usage limit. This review does not start or delegate work to another pair.
The section 6.1 distinction between packages and pairs preserves package responsibilities when staffing changes.

The change retains the HOLD on new real-process test code until #181 lands.
P6 still owns the shared process crate and the real-harness work.
A staffing change supplies no missing real-process proof and permits no early merge of a change that requires one.
The existing HIGH review rules, exact-head evidence, minimum acceptance conditions, and section 8 checks remain in force.
No contract pin, pending id, production interface, or gate command changes.

The fetched plan ref names the reviewed head. `git diff --check` passes.
The fetched `v1` contains #200 at `3fa51cd2d148315883002af96495b3096242ba42`, consistent with the lead's merge notice.
The earlier 23h head receives no separate CLEAN because the second commit supersedes its fourth-pair assignment.

VERDICT: CLEAN (0 open) at 93fd39704e27fdf596127d0ab655ef6b88da1ead


## Round 10 — revision 23i — CLEAN

Reviewed head: `5e0a7500fa3f150d9f4a2ae6ba7a7b69e116ef88`.
Previous reviewed head: `93fd39704e27fdf596127d0ab655ef6b88da1ead`.
The lead assigned the complete two-commit delta. It adds the revision 23i paragraph and its revision row.
The reviewer applied orchestrate-delivery to the changed acceptance premise.
The reviewer read the complete delta and section 8. The reviewer changed no product code and ran no tests, builds, or gates.

### Changed responsibility

The gate no longer infers that one function calls or forwards to another function.
Review must establish that an excluded shell forwards to the cited decision.
This extends revision 23g's explicit review responsibility to the call relation that used the same invalid syntax inference.

The remaining mechanical contract is explicit:

- The exclusion must use the strict citation form.
- The cited decision name must name at least one xtask function.
- No exclusion may cover any xtask function with that name.
- Each cited proof must be a test selected by a gate tier.

A name collision can therefore reject an exclusion; it cannot supply evidence that an unrelated function calls the cited decision.
The gate-decision exclusion ban stays in force. Every `.cargo/mutants.toml` change remains HIGH.
Both package and integration reviewers retain responsibility for each new or changed exclusion.
The other checks retain their closed-form and reserved-name rules.

### Implementation acceptance and evidence scope

The next #181 review must verify removal of the call-relation inference and its acceptance callers.
It must verify both the cited-name existence rule and rejection when any same-named xtask function is excluded.
It must retain strict proof selection and the other source checks.
This plan verdict grants no CLEAN verdict to #181 and supplies no implementation proof.
P6 withdrew the `a94e02d6` request while it prepares the replacement head and exact-head gate.

The statement that mutation tests prove the decision is tested remains subject to section 8's actual mutation scope.
The gate uses `--in-diff`; a passing run proves only its executed mutants, not arbitrary unchanged decisions.
The review must continue to inspect the cited test body and the actual mutation outcomes.
The revision changes no mutation command, evidence requirement, or merge condition.

No contract pin, pending id, package owner, product interface, or gate command changes.
The fetched plan ref names the reviewed head. `git diff --check` passes.
No gate was run for this documentation-only plan review.

VERDICT: CLEAN (0 open) at 5e0a7500fa3f150d9f4a2ae6ba7a7b69e116ef88

## Round 11 — revision 23j — NOT CLEAN

Reviewed head: `83b2afcc1cc7a785e1459eb65a5820f9e1f1d169`.
Previous reviewed head: `5e0a7500fa3f150d9f4a2ae6ba7a7b69e116ef88`.
The reviewer read the complete one-commit, three-file delta and the affected plan sections.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

### R11-1 — MEDIUM — The fixed inputs do not include the stated manifest and ruling

Section 0 now names contracts-v0.1.21 and final36, but its Joint manifest row still pins final35.
That row excludes the A18 amendment that this revision adds to the work lists.
Update the current manifest pin to final36, commit `01814c0e1e34ef4336bb62a2d8e9862dcd740cd8`, and retain final35 as history.

The same section adds R-42 but instructs implementers to read only its fixed revisions.
R-42 is absent from contracts-v0.1.21 at `60a4169978e3f704f46ab0578f9993013fd4b810`.
The reviewer read R-42 in contracts origin/main at `c2df50c3b6f7721cfc314cb1052d66915d88124e`.
Name a fixed revision containing R-42 for the ruling input. This does not require another crate pin move.
The short R-41 and R-42 descriptions match the rulings.

### R11-2 — LOW — The package count and active total exclude the new A18 IDs

Section 6.1 still lists 55 IDs for P5 and 673 active IDs at the pinned tag.
The generated lists contain 59 P5 IDs and 677 active IDs.
Update the P5 count and total. Add the four A18 IDs to the total's amendment arithmetic.
These are required IDs, not passing IDs. The minimum list remains unchanged.

### Completed checks

The reviewer compared every generated package list with the pinned ledger, pending list, withdrawals, and unchanged owners.py rules.
Every list matches exactly, including each clause and transcript status.
The four new A18 IDs belong to P5 and remain pending without transcripts.
The two P4a status changes correctly record transcript availability. They do not claim that Core passes either transcript.
The minimum list is byte-identical to the previous plan revision.
The complete delta changes no product code, gate command, or acceptance requirement.
`git diff --check` passes. No execution gate applies to this documentation-only delta.

VERDICT: NOT CLEAN (2 open) at 83b2afcc1cc7a785e1459eb65a5820f9e1f1d169

## Round 12 — revision 23j correction — CLEAN

Reviewed head: `96c5537660230687c645198894b574caa186622b`.
Previous reviewed head: `83b2afcc1cc7a785e1459eb65a5820f9e1f1d169`.
The reviewer read the complete one-file correction and the cited ruling sources.
The reviewer changed no product code and ran no builds, tests, mutation jobs, or gates.

R11-1 is closed. The Joint manifest row now pins final36 at `01814c0e1e34ef4336bb62a2d8e9862dcd740cd8`.
The row retains final35 as history.
R-41 has a fixed source at `c2a04f8`; R-42 has a fixed source at `c2df50c`.
Both sources contain the named ruling. The latter also contains the corrected impostor_worker control description.
The fixed sources resolve the input ambiguity without another crate pin change.

R11-2 is closed. Section 6.1 now lists P5 with 59 IDs and the active total as 677.
The total's arithmetic includes the four A18 IDs.
Section 1 also states 677 active IDs out of 679, after two withdrawals. Risk R4 uses 677.
The clause lists, owner rules, and minimum list remain unchanged from Round 11's verified data.

The correction changes no acceptance requirement or gate command.
The fetched plan ref names the reviewed head. `git diff --check` passes.
No execution gate applies to this documentation-only correction.

VERDICT: CLEAN (0 open) at 96c5537660230687c645198894b574caa186622b

## Round 13 — revision 23k — NOT CLEAN

Reviewed head: `193cb3bcee4abbbd887fc534f71f4ad24c39cd31`.
Previous reviewed head: `96c5537660230687c645198894b574caa186622b`.
The reviewer read the complete one-commit, three-file delta and the affected acceptance text.
The reviewer applied orchestrate-delivery to the changed minimum acceptance scope.
The reviewer changed no product code and ran no builds, tests, mutants, or gates.

### Acceptance premise

Pinned Core A10-3 item 2 requires `conf::ad_2_lost_reasons` to cover every listed Lost reason through the testkit.
It explicitly includes GuardianLost and EpochExhausted, which require service behavior.
The approved minimum configuration starts no services. A partial transcript result cannot count as this ID passing.
Removing this ID from the minimum therefore preserves that configuration without weakening the full Stage 1 requirement.
The ID remains owned by P5 in its full clause list. This revision changes no product pending list or contract ledger.

The minimum list removes exactly this one ID. It contains 69 IDs with P5 reduced from 15 to 14.
The other package counts are unchanged and sum to 69.
The pinned A6-1 transcript includes a WorkerGone outcome after restart, as well as the distinct WorkerUnreachable outcome.
The minimum also retains the AD-6 reused-pid proof. Removing AD-2 does not remove all WorkerGone acceptance coverage.
The plan retains separate testkit and real-process passing counts and the full Stage 1 done criteria.

### R13-1 — LOW — Two active statements retain the old minimum counts

`docs/stage1-minimum-core.md:137` labels Step 6 as 14 IDs, but only 13 rows remain active after the AD-2 removal.
Change the heading to 13, retaining the struck-through row as the removal record.

At line 299, section 7 still says that milestones report passing IDs out of 70 "From now on".
Update that instruction to 69, or label it explicitly as superseded history.
The unchanged historical ledger snapshot and its earlier pass counts need not be rewritten as current evidence.
These corrections will align the detailed document with the new list and the plan's revised progress measure.

The fetched plan ref names the reviewed head. `git diff --check` passes.
No execution gate applies to this documentation-only change.

VERDICT: NOT CLEAN (1 open: R13-1 LOW) at 193cb3bcee4abbbd887fc534f71f4ad24c39cd31

## Round 14 — revision 23k correction — CLEAN

Reviewed head: `2cec1a39f65b86798bea993c1bbcd8015cabaa7c`.
Previous reviewed head: `193cb3bcee4abbbd887fc534f71f4ad24c39cd31`.
The reviewer read the complete two-line correction in the minimum Core document.
R13-1 closes: Step 6 now names 13 active IDs and labels 14 as the earlier count.
The progress instruction now names separate testkit-passing and real-passing counts out of 69.
It identifies the earlier denominator of 70 as history.

Round 13's acceptance checks remain valid. The minimum list and other plan text are unchanged.
The full Stage 1 requirement remains intact; this revision changes only the service-free minimum scope.
The fetched plan ref names the reviewed head. `git diff --check` passes.
The reviewer changed no product code and ran no builds, tests, mutants, or gates.

VERDICT: CLEAN (0 open) at 2cec1a39f65b86798bea993c1bbcd8015cabaa7c

## Round 15 — revision 23l — NOT CLEAN

Reviewed head: `0132ceff1638f7d1975d89c8d0ec834f694b0d1b`.
Previous reviewed head: `2cec1a39f65b86798bea993c1bbcd8015cabaa7c`.
The lead assigned both commits, including the shared composition function added after `714d3c1f`.
The reviewer applied orchestrate-delivery to the changed real-tier architecture and acceptance rules.
The reviewer read the complete delta, related plan sections, steward R-43 at contracts `95b96c9`, and current production callers.
Production source was read at v1 `9b255cff007552f3952563852104f00a14ddf842`.
The reviewer changed no product code and ran no builds, tests, mutants, or gates.

### Sound parts of the premise

The harness can supply the transcript clock without replacing real workers, PTYs, processes, sockets, or files.
R-43 assigns this clock to the harness as host under TM-1.
The existing Core facade forwards CoreApi operations to the production HostDriver.
Sharing an `open_parts` function with Core::open can preserve production validation, registry locking, host epoch, features, and terminal configuration.
It avoids a separate copy of the production composition logic in the harness.
Exporting that production composition and its edges does not itself create a test branch.

The wrapper must forward ordinary operations to the production edges and apply controls only at the specified boundary.
A pass-through proof establishes that exercised composition; it does not establish every future wrapper or control automatically.
The implementation review must still inspect the shared caller and each control's real resource ownership.
The proposed Limits::real use remains conditional on a contracts pin that supplies it.
No probe count in this plan is accepted as completed real-tier gate evidence by this review.

### R15-1 — HIGH — The stated exports do not expose the resources required by the controls

Current RealEdges owns its listener and streams privately (`crates/botster-core/src/real.rs:267-281`).
Its production accept path inserts each socket into a private LinkIo and returns only LinkId.
HostEdges exposes receive, send, close, and interest operations, but no socket handle or descriptor-readiness query.
The proposed `open_parts` returns this same aggregate; exporting the aggregate does not expose its contained resources.

An external wrapper therefore cannot perform the plan's `shutdown(SHUT_RDWR)` on the owned socket through the stated exports.
It also cannot perform the specified non-consuming descriptor-readiness check for edges_quiet.
The sentence permitting these exports and "Nothing else" leaves the required control boundary unavailable.

Specify a production resource boundary that gives the wrapper the required access while preserving ownership and lifetime.
Keep the controls outside Core and keep production and harness composition shared.
Do not substitute a Core test control, a copied real-edge implementation, or a feature-selected test path.
This is a missing architectural boundary, not a request to implement the controls during plan review.

### R15-2 — MEDIUM — Real-tier failure and selection rules conflict across sections

Section 5 still permits a real-tier failure to return an ID to core-pending.txt through a HIGH PR.
Section 8 now assigns testkit-passing, real-failing IDs to core-real-pending.txt and restricts later additions.
Section 4.2b also says a non-pass is never moved to a list.
These rules do not tell the implementer which list governs initialization, later regressions, or trial selection.

Reconcile those active instructions with the new second list.
Specify how the real harness reports and selects IDs in core-real-pending.txt without counting them as passes.
Keep already passing real IDs protected against later regression; list absence alone must not replace required proof.

The new shorthand exempts real-only IDs from the real tier, while section 4.2b requires their real-process proof.
State explicitly that such an ID can avoid duplicate transcript execution only through its required named real-process proof.
It is not exempt from real acceptance and must not count as passing merely because both pending lists omit it.
Validated deferrals retain their separate status and never count as passes.

The fetched plan ref names the reviewed head. `git diff --check` passes.
No execution gate applies to this documentation-only review.

VERDICT: NOT CLEAN (2 open: R15-1 HIGH, R15-2 MEDIUM) at 0132ceff1638f7d1975d89c8d0ec834f694b0d1b

## Round 16 — revision 23l resource boundary correction

Reviewed head: `6f80686aa1963164c6069b152c0fcf7b933bfff7`.
Scope: the complete delta from round 15 head `0132ceff1638f7d1975d89c8d0ec834f694b0d1b`.

R15-2 is closed. Sections 4.2b, 5, and 8 now agree on real-tier selection, initialization, reporting, and regressions.
The named real-process proof remains required for every applicable slow ID.

R15-1 remains open. The proposed `link_close` control and shared production composition resolve the missing socket access.
However, the proposed quiet check treats `connect_worker(instance)` as a passive input operation.
HostEdges documents it as an active adoption connection (`crates/botster-core-host/src/driver.rs:73-79` at `9b255cff`).
The driver calls it only for `Action::ConnectWorker` (`driver.rs:309-314`).
A quiet check that calls it can create a connection that the engine did not request.
Remove this operation from take-ahead and quiet probes. Forward it only when the driver requests it.

The driver also does not drain every input on every pump.
Its link reader stops for budget limits or held input (`driver.rs:452-503`).
Correct that premise. A wrapper must bound retained input and report non-quiet while that input waits.
The pass-through proof must preserve bytes and order when the driver cannot accept more input.

`git diff --check` passes. No execution gate applies to this documentation-only review.

VERDICT: NOT CLEAN (1 open: R15-1 HIGH) at 6f80686aa1963164c6069b152c0fcf7b933bfff7

## Round 17 — revision 23l corrected quiet check

Reviewed head: `a538765fe9e3a8243bad5acf2a44619510c34edc`.
Scope: the complete delta from round 16 head `6f80686aa1963164c6069b152c0fcf7b933bfff7`.

R15-1 is closed. The wrapper forwards connect_worker only when the driver requests an adoption connection.
The quiet check never calls that operation.
The plan now accounts for budget limits and held input.
The wrapper bounds retained input and reports non-quiet until the driver takes that input.
The plan also requires a proof that later operations on a closed LinkId cannot reach a reused descriptor.
R15-2 remains closed.

The shared production composition, wrapper behavior, and required proofs remain implementation review obligations.
This plan review does not count any real-tier transcript as passing.
`git diff --check` passes. No execution gate applies to this documentation-only review.

VERDICT: CLEAN at a538765fe9e3a8243bad5acf2a44619510c34edc

## Round 18 — Revision 23m pin catch-up

Reviewed head: `b186dd122f469c19fc31b96864037680240e6cab`.
Parent: `a538765fe9e3a8243bad5acf2a44619510c34edc`.
Scope: the complete four-file delta, affected plan instructions, and ledger/list consistency at contracts-v0.1.22.
The reviewer applied orchestrate-delivery to the changed transport scope.

The contracts tag resolves to `af5771cf962eb26b7074d486dafbb3360ff50d01`.
The active Core ledger contains 665 IDs, down from 677: 15 withdrawals and three additions.
The withdrawals remove 11 P4c IDs and four P4a IDs. None belongs to the minimum list.
All package lists together cover the active ledger exactly once. Every clause and pending/transcript field matches the tag.
Package counts match: P0 2, P1 126, P2 6, P3 167, P4a 151, P4b 68, P4c 4, P5 59, P6 15, P7 67.
The new A17 rule correctly assigns all three new pending IDs to P4a.
The 69-ID minimum list is byte-identical to its previous version.
The four remaining P4c IDs cover DP-10 T1 and DP-11. The plan retains them and defers staffing until real minimum acceptance.

### R18-1 — MEDIUM — Active architecture instructions still require WebRTC work

Section 2 still gives the worker driver WebRTC UDP sockets (line 113).
The RouteTransport row still specifies real UDP sockets and testkit datagram pairs (line 130).
The Entropy row still names a WebRTC stack (line 135).
The crate tree still includes worker-core/webrtc (line 232).
The new-tools instruction still proposes str0m for adoption (line 460).
These active instructions contradict A17 and the revised scope.
Remove those responsibilities or mark each as superseded history. Keep the stream route and ordinary process edges.
The explicitly withdrawn entropy item, str0m table row, and closed R6 may retain historical text.
Q9 must also describe the current four-ID P4c scope instead of saying that P4c awaits A17.

### R18-2 — MEDIUM — The frozen-input table still selects the previous manifest and codec

The contracts row advances to final37, but the Joint manifest row still selects manifest-final36 at line 14.
The Route codec row still selects revision 36 and its hash at line 35.
The final37 manifest explicitly replaces that codec with revision 37.
Update both active input rows. Keep the previous revisions as labeled history.
The manifest-final37 tag resolves to `a9c44c69819a5ef54cdd43d1066902feb9760027`.
The revision 37 hash in that manifest is `30e254d64999d3bb8285ac683e0b8a6428d36f4cd577a1dd0b8bbedb25cb2e9a`.

### R18-3 — LOW — Active acceptance and budget counts still use 677 IDs

Section 1 line 59 still requires all 677 active IDs at the pinned tag and describes the old 679-minus-two calculation.
Risk R4 at line 527 also retains 677 as its active count.
Update those active counts to match the 665-ID ledger. Preserve explicitly historical counts.

`git diff --check` passes. The reviewer used read-only ledger and file comparisons.
The reviewer ran no builds, tests, or gates. The separate code pin PR remains outside this review.

VERDICT: NOT CLEAN (R18-1 MEDIUM, R18-2 MEDIUM, R18-3 LOW) at b186dd122f469c19fc31b96864037680240e6cab

## Round 19 — Revision 23m corrected scope and inputs

Reviewed head: `e3d737395242bc7da8fe528da3c16f38a022d9a8`.
Scope: the complete three-commit correction from round 18 head `b186dd122f469c19fc31b96864037680240e6cab`.

R18-1 is closed. The worker, RouteTransport, Entropy, crate-tree, and proposed-tools instructions no longer assign WebRTC work to Core.
Q9 retains the four performance IDs and their real-minimum staffing condition.
The A7 input row explicitly identifies the withdrawn AttachWebRtc and SCTP parts.
Remaining WebRTC references describe the boundary, rejected tools, or explicitly withdrawn history.
The str0m interface analogy does not assign implementation work.

R18-2 is closed. The input table selects manifest-final37 and route codec revision 37.
The manifest tag resolves to `a9c44c69819a5ef54cdd43d1066902feb9760027`, an ancestor of contracts tag `af5771c`.
The reviewer hashed the codec file at that contracts tag. Its SHA-256 matches the plan:
`30e254d64999d3bb8285ac683e0b8a6428d36f4cd577a1dd0b8bbedb25cb2e9a`.
Section 7.2 also names codec revision 37.

R18-3 is closed. Section 1 and R4 now use 665 active IDs. Historical counts remain labeled as history.
The previously verified clause lists and 69-ID minimum list do not change in this correction.
The separate code pin PR still requires its own review and execution evidence.

`git diff --check` passes. No execution gate applies to this documentation-only correction.
The reviewer ran no builds, tests, or gates. No finding remains open.

VERDICT: CLEAN at e3d737395242bc7da8fe528da3c16f38a022d9a8

## Round 20 — Revision 23n mutation stages

Reviewed head: `3453f5a6c6397abf4464c259dc675472025c7d54`.
Scope: the complete one-commit delta from `e3d737395242bc7da8fe528da3c16f38a022d9a8`.

The reviewer applied the orchestrate-delivery premise and evidence checks.
The new rule correctly distinguishes the default mutation run from tests compiled with the slow feature.
Current xtask source explicitly selects the mutants profile and passes no slow feature.
The #211 log confirms that the extra environment-only invocation repeats the default mutation result.
The plan requires a slow-only catch proof, a both-stages miss proof, and failure on a timeout in either stage.
The existing rule requires profiles without terminate-after. The two-stage design does not contradict that invariant.
P6 owns the HIGH gate PR after the RealCoreHarness PR. Exclusions remain until individual gate proofs replace them.

### R20-1 — MEDIUM — State the current manual evidence command explicitly

The old timeout bullet still describes the pre-#181 instruction, conditioned on a profile that has already landed.
The new final sentence calls its NEXTEST_PROFILE=slow READY run the manual features run.
The preceding diagnosis correctly explains why that environment-only xtask command does not select slow mutation tests.
The instructions therefore leave two different commands under the same name.

Mark the old interim instruction as historical and state the current manual evidence requirements explicitly.
The run must enable the slow feature, bypass the exclusions under review, and select the slow nextest profile explicitly.
Preserve the no-terminate-after rule and the existing first-failure behavior.
An environment-only xtask invocation must not qualify as this manual evidence.

### R20-2 — LOW — Limit the worker gap to the prebuilt execution path

The statement that a real-process test does not see a worker mutant is too broad.
The #210 slow_driver fixture executes the changed Driver in its test binary, in a real child process.
Its supplied slow mutation run catches worker mutants through that path.
The gap applies to tests that execute the unchanged target/candidate worker.
State that scope explicitly. Keep the requirement to document or close that gap in the gate PR.

Both findings were sent to the lead. They require plan text corrections, not a larger implementation scope.
The reviewer ran no builds, tests, or gates.

VERDICT: NOT CLEAN (R20-1 MEDIUM, R20-2 LOW) at 3453f5a6c6397abf4464c259dc675472025c7d54

## Round 21 — Revision 23n evidence instructions corrected

Reviewed head: `f545597b2213f0373a69696840dec560e708b284`.
Scope: the complete correction from `3453f5a6c6397abf4464c259dc675472025c7d54`.

R20-1 is closed. The timeout bullet records that #181 installed the mutants profile.
Its former READY instruction is historical.
The current manual evidence rule explicitly requires no configuration exclusions, in-place mutation, the slow feature, and the slow nextest profile.
It preserves the gate's first-failure setting and rejects an environment-only xtask run as slow evidence.
Both stages retain profiles without terminate-after. A timeout in either stage fails the step.

R20-2 is closed. The worker gap now applies to tests that execute target/candidate.
The text explicitly distinguishes the slow_driver fixture that runs its own changed Driver copy.
The implementation order, individual exclusion proofs, and P6 ownership remain unchanged.
This plan verdict does not certify the future gate implementation.

`git diff --check` passes. No execution gate applies to this documentation-only correction.
The reviewer ran no builds, tests, or gates. No finding remains open.

VERDICT: CLEAN at f545597b2213f0373a69696840dec560e708b284

## Round 22 — Revision 23o clock ownership and delivery order

Reviewed head: `0ec6d6b06bd0fa3d64991b58757fca685426ff92`.
Scope: the complete plan delta from `f545597b2213f0373a69696840dec560e708b284`, including its replacement drafts.

The reviewer applied the orchestrate-delivery premise and evidence checks.
The source at contracts `af5771c` confirms that `idle_wait` jumps an injected clock to Core's next deadline.
The real workers continue on their own clocks. The jump can therefore expire a host deadline before a worker responds.

The reviewer checked both named raw logs and the complete source delta between their heads:

- Injected clock: `019b277cb55cf271b895b946aca755211559ca49`, 21 passing and 100 failing real conformance trials.
  Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-core-019b277c-pool-20261009-182809-79811.log`.
- Real clock: `4775acfe0f15f5412b3ae730fc080258ed70b181`, 55 passing and 66 failing real conformance trials.
  Log: `~/botster-sessions/gates/botster-core-stage1-p6-real-core-clockprobe-4775acfe-pool-20261009-184010-16861.log`.

The heads differ only in `RealCoreHarness::injects_clock`, from true to false.
Both runs use `Limits::real()` and contain the same 121 real conformance IDs.
The real clock adds 34 passes. No ID passes only with the injected clock in these runs.
These are failing probes, not acceptance gates. Their commands differ: the second runs only prebuild and real conformance.
The final text corrects the earlier claim that the probes used the same head.
It also distinguishes assertion failures from timeouts before the target assertion for the four R-43(A) IDs.

R-46 replaces the initial proposal to use the real clock.
The lead supplied the steward's verbatim ruling in message `msg_plugin-w_1791597035_3312b5`.
The steward explicitly authorized immediate use. Publication in the rulings document remains pending.
The final plan agrees with that ruling:

- The real tier keeps the transcript's injected clock and exact time assertions.
- The driver jumps only when the harness controls every source of progress.
- On the real tier, explicit `advance_clock` steps move that clock. Idle waits use the wake handle and step limit.
- P6 measures the affected transcripts after the driver change. Transcript fixes put clock advances after the required real event.
- Worker timers retain their real-process proofs. The four R-43(A) IDs must pass rather than enter the real pending list.
- The contracts driver and transcript changes precede the tag, Core pin move, and RealCoreHarness PR.
- The real pending list is initialized once under the final clock rule.

R-46 also requires tests of both driver branches. The contracts review must check those tests and any remaining failing step.
The local path override supports measurement before the tag; it does not replace the tagged acceptance evidence.
This verdict approves the plan only. The driver, transcripts, pin move, and harness still require their own reviews and evidence.

`git diff --check` passes. No execution gate applies to this documentation-only change.
The reviewer ran no builds, tests, or gates. No finding remains open.

VERDICT: CLEAN (0 open) at 0ec6d6b06bd0fa3d64991b58757fca685426ff92

## Round 23 — Revision 23p contracts pin and ownership lists

Reviewed head: `a79803845973128942564dee79983923d458a5fa`.
Scope: both commits after `0ec6d6b06bd0fa3d64991b58757fca685426ff92`, including all four changed files.

The reviewer verified the tagged inputs:

- `contracts-v0.1.23` resolves to `fe3eb952d6f61b89cf7bcbe8c54ab079987ec002`.
- `manifest-final38` resolves to `9b1cf0fcde3d76794b850a0d313a351f17066948`.
- The frozen A19 candidate 2 file hashes to `14e2bac0f54a523a5323e27f171bce64615b6102908307586c51b07a814c123d`.

The pin and manifest descriptions agree with these inputs.
A19-1 replaces the attached-frame floor with the largest worst-case floor frame.
It also specifies the u64 decimal-string query identifier.
A19-2 checks every possible path-bearing frame before file creation.
The A9 row correctly records the replacement without changing the remaining A9 clauses.

The tag contains published R-44 through R-47 and the R-46 driver change with `progress_is_injected`.
The only changed Core transcript is `ou_2b_healthy_reasons_send_route_closed_last`.
It adds host pumping until the route-close events before reading the corresponding wire close frames.
The tag does not contain the R-46 clock-advance transcript fixes or frozen A20.
The plan distinguishes those remaining changes from the published ruling and A20 proposal.

The reviewer independently applied the revised ownership function to the tagged ledger and pending and withdrawn lists.
Every generated package list exactly matches the plan branch.
There are 671 active Core IDs, with no duplicate ID.
The six new A19-1 IDs belong to P4a, and the A19-2 path-frame ID belongs to P4b.
All seven are pending in the tag. The former attached-frame equality ID is withdrawn.
P4a has 156 IDs. P4b has 69. Every other package count is unchanged.
The plan's acceptance count, package total, and budget row all use 671.

The minimum list remains byte-identical and contains 69 IDs.
None of the seven new IDs or the withdrawn equality ID belongs to that list.
The code pin move remains P6's separate HIGH PR and requires its own review and execution evidence.
This plan verdict does not approve that future implementation.

`git diff --check` passes. No execution gate applies to this documentation and ownership-list change.
The reviewer ran no builds, tests, or gates. No finding remains open.

VERDICT: CLEAN (0 open) at a79803845973128942564dee79983923d458a5fa


## Round 24 — Revision 23q contracts pin and A20 exception

Reviewed head: `9512a9af320474f23a44b5518b5ced67a197c6b0`.
Scope: the complete three-file delta after `a79803845973128942564dee79983923d458a5fa`.

The reviewer read the orchestrate-delivery skill and its Botster lessons for the changed acceptance premise.
The frozen A20 text permits one named testkit-only proof at the worker process allocator boundary.
That class has only `conf::dp_3_frame_limit_checked_before_allocation`.
A new member requires a Core amendment.
The real tier reports the class as testkit-proven and does not execute it.
The new plan paragraph follows that rule and keeps these IDs out of `core-real-pending.txt`.

### Pin and ownership checks

- `contracts-v0.1.24` resolves to `d79aed5e84d4a8bc9b76f6163ca6bc4a9d89b951`.
- `manifest-final39` resolves to `4156e7118f7fcd09d1dcab8cff6c4f18b7fa8c6e`.
- The frozen A20 file hashes to `571acfe63c1de73e71e516aff554b894b1f4ef66d6ab5217d5c636cae16c53d0`.

All three match the plan.
The reviewer independently applied the revised ownership function to the tagged ledger and status lists.
All ten generated package lists match exactly.
The total is 672 active IDs. P6 owns 16 IDs, including the new A20 static-check ID.
The minimum list remains byte-identical with 69 IDs.
The minimum includes `dp_3_frame_limit_checked_before_allocation`.

The tagged source contains three stop-grace clock advances after Stopping events and the `in_9` assertion reorder.
The probe changes follow published R-48: terminal input disables canonical mode and echo; pipe input remains unchanged.
The tag includes the R-47 route-fill ruling, but no route-fill implementation.
The code pin move remains a separate HIGH PR.

### R24-1 — MEDIUM — real-tier selection and counts still include the A20 exception

The end of section 8 still says that every ID outside `core-pending.txt` and the deferred set runs on the real tier.
It also defines real-passing as minimum IDs in neither pending list.
The A20 ID is a minimum ID and must never enter `core-real-pending.txt`.
Thus, those definitions would execute an excluded ID or count its testkit proof as a real-tier pass.

Update trial selection to exclude the contract-derived A20 set.
Report its testkit-proven minimum count separately from actual real-tier passes.
If the plan needs a combined acceptance count, name that count explicitly.
The reviewer sent this finding to the lead.

The reviewer also requested the existing record for the new reference-tier claim of 54 to 67 passes out of 121, with no regression.
No new execution was requested.
`git diff --check` passes. The reviewer ran no builds, tests, or gates.

### R24-1 closure and final head

Final reviewed head: `bae71c81e0d9f1bd6fc291910f496be495414076`.
The reviewer read all corrections after `9512a9af`, including `docs/stage1-minimum-core.md`.
Section 8 now excludes the contract-derived A20 list from real-tier trials.
It never counts an A20 ID as real-passing.
Sections 1 and 8 and the minimum-Core document use distinct measures:

- Testkit-passing: 69 required IDs.
- Real-passing: 68 required IDs, excluding the one A20 minimum ID.
- Real-accepted: 69 required IDs, adding the A20 minimum ID only when its testkit proof passes.

Minimum acceptance requires 69 testkit-passing and 69 real-accepted IDs.
This preserves the full minimum while distinguishing actual process evidence from the authorized A20 proof.
R24-1 is closed.
The lead also corrected the revision-history table cell at the final head.

### Reference-run evidence

The reviewer read [contracts PR #22](https://github.com/trybotster/botster-contracts/pull/22) and its named existing Core probe logs:

- `~/botster-sessions/gates/botster-core-stage1-p6-real-core-r46probe-cde60ed1-pool-20261009-194110-48535.log`.
- `~/botster-sessions/gates/botster-core-stage1-p6-real-core-r46probe-99f643b7-pool-20261009-195613-20654.log`.
- `~/botster-sessions/gates/botster-core-stage1-p6-real-core-r46probe-a46b7958-pool-20261009-200839-70465.log`.

Each real run has 121 outcomes.
The raw per-ID results exactly match the corresponding saved outcomes files.
The real-pass counts are 54, 58, and 67.
The first transition changes four failures to passes. The second changes nine failures to passes.
Neither transition loses a prior pass.
The latter two runs also pass all 121 testkit trials.
The final probe log builds the conformance probe from `/work/probe-bin`.
The PR describes that copy and the later equivalent rewrite of the terminal-mode flag removal.
These are reference measurements, not acceptance gates for the future Core harness.
The real runs retain failures; the latter commands finish with an echo, so their outer zero exits do not establish passing suites.

The tag, manifest, frozen hash, ownership lists, and unchanged minimum checks remain valid at the final head.
`git diff --check` passes. No execution gate applies to this documentation and ownership change.
The reviewer ran no builds, tests, or gates.

VERDICT: CLEAN (0 open) at bae71c81e0d9f1bd6fc291910f496be495414076

## Round 25 — revision 23r — 2026-10-10

Reviewed head: `09ac30a747a9a8013da3d4956e4ac5ccf32ff893`.
Prior accepted head: `bae71c81e0d9f1bd6fc291910f496be495414076`.
Scope: the complete delta, which changes only `docs/stage1-plan.md`.

The staffing paragraph records the fourth pair's bounded program-controls assignment.
It names all four reported orchestrator conditions, retains package ownership, and requires a file split with P6.
It assigns a separate package reviewer and keeps integration review for the HIGH pull requests.
The shared brief agrees with the group and its stop condition.
I independently checked the group against #220's pending list: 32 ids, including the seven named minimum ids.
This plan review does not approve the unfinished #220 implementation or its evidence.

The mutation paragraph retains the existing two-stage acceptance rule and adds the execution design.
Both stages stay inside the gate and report separate times.
Stage 2 selects the in-diff misses, enables the slow features, and uses immediate fail-fast with the existing slow-tier deadline value.
Whole-file evidence runs remain separate pool jobs.
The second, duplicate gate command is removed only when 23n lands, with an instruction to all pairs.
The existing timeout failure rule and the prebuilt-worker coverage limitation remain explicit.
A later change to move required stage-2 work outside the gate still requires the lead's decision.
The revision-history row matches these two changes.

The remote plan head matches the reviewed revision. `git diff --check` passes.
No execution gate applies to this documentation-only revision.
I ran no builds, tests, or gates.

VERDICT: CLEAN (0 open) at 09ac30a747a9a8013da3d4956e4ac5ccf32ff893

## Round 26 — revision 23s — 2026-10-10

Reviewed head: `8a9727e5d6a089fec14e29069b0cdfd561fb096c`.
Prior accepted head: `09ac30a747a9a8013da3d4956e4ac5ccf32ff893`.
Scope: the complete delta in `docs/stage1-plan.md`, comprising the progress-count clarification and revision-history row.

A non-pending real-only minimum id now contributes to both progress counts through its named real-process proof.
The report must print the proof source, so the count does not imply that a TestkitHarness trial ran for that id.
Section 5 still requires the named proof to pass in the gate.
The change therefore resolves the unreachable testkit total without removing a proof requirement.
The minimum list, denominators, A20 rule, and acceptance threshold remain unchanged.
The revision-history row agrees with the new rule.

The remote plan head matches the reviewed revision. `git diff --check` passes.
No execution gate applies to this documentation-only revision.
I ran no builds, tests, or gates.

VERDICT: CLEAN (0 open) at 8a9727e5d6a089fec14e29069b0cdfd561fb096c

## Round 27 — revision 23t — 2026-10-10

Reviewed head: `23c132e06cbf32c8a5d9498da3c1c0db18ded5a9`.
Prior accepted head: `8a9727e5d6a089fec14e29069b0cdfd561fb096c`.
Scope: the complete delta in `docs/stage1-plan.md`: the contracts input, manifest description, and revision-history row.

The annotated tag `contracts-v0.1.25` resolves to commit `ff3405992fd3a76f324d3b683b1f1631c034f43c`.
The prior tag resolves to `d79aed5e84d4a8bc9b76f6163ca6bc4a9d89b951`.
The manifest, ledger, and withdrawn list are identical between the two tags.
Each tag has 690 Core ledger IDs and 18 withdrawn Core IDs, leaving 672 active Core IDs.
The withdrawn list has 27 entries across all contracts. It does not withdraw 27 Core IDs.
The existing clause lists therefore remain applicable.

The new input description identifies the failed-start route cases, quiet fences, adoption follow-up, route-fill control, and R-49 through R-51.
R-47 requires the progressing-reader fill before real acceptance. R-51 retains the input-before-live ID as real-pending because the real precondition is unavailable.
The plan preserves those limits and assigns the code pin change to P6's separate HIGH pull request.
This documentation review does not approve that code change or claim additional conformance passes.

The remote plan head matches the reviewed revision. `git diff --check` passes.
No execution gate applies to this documentation-only revision.
I ran no builds, tests, or gates.

VERDICT: CLEAN (0 open) at 23c132e06cbf32c8a5d9498da3c1c0db18ded5a9

## Round 28 — revision 23u — 2026-10-10

Reviewed head: `558a6c8dcb07a5c77ee6713b26bbec9cf20e9ffc`.
Prior accepted head: `23c132e06cbf32c8a5d9498da3c1c0db18ded5a9`.
Scope: the complete delta in `docs/stage1-plan.md`: the section 5 exception and revision-history row.

The exception requires all three conditions: the same commit changes the contracts pin, the id's transcript changes between the tags, and the new transcript fails.
It applies to `core-pending.txt` and, for a real-tier failure, `core-real-pending.txt`.
The plan requires a tested `lists` decision and a regression proof that fails when the fix is reverted.
The PR body must identify each added id, its transcript change, and the PR that will remove the id.
An added minimum id reduces the progress count, and the PR body must report that reduction.
The pin move remains HIGH. This exception does not change the acceptance requirement for empty pending lists.

I compared the named transcript between contracts commits `d79aed5e84d4a8bc9b76f6163ca6bc4a9d89b951` and `ff3405992fd3a76f324d3b683b1f1631c034f43c`.
The new transcript adds a failed-start case with an attached route and checks one `SessionLost` close plus the endpoint reason.
That change matches the stated R-50 premise and the #219 work under review.
This plan review does not establish a new runtime result or approve a specific pending-list addition.
The implementation review must verify every eligibility condition and its evidence at the pin-moving commit.

The remote plan head matches the reviewed revision. `git diff --check` passes.
No execution gate applies to this documentation-only revision.
I ran no builds, tests, or gates.

VERDICT: CLEAN (0 open) at 558a6c8dcb07a5c77ee6713b26bbec9cf20e9ffc

## Round 29 — revision 23v — 2026-10-10

Reviewed head: `596082d435ba30bfb0b6521e4566b2af6d97cbf1`.
Prior accepted head: `558a6c8dcb07a5c77ee6713b26bbec9cf20e9ffc`.
Scope: the complete three-file delta that points the plan to the canonical minimum list on `v1`.

PR #220 merged at `b41e88be535c7e1c62ddf272c4295a87b94f5685`.
Its `conformance/minimum-core.txt` is byte-identical to the list reviewed at `8e732787`.
I compared all non-comment rows with the prior plan copy: all 69 rows match, including their order, clauses, and owning packages.
All 69 ids are unique. The package counts match section 6.1.
The old copy's stale 70-id comment is removed with the copy; the new pointer correctly states 69 ids.

The plan, minimum rationale, and former list now identify the same canonical file.
No other tracked file at the reviewed plan revision references the former list path.
The change preserves the minimum membership, progress denominators, and acceptance conditions.
Changes to the canonical list require a HIGH Core PR.

The remote plan head and `v1` match the reviewed commits. `git diff --check` passes.
No execution gate applies to this documentation-only revision.
I ran no builds, tests, or gates.

VERDICT: CLEAN (0 open) at 596082d435ba30bfb0b6521e4566b2af6d97cbf1

## Round 30 — revision 23w — 2026-10-10

Reviewed head: `c0860d7e7e8e4dd269c69b979d72aecfcc18a724`.
Prior accepted head: `596082d435ba30bfb0b6521e4566b2af6d97cbf1`.
Scope: the complete delta in `docs/stage1-plan.md`: the contracts input, manifest description, and revision-history row.

The annotated tag `contracts-v0.1.26` resolves to `0ac061d225724f6f7fc7b2368f637ba2c9a6c2b6`.
Contracts PR #29 merged at that commit. The prior tag resolves to `ff3405992fd3a76f324d3b683b1f1631c034f43c`.
Only five checker source and test files differ between those tags.
The frozen files, schemas, transcripts, manifest, ledger, withdrawn lists, and pending lists do not change.
The previously verified 690 Core ledger ids and 18 withdrawn Core ids therefore still give 672 active Core ids.

The input description matches the hex lookup changes and the typed route output check.
The route driver still validates non-payload fields and compares the complete output bytes.
The schema proof pins the payload-only assumption used by its shorter validation representation.
This plan review does not replace the contracts code review or the separate HIGH Core pin review.

I read the saved timing script and `p3-contracts-c/c-measure-ab.log`.
The Mac runs use seeds 0–31 and report 0.75, 0.75, and 0.76 seconds with A+B+C.
Those measurements use contracts head `891188d4`, before the final equivalent-code cleanup in `0ac061d2`.
I inspected that cleanup. The measurements are supporting evidence, not an exact-tag Core gate.
The earlier pool measurements of 12.93–12.98 seconds and A+B at 2.10–2.11 seconds remain as recorded in the #223 review.
The plan's 87% profile figure is the lead's reported attribution; I did not independently run a profiler.
The Mac and pool figures do not establish a controlled cross-host speed ratio.

The plan directs P6 to move from v0.1.24 straight to v0.1.26 in a separate HIGH PR.
It does not change the two-second budget, use the held budget class, or waive the pin PR's gate.
The implementation review must verify the final combined head and its execution results.

The remote plan head matches the reviewed revision. `git diff --check` passes.
No execution gate applies to this documentation-only revision.
I ran no builds, tests, gates, timing jobs, or profiling jobs.

VERDICT: CLEAN (0 open) at c0860d7e7e8e4dd269c69b979d72aecfcc18a724

## Round 31 — revision 23x — 2026-10-10

Reviewer: Sol, successor to Astra under rule 23e.
Reviewed head: `c0b6f32ef7edaccf871051749399302604962be1`.
Prior accepted head: `c0860d7e7e8e4dd269c69b979d72aecfcc18a724`.
Scope: the complete two-hunk delta in `docs/stage1-plan.md`.

Section 6.3 records the seat change and the continuing verdict branch.
The revision row records the same decision.
The change applies rule 23e and agrees with the user's explicit handover instruction.
Astra remains available for handover questions only.
The change preserves package ownership, acceptance conditions, gate requirements, and the approved staffing exceptions.

The prior plan pin has the same bytes as the parent revision.
The remote plan head matches the reviewed head.
This documentation change requires no execution gate.
I ran no builds, tests, gates, mutation jobs, or reversal jobs.

VERDICT: CLEAN (0 open) at c0b6f32ef7edaccf871051749399302604962be1

## Round 32 — revision 23y — 2026-10-10

Reviewed head: `214684de9e15ea9fa54dbbeeacf2a2ebf2113153`.
Prior accepted head: `c0b6f32ef7edaccf871051749399302604962be1`.
Scope: the complete two-hunk delta in `docs/stage1-plan.md`.

### P23y-1 — LOW — The interim rule does not preserve the existing permitted additions

Section 5 says both pending files are "shrink-only with no exception" until the separate exception PR lands.
The revision row also says both files are shrink-only.
These sentences do not distinguish pin-move re-pending from the existing permitted additions.
Section 5 still permits new ledger ids in the pin-moving commit.
Section 8 still permits a real-pending addition when an id leaves core-pending or enters the ledger.
The option A ruling removes the new 23u exception from #226, not those existing rules.

Qualify both sentences to prohibit pin-move re-pending of existing ids until the separate PR lands.
State that the existing rules for new ledger ids and real-pending additions remain in force.
I sent the lead this clarification as a QUESTION.
Status: OPEN.

The separate HIGH PR, its dependency on #218, and the strict real-tier check address #226 R1-1.
The revision retains the required proof that a passing real-pending id fails the step.
This plan review does not review #218 or remove its user-approval hold.
`git diff --check` passes. This documentation change requires no execution gate.
I ran no builds, tests, gates, mutation jobs, or reversal jobs.

VERDICT: NOT CLEAN (1 open) at 214684de9e15ea9fa54dbbeeacf2a2ebf2113153

## Round 33 — revision 23y correction — 2026-10-10

Reviewed head: `65f78ee18a6d574ecdeba488dc97dbb8dee0e5cf`.
Prior reviewed head: `214684de9e15ea9fa54dbbeeacf2a2ebf2113153`.
Scope: both corrected sentences and the complete 23y delta from `c0b6f32ef7edaccf871051749399302604962be1`.

P23y-1 is CLOSED.
Both sentences now prohibit only the pin-move re-pending exception until its separate HIGH PR lands.
Both sentences preserve the existing new-ledger and core-pending to core-real-pending entry rules.
The correction agrees with the lead's clarified option A ruling.

The separate PR must follow #218 and enforce the required failing result on both tiers.
Its strict real-tier check retains the red-on-revert proof requirement.
The pin move can proceed independently after the exception code leaves #226 and its reviewers accept the replacement head.
This plan verdict does not close #226 R1-1 or release the user-approval hold on #218.

The remote plan head matches the reviewed head. `git diff --check` passes.
This documentation change requires no execution gate.
I ran no builds, tests, gates, mutation jobs, or reversal jobs.

VERDICT: CLEAN (0 open) at 65f78ee18a6d574ecdeba488dc97dbb8dee0e5cf

## Round 34 — revision 23z — 2026-10-10

Reviewed head: `515cf3b61c94800e00ac2489f76fdab707f47ee1`.
Prior accepted head: `65f78ee18a6d574ecdeba488dc97dbb8dee0e5cf`.
Scope: the complete two-hunk delta in `docs/stage1-plan.md`.

Section 6.3 records real-only production work, its owner, and its priority until it lands.
The first item assigns the DP-2 descriptor handoff to P4a, the P3 pair, at top priority.
It includes the host send, host copy closure, worker receive, PTY read budget, and socket route edge.
The revision row records the same assignment and tracking rule.
A PR that adds a real-only stub must include its tracking entry. A reviewer must reject an untracked stub.

I checked the producer and consumer at current v1 `3677b37e502497d5e63a91dfad92e5671bd19196`.
The host's `link_send_descriptor` always returns the endpoint with `DescriptorSendError::Failed`.
The worker reads control bytes with `Read::read` and provides no `Input::Descriptor`.
Its route binding, route write, route close, and PTY budget action branches do nothing.
The worker-core DESIGN.md "Real-only" section lists these same missing paths.
The new entry therefore includes both sides of the descriptor handoff and the remaining route driver work.

The revision preserves package ownership, pending rules, acceptance conditions, and staffing limits.
It changes no ledger id, contract pin, passing count, or required real-tier proof.
The program-controls pair retains its existing scope and shared-file agreement requirement.
This plan verdict accepts the work assignment and tracking rule. It does not accept an implementation or real-tier completion.

The prior plan pin equals the parent revision, SHA256 `b3dd30aa073d8332db34e4873830124506378a5f5556c3bae416341ff5c5c3d8`.
The remote plan head matches the reviewed head. `git diff --check` passes.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at 515cf3b61c94800e00ac2489f76fdab707f47ee1

## Round 35 — revision 23aa — 2026-10-10

Reviewed head: `5774a0a0954ac49f1cf4e992a75c21f468479f88`.
Prior accepted head: `515cf3b61c94800e00ac2489f76fdab707f47ee1`.
Scope: the complete two-hunk delta in `docs/stage1-plan.md`.

### P23aa-1 — MEDIUM — OPEN — The link-write failure claim does not match the current driver

Section 6.3 item 2 says Core uses `write(2)` without signal suppression on worker links.
It states that a host with default `SIGPIPE` handling dies when it writes to a gone worker.
The current driver and its pinned dependencies contradict that premise.

At v1 `3677b37e502497d5e63a91dfad92e5671bd19196`, `RealEdges::link_send` calls `mio::net::UnixStream::write`.
Cargo.lock pins Mio 1.2.3. Mio's stream contains `std::os::unix::net::UnixStream` and forwards `write` to that stream.
In the local Rust 1.97.0 source, `UnixStream::write` calls `send_with_flags(buf, MSG_NOSIGNAL)`.
The Linux branch imports `libc::MSG_NOSIGNAL`.
Mio's Unix `new_socket` also sets `SO_NOSIGPIPE` on macOS before returning a socket.
The connection path uses that constructor. The listener construction also uses that constructor.
Core therefore already receives suppression through its dependencies on the ordinary link-write path.

Primary source checked:

- `mio-1.2.3/src/net/uds/stream.rs`: the stream type and both `Write` implementations.
- `mio-1.2.3/src/sys/unix/uds/stream.rs`: `connect_addr` uses `new_socket`.
- `mio-1.2.3/src/sys/unix/net.rs`: `new_socket` sets `SO_NOSIGPIPE` on Apple systems.
- Local Rust 1.97.0 source documentation, `std/os/unix/net/stream.rs:675-677`: the Linux write uses `MSG_NOSIGNAL`.
- Local Rust 1.97.0 source documentation, `std/sys/net/connection/socket/unix.rs:100-103`: Apple socket construction sets `SO_NOSIGPIPE`.

The [Rust UnixStream source](https://raw.githubusercontent.com/rust-lang/rust/1.92.0/library/std/src/os/unix/net/stream.rs) also documents this suppression.
The current descriptor-send method is a typed failure stub. It performs no syscall and has no current signal exposure.
A future Linux `sendmsg` implementation must preserve suppression. That is a valid requirement for the descriptor work.

Replace the asserted existing link-write defect with the supported remaining scope.
The entry can require a real-process proof with default `SIGPIPE` handling and suppression in the future descriptor send.
If a current path is exposed, name its exact syscall and explain how it bypasses the dependency behavior.
Remove or correct the "section 0 known gap" reference: section 0 contains no SIGPIPE or A49 entry at this head.
Update the revision row to match the corrected scope.
I ran no process proof and make no observed host-death claim.

The owner, separate PR, order after DP-2, and non-minimum status do not change the stage's acceptance conditions.
The remote plan head matches the reviewed head. `git diff --check` passes.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: NOT CLEAN (1 open: P23aa-1 MEDIUM) at 5774a0a0954ac49f1cf4e992a75c21f468479f88

## Round 36 — revision 23aa correction — 2026-10-10

Reviewed head: `d8fbe67afc9e7192268cb36b97690fa04b17401e`.
Prior reviewed head: `5774a0a0954ac49f1cf4e992a75c21f468479f88`.
Prior accepted head: `515cf3b61c94800e00ac2489f76fdab707f47ee1`.
Scope: both corrected entries and the complete 23aa delta from the prior accepted head.

P23aa-1 is CLOSED.
Item 2 now records the existing ordinary link-write suppression in Rust and Mio.
It identifies the incorrect A49 row in `botster-core-host/DESIGN.md`, which I read at v1 `3677b37e`.
It requires that row's correction in the descriptor-handoff PR A.
The unsupported reference to a section 0 known gap is removed.
The revision row matches the corrected scope.

The remaining requirement applies to the new descriptor `sendmsg` implementation.
That implementation must pass the no-signal flag on Linux and retain and check `SO_NOSIGPIPE` on the macOS link socket.
A real-process test must use the default `SIGPIPE` disposition on both operating systems.
It must send a descriptor to a gone worker and observe `EPIPE` and a `Failed` handoff, without a signal.
The correction places this implementation and proof inside item 1's PRs, under the same P3 pair.
It removes the separate follow-up PR and preserves DP-2's top priority.

This plan verdict accepts the corrected requirement and assignment. It accepts no descriptor implementation or process proof.
The revision changes no ledger id, contract pin, pending rule, passing count, staffing limit, or stage acceptance condition.
The remote plan head matches the reviewed head. `git diff --check` passes.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at d8fbe67afc9e7192268cb36b97690fa04b17401e

## Round 37 — revision 23ab — 2026-10-10

Reviewed head: `77739f966cd99042b0db56085994cb55dece6dde`.
Prior accepted head: `d8fbe67afc9e7192268cb36b97690fa04b17401e`.
Scope: both section 0 pin entries and the new revision row.

The remote annotated tag `contracts-v0.1.27` resolves to `b161db87ca82873b4f6b21d8433a69f086abb7cf`.
The plan names the correct commit and retains the prior pin history.
The new tag contains the probe control socket, the ad_3 Activity output fence, R-52, and R-53.
I read the changed controls documentation, ruling text, and ad_3 transcript.
The transcript adds the named output fence before the terminal-state call.
The control documentation names the real PTY probe and distinguishes worker writes from probe reads.
R-52 retains allocator observation at the calling thread and requires positive and negative proofs.
R-53 permits one whole admitted frame and requires the exact-max-frame proof.
The complete changed-path list contains no ledger, withdrawn, pending, frozen, or manifest change.
The only changed transcript is `conf::ad_3_adopted_equals_started`.
The joint manifest therefore remains final39, with 672 active Core ids.

The program-controls pair owns the separate HIGH code pin move.
This plan review accepts the pin reference and assignment. It does not accept that code PR or the probe implementation.
The pin move must retain the existing pending rules and exact-head gate requirements.
The revision changes no staffing limit, clause ownership, passing count, or stage acceptance condition.

The remote plan head matches the reviewed head. `git diff --check` passes.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at 77739f966cd99042b0db56085994cb55dece6dde

## Round 38 — revision 23ac — 2026-10-10

Reviewed head: `aefed7592c07e453d3e5733d8ea2abef0616baf6`.
Prior accepted head: `77739f966cd99042b0db56085994cb55dece6dde`.
Scope: the section 6.3 item 2 correction and its revision row.

The revised proof installs a recording SIGPIPE handler in a separate test target without the test harness.
It requires EPIPE and a Failed descriptor handoff to a gone worker, with no recorded signal, on both operating systems.
A control write must generate SIGPIPE and prove that the recording handler works.
This control prevents the ignored startup disposition from concealing a missing signal-suppression flag.

I checked the installed Rust 1.97.0 source, `std/sys/pal/unix/mod.rs:31-39,163-181`.
Its default startup branch installs SIG_IGN for SIGPIPE.
I checked `signal-hook` 0.3.18, the version in the current v1 lock file.
Its safe `flag::register` API sets an AtomicBool when the signal arrives.
The registry's forbidden-signal list does not include SIGPIPE.
The proposed proof can therefore record delivery through the stated safe API.

Linux still requires the no-signal send flag.
macOS still requires the link socket's SO_NOSIGPIPE setting and its assertion.
The P3 pair still owns the work inside the descriptor-handoff PRs.
Those PRs still correct the host DESIGN.md A49 row.
This verdict accepts the revised proof requirement, not an implementation or completed process proof.

The revision changes no contract pin, ID list, passing count, staffing limit, or stage acceptance condition.
The remote plan head matches the reviewed head. `git diff --check` passes.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at aefed7592c07e453d3e5733d8ea2abef0616baf6

## Round 39 — revision 23ad — 2026-10-10

Reviewed head: `5b335476838e3872c9c8cb2b016ad6e6d1d1a030`.
Prior accepted head: `aefed7592c07e453d3e5733d8ea2abef0616baf6`.
Scope: the new section 8 receipt rule and its revision row.

The lead chooses to replan the receipt decisions after PR #227's third NOT CLEAN round.
The received queue must hold decided route operations instead of frames that application evaluates against later route state.
Receipt decides the client revision, readiness, route role, and validity.
Application waits for refusal room and the PTY transaction. A route close discards its received queue.
P3 must send a QUESTION if a required decision cannot occur at receipt before Round 4.

This rule addresses the remaining F107 path.
R-51 defines early input by worker receipt before the worker's own live transition.
The required proof holds an early complete batch under refusal pressure across that transition.
Every early operation must retain NotReady, and the batch must produce no PTY write.
The transport-order exception cannot excuse input that the worker has already received.

The rule concerns route-state decisions.
The binding AM-2 admission point still orders contiguous PTY transactions.
DP-5 still defines outcomes from later session end, write failure, and partial writes.
IN-8 still requires its mode decision at transaction start.
I checked these clauses at contracts-v0.1.27. This plan review accepts no change to those contract boundaries.

The proof rule settles the F103 correction method: use actions, decoded frames, and transport progress without private queue accessors.
It does not close F103 or F107 on the existing PR head.
Those findings require source and proof review on the replacement.
The lead expressly authorizes a full Round 4 review of that replacement.

The revision changes no contract pin, ID list, passing count, staffing limit, or stage acceptance condition.
The remote plan head matches the reviewed head. `git diff --check` passes.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at 5b335476838e3872c9c8cb2b016ad6e6d1d1a030

## Round 40 — revision 23ae — 2026-10-10

Reviewed head: `b895ad493b9919dd63a9e11a596ba5fd210f1ea5`.
Prior accepted head: `5b335476838e3872c9c8cb2b016ad6e6d1d1a030`.
Scope: the descriptor framing rule in section 6.3 item 1 and its revision row.
The revision changes one plan file in one commit on the prior accepted head.

The lead removes the condition that a read's descriptors belong to its first byte.
The supplied Linux evidence disproves that condition.
I read `~/botster-sessions/gates/botster-core-stage1-p4a-real-handoff-send-259fa4fd-pool-20261010-163135-22828.log`.
The header identifies head 259fa4fd and base 78b011d1.
The old proof expected plain, but its first read returned plainattach.
The source sends plain bytes, then attach with one descriptor. Its buffer holds both sends.
The same gate passes the proof of two descriptor sends with separate descriptor-bearing reads.
This gate fails two tests overall. I use its boundary measurement, not a passing implementation claim.

The [Linux unix(7) manual](https://man7.org/linux/man-pages/man7/unix.7.html) describes this receive barrier.
Its example combines preceding plain bytes with the descriptor-bearing send and leaves following plain bytes for the next read.
I also read Linux v6.6's [unix_stream_read_generic source](https://github.com/torvalds/linux/blob/v6.6/net/unix/af_unix.c).
The normal receive path detaches the descriptors after copying bytes and stops before another descriptor-bearing segment.
It delivers descriptors with the first bytes consumed from their send, including a partial read of that send.

I read the retained macOS record:
`~/botster-sessions/shared/core-stage1/evidence/p3-handoff-send/descriptor-read-boundary-macos.log`.
It identifies macOS 27.0.1 arm64 and source head d563187edd10218d8eb757d51cb628b923eedace.
Both named boundary proofs pass. I read their source at that head.
For plain, attach with a descriptor, then after, the macOS proof expects plain with zero descriptors, then attachafter with one.
The two-send proof expects one with its descriptor, then two! with its descriptor.
These fixed sends fit the proof's 64-byte receive buffer.
The retained temporary probe also records a descriptor-bearing send followed by plain bytes in one macOS read.
The revised Linux branch expects plainattach with one descriptor, then after with none. Its new gate is still pending.
I ran no measurement or proof.

The replacement associates descriptors with the one AttachRoute frame that starts in their read.
The host must start each AttachRoute at its descriptor send and send no other frame with a descriptor.
The operating-system receive boundaries therefore prevent two valid descriptor sends from sharing a descriptor-bearing read.
Preceding plain frames on Linux and following plain frames on macOS do not change the association.
The rule concerns the frame start, not completion. A frame can span reads under section 3's length-prefixed framing.
A later complete-frame decode must retain the starting read's descriptor association.
The worker implementation remains subject to source and proof review.

The rule requires a broken link for each mismatch: no AttachRoute start, multiple starts, a start without descriptors, or a wrong count.
The worker must close every descriptor received in the offending read.
The required worker proofs cover these errors without descriptor leaks.
The per-OS proofs must establish the receive boundaries used by the association.
This replaces the false kernel premise with a private-link framing requirement.

I checked DP-2 and Amendment 17 at contracts-v0.1.27.
Core still owns the route after successful attach, transfers the stream to its worker, and closes the host copy after successful handoff.
A failed handoff still closes the descriptor and route as HandoffFailed.
The revision changes no public route framing, contract pin, ID list, passing count, staffing limit, or stage acceptance condition.
This verdict approves the plan rule. It accepts no #234 source, completed worker receive path, or conformance proof.

The remote plan head matches the reviewed head. git diff --check passes.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at b895ad493b9919dd63a9e11a596ba5fd210f1ea5

## Round 41 — revision 23af — 2026-10-10

Reviewed head: `4f6516bd638bcf835dae5d7496e6ed9dc660bf5a`.
Prior accepted head: `b895ad493b9919dd63a9e11a596ba5fd210f1ea5`.
Scope: the R-54 reference in section 0 and its revision row.
The revision changes one plan file in one commit on the prior accepted head.
I compared the complete plan texts. Those two additions are the only changes.

I read the complete R-54 ruling and its commit delta at botster-contracts `99d21663193240563daa447118e4846aeb4ef647`.
That commit adds only the ruling to `docs/steward-rulings.md`.
I checked EV-8 at contracts-v0.1.27, including its refusal set, fallback admission, transaction ordering, revision rule, and retirement events.
EV-8(c) does not permit a SessionEnded refusal for a query reply.
EV-8(h) does not retire a query at Stop. Payload end still retires it.
EV-8(e) retains a fallback that cannot yet be admitted and retries it when capacity becomes available.
EV-8(f) keeps every reply contiguous at the shared admission point and advances no input revision.
The plan accurately references R-54's reply exception during Stopping, including the interval after Kill and before payload end.
The exception covers both client query replies and shadow replies.
It adds no exception for host writes or other client input. Their Stopping and SessionEnded refusals remain required.

R-54 requires the query-after-Stop proof with a host write refused as NotWritten(Stopping) in the same interval.
The plan retains P3's proof and the typed reply exception in `Worker::admission(source)`.
This review accepts the plan rule. It accepts no implementation, correction proof, or gate for #227.
F113 remains open at the last reviewed PR head, `27e1dae68638421a9820ee893ccf6dbcaec1c67c`.
The authorized Round 5 must still verify the F113 correction and every admission-parity row against the replacement source.

The revision changes no code pin, contract text, conformance ID, coverage list, passing count, staffing limit, or stage acceptance condition.
The remote plan head matches the reviewed head. `git diff --check` passes.
The reviewed plan bytes have SHA256 `8e463cfda8bffa98cbfd321995b13c57e4eb752ef9fc9ccff366ca9c07f40bc6`.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at 4f6516bd638bcf835dae5d7496e6ed9dc660bf5a

## Round 42 — revision 23ag — 2026-10-10

Reviewed head: `cbbdb74e27b0492ddf99104f6076546d00e8ea8f`.
Prior accepted head: `4f6516bd638bcf835dae5d7496e6ed9dc660bf5a`.
Scope: the section 8 admission rule and its revision row.
The revision changes one plan file in two commits on the prior accepted head.
I compared the complete plan texts. The new admission paragraph and revision row are the only final changes.

The rule makes `Worker::admission(source)` the first session decision for host writes, route input, and EV-8 replies.
No accepted input path may write, become a no-op, or discard its input before that session check.
This includes empty bytes, empty text, and empty replies.
The reply source keeps only R-54's typed Stopping exception. It still takes the shared Live/exit check.
The rule addresses both F114-class empty-input exits without duplicating the session checks.

The plan names three permitted classes of earlier exits:

- Receipt-time route decisions: readiness, role, validity, and a bad-frame close retain the final receipt outcomes under revision 23ad.
- Scheduling waits: Spawning or no pending input gives no outcome and waits for further work.
- Contract retirements: queued host cancellation, the adoption fence, and route-close discard retain their required behavior.

I checked IN-6 and DP-8 at contracts-v0.1.27 and section 3, step 4, of `botster-core-host/DESIGN.md`.
A queued cancel retains Cancelled with exact zero. It does not become a session-state refusal.
An adoption fence discards queued requests of the old host without sending their results to the new host.
A route close discards waiting route frames as the accepted receipt rule requires.
Those retirements do not admit a new transaction. They need no renewed session admission.
Started transactions retain their original admission and AM-2 ordering.

The receipt exception resolves my scope question, `msg_plugin-w_1791688931_e810f9`.
An input received before Live retains NotReady even if the session is already stopping when application becomes possible.
The rule adds no later session decision that could replace that receipt outcome.
Invalid and malformed receipt paths also remain exceptions. The required source audit must name their actual producer functions and lines.
Scheduling waits cannot be treated as an admission failure or a completed no-op.

P3 must list every return and early exit in the three paths, with its source line and decision order.
Round 6 must verify that list against the source and the permitted exceptions.
The lead permits no Round 7. If Round 6 is not CLEAN, the lead must split or replan.
This verdict accepts the plan rule only. It accepts no replacement source, completed audit, gate, or proof for #227.

The revision changes no code pin, contract text, conformance ID, coverage list, passing count, staffing limit, or stage acceptance condition.
The remote plan head matches the reviewed head. `git diff --check` passes.
The reviewed plan bytes have SHA256 `b7c41291e83b89b8224cc7b18f383912440555e999f729c1cfcb1111ce922bcb`.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at cbbdb74e27b0492ddf99104f6076546d00e8ea8f

## Round 43 — revision 23ah — 2026-10-10

Reviewed head: `9fcddc35d719e3561afca768f1fd3d5905589507`.
Prior accepted head: `cbbdb74e27b0492ddf99104f6076546d00e8ea8f`.
Scope: section 0 adds R-55, and the revision table adds one row.
The revision changes one file in one commit on the prior accepted head.
I compared both complete plan texts. These are the only changes.

I read R-55 at contracts commit `70dff23d1654040ae6f8e33dc0416078e5372085`.
The plan references that fixed ruling and preserves its conditions.
`host_crash` requires a quiet point, no surviving handle thread or descriptor, and no directory change except the socket file.
The restored file must be a real owner-only socket node without a listener.
`reuse_pid` requires the worker to end through an identity-checked group kill and `wait_worker_exit`.
No Core may hold the directory during the rewrite.
Core's Row encoder and decoded comparisons must prove that only the pid changes.
The bystander must have a different start time.
The referenced ruling also requires an unrelated child in its own group and a pid outside all other recorded identities.
`bystander_alive` requires a live process that is not a zombie. The parent reaps it only at teardown.
`signals_received` includes every signalling edge call by the reading handle, with its target, under R-49 Q1.
The new handle records the complete set because the bystander starts after the old handle ends.
Each fictional history in the control code must cite R-55.
The contracts control descriptions must also receive the R-55 references required by the ruling.

The separate runner PR must skip a non-pending real-only id only on the testkit tier.
The real tier must still require its named real-process proof.
This agrees with sections 1 and 4.2b and revision 23s.
This verdict accepts no runner code, control implementation, pending-list removal, proof result, or gate.
Those changes require their own reviews.

The revision changes no code pin, frozen contract, conformance ID, coverage list, passing count, staffing limit, or stage acceptance condition.
The fetched plan head matches the assigned head. `git diff --check` passes.
The reviewed plan bytes have SHA256 `67d7b35062e9ae8ab6e9f5272916ac27959e95a480070fd671eb7c06eb0f90db`.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at 9fcddc35d719e3561afca768f1fd3d5905589507

## Round 44 — revision 23ah amendment — 2026-10-10

Reviewed head: `247f8e96678f7a4eb74d137a045897a2fc9d41ea`.
Prior accepted head: `9fcddc35d719e3561afca768f1fd3d5905589507`.
Scope: R-55's amended `host_crash` condition 2 and the revision row.
The revision changes one file in one commit.
I compared both complete plan texts. These are the only changes.

I read the steward amendment at contracts commit `af17b973d2b4df76be68c5788e11b1b9fe643667`.
The plan removes the earlier requirement that a drop leave no thread or descriptor.
The plan permits exactly one reaper thread for each live worker that the dropped handle spawned.
It also permits the wake object's fixed descriptor count. That count must not grow with the worker count.
The reaper waits for its worker. It preserves production reaping without killing the live worker under LC-12.
The proof must compare thread and descriptor counts before open and after drop.
The dropped handle's tap must record no edge call after drop, including row writes, link writes, and signals.
Each reaper must end after its worker ends.
After all workers end, the counts must return to their initial values, except for the fixed wake descriptors.
The remaining R-55 conditions and the separate runner scope stay unchanged.

This verdict supersedes Round 43's description of condition 2.
It accepts the amended plan only. It accepts no implementation, count proof, control proof, pending-list removal, or gate.
The implementation reviewer must verify the counts and the absence of edge calls against the amended ruling.

The revision changes no code pin, frozen contract, conformance ID, coverage list, passing count, staffing limit, or stage acceptance condition.
The fetched plan head matches the assigned head. `git diff --check` passes.
The reviewed plan bytes have SHA256 `b9c1871ce3ad595502a3716fc7490c9b4b9fd2f4ca5caa96b73e7b12b90c1daa`.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at 247f8e96678f7a4eb74d137a045897a2fc9d41ea

## Round 45 — revision 23ai — 2026-10-10

Reviewed head: `c014064c3d375595b0510de3a411a0097394ae9a`.
Prior accepted head: `247f8e96678f7a4eb74d137a045897a2fc9d41ea`.
Scope: section 8 adds a focused Mac run for portable code whose behavior depends on the OS.
The revision changes one file in one commit, with one rule addition and one revision row.

The rule covers process identity, signals, reaping, PTYs, sockets, and descriptor passing.
It applies even when the source has no `cfg(target_os)`.
The implementer must supply the real-process test log in the PR before the first READY.
The focused Mac run adds evidence. The Linux gate remains the merging gate.
The rule keeps the pool queue and the limit of one heavy job on the Mac.
It does not require the reviewer to run any execution job.

The lead reports that #239's zombie-identity test fails on macOS and assigns P5 to investigate.
The lead handoff records that macOS reports Absent for that zombie.
The lead has not yet decided whether this is a Core defect or a test expectation that differs by OS.
This verdict accepts the prospective evidence rule. It does not decide that existing issue or accept a correction.
I will apply this rule to later implementation reviews.
For #240, the supplied Mac log already covers the descriptor receiver's real-process tests at d901242d.
Its three source findings remain open despite that passing log.

The revision changes no code pin, frozen contract, conformance ID, coverage list, passing count, staffing limit, or stage acceptance condition.
The fetched plan head matches the assigned head. `git diff --check` passes.
The reviewed plan bytes have SHA256 `339b130fdc232f09265848fa6a723971fc74db484e421ecb29de70c38898782c`.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at c014064c3d375595b0510de3a411a0097394ae9a

## Round 46 — revision 23ai attribution correction — 2026-10-10

Reviewed head: `c7cbc474d2528b4d4b83b9e8a0ebcc647b27db4d`.
Prior accepted head: `c014064c3d375595b0510de3a411a0097394ae9a`.
Scope: correct the source of the zombie-identity test in section 8 and the revision row.
I compared both complete plan texts. Only those two references change.
The test came from #228, commit `7251ba42d7023585cadeac00e9f8a9f18fa3c62f`.
I checked that commit and its test addition.
The earlier attribution to #239 was incorrect.
This round supersedes that attribution in Round 45.
The Mac evidence rule, its scope, and every other plan condition stay unchanged.
The lead still assigns P5 to investigate the existing Mac failure.
This verdict accepts no diagnosis, correction, implementation, or test result.

The fetched plan head matches the assigned head. `git diff --check` passes.
The reviewed plan bytes have SHA256 `ea4b778d22a7421c0da60a224e7624ffe50e30799dfc919087b6e9787df9bd6b`.
This documentation change requires no execution gate.
I ran no build, test, gate, mutation job, measurement, merge-check job, or reversal job.

VERDICT: CLEAN (0 open) at c7cbc474d2528b4d4b83b9e8a0ebcc647b27db4d

## Round 47 — revision 23aj — 2026-10-11

Reviewed head: `6dbe1c43d55e31cba038cee1008098bc0ba41ef0`.
Prior accepted head and sole parent: `c7cbc474d2528b4d4b83b9e8a0ebcc647b27db4d`.
READY: `msg_plugin-w_1791699687_f6efa2`.
Scope: the contracts input in section 0 and the revision row.
The revision changes one file in one commit. Those are its only changes.

I verify `contracts-v0.1.28` at `94b63b83d8b0eb9f3b3d2b65efb8dbf4e3772eb2`.
Its delta from .27 changes six transcripts, the R-52 control rows, and the steward-rulings document.
It changes no frozen contract, manifest, ledger, or status list.
The transcript changes add only seven await_pty steps. Their existing assertions remain unchanged.
The control rows match the previously accepted calling-thread allocation scope.

I read R-56 and R-57 at fixed contracts main `4e97752`, including the follow-up conditions after the new tag.
R-56 uses the existing await_pty step to order a probe input read after the program receives the written bytes.
IN-5 keeps its exact assertions and adds cumulative-log fences of four bytes for s1 and three bytes for s2.
The other five transcripts keep their exact assertions and add their corresponding same-session fences.
The plan states that IN-5 leaves the real-pending list only when the pin-move PR's Linux gate reports PASS.
The full ruling also requires a contracts correction before the pin move if any touched ID regresses on Linux, with no waiver.
The later lint keeps the 238 current unfenced captures as an exception list that can only shrink.
That list must be empty before the real-pending count reaches zero. The plan does not waive that condition.

R-57 uses an OSC 2 title sentinel on the same ordered output path.
The proposed await_title step pumps and reads the cached title. It takes no event from the queue.
The plan cites Core #242 as the condition-3 proof and retains the launch/adoption scope limit.
I read the cited test-only delta at `75cb8b37c2691bf156d15f62b06b45fb3879b010` and its PR description.
The two tests use pump_events 1. They cover an exhausted budget between observations and a full event queue.
They read terminal_state before they drain events and check the earlier metadata and sentinel TitleChanged afterward.
Launch and adoption can replace the terminal baseline without a per-field event.
The fence therefore requires no launch or adoption report between its output and its wait, as the plan states.
The new driver step and EV-2 fence land in a later tag. This revision does not claim they exist in .28.

The program-controls pair still owns the separate HIGH code pin move.
This plan verdict accepts no implementation PR, pending-list removal, minimum PASS, or gate result.
The later pin review must check the touched transcripts and all R-56 evidence conditions against the retained Linux and Mac logs.
The revision changes no passing count, staffing limit, stage acceptance condition, or prospective Mac evidence rule.

The fetched plan head matches the assigned head. The diff check passes.
The reviewed plan bytes have SHA256 `25f26484bc439817c213a1766fc6392f774b276b0bd4796eaea3bb890a5e82e9`.
This documentation change requires no execution gate.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 6dbe1c43d55e31cba038cee1008098bc0ba41ef0


## Round 48 — revision 23ak — 2026-10-11

Reviewed head: `4eceef06434d643bf31e965a35a75d4252d27d19`.
Prior accepted head and sole parent: `6dbe1c43d55e31cba038cee1008098bc0ba41ef0`.
Request: `msg_plugin-w_1791705383_a8b9b9`.
Scope: the contracts input in section 0 and the added revision row.
I compare both complete plan texts. Only the input-row prefix and revision row change.
The input row retains its previous suffix of 10096 characters. No other plan section changes.

I verify tag `contracts-v0.1.29` at `65f76ab9829d7a0fda2835c10abc7ffae2c35215`.
The tag's delta from .28 changes seven paths: one transcript, the conformance driver and exports, tests, and steward rulings.
The only changed transcript is `ev_2_one_marker_per_session`.
It appends the title sentinel to the final bell output, adds await_title, and adds TitleChanged to the later ignore list.
Its existing expectations remain unchanged. No other transcript changes in this tag.
The tag changes no frozen contract, manifest, ledger, pending list, or withdrawn list.

I read the new await_title implementation and its public vocabulary.
It pumps and reads the cached ST-4 title. It does not poll events or move the clock.
Its documentation retains R-57's limit: no launch or adoption report between the sentinel output and the wait.
The accepted Core #242 proof and its budget and queue cases remain recorded in Round 47.
This revision correctly identifies the driver step and EV-2 fence as present in .29, rather than planned for a later tag.

I read R-58 at .29 and its A17 correction at fixed contracts main `d8c0137076c5b0cc7d934fbe615df08f6b846a4d`.
The driver requires both the client's stream end and the recorded host RouteClosed reason for an ended expectation.
One derivation serves both tiers. It ignores the route endpoint's claimed cause and uses the driver's host-event record.
The record preserves the first reason by handle generation and route ID. It cannot borrow a prior handle's reason.
The mapping contains five failed reasons. No Core reason maps to not_connected, as A17 requires.
The tag already contains that five-row mapping and the negative not_connected proof; main adds the explicit ruling correction.
R-50's correction is also present in .29: the holder writes nothing for SessionLost on a pending-handoff route.
The plan states that correction and retains the host's OU-2b mapping. It changes no closed expectation or ID.

I read R-59 at the new tag and main. I check its tally rule against the lead's ~03:42 steward receipt.
The planned start_held control distinguishes a hold that stopped its step from a hold that is merely armed.
The nine hold transcripts require the stated fence or proof that their held point is already certain under the full ruling.
For AD-7's crash form, held:true must precede host_crash. The control and transcript fences remain later work.
AD-7 stays in its current list, but its unfenced pass cannot count toward the real minimum.
The current tally remains 35 / 68 plus one unfenced pass. The plan grants no acceptance for the current unfenced form.

I read R-60 and R-61 at fixed main `d8c0137`.
The program_ready control waits for probe setup before the first input when a later step asserts output or the input log.
The real form observes the probe's connection after R-48 setup. The testkit form is true at once.
R-48 keeps ONLCR. Exact output assertions avoid 0a and other bytes that OPOST changes.
The three named output transcripts require later corrections. They do not change in .29.
R-61 distinguishes waits for a newly produced value from waits that an earlier aggregate can satisfy.
It counts pty_output containing 1b as a possible write and permits a named no-query exception.
Its exact-output definition includes route bytes, literal output frames, and Tap bytes.
Its input rule excludes 0d from exact pty_input and route_expect_pty assertions because R-48 keeps ICRNL.
The plan summaries agree with these fixed rulings. This review waives none of their evidence or exception conditions.

The lead's ~05:20 decision sends the separate HIGH code pin move before #233.
The plan permits ID removal only on a PASS in that PR's Linux gate.
IN-5 remains pending until #233's final gate on the new pin reports PASS. The plan states that specific boundary.
The later pin review still checks R-56 and R-57 evidence and all touched-ID regressions, with no waiver.
This verdict accepts no code pin, implementation PR, conformance removal, minimum PASS, or execution result.
The revision changes no staffing limit, stage acceptance condition, or prospective Mac evidence rule.

The exact-head diff check passes.
The reviewed plan bytes have SHA256 `db7ba8e5c0748aa53257fb53a5e6f8923a9549c6c9871127a7d2ef2c97000813`.
This documentation change requires no execution gate.
I run no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 4eceef06434d643bf31e965a35a75d4252d27d19


## Round 49: revision 23al

VERDICT: NOT CLEAN (1 open)

- Head: `2cb5eba4b93273a142a36adf91c04b883602be84`.
- Base and sole parent: `4eceef06434d643bf31e965a35a75d4252d27d19`.
- Request: `msg_plugin-w_1791707641_01498a`.
- Scope: one new section 8 bullet and its revision row.

### AL-SI-1 MEDIUM: the stated #249 application fails condition 2

Status: OPEN.

Condition 2 requires that the PR diff cannot reach the failing ID's code path.
The lead applies the exception to #249's EV-5 failure in the request and lead handoff.
That PR changes LinkTap::observe and the calls from Tap::take_ahead and EdgeTap::link_recv.
The real EV-5 transcript starts the worker and runs attach and detach through that same tap.
Its hello and worker reports reach the changed observer. Its await_quiet control also uses the tap.
Thus Sol cannot confirm that #249's diff cannot reach the ID's code path.
The changed record may leave the diagnosed worker descriptor defect unchanged. That is a different condition from code-path reach.

Required correction: make the intended eligibility condition and its example agree.
If condition 2 keeps its present meaning, #249 does not qualify and the example must not imply that it does.
If the intended condition is about the diagnosed cause, state that condition precisely and submit the revised rule for review.
The reviewer must check the affected source and the failure evidence before confirming eligibility.
A shared harness change cannot qualify merely because the failing transcript calls no changed control.
Keep the one-attempt limit, written approval, retained cause and owner, and the requirement for a green exact-head gate.

### Other checks

The rule requires diagnosis and reproduction on v1. It requires a written reviewer confirmation and written lead approval.
The rule stops after one further red gate on any ID. It grants no merge of a red head.
The existing Merge bullet still requires both reviews, the current v1 base, and a green gate on the exact head.
The new bullet does not skip mutants or fuzz. Its stated purpose is to run steps that the failed slow stage prevented.
Those limits can support a controlled exception, but they do not resolve AL-SI-1.
Sol grants no application of this revision to #249 at this record.
The #249 source CLEAN at 03094697 remains a source review; its failed final gate is not merge approval.

This revision changes no contracts pin, transcript, conformance list, minimum, staffing limit, or Mac evidence rule.
Sol read the complete two-part diff and checked the example against the already reviewed #249 source and EV-5 transcript.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

The reviewed plan bytes have SHA-256 `8ab4b82bd25c215dd007253aae02b752f308c78ae2562cc0cd148d775dffd549`.


## Round 50: revision 23al correction

VERDICT: CLEAN (0 open)

- Head: `1555a98b196c259dfe5f5d56a87c473ba49d49dd`.
- Sole parent: `2cb5eba4b93273a142a36adf91c04b883602be84`.
- Approved-plan comparison base: `4eceef06434d643bf31e965a35a75d4252d27d19`.
- Request: `msg_plugin-w_1791708097_73e0b6`.

AL-SI-1 is CLOSED. This verdict supersedes Round 49 for the revised plan text.
The condition now concerns the diagnosed cause. It explicitly permits the failing ID's path to pass through changed code.
The reviewer must confirm in writing that the change does not alter that cause.
The same failure and mechanism must reproduce on plain v1. A trace diagnosis remains required.
The lead must approve the exception in writing and record the cause, evidence, owner, and removing PR in the handoff.
Only one further gate is permitted per head. A further red result on any ID requires a new commit.
Every step must be green on the exact head. The rule grants no merge of a red head and skips no mutation or fuzz step.
The unchanged Merge bullet still requires both reviews, the current v1 tip, and the exact-head gate against that tip.

Sol read the complete diff against 23ak and the comment correction against the first 23al head.
Only the new section 8 bullet and revision row differ from 23ak.
This revision changes no pin, transcript, conformance list, minimum, staffing limit, or Mac evidence rule.
Its example names the #249/#240 dependency. That example does not replace the written eligibility checks for a specific head.

### Limit for the current #249 request

This CLEAN approves the rule. It does not classify the running re-gate as complete or green.
Sol read the final gate on 03094697: EV-5 fails at seed 0, step 21 with PendingLimit and four pending operations.
The summary has 427 slow passes and one failure. Mutants and fuzz do not run. The job and wrapper exit 1.
Log: `~/botster-sessions/gates/botster-core-stage1-p5-alive-after-hello-03094697-pool-20261011-013016-71458.log`.
The preceding plain-v1 Mac sample reproduces the success-expectation failure with UnknownRoute, not that exact PendingLimit result.
The trace records four pending operations and zero queued events before the recovery poll in both comparison samples.
R-63 retains the recovery fence. R-64 identifies the v1 descriptor trigger and keeps the missing completion question for A21.
The applying reviewer and lead must record the same-mechanism conclusion with those distinctions intact.
The package addendum at f606fd09 addresses the changed tap and the unchanged admission and descriptor paths.
Sol has received that addendum's summary. Sol has not yet read its complete artifact or a confirmation under the revised wording.
The existing #249 source CLEAN remains. This plan verdict grants no final-gate approval.

The reviewed plan bytes have SHA-256 `a3965b731b7b00fbe56e28581d298f38fe4d1b2836ff322f33d4c988cc5183ab`.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.


## Round 51: revision 23am

VERDICT: CLEAN (0 open)

- Head: `13174c48633edf0bab0b84f750785223c82513bc`.
- Base and sole parent: `1555a98b196c259dfe5f5d56a87c473ba49d49dd`.
- Request: `msg_plugin-w_1791712489_bc1609`.
- Scope: the section 0 contracts prefix, one section 8 bullet, and the revision row.

Sol checked the complete plan delta. The old contracts row remains byte-identical after the inserted prefix.
No other plan path changes. The active pin stays `contracts-v0.1.29`.
The reviewed plan bytes have SHA-256 `91b369632fd6d3648819f8ae7791d972e18355f0a65222fed06464c43b4afaa1`.

### Contracts tag checks

Sol checked each tag's commit and content in the contracts repository.

| Tag | Commit | Content checked |
| --- | --- | --- |
| `contracts-v0.1.30` | `f3fce71d9dd824c4813c79aa325c9ef88f1713ed` | R-60, `program_ready`, two readiness fences, OU-7 bytes and metadata, and two DP-3 byte edits. |
| `contracts-v0.1.31` | `2f38d8bbc7fed058fedcfb8c6344f1cce3e7d92b` | R-62 through R-64, the independent oracle anchor, three ST-6b fences, three IND replacements, and `UnsupportedFor` propagation. |
| `contracts-v0.1.32` | `91394e726b1bd697307d0d5b05712b876f9189be` | The `start_held` specification, vocabulary entries, and hold fences in eight transcript files. |
| `contracts-v0.1.33` | `fde56324102cbc38efabe99b5c79aae73080e02c` | The EV-5 completion fence and the BUILD.md rule 9 mutation-profile exception. |

The .30 delta also changes OU-2b's readiness fence, the probe setup test, and the fake replacement map.
The two DP-3 transcripts change only the seven hexadecimal fields that replace `0a` with `2e`.
Sol verified every changed byte field by static parsing. This check runs no transcript.
The .31 oracle specification requires both comparisons against raw output known independently of the subject's snapshot encoder.
It requires comparator completeness checks and rejects unknown output history as unsupported, never PASS.
The .32 control distinguishes a due, blocked step from a hold that is only armed.
The .33 EV-5 fence collects exactly the four successful Detach completions before the existing recovery poll.
It ignores RouteClosed and Activity events during that collection. The existing recovery steps remain.
No tag delta changes a frozen contract, ledger, status file, or manifest.

R-64 leaves the live-worker link-loss completion unresolved for amendment A21. The plan preserves that boundary.
The BUILD.md exception permits one retry only in the mutation profile.
Fail then pass is FLAKY and counts as MISSED. The retry can remove a catch but cannot create a catch.
Every FLAKY result names its test and requires a structural fix. Other profiles retain zero retries.
This exception differs from 23al's single approved repeat of a whole gate. Neither rule permits a red merge.
This plan verdict approves no retry implementation or mutation verdict.

### Merge and acceptance checks

The controls land before the .32 pin move. Each control PR retains its own tests and exact-head gate.
The combined #240/#243 PR then carries the .33 pin and the full real-tier before/after table.
The stack rule requires both package reviews and the integration review of the exact combined diff.
The unchanged Merge bullet requires a green full gate on that exact head against the current v1 tip.
A moved v1 tip still requires a new import, review, and gate. The rule grants no approval from constituent CLEANs alone.
The combined pin delta and any conflict resolution remain part of the later review.
The lower PR cannot merge alone when it cannot pass alone.

An ID leaves a pending file only after PASS in the corresponding PR's gate.
Unsupported controls remain pending. A5-4 evidence and the existing Mac source-identity rule still apply.
The revision changes no active ID, stage minimum, staffing limit, or manifest.
This verdict accepts no control implementation, pin move, combined PR, removed ID, or execution result.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 13174c48633edf0bab0b84f750785223c82513bc


## Round 52: revision 23an, initial text

- Head: `fde30774815004a99dcb8281583669c67162dc26`.
- Base and sole parent: `13174c48633edf0bab0b84f750785223c82513bc`.
- Request: `msg_plugin-w_1791714443_e3f977`.
- Scope: one new section 8 bullet. No other path changes.
- Plan SHA-256: `e3b630272c2a56a98510fb182ae6d6823758d760d33afbca4081cb4ab8fd056a`.

Sol read the complete delta and the surrounding red-gate, merge, stack, base-import, and READY rules.
The new rule correctly requires cause attribution before an unchanged-cause v1 import can supply another acceptance gate.
It retains the red log and failing ID in the PR body. It refers to all three 23al eligibility conditions.
The exact-head, current-base, full-green gate and both review requirements remain unchanged.

### AN-SI-1 MEDIUM: the operative condition also covers a v1 import that fixes the cause

Status: OPEN.
The operative sentence applies when the next commit only merges v1, without a condition that the imported source leaves the cause unchanged.
It says that the new gate counts only when the earlier failure meets all 23al conditions.
The next sentence permits waiting for a cause-removing commit from v1. Those two sentences have conflicting scopes.

Example: a PR's gate fails EV-5. The next commit merges a v1 tip that contains the reviewed EV-5 fix.
That merge removes the cause and supplies a real source repair, although it only merges v1.
The first sentence still requires the same failure on plain current v1, where the fix can prevent that failure.
The example can therefore reject a legitimate repaired head or require an obsolete comparison as an exception.

Required correction: apply the exception requirement only to a v1 import that does not remove the diagnosed failure's cause.
Explicitly retain a reviewed cause-removing v1 import as a source repair that needs its own exact-head gate and applicable source review.
A merge SHA alone must still supply no rerun channel when the cause remains.

This finding concerns the general plan wording. It grants no exception or acceptance to #256.
The current #256 source CLEAN remains; its red gate has no accepted completion evidence.
The GS reviewer supplied a written unchanged-cause confirmation in `msg_plugin-w_1791714486_f62ee4`.
The reviewer preserves the limit: no independent reproduction of this occurrence, and no equivalence inferred from error text alone.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: NOT CLEAN (1 open) at fde30774815004a99dcb8281583669c67162dc26


## Round 53: revision 23an, cause-removing import correction

- Head: `05b18303c0afe4cf9b5e0437f6307f56fb39045b`.
- Base and sole parent: `fde30774815004a99dcb8281583669c67162dc26`.
- Request: `msg_plugin-w_1791714614_c064c9`.
- Plan SHA-256: `361935e2e23c7327d1d30cbc84dd6c9fbe41108d0e97a1304505928e647c4a63`.

AN-SI-1 MEDIUM is CLOSED. This verdict supersedes Round 52 for the corrected text.
Sol read the complete correction and checked the complete net delta against approved 23am.
Only the new section 8 bullet changes. No other path changes.

Case (a) covers an import that leaves the diagnosed cause unchanged, or a cause that is not diagnosed.
Its gate needs all three 23al attribution conditions, with the red log as the record.
Without those conditions, the PR waits for a commit that removes the cause.
Case (b) covers an import that removes the diagnosed cause. That import is a source repair and needs no 23al approval.
It still needs its own exact-head gate and the applicable source review or base-merge check.
The lead records the v1 commit that removes the cause.
The PR body retains the red log path and failing ID. A merge SHA alone supplies no acceptance while the cause remains.

The existing exact-head, current-base, full-green, package CLEAN, integration CLEAN, and one-re-gate limits remain.
The rule changes no pin, transcript, conformance ID, minimum, staffing, or Mac evidence requirement.
This verdict approves the rule. It accepts no #256 cause attribution, import, gate, or merge.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 05b18303c0afe4cf9b5e0437f6307f56fb39045b

## Round 54: revision 23ao, four echo fences and the combined pin

- Head: `9dba78c901e3c3a5225d437b91378f8143313e03`.
- Base and sole parent: `05b18303c0afe4cf9b5e0437f6307f56fb39045b`.
- Request: `msg_plugin-w_1791715247_6d380e`.
- Plan SHA-256: `e9faf7490d9dd055a7b6cf68c5903fccc242df7547b94a4cbc2037c0916e9cdd`.

Sol checked the complete net delta. Only docs/stage1-plan.md changes: three added lines and one deleted line.
Section 0 adds the four-transcript R-60 rule 3 correction and conditional `.34` pin.
The revision table adds the missing 23an history row and the new 23ao row. No other operative rule changes.

The four transcripts are `ou_7_exit_after_tail_queued_then_close`, `a2_4_other_osc_forms_stay_in_output`,
`e2_1_osc_1_gives_no_title_changed_and_stays_in_output`, and `ev_7_event_and_output_both_carry_the_sequence`.
Only the first is a minimum ID. Each correction adds one `await_control program_ready` before the first input.
Each transcript also declares that control in `requires.controls`. Existing expectations remain.
The rule matches the published R-60 input fence. The lead's handoff records steward approval, P3 ownership, and the tag condition.

Sol read the retained local diff and proof script in shared/core-stage1/evidence/p3-echo-fence-r60/.
Sol statically parsed both echo-proof logs. Each log has ten unique runs for each of the four IDs.
All forty unfenced runs report FAIL. All forty fenced runs report PASSED on the real tier.
The headers identify the scratch source and macOS. These local proofs support the proposed correction; they are not a Core merge gate.

The combined #240/#243 merge pins `.34` if the steward tags it before that merge's final gate.
Otherwise, the existing `.33` order remains. The control-first order and full real-tier before/after table remain.
Whichever contracts PR merges second removes these four IDs from #39's program_ready exception list.
That condition preserves the shrink-only list after both transcript and lint changes exist.
The revision changes no manifest, active ID, minimum list, pending file, acceptance threshold, or current code pin.
Exact-head gates, current-base review, package CLEAN, integration CLEAN, and the 23al/23an limits remain.

This verdict accepts the plan only. It accepts no future tag, contracts PR, combined source, pin move, gate, or pending removal.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 9dba78c901e3c3a5225d437b91378f8143313e03

## Round 55: revision 23ap, current pin and no flaky test at completion

- Head: `3a5b20697967ca7c794f4ee54b988e7f4a74f5aa`.
- Base and sole parent: `9dba78c901e3c3a5225d437b91378f8143313e03`.
- Request: `msg_plugin-w_1791716003_a2d47f`.
- Plan SHA-256: `9cc775e0266f1095213e2e2a482b8b41667dc24d1471975348481bf9a1c783b8`.

Sol checked the complete net delta. Only docs/stage1-plan.md changes: three added lines and one deleted line.
Section 0 names the current .32 pin and qualifies the previous .29 history text.
It records #259's fenced AD-7 result and real-passing tally. Section 1 adds completion item 7. The revision table adds 23ap.

Sol independently verified #259's merged tree at v1 4ab8a985 against the reviewed and gated 6b20f6f2 head.
The pin is contracts-v0.1.32 at 91394e726b1bd697307d0d5b05712b876f9189be.
No Core source or pending ID changed in that PR.
The AD-7 transcript's step 4 requires start_held with held:true. The driver returns success only on a match.
The retained real trial passes on Linux and macOS. R-59 therefore permits that ID to count on this reviewed fenced form.
The Linux gate reports real-passing 40/68. This plan entry does not accept other held lifecycle models or the six failed Mac trials.

The new completion criterion requires ci/known-flaky.toml to be empty when Stage 1 ends.
Each interim entry needs a diagnosis, owner, and removing PR. That PR removes the entry.
The plan requires no early empty-list xtask check while work still needs entries.
This criterion strengthens stage completion. It does not itself approve an exclusion, parser, schema, or future gate change.
The separate HIGH review remains required for that implementation and its exact-head evidence.

The controls-first order, combined #240/#243 conditional .34 pin, and second-merge exception cleanup remain.
Manifest, active IDs, minimum list, existing completion criteria, current-base gates, and 23al/23an limits remain.
This verdict accepts the plan only. It accepts no future source, exclusion, pin, gate, or pending removal.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 3a5b20697967ca7c794f4ee54b988e7f4a74f5aa

## Round 56: revision 23aq, tagged fence changes and the combined .35 pin

- Head: `d4f67d0980bb66727f2a439ea7c7fe5b01bb5039`.
- Base and sole parent: `3a5b20697967ca7c794f4ee54b988e7f4a74f5aa`.
- Request: `msg_plugin-w_1791716584_208881`.
- Plan SHA-256: `b83ed050290e2b3a75a0b57ccdaa7ea8c4e3da3bb3cfac8bf5c49e589fe826b9`.

Sol checked the complete net delta. Only docs/stage1-plan.md changes: two added lines and one deleted line.
Section 0 adds the tagged .34/.35 facts and changes the combined #240/#243 target to .35.
The revision table adds 23aq. No other operative rule changes.

Sol fetches and checks both exact tags:
- contracts-v0.1.34: `26326b26c71d4848ef5f94266107e7a26c4ebbb7`.
- contracts-v0.1.35: `e3180652a3447caef7595e76640a0f383b018729`.

The .34 tag contains the four program_ready fences already checked in Round 54.
The .35 tag contains .34 as an ancestor. Its transcript delta adds only one await_pty with min_bytes:6 to each of three IN-10 IDs.
Correction: those three IDs are core-pending on current v1, not real-pending. Round 57 withdraws this round's CLEAN.
The plan claims no pending removal or passing result for them.
The fence lint's declared scope is R-57, R-60, R-61, and R-62 Q3. Its exception file has 127 entries.
All four corrected echo IDs are absent from that file. The second-merge exception cleanup required by 23ao is present.
The steward and lead split all R-56 lint work into later B2. This plan preserves that split and requires no separate Core pin PR for B1.

The tag also records the approved OSC 0 sentinel reading and R-55 reaper proof form.
Sol checked that R-55 text against the previously read accepted draft.
That published ruling does not correct #257's old source or supply replacement evidence. Its four findings remain open on 96f6f387.
The plan's .35 target does not approve any future host_crash or reuse_pid implementation.

No frozen contract, ledger, deferred file, withdrawn file, or BUILD.md changes from .33 to .35.
The current code pin remains .32. The combined merge still requires its source review, full real-tier before/after table, and exact-head full gate.
An ID leaves pending only on a PASS in that PR's gate. Manifest, active IDs, minimum list, and completion criteria remain.
The lead records the steward's confirmation that .34 remains valid. The later 23aq entry sets .35 as the combined merge's target.

This verdict accepts the plan only. It does not replace the contracts reviewer or steward's review of B1's implementation.
It accepts no future combined source, code pin, gate, mutation exclusion, R-56 lint, or pending removal.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at d4f67d0980bb66727f2a439ea7c7fe5b01bb5039

## Round 57: revision 23aq, pending-file classification correction

- Head remains `d4f67d0980bb66727f2a439ea7c7fe5b01bb5039`.
- Base remains `3a5b20697967ca7c794f4ee54b988e7f4a74f5aa`.
- This round supersedes and withdraws Round 56's CLEAN.

### AQ-SI-1 LOW: the three changed IN-10 IDs are not real-pending

Status: OPEN.
Section 0 describes the three IN-10 await_pty changes as applying to IDs "which are real-pending".
Current v1 cfd6ee36 holds all three in conformance/core-pending.txt, at lines 333 through 335:
- conf::in_10_guard_does_not_withdraw_after_start.
- conf::in_10_later_client_frame_is_ordered_after.
- conf::in_10_started_write_is_contiguous.

None is in conformance/core-real-pending.txt. These IDs have not passed the testkit harness and remain pending on both tiers.
The three different IN-10 IDs in the real-pending file are both_guards_are_checked_atomically_with_ordering,
guard_refuses_when_client_frame_received_before_start, and lagging_host_view_gets_stale_not_a_wrong_write.
The .35 transcript delta does not change those three real-pending IDs.

Required correction: replace the .35 classification with core-pending, or remove the incorrect classification.
Keep the rule that an ID leaves a pending file only on a PASS in the implementing PR's gate.
This is a factual accounting correction. It does not require a pin-order change or a new execution job.

Sol's first static membership check returned false for all three real-pending memberships.
Sol incorrectly wrote the opposite in Round 56. Direct reads of both exact-base files confirm this finding.
The remaining tag, fence, exception-list, scope, and merge-order checks in Round 56 remain valid.
Sol ran no execution job.

VERDICT: NOT CLEAN (1 open) at d4f67d0980bb66727f2a439ea7c7fe5b01bb5039

## Round 58: revision 23aq, classification and R-65 corrections

- Head: `0be2ad2f1f74ddef0134bd424676009d823204bf`.
- Parent: `bf9e27343a83ac21281dc7cf9ffaa0c323fb749b`.
- Prior approved plan: `3a5b20697967ca7c794f4ee54b988e7f4a74f5aa`.
- Plan SHA-256: `841968451e1461d42276cb81166a932ee9755306efeff71f9aa130f4d196ee03`.
- Lead receipts: `msg_plugin-w_1791716964_83e9a3` and `msg_plugin-w_1791717222_49d39e`.

AQ-SI-1 LOW is CLOSED. Section 0 now correctly puts the three changed IN-10 IDs in core-pending.txt on v1 cfd6ee36.
It explicitly excludes core-real-pending.txt. Sol independently rechecks all three memberships in both exact-base files.
The rule still requires a PASS in the implementing PR's gate before an ID leaves pending.
The tag, fence, exception-list, scope, and merge-order checks from Round 56 remain valid.
Round 56's withdrawn CLEAN remains withdrawn. This round approves the corrected head above.

The new R-65 paragraph cites botster-contracts main `8f936526406744f6882db0fb4ec79dc96c0ea31a`.
Sol read that complete eleven-line ruling and P3's complete diagnosis README.
The diagnosis records macOS results on a local combined tree. It does not measure Linux or plain v1 cfd6ee36.
The diagnosis identifies the kernel mechanism as an inference. This verdict supplies no independent kernel-cause proof.

R-65 reads DP-2's close order: Core retains its endpoint copy until the worker acknowledges receipt, then closes the copy.
The acknowledgement uses Core's private link. No Core amendment is required.
Protocol 1 is allowed only if no released worker speaks protocol 1; otherwise protocol 2 is required.
R-65 requires the lead to confirm the release condition in the PR. This plan does not waive that condition.
A refused descriptor gives HandoffFailed and closes Core's copy. The plan adds no timer.
Holding Core's copy for the route's whole life remains rejected under DP-1.
The fix and a real-process proof must enter #243 before the combined merge review, regardless of the later Linux diagnosis.
The DP-2 minimum ID keeps its meaning. Any needed transcript fence remains subject to the steward's review.

Sol's QUESTION `msg_plugin-w_1791717204_853dbe` identified an unconditional lost-acknowledgement sentence in intermediate bf9e2734.
The lead corrected that sentence before this verdict.
The lead supplies steward receipt `msg_plugin-w_1791717053_412997` from session sess-1790792742-003c-b2e696997f3d2e34d8ee89cb35f04cf9.
The later clarification has no contracts commit yet. Sol records the lead's supplied receipt as its authority.
In Starting or Stopping, link loss ends the session. R-50 governs: the route closes after the state with its end-state reason.
OU-2's first-reason rule remains. In Running, the session stays Running with a broken link under lc_5.
The unacknowledged route then closes HandoffFailed, and Core closes its copy under R-65.
The plan preserves the caveat that the worker might have received the descriptor while its acknowledgement was lost.
A21 may refine that Running case. This plan approves no future A21 text or implementation.

Only docs/stage1-plan.md changes from the approved 23ap plan.
Section 0 and the 23aq revision row contain the net changes. No other operative rule changes.
The current code pin remains .32. The combined merge's target remains .35.
The combined merge still needs source review, a full real-tier before/after table, and an exact-head full gate.
Manifest, active IDs, minimum list, pending rules, and completion criteria remain unchanged.
This CLEAN approves the plan only. It approves no future source, pin, gate, pending removal, or mutation exclusion.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at 0be2ad2f1f74ddef0134bd424676009d823204bf

## Round 59: revision 23aq, published R-65 follow-up citation

- Head: `8140a92991e82d69381cd618e9ce154afa746f97`.
- Parent: `0be2ad2f1f74ddef0134bd424676009d823204bf`.
- Plan SHA-256: `3b50a4d6564d9ec7bc32f9aa028f1a6f355cb3a2e2d0f03cefbf7a7358e3d77d`.
- Lead receipt: `msg_plugin-w_1791717262_719b4c`.

Sol checked the complete delta. Only one line in docs/stage1-plan.md changes.
It adds the fixed contracts citation `6243fdcbd0971f417186ed202855a5dd11a03274` for the R-65 follow-up.
Sol read that complete four-line contracts addition. It publishes the state qualification and caveat checked in Round 58.
Starting and Stopping follow R-50's state-before-route order and end-state reason. Running follows R-65's HandoffFailed reading until A21.
OU-2's first-reason rule and the received-descriptor caveat remain explicit. No other rule changes.
The previous receipt-only source limit is closed by this published citation.
All Round 58 checks and scope limits remain. AQ-SI-1 stays closed.
This CLEAN approves only this plan. It approves no future combined source, pin, gate, A21 text, or pending removal.
Sol ran no execution job.

VERDICT: CLEAN (0 open) at 8140a92991e82d69381cd618e9ce154afa746f97

## Round 60: revision 23ar, contracts .36 target

- Head: `a2897c1204dbfe6bbc44f476783686a68cabf57a`.
- Parent and previous approved plan: `8140a92991e82d69381cd618e9ce154afa746f97`.
- Plan SHA-256: `2e4d6e962f37ddafe2a68fd50598a1bd218fe3a6a53358f8cccd970884e0daed`.
- Lead receipt: `msg_plugin-w_1791718432_8dfd0d`.

Sol checks the complete delta. Only docs/stage1-plan.md changes, with two added lines and one deleted line.
Section 0 records contracts-v0.1.36 and sets it as the combined #240/#243 target instead of .35.
The revision table adds 23ar. No other operative rule changes.

The local annotated tag object is `49f82ad13472a21cf47415f296f96dff0fae277f`.
It resolves to the plan's commit `96f48d6d3d7a0e75e752d78433eff92efad0e77e`.
The lead reports that the steward tagged it. Its annotation records #41 B2, the crate review, and the gate.
Sol's separate remote-tag query fails because DNS cannot resolve github.com. It supplies no independent remote publication check.
The local exact tag and the lead's release receipt are the tag evidence for this plan review.

Sol independently confirms .35 and the R-65 follow-up commit 6243fdcb are ancestors of .36.
Only three contracts paths change from .35:
- crates/botster-core-conformance/tests/fence_lint.rs.
- conformance/core-fence-exceptions.txt.
- docs/steward-rulings.md.

No transcript, runtime crate, frozen contract, ledger, manifest, or BUILD.md changes.
The lint's declared scope adds R-56 input fences and the capture ratchet to the previously reviewed fence rules.
Sol checks that scope and the exception data. This plan review does not replace the contracts implementation review.
The exception file has 398 unique entries: 271 initial R-56 capture entries and the same 127 R-60 entries from .35.
Sol independently compares the complete R-60 sets. They are equal. Every new entry has the R-56 capture reason.
These lint exceptions do not remove a Core pending ID or count a Core pass.

The steward-rulings blob at .36 equals the previously read blob at 6243fdcb exactly.
Thus the new target carries R-65 and its complete state qualification without changing either ruling.
The #243 fix and real-process proof remain required before the combined review.
The protocol release condition, R-50 ordering, Running HandoffFailed reading, and A21 caveat remain unchanged.

The current Core code pin remains .32. This plan changes only the future combined target.
The combined head still needs source review, a full real-tier before/after table, and its exact-head full gate.
The pending-removal rule, active IDs, minimum list, and Stage 1 completion criteria remain unchanged.
This CLEAN approves only the plan. It approves no future combined source, pin implementation, gate, lint implementation, or pending removal.
Sol ran no build, test, gate, mutation, reversal, measurement, or executable merge-check job.

VERDICT: CLEAN (0 open) at a2897c1204dbfe6bbc44f476783686a68cabf57a

### Round 60 publication checks

Sol pushes verdict 1269657511861ce87e76a8082c0fc5f181154840 after DNS returns.
Sol independently hashes pins/stage1-plan.a2897c12.md. Its SHA-256 equals the exact plan hash in Round 60.
Sol queries the contracts remote. Its .36 tag object is 49f82ad13472a21cf47415f296f96dff0fae277f.
The remote tag resolves to 96f48d6d3d7a0e75e752d78433eff92efad0e77e, equal to the checked local tag and plan target.
The DNS publication limit is closed. All Round 60 scope limits remain.

## Round 61 — revision 23as

- Head: `12ba52e392474dfc96be688aacb55ef2d4642099`.
- Parent: accepted revision 23ar, `a2897c1204dbfe6bbc44f476783686a68cabf57a`.
- Pin: `~/botster-sessions/pins/stage1-plan.12ba52e3.md`.
- SHA256: `061c5db2d84709c49c09e7ed8482ca29cbf696ad2e115b1ce171eb72b60702da`.
- Lead receipt: `msg_plugin-w_1791727160_50e4ab`.

Sol confirms the hash and exact byte equality between the pin and the committed plan. Only the section 0 contracts row and revision table change. The manifest, active IDs, package boundaries, pending rules, and section 8 criteria remain unchanged.

Sol reads the complete inserted text and complete R-68 draft at the named path. The plan records pump/read/wait behavior for waiting route reads. It preserves events by forbidding poll_events. It forbids the injected-clock step. The referenced draft requires waiting on both the handle wake and route transport. route_expect_refusals retains its separate rule. TH-3 remains unchanged: data bytes need no host pump; the descriptor's end needs the acknowledgement processed by a host pump.

The plan preserves the required deterministic real-tier red-on-revert proof, the full A5-4 table on both tiers without PASS-to-non-PASS, and N >= 5 ou_7 runs on each operating system. P5 owns the HIGH contracts PR. Reviewers 016c and 0167 review it; the steward merges it. The ID remains real-pending until the later Core pin. This review accepts no implementation or evidence for that future PR.

The job.sh paragraph matches the steward's ruling (b) and accepted Contracts #45. Contracts owns the common file. Core copies it at the pinned revision, adds the byte comparison check, and updates the copy during the pin roll. Contracts gets no check. Sol independently queries Contracts main; it equals `56e5b744588b98ccbbb892d6b0a63522317284f0`. The Core copy remains future work.

No finding appears in the plan text.

### AS-SI-1 LOW — the plan pin is writable. OPEN.

The pin has mode `0644`. BUILD.md requires plans to be copied and made read-only with `chmod 444`. The hash is correct, but the file does not meet that pin rule.

Required change: make this exact pin read-only. Preserve its bytes and SHA256. Send the completion receipt. No plan text change, new source commit, or execution job is required.

The previous pin also has mode `0644` when inspected now. Sol does not infer when that mode changed or claim a prior mode check.

This round approves no future driver change, Core pin, gate, or pending removal. Sol runs no execution job.

VERDICT: NOT CLEAN (1 open) at 12ba52e392474dfc96be688aacb55ef2d4642099

### Round 61 closure — AS-SI-1

Lead receipt `msg_plugin-w_1791727694_a50492` reports the pin mode correction. Sol independently checks the exact pin.

- Mode: `0444`.
- SHA256: `061c5db2d84709c49c09e7ed8482ca29cbf696ad2e115b1ce171eb72b60702da`.
- The bytes still equal `docs/stage1-plan.md` at `12ba52e392474dfc96be688aacb55ef2d4642099`.

AS-SI-1 closes. The plan text has no finding. All Round 61 scope limits remain. This closure approves no future implementation, pin change, gate, or pending removal. Sol runs no execution job.

VERDICT: CLEAN (0 open) at 12ba52e392474dfc96be688aacb55ef2d4642099

## Round 62 — revision 23at

Head: `70cf75fb51d3e96df340685bcabc2691ef2cf2f4`.
Parent: `12ba52e392474dfc96be688aacb55ef2d4642099`.
Lead receipt: `msg_plugin-w_1791729692_5a12c8`.
Pin: `~/botster-sessions/pins/stage1-plan.70cf75fb.md`.
SHA256: `98aa4ed21c774afc42798baad2d4baa4b83729bed340c178928e01c9366d39a6`.
Sol independently verifies mode `0444`, this hash, and exact committed bytes.

Sol reads the full added text in section 0 and the revision row.
All other text is unchanged. The manifest and active Core IDs are unchanged.
Sol reads the R-68 and R-69 drafts cited by the plan.
The R-68 wait design, wake duplicate, no-progress bound, diagnostic, and deferred enum change agree with those rulings.
The R-69 refusal, transcript placement, and evidence allocation agree with the ruling and the lead's agreed order.
#269 lands before the combined pin and keeps its three IDs real-pending.
P5 owns the one combined Core pin. GS owns the later common-file copy and byte check.
The stall rule leaves omitted work for `.39`; it does not authorize an unreviewed change or a pending removal.

### AT-SI-1 LOW — The A5-4 table names the skipped tag. OPEN.

Section 0 says `.37` is skipped and the combined tag is `contracts-v0.1.38`.
The same paragraph says P5's A5-4 table "lists the `.37` contents".
That citation conflicts with the specified combined tag.

Required change: name `.38` as the table's contents. Preserve `.36` as its comparison base.
Publish the corrected pin, its hash, and mode `0444`. No implementation or execution job is required.

No other text finding appears. This round approves no future implementation, code pin, gate, or pending removal.
Sol runs no execution job.

VERDICT: NOT CLEAN (1 open) at 70cf75fb51d3e96df340685bcabc2691ef2cf2f4

## Round 63 — revision 23au, AT-SI-1 closure

Head: `0af4db8d9278b9062c166dfb3ac382d944491b89`.
Parent: `70cf75fb51d3e96df340685bcabc2691ef2cf2f4`.
Lead receipt: `msg_plugin-w_1791730078_77fe73`.
Pin: `~/botster-sessions/pins/stage1-plan.0af4db8d.md`.
SHA256: `8c4f8bb51f2d3defe02822c4ed0b03d6f7a08f5b693950d4746fd6fd8139e3d9`.
Sol independently verifies mode `0444`, this hash, and exact committed bytes.
The corrected plan commit is published on `origin/stage1/plan`.

Sol reads the complete correction and the added revision row. No other text changes.
The table compares against `.36`. The pin PR body lists every contracts change from `.36` to `.38`.
That list includes `.37` contents (#42), because Core never pins `.37`.
The lead states that the steward requires the `.37` contents in that list.
This wording separates the table's comparison base from the body's complete change list.
AT-SI-1 closes. Zero findings remain in the plan text.

The manifest and active Core IDs are unchanged. All Round 62 scope and future-work limits remain.
This review approves no implementation, code pin, gate, pending removal, or future import.
Sol runs no execution job.

Publication recovery: the lead reports restored DNS in the same receipt.
Sol successfully pushes local verdict commit `05eb66ad5e0bfc5df7336723e6b3d551750bc1ed` to `origin/stage1/integration-review-sol`.
The #269 Round 1 and plan Round 62 publication limit is closed. Their historical failed push results remain failed.
#269 stays NOT CLEAN with three open findings on its recorded exact head.

VERDICT: CLEAN (0 open) at 0af4db8d9278b9062c166dfb3ac382d944491b89

## Round 64 — revision 23av, 2026-10-11

Head: `70cc9bcb28d9b816e3f6a7768eb33c4f31eeac3e`.
Parent: `0af4db8d9278b9062c166dfb3ac382d944491b89`.
Pin: `~/botster-sessions/pins/stage1-plan.70cc9bcb.md`.
Mode: `0444`.
SHA256: `b693d3f9318d31fb1f3cd1e0b557621f985a2da9320f7625752dad5e14658ab7`.
Lead receipt: `msg_plugin-w_1791732664_89da4f`.

I verify the exact parent, pin mode, hash, and equality with the committed bytes.
I read the complete inserted section-0 text and revision row. Removing those two additions restores exact parent bytes.
No other plan text changes. The manifest and active Core identifiers remain unchanged.

The R-66 definition and three immediate-post conditions agree with the complete steward correction draft.
The R-69 fence and final sentinel statements agree with the reviewed ruling and the lead's follow-up approvals.
The R-43(5) eight-transcript scope, same-now pump, tm_3 form, strict-wake driver rule, and negative test agree with the complete follow-up draft.
The tm_3 form correctly leaves more unasserted and checks exactly Silent after pump_idle.
The tag order preserves .39 as the default and forbids new gates for #48 or #47 because of this work.
The #264 own-trial rule agrees with the lead's four conditions and the reviewed source correction.
The do-not-chase-v1 summary references the binding pair-common rule. That rule retains its earlier-merge exception for overlapping files or a build need.

I independently read the minimum and pending files at v1 `e4b6cdf54a7453af182b9e3e44fc08b5f58c7b47`.
The minimum contains 69 identifiers. Thirteen are testkit pending; nine are real pending; the sets do not overlap.
Their union contains 22 identifiers, leaving 47 outside both pending lists. The named groups sum to 22.
The existing accepted gate reports 47 real-accepted identifiers. This plan count supplies no new per-identifier execution proof.
The plan retains the stale-header correction as future work and identifies the open owners.

No plan finding remains. This review approves no future implementation, pin, gate, source import, or pending removal.
The code pin remains .36. Historical proof limits and the open ou_9 structural defect remain.
I ran no execution job.

VERDICT: CLEAN (0 open) at 70cc9bcb28d9b816e3f6a7768eb33c4f31eeac3e

## Round 65 — revisions 23aw through 23ay, 2026-10-11

Head: `ed7604f382c687319c15d39a1d7689c006fc7e30`.
Comparison base: accepted 23av `70cc9bcb28d9b816e3f6a7768eb33c4f31eeac3e`.
Parents: 23aw `e66abd21df3c8b446cbaf06423413a7556dbbb70`, then 23ax `75268739895906d2d98bab342728c08e2e1ea2a6`, then this head.
Pin: `~/botster-sessions/pins/stage1-plan.ed7604f3.md`, mode `0444`.
SHA256: `d17895aedf41bfa918ef7612ded50904656390ff04b07a9d1ed871740a2b91cf`.
Lead receipts: `msg_plugin-w_1791733665_3e51d9`, `msg_plugin-w_1791733888_397544`, and `msg_plugin-w_1791733928_0d5c6b`.

I verify the complete parent chain, pin mode, hash, and committed bytes.
I read the full added section-0 text and all three revision rows against 23av.
The manifest and active Core identifiers do not change. Revision 23ay withdraws the 23ax resize replay branch.

I read pinned TM-6, IN-6, TH-2, and the three oracle control rows.
I read the existing attach, cancel, wake, ModelLog, oracle helper, and DESIGN dispatch paths at Core `e4b6cdf5`.
Queued handoffs leave local actions that `engine.runnable()` does not count in `sync_wake`.
The proposed queued-action signal addresses that producer-to-host gap. Parked work must remain excluded.
IN-6 separates queue admission from the operation's final progress. TM-6 requires a signal when the call leaves runnable work.
The steward concurs with the queued cancel form, according to the lead.
The lead states the required precision: cancel returning Admitted must signal; other results create no new work, and TH-2 permits spurious Woken.
This interpretation does not require a transcript change. The future PR must preserve that conditional signal requirement and the typed cancel results.

The shared oracle module and existing output anchors avoid a second feed to a live per-session oracle.
The future PR must update DESIGN.md and prove actual dispatch on both tiers.
The existing ModelLog stores the first size but no Resize or SetColorProfile record.
Condition 3 of the oracle_resume specification requires a ruling for histories with those changes.
A shared refusal for history-based oracle_resume and oracle_screen avoids false comparisons until such a ruling exists.
The minimum and pending changes remain conditional on the future gate's actual PASS results.

### AY-SI-1 — MEDIUM — the blanket resize refusal lacks a session for the payload control

Section 0 requires every oracle control on both tiers to refuse after Resize or SetColorProfile through one shared check.
The same paragraph defines `oracle_screen_payload {payload}` with no session argument or output history.
The pinned controls document also defines that control as a fresh oracle that loads only the GHOSTSNP payload.
The adapter cannot identify the source session's resize history from those arguments.
A handle-wide refusal could reject a payload from an unrelated session. An added session requirement would change the specified control form.
The stateless oracle_query_reply helper also differs from an oracle that replays a session's output history.
The cited condition 3 belongs to the oracle_resume history specification; it does not define the blanket control scope.
The lead must state the exact affected controls and the check each control can perform with its specified arguments.
Keep the approved history refusal. Define the payload-only and stateless query forms explicitly before implementation.

This review approves no future implementation, pin, gate, source import, or pending removal.
I ran no execution job.

VERDICT: NOT CLEAN (1 open: AY-SI-1) at ed7604f382c687319c15d39a1d7689c006fc7e30

## Round 66 — revision 23az correction, 2026-10-11

Head: `16ce6e6fc919467b2fe6bafdd45707f0fb26cada`.
Parent: `ed7604f382c687319c15d39a1d7689c006fc7e30`.
Pin: `~/botster-sessions/pins/stage1-plan.16ce6e6f.md`, mode `0444`.
SHA256: `759f464a7b8deee176525246adcc962696e7e557c68ce0710e1a535e353a45ce`.
Lead receipts: `msg_plugin-w_1791734089_82a1e1` and `msg_plugin-w_1791734110_6a955d`.

I verify the exact parent, pin mode, hash, and committed bytes. I read the complete correction and revision row.
The history refusal now guards only oracle_resume and oracle_screen on both tiers.
The stateless oracle_query_reply and payload-only oracle_screen_payload do not check session history or gain a session argument.
The host wrapper records Resize and SetColorProfile. The real CaptureLayer already records those calls at begin.
AY-SI-1 closes. The shared refusal and future DESIGN update retain their approved scope.

### AZ-SI-1 — MEDIUM — the Resize description omits the forwarding branch

The oracle paragraph says Core sends neither Resize nor SetColorProfile to the worker.
The new gap paragraph says Op::Resize is a host-only setter at v1 d582853d.
I read the exact source at `d582853da895688710ab97f8f1a0858599e8ba18`.
`admit.rs:705` selects Next::Setter only for Admit::Created.
For Running at the current size, with no resize in flight, it selects a direct Applied completion under SZ-2.
Every other admitted Resize selects Next::Forward. Starting operations can wait for the worker launch.
`run.rs:462` forwards those operations as HostMsg::Op and waits for the worker result.
The existing driver tests explicitly inspect the Resize frames sent to the worker.
The quoted setter block therefore does not prove that Resize is always host-only.
This finding does not claim that the worker resizes the PTY or posts SizeChanged correctly.

Correct both blanket claims. Preserve the host-wrapper record point, which does not depend on worker execution.
State the actual missing worker behavior and its evidence before assigning a production gap from this premise.
I independently verify all eleven identifiers in the listed Resize groups are pending and none is a minimum identifier.
Those list facts do not prove the stated cause of their failures.

No implementation, pin, gate, source import, or pending removal is approved by this plan review.
I ran no execution job.

VERDICT: NOT CLEAN (1 open: AZ-SI-1; AY-SI-1 closed) at 16ce6e6fc919467b2fe6bafdd45707f0fb26cada

## Round 67 — revision 23ba correction, 2026-10-11

Head: `c7865db7563a527fb93cb65222b2f30395df74b5`.
Parent: `16ce6e6fc919467b2fe6bafdd45707f0fb26cada`.
Pin: `~/botster-sessions/pins/stage1-plan.c7865db7.md`, mode `0444`.
SHA256: `23f292e6fa11dda0493cf548af37a49f7496e5b1651214b167c19c519908900b`.
Lead receipt: `msg_plugin-w_1791734366_49e58a`.

I verify the parent, pin mode, hash, and committed bytes. I read the complete correction and revision row.
The plan now distinguishes Created setters, the unchanged-size completion, and operations forwarded to the worker.
The oracle record point stays at the host wrapper in every session state. AY-SI-1 remains closed.
I read the complete worker on_op match at the named d582853d base.
The worker handles WriteInput, ReadScreen, ReadCursor, ReadModeFlags, CaptureSnapshot, and Signal.
The catch-all reports Internal. Forwarded Resize, SetSizePolicy, and SetColorProfile reach that catch-all.
I check the worker and sys source for PTY resize and SizeChanged. Only the spawn PTY resize call appears.
The plan labels this diagnosis as source evidence and does not claim proof from a pending identifier's failing step.
AZ-SI-1 closes. The pending and minimum facts retain the Round 66 scope.

### BA-SI-1 — LOW — unchanged-size Resize completes in the next pump

The corrected paragraph says the unchanged-size Resize completes Applied at once.
Pinned SZ-2 says it completes Applied in the next pump and posts no SizeChanged.
At d582853d, admit.rs stores Step::Ready(Next::Complete(...)); it does not complete the operation in begin.
Change at once to in the next pump. Preserve the current-size and no-resize-in-flight conditions.
This is a timing correction in the plan text. It requires no product change or new gate.

No future implementation, pin, gate, source import, or pending removal is approved.
I ran no execution job.

VERDICT: NOT CLEAN (1 open: BA-SI-1; AZ-SI-1 closed) at c7865db7563a527fb93cb65222b2f30395df74b5

## Round 68 — revision 23bb correction, 2026-10-11

Head: `6251ffcb11b85ef5caf0973673f057a817534bc8`.
Parent: `c7865db7563a527fb93cb65222b2f30395df74b5`.
Pin: `~/botster-sessions/pins/stage1-plan.6251ffcb.md`, mode `0444`.
SHA256: `fa582f203df6e3ccc51df2d6756e5ef96002c5d52a12896a056c5a1ee8dc2e9c`.
Lead receipt: `msg_plugin-w_1791734518_bf1048`.

I verify the exact parent, pin mode, hash, and committed bytes.
I read every changed word and the complete new revision row.
Only docs/stage1-plan.md changes. All other plan words equal revision 23ba.
The unchanged-size Resize now completes Applied in the next pump under SZ-2.
The text states that admit.rs stores Step::Ready(Next::Complete(..)) and begin does not complete the operation.
The Running, current-size, and no-resize-in-flight conditions remain unchanged.
BA-SI-1 closes. AY-SI-1 and AZ-SI-1 remain closed with their recorded evidence.
The attach, cancel, testkit, and oracle decisions retain the Round 65 to Round 67 scope and limits.

This CLEAN accepts the plan text. It approves no future implementation, pin, gate, source import, or pending removal.
The active code pin remains contracts-v0.1.36.
I ran no execution job.

VERDICT: CLEAN (0 open) at 6251ffcb11b85ef5caf0973673f057a817534bc8

## Round 69 — revisions 23bc and 23bd, 2026-10-11

Head: `03c56e269b9af7a38ab893973ef3109772d66e81`.
Parent: `d9fbc7471d26f6cd4b0708e5ad6e12c00e61223b` (revision 23bc).
Reviewed base: `6251ffcb11b85ef5caf0973673f057a817534bc8` (revision 23bb).
Pin: `~/botster-sessions/pins/stage1-plan.03c56e26.md`, mode `0444`.
SHA256: `885c9816bc027b8779db1b06488b5de05a9ad34a3cf395b2b3b6a401b6a3202e`.
Lead receipt: `msg_plugin-w_1791734941_a9c74d`.

I verify the parent chain, pin mode, hash, and committed bytes.
I read every inserted word in revision 23bc and the complete combined delta through revision 23bd.
Only docs/stage1-plan.md changes. All other plan words equal revision 23bb.
Revision 23bd replaces the temporary guard in 23bc. The revision table preserves that history.
The final plan requires one whole-log segment with one shared fixed label on both tiers.
The implementation PR must prove that the label cannot collide with a contract operation label.
The final plan does not authorize the withdrawn guard or an operation-free segment.

I read pty_matches and its literal and contains helpers at Contracts .36, commit `96f48d6d3d7a0e75e752d78433eff92efad0e77e`.
Literal hex and var parts compare against the concatenation of all segment bytes at the current position.
A relation part first requires a segment that starts at that position, then compares its operation before its length or contains checks.
A written_without_op part requires None. A segment with the fixed string label supplies Some(label), so that relation fails.
A written_by_op part also fails if the PR proves the required label cannot equal the fixed label.
A relation at another position fails because no segment starts there. No length or contains check bypasses that operation comparison.
Thus a single segment can supply literal bytes without falsely passing these relation forms under the stated noncollision condition.
The required reversal changes the labeled segment to an operation-free segment and proves that written_without_op then passes.
That proof, actual control dispatch, and the label's noncollision evidence remain implementation review requirements.

I read the route_expect_pty steps of both named dp_3 transcripts at the pinned contracts commit.
They contain the relation forms that require the later operation-attributed segments.
Both identifiers remain in the Core pending list and minimum list at d582853d.
They are absent from core-real-pending.txt there; that fact alone establishes no real-tier PASS proof.
The implementation PR must list the affected relation identifiers and keep their conformance accounting accurate on both tiers.
Form (B), including the probe protocol change, remains a steward question. This plan does not approve that protocol change.

AY-SI-1, AZ-SI-1, and BA-SI-1 remain closed with their recorded evidence.
This CLEAN accepts the final plan text. It approves no future implementation, pin, gate, source import, or pending removal.
The active code pin remains contracts-v0.1.36.
I ran no execution job.

VERDICT: CLEAN (0 open) at 03c56e269b9af7a38ab893973ef3109772d66e81

## Round 70 — revision 23be, 2026-10-11

Head: `eb417dc21a2d3ef5413da850d5abc076275dcf2f`.
Parent: `03c56e269b9af7a38ab893973ef3109772d66e81`.
Pin: `~/botster-sessions/pins/stage1-plan.eb417dc2.md`, mode `0444`.
SHA256: `d4f601b7147a151ce9e07f7a858674eea67fce9c65004ba4d6dee2990a4ac635`.
Lead receipt: `msg_plugin-w_1791735115_0eb697`.

I verify the parent, pin mode, hash, and committed bytes. I read every inserted word and the revision row.
Only docs/stage1-plan.md changes. All other plan words equal revision 23bd.
The final paragraph assigns the five real-tier controls to P6 and requires a design note first.
That explicit assignment supersedes the older no-owner statement; it does not approve an implementation or protocol change.

I read RealRoute's control set and RealHarness's dispatch at a0adb132.
They do not provide route_stream_holders. The pinned ou_1 transcript explicitly requires that route observation.
The plan directs ou_1 into core-real-pending with that reason. Actual source/list review and its gate remain due.
I treat the paragraph's local PASS and failure results as P3's reported evidence, not an execution result that I independently read.

I read the complete pinned am_2 steps, route driver's expect_pty, and R-56 plus R-61(b)/(c) at Contracts .36.
Step 10 submits five B bytes. Step 12 submits one A byte through r1. Step 13 waits for the host write completion.
Step 14 reads the route's PTY log at once and requires the six bytes BBBBBA.
The earlier await_pty at step 11 requires only one byte and precedes the route write.
An await_pty for s1 with min_bytes six immediately before step 14 fences the required total log length after both writes.
It adds a wait without weakening the literal expectation. The real probe read remains distinct from WriteInput completion under R-56.
R-61(b) names route_send and route_send_raw as session writes. R-61(c) identifies route_expect_pty as the input direction.
The proposed lint extension retains the shrink-only ratchet and sends unclear absence or relation cases to the steward.
The contracts PR remains HIGH. am_2 remains pending until its corrected tag is pinned and the applicable proof passes.

This CLEAN accepts the plan text. It approves no future implementation, protocol change, pin, gate, or completed list change.
The active code pin remains contracts-v0.1.36. The prior plan findings remain closed.
I ran no execution job.

VERDICT: CLEAN (0 open) at eb417dc21a2d3ef5413da850d5abc076275dcf2f

## Round 71 — revisions 23bf through 23bj, 2026-10-11

Head: `f561dd4e7132c801395f190b90e5dd0ebe020ec5`.
Prior accepted head: `eb417dc21a2d3ef5413da850d5abc076275dcf2f` (23be).
Pin: `stage1-plan.f561dd4e.md`, mode 0444.
SHA256: `bd43104c8b237618e3249f570899fb19c5b6d3e8c71d684fbd114d62f7691007`.
The pin bytes equal the committed plan bytes.

Sol read every added term in all five sequential plan deltas. Only the section-0 contracts row and five appended history rows change. Sol read the complete R-71 and R-72 drafts and PC's complete e1_1/a2_3 design note. Sol read the relevant current Core wrapper, control, and failed-start paths. Sol verified the annotated .38 tag and its target.

### Route controls and start hold

The route-control design uses the existing edge tap. The handoff hold suppresses write interest and restores it with a wake on release. Repeated hold before release is Bad. The cumulative write count uses bytes read plus unread bytes at the client socket. The plan requires an OS-specific proof on Linux and macOS. These are proposed implementations and proof requirements, not accepted measurements.

The holders control counts descriptors in the host process and worker group. The host byte count covers edge calls. The plan requires the sans-IO argument and its limit in the PR body. This scope does not make a descriptor count an independent proof of every byte access. The implementation review must check that limit and the named handoff proof.

R-71 replaces the open hold_start question with form (a) on both tiers. The hold occurs at control-link accept after spawn. The session stays Starting. The plan preserves the startup-clock condition, the contracts text correction, and 016c's transcript check. The plan rejects a product deferral added only for a test. The ad_1 and ad_7 paths preserve pending status when a faithful real form is unavailable. No future control implementation or probe change is accepted here.

### Minimum proofs and failure classification

The e1_1 design exposes the existing worker_file_name capability. The current wrapper selects the verified worker by role and uses the host-supplied file name. The future PR must use #264's own-trial rule and obtain the exact-head Linux trial PASS before pending removal.

R-72 permits the proposed failed-start timing change only for workers this host spawned. Sol compared all three conditions and all required evidence items with the draft. At the startup deadline, the failure keeps the existing link-lost reason. The session stays Starting while Core waits. The EOF posts no event. Routes close after the state. Adopted and Running workers retain their separate rules. The proposal adds no timer. Both edge orders, repeated real runs, and the A5-4 comparison remain implementation acceptance conditions.

The three additional real-pending entries have named owners and reasons. The plan reports their testkit passes. Sol does not independently claim those execution results. The later PR must supply its source, list, and exact-head evidence. The undiagnosed unknown-type route close remains a defect to diagnose. The ou_10 fence must still require input_refused.

### Tag order and current limit

Annotated tag `bc8d6fd139b073150007cc44225c6ce0234f0699` targets `5ff1b52be672bc107e80cd0d911110034745eda4`. Its complete annotation identifies #43, #44, #45, #46, and #48. It excludes #47 under the stall rule. Sol's accepted #48 evidence already covers this target.

The plan keeps #47 fixed at f303ccdb while reviewers assess R68-E1. All three reviewers must close the controlled real-tier proof before its merge and .39 tag. The suggested SIGSTOP/SIGCONT seam is future evidence, not a proof accepted by this review. The plan requires one Core pin move from .36 to .39 with the R-68 Core adapters. The comparison base stays .36. The body must include all .37 and .38 changes. The .38 fallback at about 00:30 PT on 2026-10-12 retains a later pin for R-68. No changed head inherits an earlier gate.

This CLEAN accepts the plan text. It accepts no future PR, tag, implementation, gate, or pending removal. The active Core code pin remains .36. Earlier findings remain closed. Sol ran no execution job.

VERDICT: CLEAN (0 open) at f561dd4e7132c801395f190b90e5dd0ebe020ec5
