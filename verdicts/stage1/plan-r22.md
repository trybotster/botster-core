# Integration review: Stage 1 plan, revision 22

Reviewer: integration reviewer (Opus), `sess-1791517267-0168-401cc27284f329b42ec84da0de9d91e0`. This reviewer runs no
build, test or gate.

## Round 1 — NOT CLEAN on head b3d142d4

Reviewed head: `stage1/plan` `b3d142d4344afeea48b0527de99ee2f3f053c3c5`. Delta `ad03636f..b3d142d4`: `docs/stage1-plan.md`
and `docs/stage1-clauses/` (owners.py, p1, p3, p4a, p7 lists). Sources for the rulings: the lead handoff
(`handoffs/core-lead.md`, rulings of 2026-10-04 and 2026-10-08) and BUILD.md at `contracts-v0.1.17`.

### Checked, no finding

- **Section 0 pins.** `contracts-v0.1.17` = `1725abf1d95aa880cb38b80a2010470e5abcb98f`, and `manifest-final35` =
  `6271f1043e2160752f138d7c624b33ea31444f2a`, which is an ancestor of the tag. The sha256 of A14 candidate 3, A15 candidate
  2 and A16 candidate 3 at the tag equal the plan's three values. MANIFEST.md at the tag gives final31 = HP A8, final32 =
  HC A6, final33 = Core A14, final34 = Core A16, final35 = Core A15, as the plan says.
- **owners.py.** I ran the plan's `owners.py` (`python3 -I`) on an export of `contracts-v0.1.17` `conformance/`. The lists
  it writes are byte-identical to the committed lists. Total 673. P1 126, P3 167, P4a 152, P7 67. The ledger has 675 Core
  ids; 2 are in `withdrawn.txt` (A9-2). The third withdrawn id is HP's (`wp_3_...`), so "675 minus 2" is correct.
- **#163 split and M2a.** Section 6.1 and section 11 items 3 and 4 match the lead's M2a ruling: develop and review now,
  and merge only after the real-PTY regression is rebuilt on botster-test-process and passes the gate.
- **#164 rulings.** The bounded Silent loop, the slow-tier selection of every in-crate `slow_tests` module, and Q6 (the
  AppArmor path limit, documented by #164, open with the steward) match.
- **Q7** correctly goes to the orchestrator, because BUILD.md requires a reviewer CLEAN on the exact head.
- **Leak detection.** "Fails any test that leaves a process behind" matches BUILD.md testing rule 10 ("CI fails if a test
  leaves processes behind"), so it adds no new requirement.

### PR1 MEDIUM — the gate-decision rule does not say what happens to v1's `mutants_job` exclusion

Section 8 says: "A function that decides whether a gate step passes ... is a tested pure function; an exclusion of it is
refused." The P6 check enforces "no exclusion names a gate-decision function". The lead's r22 note names `mutants_job` as
an example. But v1 (`ee7dd16c`) has the entry `xtask/src/ci\.rs:\d+:\d+: replace mutants_job -> Result<\(\)> with
Ok\(\(\)\)$`. #167 round 10 accepted that entry on these grounds:
- the decisions of `mutants_job` were moved into pure, mutation-tested functions (`mutation_verdict`, `parse_outcomes`,
  `platform_exclusions`);
- only the whole-body `-> Ok(())` replacement of the glue is excluded;
- every gate log shows the step's summary line, which that mutant would remove.

As written, P6's check has no defined result for v1's own entry. The plan must say which rule holds:
- (a) the #167 rule: a decision function is the pure function, and a glue body whose decisions are all such functions may
  have its whole-body replacement excluded, with the argument; or
- (b) no exception: the `mutants_job` entry is removed, and a test proves that a `-> Ok(())` of the step turns the gate
  red (owner P6, PR B).

### PR2 MEDIUM — two revision-22 rules differ from BUILD.md at the pin, and the plan cites no authority for either

- **The full gate before the first READY** (section 8). BUILD.md "How each stage runs" says: "The gate is ONE CI run on
  the exact head after CLEAN. No local full suites; implementers run focused tests only."
- **The pool gate, with `botsterq` and `testq` retired** (section 8, R8). BUILD.md at the pin describes the gate through
  `testq` and the Mac fallback through `botsterq`. Its "Resources" rule says: "on the Mac: ONE heavy job at a time ...
  through `botsterq run --exclusive`".

The lead may not change BUILD.md by plan. For each rule, cite the decision that allows it (the orchestrator's or the
user's, with its date), as Q7 does for the merge-delta check. If no such decision exists, make it an open question to
the orchestrator.

### PR3 LOW — the mutation-timeout ruling is not stated exactly

The lead's ruling: the profile is P6's, in PR B, "with a red-on-revert proof"; there is "no grandfathering"; and (from
ruling (a)) the default tier's 2 s budget stays. Section 8 says none of these three things. Also, the red-on-revert list
in "Policy is enforced mechanically" does not include the profile. Add all three, and add a proof that a hanging mutant
fails the step.

### PR4 LOW — the macOS rule is attributed to the wrong ruling and is incomplete

"A macOS-only mutant may be excluded off macOS only (`OFF_MACOS_EXCLUSIONS`) when a focused Mac mutation run catches it
(lead ruling of 2026-10-04)." These are two different rules:
- **2026-10-04:** macOS-only (`cfg`) code needs a focused Mac mutation run, because the Linux gate cannot compile it. The
  Mac log is the required evidence, named in the READY and the PR.
- **#167, 2026-10-08 (E2, F51):** a mutant that is equivalent off macOS but not on macOS is excluded off macOS only,
  through `OFF_MACOS_EXCLUSIONS`. The entry gives the off-macOS argument. A unit test
  (`only_a_gate_off_macos_adds_the_off_macos_exclusions`) proves the per-OS scope. A macOS test or Mac run catches the
  mutant there.

State both rules, each with its own date.

### PR5 LOW — anchor items 10 (amended) and 12 (extended) are not stated

Section 6.1 and R12 say only "the one anchor binary, which never reaps what production reaps". Section 8 describes the
run-wrapper. The rulings of 2026-10-08 also require the following:
- **Item 10:**
  - The anchor keeps at most ONE reserve child in the group.
  - It reaps only that reserve, by exact pid (never `waitpid(-1)`, 0 or a negative pgid).
  - The KILL rounds are bounded by the existing CLEANUP bound, with no new value, and they report failure at the bound.
  - The pid + start-time checks and the group-move refusal stay.
  - A test proves that the anchor never reaps a production child.
- **Item 12:**
  - No production code may depend on reparenting to pid 1. If some does, that is a QUESTION, and production code is not
    changed to suit the wrapper.
  - The wrapper has a red-on-revert proof.
  - The macOS gap is documented.
  - Mutation runs use the same wrapper and census.

State these rules, or name the P6 brief (path and hash) as their binding source.

### PR6 LOW — section 6.3 has two staffing statements that disagree

The older bullets still say "a Sonnet implementer and a Sol reviewer" (Opus for P3, P4a and P5) and "One integration
reviewer (Sol)". The new bullet says that Opus implementers, Sol high-effort reviewers and an Opus integration reviewer are
"in force". Mark the older bullets as superseded, so the plan states one rule.

VERDICT: NOT CLEAN (6 open: PR1, PR2 MEDIUM; PR3 to PR6 LOW)

## Round 2 — NOT CLEAN on head ada086b8

Reviewed head: `ada086b8cdce96ffa205934a6a83b1da23a23612`. Delta `b3d142d4..ada086b8`, one commit, `docs/stage1-plan.md`
only.

- **PR1 CLOSED.** The #167 rule is stated: the decision is a pure function with red-input tests, and an exclusion may cover
  only the I/O shell, whose entry names that function. v1's `mutants_job` entry names `mutation_verdict`,
  `parse_outcomes` and `platform_exclusions`, so P6's check has a defined result for it.
- **PR2 CLOSED.** The pool gate cites the orchestrator's RESUME of 2026-10-08. The orchestrator state log (line 630,
  20:38, "RESUME (user)") gives "gates via botster-gate on Nomad pool tier 3 cap 3+1", and `brief-resume-20261008.md`
  "What changed" item 1 gives the same rule. (The plan says "item 4"; neither source numbers it 4. Give the source's own
  number, or no number.) The full gate before READY is withdrawn and became Q8.
- **PR3 CLOSED.** The owner (P6, PR B), the red-on-revert proof, "no grandfathering" and the 2 s default budget are stated.
- **PR6 CLOSED.** The old staffing bullets are marked superseded.
- **PR4 still open (LOW, wording).** "with a unit test of the off-macOS argument" is not what #167 has. The entry states
  the argument (why the mutant is equivalent off macOS). The unit test
  `only_a_gate_off_macos_adds_the_off_macos_exclusions` proves only the per-OS scope. Fix: "with the off-macOS
  equivalence argument in the entry and a unit test of the per-OS scope".
- **PR5 still open (LOW).** Item 12's precondition is stated without its other branch: if production code relies on
  reparenting to pid 1, that is a QUESTION to the lead, and production code is not changed to suit the wrapper. Add it.
  The P6 brief sha256 `30b8f604…f144` matches the file.
- **PR7 LOW (new): Q7 and Q8 are answered.** The orchestrator state log, line 660 (2026-10-08 23:20): "r22 Q7 ... YES w/
  conditions; Q8 (one full pool gate before first READY; counts as merge gate if head unchanged) YES. BUILD.md 629c177."
  botster-contracts `629c177a6fdea316b44689171c691a8e30240b79` (23:20:32, 32 s after this head) adds both rules to
  BUILD.md, with Q7's three script conditions, and it also records the pool gate (2026-10-05). That commit is after
  `contracts-v0.1.17`. The plan must record: Q7 and Q8 ANSWERED with the exact conditions; the BUILD.md commit that
  holds them (it is not in the pinned tag); and the section 8 READY rule as BUILD.md now allows it (one optional full gate
  before the first READY, which counts as the merge gate when the CLEAN head is the same head).

VERDICT: NOT CLEAN (3 open: PR4, PR5, PR7, all LOW)

## Round 3 — NOT CLEAN on head be9b9c7a

Reviewed head: `be9b9c7a316085e9ac7741e32c47222fc4a43564`. Delta `ada086b8..be9b9c7a`, one commit, `docs/stage1-plan.md`
only. The whole revision-22 delta `ad03636f..be9b9c7a` was read across rounds 1 to 3. The section 0 pins and the
`owners.py` lists did not change after round 1.

- **PR7 CLOSED.** Q7 and Q8 are marked ANSWERED and cite BUILD.md at contracts `main` `629c177`. The Q7 text matches
  BUILD.md's three conditions exactly. The Q8 rule makes BUILD.md's "may run ONE full gate" mandatory in this stage. That
  is stricter than BUILD.md, and BUILD.md allows it, so it is not a finding. P6 owning the script is the lead's
  assignment.
- **PR4 still open (LOW)**, unchanged at this head: "with a unit test of the off-macOS argument". See round 2.
- **PR5 still open (LOW)**, unchanged at this head: the "else a QUESTION; no production change to suit the wrapper" branch
  of item 12. See round 2.

### PR8 LOW — three citations do not match their sources

1. The pool-gate bullet cites "the orchestrator's RESUME order of 2026-10-08, item 4". Neither the orchestrator state log
   (line 630) nor `brief-resume-20261008.md` ("What changed" item 1) numbers that rule 4. BUILD.md at `629c177` now
   states it ("Gates run on the shared Nomad pool (2026-10-05)"). Cite that, as Q7 and Q8 do.
2. BUILD.md at `629c177` dates the Q7 and Q8 rules "orchestrator, 2026-10-09". The plan says "2026-10-08". The log
   time, 23:20 local, is 2026-10-09 in UTC. Use the date that the cited document gives.
3. BUILD.md's Q7 rule says: "the integration reviewer's CLEAN is still required where rule 3 below applies". The plan
   says "the integration reviewer's CLEAN still applies where required". Neither text says whether the integration
   reviewer must give a new CLEAN on the merged head, or whether the CLEAN on the reviewed head carries over. State which,
   because it decides whether a cross-package base-only merge needs a round from this reviewer.

VERDICT: NOT CLEAN (3 open: PR4, PR5, PR8, all LOW)
