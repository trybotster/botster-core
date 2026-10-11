# Contracts (PR number and branch pending): integration review (R-85, P3)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
Implementer: P3. Package reviewer: 016c. Ruling: R-85 (pending; it governs now). Tier: HIGH (steward).
When the PR's branch is known, this file moves to `verdicts/contracts/<branch>.md`.

## R-85 checks for the READY head (steward, 2026-10-11)

- No input-log assertion compares a `0d` byte, whatever the source of the expected bytes: a literal, a `let` value or an `oracle_encode` result.
- The driver enforces this at run time, on every tier:
  - it resolves the expected bytes of `pty_input` assertions, of `route_expect_pty`, and of `hex` checks over an input log;
  - when the resolved bytes contain `0d`, it fails the step with a typed transcript-defect error.
- There is no lint key list (R-7).
- `in_9_application_cursor_and_keypad` asserts `kp_enter` without the input log.
