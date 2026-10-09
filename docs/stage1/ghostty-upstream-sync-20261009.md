# Ghostty upstream sync, 2026-10-09 (before the A6 pin move)

The Botster patch stack of the trybotster/ghostty fork, rebased onto the current upstream `main`, as the BUILD.md
"Ghostty fork policy" requires before every pin move. The stack of the 2026-10-04 sync (with patch 14 for audit A6,
R-32 and R-33) was five days old when the A6 pin move resumed, so it is synced again before the pin moves.
This file records the sync, a decision for each patch, the test results and the new pin candidate. The format follows
`ghostty-upstream-sync-20261004.md`, which stays the record of patch 14 itself.

## Summary

| Item | Value |
|---|---|
| Old base | upstream `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94` (#14509) |
| Old stack head | `botster/upstream-sync-20261004` = `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a` (never pinned on `v1`; `v1` pins `3f8eb6810`) |
| New base | upstream `main` = `9d479dcb1664e8dc3c66c7302ce596dc56b36d6d` (#14614), fetched 2026-10-09 |
| New stack head (pin candidate) | `botster/upstream-sync-20261009` = `39a68e822e505685d1e7fa9c6abeff21125b489c` |
| Patches | 24 commits in 15 patch groups (0 to 14). KEEP 23, REWORK 1 (patch 1, option numbers), DROP 0 |
| Conflicts | One commit, patch 1's first commit: upstream took terminal option 46 |
| Zig | 0.16.0, unchanged |
| Zig packages | Unchanged: no build file changed upstream, and the empty-cache build fetched exactly the 7 packages of `build_data.rs` |
| `xterm-ghostty` terminfo | Unchanged (no change under `src/terminfo/`) |
| Snapshot sources | Unchanged (`git diff 0bfddc16f 39a68e822 -- src/terminal/snapshot include/ghostty/vt/snapshot.h src/terminal/c/snapshot.zig src/terminal/stream_continuation.zig src/terminfo` is empty) |
| Commit authors | Every commit of the new branch has the author `Tonksthebear <5505282+Tonksthebear@users.noreply.github.com>`, with its earlier author date (see "Action audit") |

## What upstream changed since our base

Upstream `main` has 32 commits after `5dc28bb8e` (11 merges, 21 others). The ones that touch libghostty-vt:

| Upstream PR | Subject | Files |
|---|---|---|
| #14560 | libghostty-vt: program status protocol (OSC 7501) | new `osc/parsers/program_status.zig`, `osc.zig`, `stream.zig`, `stream_terminal.zig`, `Terminal.zig`, `c/terminal.zig`, `c/types.zig`, `terminal.h` (option 46), `osc.h` |
| #14538 | terminal: support DECSTR | `Terminal.zig` (`softReset`), `stream.zig`, `stream_terminal.zig`, `modes.zig` (`getDefault`), `terminal.h` (comments) |
| #14576 | terminal: apply a single shift to exactly one printed character | `Terminal.zig` |
| #14554 | terminal: print codepoints above 0xFF unmapped in a charset, as xterm | `Terminal.zig` |
| #14588 | formatter: write the cursor position relative to the margins in origin mode | `formatter.zig` |
| #14532 | input: UTF-8 encode the button in 1005 mouse reports | `input/mouse_encode.zig` |
| #14553 | terminal: stop line selection at prompt boundaries across blank cells | `Screen.zig` |

The others change the renderer (#14542), the CLI (#14605), the Danish translation and the VOUCHED list.

### Benefits for Botster

- **#14576** applies a single shift (SS2, SS3) to exactly one printed character, and **#14554** prints a codepoint above
  0xFF unmapped in a charset, as xterm does.
- **#14538** implements DECSTR (`CSI ! p`), the soft reset. Before, the model ignored it.
- **#14532** encodes a button code above 95 in mode 1005 as UTF-8, as xterm does. Botster's mouse input goes through
  the encoder (BUILD.md rule 1), so it takes the fix.

### Risks and behavior changes

- **The option numbers (patch 1).** #14560 took `GHOSTTY_TERMINAL_OPT_PROGRAM_STATUS` = 46. Patch 1's
  `GHOSTTY_TERMINAL_OPT_QUERY` and `_QUERY_MAX_BYTES` move from 46 and 47 to 47 and 48. The binding's `sys.rs` must
  follow, or it would set the program status callback with the query callback. I compared every constant of `sys.rs`
  with the `GHOSTTY_*` values of the new headers: these two are the only differences.
- **Core EV-8 queries.** `OSC 7501 ; ?` is a new request that upstream answers only while a program status callback is
  set. The binding sets none, so the model neither answers nor reports anything: **no behavior change**. Patch 1 does
  not report it as a query (the conflict resolution keeps upstream's `.program_status` arm unchanged). The 2026-10-04
  record treats DECRQCRA the same way. If Botster ever sets the callback, OSC 7501 needs a query kind first.
- **DECSTR.** `Terminal.softReset` sets ten modes to their defaults (cursor_visible, insert, origin, wraparound,
  reverse_wrap, reverse_wrap_extended, disable_keyboard, cursor_keys, keypad_keys, enable_left_and_right_margin), the
  scrolling region to the full screen, the pen and the protection to the defaults, the charsets to their start state,
  clears the saved cursor, sets the status display to main and the cursor style to the default, clears
  `modify_other_keys_2` (which patch 9's getter reports), resets XTCHECKSUM to its default, and resets a changed
  palette (OSC 4 colors; OSC 10 to 19 colors are kept). Before the sync a DECSTR changed none of these. It is model
  behavior: no contract clause names DECSTR or the palette, and E2-2 makes these modes model state, so Core reports
  what the model holds after the reset. The snapshot carries this state in the same fields as before. DECSTR does not
  call the `reset` callback (that is RIS only).
- **Formatter (#14588).** It changes only VT output that restores both the modes and the scrolling region. The
  binding's only formatter use (`reads.rs::screen_text`) emits plain text with every extra off: no effect.
- **Snapshots.** Unchanged. `modes.zig` gains only a getter, so `ModePacked` and the snapshot's mode bit registry
  (`snapshot/terminal.zig`) are unchanged; no snapshot source changed.

### Open upstream pull requests to watch (read-only check, 2026-10-09)

Method: `git fetch ghostty-org` and `git log 5dc28bb8e..ghostty-org/main --grep="#<n>"`. The user rule forbids any
API call against ghostty-org, so the status of a PR that did not merge is not checked.

| Upstream PR | Merged into `main`? | Effect |
|---|---|---|
| #14439 | no | Re-audit patch 6 and GHOSTSNP (ST-6, ST-6b) and DECRQM mouse replies (EV-8) when it merges. |
| #13332 | no | A new EV-8 query kind (XTQMODKEYS); possible conflict with patch 1. |
| #14362, #14200 (drafts) | no | DECRQM replies for modes 47/1047/1048/1049; #14200 changes `complete-v1.hex`. |

None merged, so no patch needs a re-audit for them.

## Zig packages

No `build.zig.zon`, `build.zig` or `pkg/` file changed upstream. The Mac run with an empty Zig global cache fetched
exactly the 7 packages of `build_data.rs` `ZIG_PACKAGES` (final Mac run below). The Linux gate's fetch step mirrors the
submodule URL and checks out the pinned commit, and its package store needs nothing new. `test-lib-vt` with the
shipped options needs no package outside `ZIG_PACKAGES`, on Mac and on Linux (step 2b below), so the list gets no
test-only part.

## Patch decisions

The patch numbers are those of `docs/stage1/libghostty-audit.md` (revision 12 has the same table). A `git patch-id
--stable` comparison of each old and new commit (the authors changed, so `git range-diff` marks every commit) shows 21
commits with the same patch and three changed: patch 1's first commit (the conflict), patch 6 (a context line: the
surrounding mouse code now writes the button with `printUnicodeCodepoint`, from #14532) and patch 14's first commit (a
context line: upstream's `.program_status = null` in `wrap()`).

| # | New commit | Old commit | Subject | Decision | Reason |
|---|---|---|---|---|---|
| 0 | `9ace459ce` | `cba7ba134` | terminal: own startHyperlink uri and id across capacity retries | KEEP | Use-after-free fix; upstream code unchanged. |
| 1 | `32a783ad9` | `13f66d661` | report every query with its exact bytes and stop after it | REWORK | Options renumbered to 47 and 48 after upstream's 46. `.program_status` and the `.xtversion` query report both kept. No behavior change. |
| 1 | `2d78928e8` | `0be524a08` | track query request boundaries; add CSI 14;2 t and 13;2 t | KEEP | EV-8 (c). |
| 1 | `85e7d8429` | `ed563afdd` | apply R-17 to every executed C0 control; restart at a C1 CSI introducer | KEEP | EV-8 (c), R-17. |
| 1 | `9c40369f5` | `50df7c7e1` | test that a C1 byte inside a string sequence is payload | KEEP | Pins parser behavior. |
| 1 | `5e4328efe` | `8cea067c1` | a C1 introducer ends an APC string in the query request tracking | KEEP | EV-8 (c). |
| 2 | `473f19472` | `09d186fd6` | report the notification source (OSC 9 or OSC 777) | KEEP | A2-4, A3-1. |
| 3 | `17ae694b1` | `c2dc8118c` | paste marker frame without payload rewrite | KEEP | IN-8, DP-5. |
| 4 | `a7f2a8910` | `cdefd75c3` | key events carry hyper, meta, shifted and base layout keys and F26 to F35 | KEEP | IN-9, 5.1A. |
| 4 | `e045bd0de` | `2dba9306f` | legacy Shift with no text; key tests use the structured sequence encoder | KEEP | IN-9, 5.1A rule iii. |
| 4 | `3fbf36adc` | `6d09770e4` | the legacy Alt test compares with the real base-character result | KEEP | Test correction. |
| 5 | `30541bfc4` | `7e96afe46` | export the xterm-ghostty terminfo name and source | KEEP | TI-1, A2-8. |
| 6 | `a10ad1bf3` | `91b06a537` | mouse cells as given, and getters for the active mouse enums | KEEP | IN-9, ST-4, R-13. Context line only (#14532). #14439 not merged. |
| 7 | `5dec71c70` | `786cd9e7b` | encode the typed replies to terminal queries | KEEP | EV-8. |
| 7 | `85086ba4f` | `1148713f6` | query reply encoders check their enums, use unsigned positions and parse in tests | KEEP | EV-8. |
| 8 | `64b68fe6e` | `db6c206a8` | report the OSC 52 selection and terminator on clipboard requests | KEEP | EV-3, EV-8. |
| 8 | `96002bd35` | `8da81d4cb` | the OSC 52 parser reads the whole selection | KEEP | Parser bug; upstream unchanged. |
| 9 | `014c0395c` | `fa481e17b` | terminal data getters for modifyOtherKeys state 2 and XTSHIFTESCAPE | KEEP | E2-2. #13332 not merged. |
| 10 | `622e4fbf2` | `5aa1e708b` | link libc on Linux so the static archive defines no allocator symbol | KEEP | P32. |
| 11 | `b2e4a7c44` | `c13162d1d` | the OSC 5522 write acknowledgement is written with the host's reply | KEEP | A13-1b. |
| 12 | `e5c8b2827` | `bed0c871a` | the keypad equals key has an application keypad sequence | KEEP | IN-9; test kept under P35. |
| 13 | `2b0dd287e` | `c370ef4d9` | the snapshot decoder takes the host's Kitty image storage limit | KEEP | ST-6b, P34. |
| 14 | `5cb2efb68` | `779907e0e` | an OSC 5522 write over the transaction limit reaches the callback with its size | KEEP | Audit A6, R-32. Context line only (`.program_status`). |
| 14 | `39a68e822` | `0bfddc16f` | tests that ignored MIME types stay outside the over-limit size | KEEP | R-33. |

No upstream change covers a patch, so nothing is dropped. Patch 14 is described in `ghostty-upstream-sync-20261004.md`.

## Test results

Raw logs and the script are in `docs/stage1/ghostty-upstream-sync-20261009/`. `evidence.sh` runs inside a botster-core
gate tree. It reads the build options and the package list from `crates/botster-terminal-ghostty/build_data.rs`, so the
tested configuration cannot drift from the shipped one. Its steps:

1. The lib-vt build with the binding's `GHOSTTY_BUILD_ARGS` and an empty Zig global cache, and the fetched package list
   against `ZIG_PACKAGES`. This step needs network.
2. `zig build test-lib-vt --summary all`, in two configurations:
   - 2a. upstream's default configuration (SIMD, and the Ghostty application's packages). This step needs network.
   - 2b. the shipped options: `GHOSTTY_BUILD_ARGS` without `build` and without `-Doptimize` (`-Demit-lib-vt -Dsimd=false
     -Dcpu=baseline -Demit-xcframework=false`), in Debug, so the tests run with the safety checks. Its Zig global cache
     holds only the 7 packages of `ZIG_PACKAGES`, copied from the gate's package store. The step reports any package
     that the run fetched outside that list.
3. `cargo nextest run -p botster-terminal-ghostty`.

The script also reports whether the fork tree has a project-local `zig-pkg/` directory, because Zig 0.16 takes packages
from it too. Both final runs report it absent.

**Scope.** Linux test-lib-vt uses the shipped options (emit-lib-vt, `simd=false`, `cpu=baseline`, Debug). Upstream's
default configuration (SIMD, the application packages) is covered by the Mac run only. The gate has no network, and the
package store is filled for `ZIG_PACKAGES`, the packages of the shipped build.

### Final runs (fork `39a68e822`, Core `2c636dbe`, 0 tracked changes in both, Zig 0.16.0)

The commit after `2c636dbe` adds only these records and logs; it changes no source and not `evidence.sh`.

| Step | Mac (`mac-run3-2c636dbe.log`) | Linux (`linux-run2-2c636dbe.log`) |
|---|---|---|
| 1. empty-cache build and packages | exit 0; `libghostty-vt.a` 10341912 bytes; **the 7 packages of `ZIG_PACKAGES`, the same list** | not run: no network (`NameServerFailure`) |
| 2a. `test-lib-vt`, default configuration | exit 0, **46/46 steps succeeded; 6751/6805 tests passed (54 skipped)** (the first fetch try failed with `TlsInitializationFailed`; the second passed) | not run: no network |
| 2b. `test-lib-vt`, shipped options, `ZIG_PACKAGES` only | exit 0, **42/42 steps succeeded; 6749/6805 tests passed (56 skipped)**; nothing fetched | exit 0, **41/41 steps succeeded; 6733/6805 tests passed (72 skipped)**; nothing fetched |
| 3. binding tests | exit 0, **134 passed** | exit 0, **134 passed** |

Linux: x86_64, image `botster-core-gate:bbc12a44852f`, Zig at `/usr/local/zig/zig`. The binding tests build
libghostty-vt from the fork through `build.rs` and link it statically, so the Linux-only patch 10 (link libc) is
exercised.

### How the Linux test configuration was found

- The default `test-lib-vt` needs the packages of the Ghostty application (JetBrainsMono, NerdFontsSymbolsOnly,
  fontconfig, harfbuzz, dcimgui, ...). The gate has no network (`NameServerFailure`).
- With `-Demit-lib-vt`, the build does not add the application's dependencies to the test and app executables
  (`build.zig` line 379, `src/build/GhosttyExe.zig` line 34). Alone, it still needs the `highway` package for SIMD
  (`linux-test-emit-lib-vt-b10a1dda.log`).
- With all the shipped options, the tests pass from the package store with no network
  (`linux-test-binding-options-b10a1dda.log`, the first pass, at Core `b10a1dda`, seeded from the whole store). Step 2b
  of the final runs shows that the 7 packages of `ZIG_PACKAGES` are sufficient.
- The lead chose this configuration for Linux. So `ZIG_PACKAGES` gets no test-only list, and the pool needs no change.

### Earlier runs

| Log | Core head | Result |
|---|---|---|
| `mac-run1-d1e747ca.log` | `d1e747ca` | (1) and (3) pass; (2) not run: a test-only package fetch failed, `TlsInitializationFailed`. |
| `mac-run2-7095fb76.log` | `7095fb76` | (2) default and (3) pass; (1) not run: a fetch from codeberg failed, `HttpConnectionClosing`. |
| `linux-run-7095fb76.log` | `7095fb76` | (3) passes. (1) reports exit 0 with no package fetched. It is not an empty-cache check: probably the gate's `prefetch-zig.sh` ran in the same fork tree first and left the packages in `zig-pkg/`. (2) default cannot run without network. |

## Pin candidate

- Fork branch: `botster/upstream-sync-20261009` on trybotster/ghostty (a new branch; no existing `botster/*` branch
  was changed: `botster/upstream-sync-20261004` is still `0bfddc16f` and `botster/upstream-sync-20261002` is still
  `3f8eb6810`).
- Head: `39a68e822e505685d1e7fa9c6abeff21125b489c`.
- Upstream base: `9d479dcb1664e8dc3c66c7302ce596dc56b36d6d`.

## What the P2-crate PR changes (branch `stage1/p2-fork-a6`), on top of the 2026-10-04 record's list

1. The submodule moves to `39a68e822`, and the `.gitmodules` branch to `botster/upstream-sync-20261009`.
2. `sys.rs`: `opt::QUERY` 47 and `opt::QUERY_MAX_BYTES` 48.
3. `docs/stage1/libghostty-audit.md` revision 12. GHOSTSNP.md: no snapshot fact changed.
4. The lead records the new pin, the upstream SHA and this patch list in the plan and the state log (BUILD.md fork
   policy).

## Action audit

| Repository or host | Action |
|---|---|
| trybotster/ghostty | One push, `git push origin botster/upstream-sync-20261009`, of the new branch at `39a68e822`, after `git remote get-url origin` showed `git@github.com:trybotster/ghostty.git` and `git ls-remote` showed no branch of that name. A first push at `750d3a4c2` (the same tree) was refused by GitHub's email privacy setting, because the commits' author email was the private one. Before the second push the author of each of the 24 commits was set to the GitHub noreply address with its author date kept (`git rebase --exec 'git commit --amend --author=... --date=...'`); the tree did not change. No force-push. |
| ghostty-org/ghostty (upstream) | `git fetch ghostty-org` only. No `gh` command, no API call, no PR, issue, comment or reaction. |
| trybotster/botster-core | Branch `stage1/p2-fork-a6` (this PR). |
