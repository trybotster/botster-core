# PR #166 — Stage 1 pause #3 status and handoffs: integration review

## Round 1 — Head 84f4ab8

Reviewed head: `84f4ab8939b5ec48789eafa71f64636139b886ef`, branch `stage1/pause3-status`. Base and merge base: current v1
`144b0234fb632bcbb5176b17c2fe55f3239405df`. One commit: `docs/stage1-status.md` is rewritten, and 12 handoffs are added or
updated in `docs/handoffs/`. Docs only. The lead assigned this review. This reviewer ran no gate.

### Checked against the repositories (accurate)

- **v1 and the merges (section 3).** The merge commits of #132 to #141 in `origin/v1` match the table: `f96a9df`, `e43225f`,
  `38e6989`, `1d25d09`, `01fd389`, `393f403`, `38bbe58`, `a3b8af5`, `e8cf150` and `144b023`. The v1 head is `144b023`.
- **Open PR heads (section 4)** against `gh pr list` and `git ls-remote`:
  - #165 `71195e72`, #162 `59cda32`, #163 `40b63dc`, #164 `b8b37a6`, #142 `6e8d5b3`, #161 `dea90ed`;
  - the branches without a PR: `p2-fork-a6` `65b2064`, `p3-m2a-v1` `cce8598`, `p5-audit-host-fixes` `b3b5fac`,
    `p5-audit-host` `5308cb1`, `p6-real-harness` `276427d`, `p7-services-wire` `39bdc39`.
  All match.
- **Verdict commits.** All are on the review branches:
  - P5 `89893b2` (accepts #164 at `b8b37a6d`) and `b91cd55`, on `review-p5`;
  - P7 `ac6e612`, on `review-p7`; its file ends `VERDICT: CLEAN`, with head `6e8d5b3`;
  - P6 `d1a3869` ("accept PR 161 pin delta at dea90ed"), on `review-p6`;
  - integration `9edbc27` (the #164 CLEAN) and `cef28d1` (P5-F4 closed on #165; C3 and C4 open);
  - `review-fork-a6` `verdicts/fork-a6.md`: its last verdict is `NOT CLEAN (1 open)`, with F-A6-02 HIGH open, as section 6
    says.
- **The integration states in section 4** match this reviewer's verdict files:
  - #165: C3 MEDIUM and C4 LOW open at `71195e72`;
  - #162: C1 closed; the combined head must take in the final #165 head;
  - #163: G2 is checked again in its merge delta;
  - #164: CLEAN.
  The note that #165 lands only through #162 matches the integration correction `9b9cc72`.
- **Scope.** #142's diff is a new crate (`botster-guardian-core`), its design note, one fuzz line in `xtask`, and the mutant
  and lock entries. That is the scope the lead exempted from integration review. #161 is a contracts pin move: Cargo files,
  the ledger and pending lists, and small edits in P6's own testkit. Under the lead's pin-move ruling, it is not
  cross-package.
- **Contracts (sections 2 and 7).**
  - `contracts-v0.1.17` resolves to `1725abf`.
  - The manifest tags exist: final33 `c014be1`, final34 `62c36f6`, final35 `6271f10`.
  - R-30 to R-34 are in `docs/steward-rulings.md` at the tag.
  - `14c86ab` (R-33) is an ancestor of the tag and is on contracts main.
- **The merge order and resume steps (sections 4 and 9).** They match the lead's combined-gate ruling and the open findings:
  #165 round 2 (C3, C4), then the merge into #162, then delta CLEANs, then one gate, then #163, #164, #142 and #161.
- **`docs/handoffs/integration-reviewer.md`** equals this reviewer's handoff file at the time it was copied. It records each
  PR's state, the pending reviews, and the lessons.

### Findings

#### S1 [LOW] OPEN — `core-lead.md` keeps two stale facts in its current-rule sections

- Location: `docs/handoffs/core-lead.md`.
  - Section 1, line 13: "The integration branch is `v1` (head `38e6989…` at writing)". v1 is `144b023`.
  - Section 3, line 40: "No integration reviewer is staffed now; ask the orchestrator before a PR that needs one."
- Evidence: the later dated sections and `stage1-status.md` section 8 list a staffed Opus integration reviewer
  (`sess-1791168757-0109`). A new lead who reads the hard rules first is told the opposite.
- Required: mark both lines as history, or correct them. For example, use "v1 head: see `stage1-status.md`", and "the
  integration reviewer is staffed (see the 20:05 staffing table)".

#### S2 [LOW] OPEN — The #165 row understates what the P3 reviewer has done at `71195e72`

- Location: `docs/stage1-status.md` section 4, the #165 row: "P3 has not yet reviewed `71195e72` at package level."
- Evidence: the P3 reviewer did review `71195e72`. By message, it accepted P5-F4's closure and adopted C3 and C4 as package
  findings. It pushed no verdict for that head: `review-p3` is still at `ea4415fe`, which is the CLEAN at `c03bcfb1` and is
  now history only. A resuming reviewer could redo that work or rely on the old CLEAN.
- Required: write it as "the P3 reviewer has no pushed verdict at `71195e72`. Its last one, `ea4415fe` (CLEAN at
  `c03bcfb1`), is history only. It has accepted C3 and C4 as package findings."

VERDICT: NOT CLEAN (2 open: S1, S2, both LOW)

## Round 2 — Head 61c82ee

Reviewed head: `61c82ee631c6bd601d2ab60fd6783139d9c05b28`. Delta `84f4ab8..61c82ee`, one commit, docs only:
`docs/handoffs/core-lead.md` and `docs/stage1-status.md`. The base is still current v1 `144b023`.

- **S1 CLOSED.**
  - `core-lead.md` section 1 now gives the v1 head as `144b023` at pause #3, with a pointer to the status file.
  - The hard rule now names the staffed Opus integration reviewer (`sess-1791168757-0109`, branch `stage1/integration-review`).
  - A reading-order note marks sections 5 to 7 as the 2026-10-04 snapshot. It keeps sections 3, 8 and 9 and every "Ruling"
    line in force.
- **S2 is overtaken by events.** The new #165 row says that P3's reviewer "pushed NO verdict for that head" and that
  `stage1/review-p3` "is still at `ea4415fe`". That was true when the commit was written. A re-check with `git ls-remote` now
  shows `stage1/review-p3` at `e31e4ae8`: "review(p3): record remaining guard waits and prohibited sleep fixtures", round 87.
  Its file says: "PR #165 at `71195e72` has F45 and F46 OPEN" (the package names for C3 and C4). F39 is closed in #165 and
  stays open for #163's later merge delta.
- No other documented head has moved: #165 `71195e72`, #162 `59cda32`, #163 `40b63dc`, #164 `b8b37a6`, #142 `6e8d5b3`,
  #161 `dea90ed`, v1 `144b023`.

#### S3 [LOW] OPEN — Record P3's round 87 in the #165 row

- Required: the #165 row says that P3's package verdict at `71195e72` is round 87, `e31e4ae8`, NOT CLEAN. F45 and F46 (the
  integration C3 and C4) are open, and the round 86 CLEAN at `c03bcfb1` is history. Section 4's #163 row can name the
  package F39 next to the integration G2 for its merge delta.

VERDICT: NOT CLEAN (1 open: S3, LOW)
