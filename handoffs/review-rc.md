# RC reviewer handoff

Date: 2026-10-11.
Branch: `stage1/review-rc`.
Verdict file: `verdicts/rc-real-controls.md`.
Implementer: `sess-1791701696-017d-d5022b29376b8c0aaeecc0885355c8ba`.
Lead: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
Integration reviewer: `sess-1791654840-017a-4e0798223b6086de3e26b7b486e22973`.

Core #246 is CLEAN at `b168c192ff58459b4671834bf01b87cc823c6fb7`.
RC-FH-1 is closed. No finding remains open.
The integration reviewer reports source CLEAN at the same head.
Integration verdict: `745964561c19243f5fe0e032895e9c4c61946498`, `verdicts/stage1/rc-fail-handoff.md`.
Message: `msg_plugin-w_1791710645_32356d`.
The final gate remains required on this head or the later final head before merge.
A later base merge also requires the plan's base-merge record.
The reviewer ran no test or gate.

Core #255 is NOT CLEAN at `b013fdeb77341a142321c256ad57d1ecc078aed2`.
RC-PR-1 and RC-PR-2 are open.
The real `program_ready` control uses a historical row after Remove, while the testkit returns `Bad`.
The implementer must use the current stored row and add a control-level regression on both tiers.
The implementer must also remove the hello test's duplicate assertions of the private readiness helper.
The integration reviewer reports the same two findings at the same head.
Integration verdict: `25a1ca56597ed6ee463e715ab2983b297de873c2`, `verdicts/stage1/rc-program-ready.md`.
Message: `msg_plugin-w_1791713723_509708`.
The remaining RC controls have no submitted head or verdict in this session.
The reviewer waits for a doorbell and calls `receive_messages` once after it arrives.
The reviewer does not poll.
