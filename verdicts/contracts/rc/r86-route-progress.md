# Contracts (PR number and branch pending): integration review (R-86, RC)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
Implementer: RC (`sess-1791701696-017d-d5022b29376b8c0aaeecc0885355c8ba`). Package reviewer: 016c. Ruling: R-86 (pending; it governs now). Tier: HIGH (steward).
When the PR's branch is known, this file moves to `verdicts/contracts/<branch>.md`.

## R-86 checks for the READY head (steward, 2026-10-11)

### The mechanism

- `RouteClient::progress()` moves the client bytes that wait on each live route into the stream, as room appears.
- On the testkit, `progress()` also runs the workers' ready route work.
- The driver calls `progress()` for every live route in every wait loop.

### The limits

- `progress()` moves only bytes that the transcript already sent, and only as far as the stream takes them.
- It reads no Core event, and it polls nothing.
- It reads no route frames for the transcript.

### Evidence required

- The A5-4 table on both Core tiers, with no PASS to non-PASS.
- `dp_5_full_input_queue` passes in R-81's form.
- A driver self-test that fails when `progress()` is removed.
