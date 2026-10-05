# Fork A6 reviewer handoff

Status: PAUSED under the lead's forwarded user order of 2026-10-04.
The reviewer must wait for an explicit resume instruction.

## Reviewer and repositories

- Reviewer session: `sess-1791171305-0113-e9a8c18db6f6331ae5c2274348370f8d`.
- Lead session: `sess-1790903471-008f-8b9f78eef51d48aba5a45748495fd673`.
- Implementer session: `sess-1791171304-0112-f36545cd8fa2e17ba1c1fe6f1863319d`.
- Worktree: `/Users/jasonconigliari/botster-sessions/trybotster-botster-core-stage1-review-fork-a6`.
- Reviewer branch: `stage1/review-fork-a6` in `trybotster/botster-core`.
- Verdict file: `verdicts/fork-a6.md`.
- Latest verdict commit: `debe4bbeae2568ffa6eee419198280334af03cd3`.
- The latest verdict commit is pushed to `origin/stage1/review-fork-a6`.

## Review rounds

| Round | Exact fork head | Exact botster-core head | Verdict commit | Verdict |
|---|---|---|---|---|
| 1 | `779907e0ec389c0a04de81dd4c092fda92b8325f` | `59abb1f3d9159962dbb5035fca54ba875404fa23` | `32600ae13cdd08d16e276d49e316f383fbdfac4c` | NOT CLEAN (2 open) |
| 2 | `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a` | `e676a2d577e07a350f8a85496fdd8e443491f28c` | `0338ca2bc6f787c6dac0b18b6c7e138c73d085dc` | NOT CLEAN (2 open) |
| 3 | `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a` | `65b2064de09a7a8c72f4edf42d6e1c2f467708ed` | `debe4bbeae2568ffa6eee419198280334af03cd3` | NOT CLEAN (1 open) |

The reviewed fork branch is `botster/upstream-sync-20261004`.
The reviewed Core branch is `stage1/p2-fork-a6`.
The upstream base is `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94`.
The synced stack head is `c370ef4d9910f9c8944c23207bdc3b425dbdaab2`.
The initial Core comparison base is `144b0234fb632bcbb5176b17c2fe55f3239405df`.

## Findings

- F-A6-01, CONTRACT: CLOSED by R-33. The model excludes data for ignored MIME types from the decoded size. The round 1 request to decode those bytes only to count them is withdrawn. The new fork tests follow the ruling.
- F-A6-02, HIGH: OPEN. Required final-head test evidence remains pending. The last implementer message states that the lead's HOLD prevents the remaining jobs.
- F-A6-03, LOW: CLOSED at round 3. The record now describes option 39 as a production setting. The record and audit state that A6 is implemented with verification pending.

No source or documentation finding remains open on the round 3 heads.
The reviewer ran no builds, tests, or gates in any round.
The supplied Mac log covers only the synced stack before patch 14.
That log does not close F-A6-02.

## Pending reviews

1. Review final-fork `test-lib-vt` evidence and the library build with `GHOSTTY_BUILD_ARGS`.
2. Review final-Core binding test evidence, the required Mac and Linux evidence, and the empty-cache Zig package verification.
3. Review the completed sync record and audit on the exact Core head that contains the evidence.
4. Review any delta from the round 3 source heads before CLEAN.
5. Review the pin-move PR metadata, including its required Prior art note. No final PR metadata was supplied in these rounds.

The lead's gate-evidence closure rule applies if only gate-dependent evidence remains after the HOLD ends.
The reviewer does not run gates.
CLEAN requires every finding to close on exact fork and Core heads.
The integration reviewer separately reviews the binding change because P3 uses the binding.

## Binding rules and review facts

- R-32 and R-7: contracts `9a00db8b85bc165ff93ee620db3c54ba3ddeac32`, tag `contracts-v0.1.14`.
- R-33: contracts main `14c86abedd59a1f528efe72603b031b304509f18`.
- Final A14: contracts main `69327d52cedb05ee9b9c63912b58d6f62a36917f`, manifest final33.
- A14 requires the native decode limit to equal the binding clipboard limit. The reviewed setter configures option 39 accordingly.
- A14 checks decoded size first and final contents size second. Each alias counts in the final contents size.
- P35 permits literal terminal bytes in the fork's Zig tests. Botster tests derive terminal expectations from libghostty.
- The reviewer independently compared all 22 rebased patches. The comparison showed 20 identical patches and two changed patches: option renumbering in patch 1 and adjacent context in patch 8.
- The inspected upstream changes replace no retained patch.
- The reviewer found no sign of an upstream push or contact in the inspected material.

## Communication and Git rules

The hard user rule remains: "nothing ever goes upstream. Push only to trybotster/ghostty; no PRs, issues, comments or any other contact with ghostty-org. Fetching upstream main is fine."
The lead clarified that this rule restricts Ghostty pushes and upstream contact. Reviewer verdict pushes to `trybotster/botster-core` remain required.
Before a verdict push, verify that `git remote get-url origin` names `trybotster/botster-core`.
Use an explicit remote and branch for every push.
Send verdicts to the implementer.
Send the lead only QUESTION, BLOCKED, a terminal CLEAN event, or the requested PAUSED event.
Never poll. End the turn while waiting. After a doorbell, call `receive_messages` once.
Do not spawn agents.

The worktree has only the spawner's existing `.gitignore` modification outside the committed verdict work.
The reviewer did not commit or alter that modification.
