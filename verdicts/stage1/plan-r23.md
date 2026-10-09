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
