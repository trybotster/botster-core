# Contracts #60: integration review (rc/r84-oracle-encode-row)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/60 (RC, `sess-1791701696-017d-d5022b29376b8c0aaeecc0885355c8ba`). Package reviewer: 016c. Ruling: R-84.

## Checks for the READY head (016c, 2026-10-11)

- Tier: HIGH. The PR adds `size` to the allowed `oracle_encode` arguments (`CONTROL_ARGS`, which today allows only `modes`). An argument list decides whether a transcript is valid, so it is gate-decision data (BUILD.md risk tiers, rule 1).
- The steward allows mouse input to use the stored session size. An explicit `size` argument is allowed and wins over the stored size.
- The `oracle_encode` row states its purity exactly: the control reads no session modes and no model state.
- The PR body states the HIGH tier and the prior art. 016c rejected the head `9bf62c77` because both were missing.

## Round 1: head `7df311e282260b29fadc143fdd0a97c1645bf1e8`

VERDICT: CLEAN (0 open) on this exact head. Merge-ready from the integration review, by fast-forward.

### What I checked and found correct

1. **The tier.** The PR body states HIGH under BUILD.md risk tiers rule 1: the `CONTROL_ARGS` entry decides whether a transcript is valid. The body has a prior-art section. These were 016c's reasons to reject `9bf62c77`.
2. **The `oracle_encode` row against R-84 and its follow-ups** (contracts main `e2c0ad9c842046454be6da7f5015f1d5b8a7dbb1`, `docs/steward-rulings.md` R-84):
   - the pinned libghostty `*_with_modes` encoder, the one that Core's write path uses (R-84 preamble, R-7);
   - (1) as corrected: a partial `modes` is laid over the modes of a fresh terminal of the pinned libghostty, computed from the binding at run time, and the author passes every mode that affects the encoding and differs from a fresh terminal. The row adds that `other_modes` merges key by key. That is the only reading that fits the author's rule: `ModeFlags::other_modes` is a `BTreeMap<String, bool>` (`botster-route-codec` `types.rs` 95), and a key that the author does not pass must keep its fresh-terminal value;
   - (2): `""` for `NotReported` and for focus with focus reporting off; `Unsupported{what}` is `ControlError::Refused({reason: "unsupported", what})`, matched with `expect_refused`;
   - (3): a paste with bracketed paste off is the text alone;
   - the mouse-size follow-up, word for word: "With `modes`, the call reads no session modes and no model state. A mouse event also uses the session's stored size (the size that the transcript set), unless the call passes `size`.";
   - without `modes`, note (1) applies (R-84 preamble).
3. **The input shapes.** The row's shapes are `key` (a `KeyInput` with no `repeat`), `mouse` (`{mouse}`), `focus` (`{focused}`) and `paste` (`{text}`). I counted every `oracle_encode` call in the Core corpus: 255 calls (216 key, 22 mouse, 11 focus, 6 paste). Each call matches its shape and passes `modes`, and no call passes `size` yet.
4. **The `oracle_state` row (R-84 (5), form (A)).** `title` and `cwd` are the last `Title` and `Cwd` events of the oracle's own replay, an empty string included, and each is absent when no event came.
5. **The vocabulary.** `CONTROL_ARGS` adds `size` as an optional argument of `oracle_encode` and nothing else. `size` already has the type `object` in `ARG_TYPES` (line 319), so `an_argument_type_belongs_to_its_control` covers it. The change only accepts more transcripts, and no committed transcript changes.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-rc-r84-oracle-encode-row-7df311e2-pool-20261011-132920-40848.log`, SHA256 `ad6bc4b1b5c2674c6cf48c0aa380b8ddab6b58c303b1780f4bd43545f7247548`.
- Header: head `7df311e282260b29fadc143fdd0a97c1645bf1e8`, base `e2c0ad9c842046454be6da7f5015f1d5b8a7dbb1` (BOTSTER_CI_BASE_REF), Linux pool.
- Result (read by me): all 10 steps PASS, with both vocabulary tests; `botster-gate: exit 0 after 379 s on msa1`.
- `git merge-base --is-ancestor` of main `e2c0ad9c842046454be6da7f5015f1d5b8a7dbb1` and the head is true, and the gate base is the main tip, so the head fast-forwards main.
