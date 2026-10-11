# Contracts (PR number pending): integration review (p3/am2-route-pty-fence)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
Implementer: P3. Package reviewer: 016c. Ruling: R-83 (pending; it governs now). Tier: HIGH (steward).

## R-83 checks for the READY head (steward, 2026-10-11)

### The rule and the lint

- A `route_expect_pty` needs an earlier `await_pty` fence on its session, with no write between the fence and the expectation.
- The lint's count is a floor:
  - the `hex` bytes count in full;
  - a relation part counts at its op's known length (`bytes`, `text` or a paste body), or 1 byte when the op's bytes are not known;
  - `var` parts count 0;
  - `one_of` counts its shortest alternative.
- The reviewer checks that each fence names the full written count: a literal total, or the R-74 count that a matcher binds.

### Scope

- All 31 findings in 29 transcripts at `63128469` are fenced in this PR.
- The only exception: `dp_5_full_input_queue`, listed with the reason "rewritten by R-81 (RC)", and only if this PR merges before R-81's.

### Addition (B): a bound `written_bytes` as the fence count

- A `written_bytes` that a route matcher binds from `input_done` or `input_refused` is a valid fence count when all of these hold:
  - it is bound in an earlier step;
  - its route is attached to the same session;
  - the op's clause defines `written_bytes` as the PTY bytes.
- Where a clause gives `written_bytes` another meaning, the transcript derives the PTY count, and the PR body cites the clause for each such finding.
- **Steward correction (2026-10-11): DP-5b is not such a case.** Frozen DP-5b defines `written_bytes` as the encoded PTY bytes of the path paste: the path, the space and any markers, never the file content. So a DP-5b `written_bytes` meets the third condition directly.
- Check `dp_5b_path`: its fence count `1 + $wb11 + 1` is valid only when the PR body names the two extra bytes as other writes.

### Addition (C): encoder-owned counts

- An encoder-owned count is bound with `oracle_encode` plus a `let` sum. It is never hand-written, and its id stays pending.
- A literal count is allowed only for bytes that are not encoder-owned.
- A class-C id that passes today keeps its form and is listed with "encoder count, oracle_encode (RC)".
