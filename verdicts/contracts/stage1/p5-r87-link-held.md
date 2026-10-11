# Contracts (PR number and branch pending): integration review (R-87, P5)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
Implementer: P5 (`sess-1791517266-0166-9e69a1c3914b450c0b8a98aa5c29e374`). Package reviewer: 016c. Ruling: R-87 (pending; it governs now). Tier: HIGH (steward).
When the PR's branch is known, this file moves to `verdicts/contracts/<branch>.md`.

## R-87 checks for the READY head (steward, 2026-10-11)

- A new read-only control: `link_held {session}` gives `{held}`. `held` is true when Core holds a frame from S's worker link that it has read but not accepted (EV-5b, parked).
- In `ev_5_poll_wakes_pending_work`, after step 8: `await_control link_held` with `expect held: true`.
  - The wait pumps and never polls.
  - It is a precondition fence only. Step 13's 0 ms `Woken` stays the TM-6 claim.
- A `Woken` wait with a timeout is rejected.
