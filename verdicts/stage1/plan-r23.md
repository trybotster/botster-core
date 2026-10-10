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
