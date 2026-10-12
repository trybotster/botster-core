# PB pair reviewer

Date: 2026-10-12.
Branch: `stage1/review-pb`.
Base: `e37b91d6d4b5c98739f0f1ebc475a1e13d48d71c`.
Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
Implementer: `sess-1791831684-0186-3c235523bd222d09db6295d105ccae49`.

The reviewer waits for the implementer's design notes and READY messages.
The reviewer builds nothing locally.
Item 0 has a round 2 source verdict in `verdicts/pb-merge-driver.md`: CLEAN.
PB0-1 is closed. The failed text fallback preserves ours and reports Git's error. Four focused tests passed.
The implementer supplied READY #326 at `797f090ac8fa8f182b4201d524e3add74d8d60e5`.
The Linux gate reports ten PASS steps and exit 0. Its head matches the CLEAN source verdict.
The verdict file records the gate log and its SHA256.
The implementer sent FINAL-READY to the lead. The reviewer waits for B1's design note or a train review request.

B1 round 1 at `d8358ee920892da144c1f6289a735451bafda28f` is NOT CLEAN with one LOW finding, PB1-1.
The source logic has no open finding. The design or PR description needs BUILD.md's prior-art note.
The B1 gate is running. The reviewer built nothing locally.

## Review authority

- The package brief is `~/botster-sessions/shared/core-stage1/brief-part-b.md`.
- The shared rules are `pair-common.md` in that directory.
- The plan pin is `~/botster-sessions/pins/stage1-plan.e13ad492.md`, revision 23cn.
- The plan's SHA256 matches `e13ad49285a086ae4b22b66668b2efc3bd2a54d6b18a2afe5d5b34e50f1e686b`.
- `~/Projects/botster-contracts/docs/BUILD.md` governs the review, including the merge train and Testing rules 9 and 10.
- The Core contract set and steward rulings govern observable behavior.
- R-106 governs a failed `waitid`. R-107 and R-108 replace the old drain premises in #163.
- PR #163 at `40b63dceb6e3f3d7be69a1488ac77eccc121a071` supplies history only.
- The lead's handoff records the 2026-10-12 `#163 part B ruling`.

## Review scope

1. Review the list merge driver first. Check the three-way ID set, reason conflicts, comments, sort order, and local installation.
2. Review B1 against R-106. Check every non-EINTR errno, including ECHILD, and the exact payload group.
3. Check that B1 sends no end message, logs the errno, exits non-zero, and completes Stop through Lost(WorkerGone).
4. Check B1's injected-error evidence and the zero-start-time test. Check the agreed file split with P3 before implementation.
5. Review B3's process guard and bounded accept, FIFO, and link waits. Require authority for every time value.
6. Review F28's focused Mac pool mutation evidence. Check every survivor's test or stated reason.

B1, B3, and F28 remain HIGH under the brief.
For item 0, the lead moved the tool outside Core under plan 23co.
The lead corrected the attributes-only Core PR's LOW label to STANDARD on 2026-10-12.
The reviewer checks the shared tool against conditions (a)-(d), with no integration review.
Every HIGH finding must close before CLEAN, including LOW findings on a HIGH PR.
Write each verdict to `verdicts/pb-<item>.md` on this branch.
Push the verdict and send its line with the reviewed head and verdict commit.
After a doorbell, call `receive_messages` once. Never poll.

## Branch preparation

The initial branch pointed to old main at `053148f6e8e63c3c38b2cbabd54a6e4e8211143c`.
The reviewer preserved it as `stage1/review-pb-bootstrap`.
The reviewer created the assigned branch from `origin/v1`.
