# Contracts #55: integration review (stage1/p5-r28-kitty-oracle)

Reviewer: contracts integration reviewer, session `sess-1791740105-017f-b6f22445c954bd9e60e1ae695593410e` (in place of Sol for contracts PRs).
PR: https://github.com/trybotster/botster-contracts/pull/55 (P5). Ruling: R-28 (1), in the oracle form that the lead gave (R-79 withdrawn).

## Round 1: head `5ee91959dc63a62469870096cd15dbbd0fd25682`

VERDICT: NOT CLEAN (3 open).

### F1 (HIGH, open): the stated tier STANDARD is wrong; the PR is HIGH

- **Evidence.** The PR body says "Risk tier: STANDARD". The PR changes `conformance/core-fence-exceptions.txt`. Each line of that file stops a fence-lint finding from failing CI, so the file decides pass or fail. That is gate-decision data under BUILD.md risk tiers rule 1, as a HIGH-path list is. #51 changed the same file and was HIGH. The PR is also routed to this integration review, which BUILD.md gives only to HIGH PRs.
- **Required.** The PR body states **HIGH** and the rule. No code change.

### F2 (LOW, open): the PR body claims that a masking model keeps the transcript passing; it does not

- **Evidence.** "If a later model masks the value instead, the transcript still holds." A model that masks `63` to `31` changes `kitty_flags` from `0` to `31`, so E2-3 posts `ModesChanged`, and step 5 (`forbid_events ModesChanged`) fails. This is correct under R-28 (1), which asserts "no `ModesChanged`" at this pin. But the claim in the merge record is false.
- **Required.** Remove the sentence, or replace it with this: a model change that acts on the sequence fails step 5, and that comes back to the steward.

### F3 (LOW, open): the oracle-free probe has no log

- **Evidence.** The body says that steps 0 to 5 plus `terminal_state` with `kitty_flags: 0` "PASSES on the testkit tier and on the real tier (macOS, local)", with no log path. My brief requires before/after evidence that I can read. The full id stays pending in Core until the oracle controls land, so this probe is the only evidence that the new prefix (the sentinel, `await_title`, `forbid_events`) works on Core's tiers.
- **Required.** The log paths (with the Core head and the contracts rev in each log), or the commands and output in a file under `~/botster-sessions/shared/core-stage1/evidence/`.

### What I checked and found correct

1. **R-28 (1).** The old form expected `kitty_flags: 63` (scripted) and waited for that `ModesChanged`. R-28 (1) says that it is wrong under either reading. The new form keeps the test's name and asserts what R-28 (1) asks, with the values taken from the oracle (R-7) and not hand-written:
   - step 3: `CSI = 63 ; 1 u` (`1b5b3d36333b3175`), then OSC 2 `r28-kitty` with ST as the last bytes. No `scripted` modes remain.
   - step 4: `await_title r28-kitty`, the R-57 fence. It pumps and never polls, and no launch or adoption is between the output and the wait.
   - step 5: `forbid_events ModesChanged{s1}` ("no `ModesChanged`", E2-3).
   - steps 6 to 9: `$k` = the oracle's `modes.kitty_flags` (`oracle_state`, "from the libghostty oracle", controls doc), `terminal_state` reports `$k`, and `$k < 32` (5.1A rule 2: no bit above `0x1F`).
   - steps 10, 16, 22: `oracle_encode` with `modes: {kitty_flags: $k}`, the reported flags (was a hand-written `31`), and each input log ends with that encoding, of the same length.
2. **Fences for later R-80 lint.** Every model read of s1 after the output (`oracle_state` and `terminal_state`) follows the distinct R-57 sentinel. So the transcript meets R-80's output-fence rule when it lands.
3. **The exception list.** The id's four entries are the same findings, renumbered for the five new steps: `no program_ready` 6 → 11 (the first `WriteInput`), `unfenced capture` 8 → 13, 14 → 19, 20 → 25 (the three `pty_input` reads). The list does not grow.
4. **FakeCore.** The id stays `"fake": "structural"`. FakeCore does not change. `requires.controls` adds `oracle_state`.

### Gate on the exact head

- Log: `~/botster-sessions/gates/botster-contracts-stage1-p5-r28-kitty-oracle-5ee91959-pool-20261011-112149-51525.log`, SHA256 `3d8f485390ecd34567905c33fa03c3cf55fbc0fa44c04053e329833906aba9f0`.
- Header: head `5ee91959dc63a62469870096cd15dbbd0fd25682`, base `8d79b70d1d68966911a844c73bdbc4bdcd8f85f0` (BOTSTER_CI_BASE_REF), Linux pool. The PR's merge base is the same commit.
- Result (read by me): all 10 steps PASS; `botster-gate: exit 0 after 425 s on msa1`.
- F1 to F3 need only PR-body and evidence changes. If no commit follows, this gate stays the merging gate for this head.

## Round 2: head `5ee91959dc63a62469870096cd15dbbd0fd25682` (unchanged; PR body and evidence only)

VERDICT: CLEAN (0 open) on this exact head. Merge-ready from the integration review.

- **F1 closed.** The body states "Risk tier: HIGH", with the reason: the fence exception file is gate-decision data (the rule of #51).
- **F2 closed.** The false sentence is gone. The body now says that step 5 is pin-specific: a pin that masks the value posts `ModesChanged{kitty_flags: 31}` and fails step 5, and such a pin roll must change the transcript.
- **F3 closed.** Logs under `~/botster-sessions/shared/core-stage1/evidence/p5-r28/`, each with a header that names Core `2866d5928f66954cf57e0a4959db2e587a543080`, contracts `5ee91959dc63a62469870096cd15dbbd0fd25682` and the command:
  - `probe-transcript.json` (SHA256 `b49bb8f931f23ceb9a928a887677bd2235232e0498b19780e2a9503d04ecc5c8`): steps 0 to 5 are the same as the PR's, then `terminal_state` with `kitty_flags: 0`;
  - `probe-testkit-mac.log` (SHA256 `e6601384b34ee95b2f1863f34b1afd226d049182002e145ebd5a7f681ee21a69`): `ok`, exit 0;
  - `probe-real-mac.log` (SHA256 `f2d6dc8142da007d9b10a15031aae3abba9745ba12db98e01618aec149d7436f`): `ok`, exit 0.
- The gate on this head stays the merging gate: SHA256 `3d8f485390ecd34567905c33fa03c3cf55fbc0fa44c04053e329833906aba9f0`, all 10 steps PASS. Its base `8d79b70d1d68966911a844c73bdbc4bdcd8f85f0` is still the main tip, and the PR's merge base is the same commit, so no main merge is needed.
- 016c ACCEPT on the same head.
