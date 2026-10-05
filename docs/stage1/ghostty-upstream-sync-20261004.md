# Ghostty upstream sync, 2026-10-04, and fork patch 14 (audit A6, R-32)

The Botster patch stack of the trybotster/ghostty fork, rebased onto the current upstream `main`, as the BUILD.md
"Ghostty fork policy" requires before a new patch. Then one new patch, for audit finding A6 (issue #148) under steward
ruling R-32.
This file records the sync, a decision for each patch, the new patch, the test results and the new pin candidate.
The format follows botster-contracts `docs/ghostty/upstream-sync-20261002.md`.

## Summary

| Item | Value |
|---|---|
| Old base | upstream `f523504ea5c9f41d150d1eb93cc7a748b90f9361` (#14506) |
| Old stack head | `botster/upstream-sync-20261002` = `3f8eb6810bb673aa782b047de21783ac81fb1121` (the pin of botster-core `v1` `144b023`) |
| New base | upstream `main` = `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94` (#14509), fetched 2026-10-04 |
| Synced stack head | `c370ef4d9910f9c8944c23207bdc3b425dbdaab2` (the 22 rebased commits) |
| New stack head (pin candidate) | `botster/upstream-sync-20261004` = `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a` (patch 14, two commits, on the synced stack) |
| Patches | 24 commits in 15 patch groups (0 to 14). KEEP 21, REWORK 1 (patch 1, option numbers), NEW 2 (patch 14 and its R-33 tests), DROP 0 |
| Conflicts | One, in patch 1 (`bcdcad95b`): upstream took terminal options 44 and 45 |
| Zig | 0.16.0, unchanged (`minimum_zig_version`) |
| Zig packages | One hash changed: `iterm2_themes` (see "Zig packages") |
| `xterm-ghostty` terminfo | Unchanged (no change under `src/terminfo/`) |
| Snapshot sources | Unchanged (`git diff 3f8eb6810 0bfddc16f -- src/terminal/snapshot include/ghostty/vt/snapshot.h src/terminal/c/snapshot.zig src/terminal/stream_continuation.zig` is empty) |

## What upstream changed since our base

Upstream `main` has 28 commits after `f523504ea` (8 merges). The ones that touch libghostty-vt:

| Upstream PR | Subject | Files |
|---|---|---|
| #14483 | terminal: add support for DECRQCRA and XTCHECKSUM | `stream.zig`, `stream_terminal.zig`, `Terminal.zig`, new `xt_checksum.zig`, `c/terminal.zig`, `terminal.h` (options 44 and 45), `lib_vt.zig` |
| #14508 | input: encode ctrl+alt+shift+backspace in legacy mode | `function_keys.zig`, `key_encode.zig` |
| #14509 | terminal: fix crash after shrinking columns splits a wide char | `PageList.zig` |
| #14530 | libghostty-vt: zero the decode_png output before the callback | `c/sys.zig`, `sys.h` (comments) |
| #14503 | terminal/tmux: don't count unstored bytes against max_bytes | `tmux/control.zig` |
| #14515 | libghostty: document allocator alignment as log2 | `allocator.h` (comments) |

The others change the Ghostty application, the CLI (`ssh-cache`), the VOUCHED list and the `iterm2_themes` package.

### Benefits for Botster

- **#14509** fixes a crash when a column shrink splits a wide character (the model resizes on every `Resize`).
- **#14530** and **#14515** do not affect the binding: it uses no `ghostty_sys_*` call and passes a null allocator.

### Risks and behavior changes

- **The option numbers (patch 1).** #14483 took `GHOSTTY_TERMINAL_OPT_XT_CHECKSUM_REPORT` = 44 and `_XT_CHECKSUM_EXTENSION` = 45. Patch 1's `GHOSTTY_TERMINAL_OPT_QUERY` and `_QUERY_MAX_BYTES` move from 44 and 45 to 46 and 47. The binding's `sys.rs` must follow, or it would set the checksum options with a callback pointer and a size. I compared every `GHOSTTY_*` enum constant of `include/` between the old and new heads; these four are the only differences.
- **Core EV-8 queries.** DECRQCRA (`CSI Pi ; Pg ; Pt ; Pl ; Pb ; Pr * y`) is a new request that upstream answers only when `XT_CHECKSUM_REPORT` is on; it is off by default, and XTCHECKSUM is ignored while it is off. Before the sync, the stream logged `CSI y` as unimplemented and did nothing. The binding does not set the option, so the model neither answers nor changes state: **no behavior change**. Patch 1 does not report DECRQCRA as a query (the conflict resolution keeps upstream's handler unchanged). If Botster ever enables the option, DECRQCRA needs a query kind first (`QueryKind::from_c` refuses an unknown kind).
- **Input encoding (IN-9).** #14508 adds the legacy table entry `ctrl+alt+shift+backspace` = ESC BS (and `CSI 27;8;127~` with modifyOtherKeys state 2). Botster takes it through the encoder (BUILD.md rule 1). It does not touch the lines of patches 4 or 12.
- **OSC 52 and OSC 5522:** unchanged by upstream. Patch 14 changes OSC 5522 (below).
- **Snapshots:** unchanged. #14483 adds a terminal flag (`flags.xt_checksum`) that the snapshot does not carry; it changes only the checksum replies, which are off.

### Open upstream pull requests to watch (read-only check, 2026-10-04)

Method: `git fetch ghostty-org` and `git log f523504ea..ghostty-org/main --grep="#<n>)"`. The user rule forbids any API call against ghostty-org, so the status of a PR that did not merge (open, closed, changed) is not checked.

| Upstream PR | Merged into `main`? | Effect |
|---|---|---|
| #14483 | **yes** (`04d685c6a`) | Conflict with patch 1 (option numbers), resolved above. No new query kind for Botster while the option is off. |
| #14508 | **yes** (`eee709f5d`) | Re-audited patches 4 and 12: no overlap; the new entry is taken through the encoder. |
| #14515 | **yes** (`3425025e5`) | Documentation only. |
| #14439 | no | Re-audit patch 6 and GHOSTSNP (ST-6, ST-6b) and DECRQM mouse replies (EV-8) when it merges. |
| #13332 | no | A new EV-8 query kind (XTQMODKEYS); possible conflict with patch 1. |
| #14362, #14200 (drafts) | no | DECRQM replies for modes 47/1047/1048/1049; #14200 changes `complete-v1.hex`. |

## Zig packages

`build.zig.zon` changed one package: `iterm2_themes` moved from `N-V-__8AAM94BAAFk_hn4UW0x_OBD2g0vOwexeAAyWNNo4eB`
(`ghostty-themes-release-20260921-150923-0b55a9e.tgz`) to `N-V-__8AAAfDBACe1jGqjr9jIG3UAK8KbzJKQ7xrzYoau-_a`
(`ghostty-themes-release-20260928-151043-99d9701.tgz`). The old hash is in `build_data.rs` `ZIG_PACKAGES`, so the lib-vt
build resolves this package. The P2-crate PR replaces it. The Linux gate has no network: its fetch step
(`ci/remote/fetch-public.sh`) must fetch the new package before the gate. The list of 7 is re-verified with an empty
cache in "Test results".

## Patch decisions

The patch numbers are those of `docs/stage1/libghostty-audit.md` (revision 11 has the same table).
`git range-diff f523504ea..3f8eb6810 5dc28bb8e..c370ef4d9` shows 20 commits as `=` (identical) and two as `!`:
patch 1's first commit (the conflict) and patch 8's first commit (a context line only: upstream's new
`.request_xt_checksum` arm sits next to the changed `.clipboard_contents` arm).

| # | New commit | Old commit | Subject | Decision | Reason |
|---|---|---|---|---|---|
| 0 | `cba7ba134` | `ee1875dd3` | terminal: own startHyperlink uri and id across capacity retries | KEEP | Use-after-free fix; upstream code unchanged. |
| 1 | `13f66d661` | `bcdcad95b` | report every query with its exact bytes and stop after it | REWORK | Options renumbered to 46 and 47 after upstream's 44 and 45. No behavior change. |
| 1 | `0be524a08` | `79ae024c3` | track query request boundaries; add CSI 14;2 t and 13;2 t | KEEP | EV-8 (c). |
| 1 | `ed563afdd` | `303013782` | apply R-17 to every executed C0 control; restart at a C1 CSI introducer | KEEP | EV-8 (c), R-17. |
| 1 | `50df7c7e1` | `99ceb84cc` | test that a C1 byte inside a string sequence is payload | KEEP | Pins parser behavior. |
| 1 | `8cea067c1` | `50d8494c5` | a C1 introducer ends an APC string in the query request tracking | KEEP | EV-8 (c). |
| 2 | `09d186fd6` | `d9531aca1` | report the notification source (OSC 9 or OSC 777) | KEEP | A2-4, A3-1. |
| 3 | `c2dc8118c` | `51c80ac71` | paste marker frame without payload rewrite | KEEP | IN-8, DP-5. |
| 4 | `cdefd75c3` | `fddc30c7c` | key events carry hyper, meta, shifted and base layout keys and F26 to F35 | KEEP | IN-9, 5.1A. |
| 4 | `2dba9306f` | `1bef3ee4a` | legacy Shift with no text; key tests use the structured sequence encoder | KEEP | IN-9, 5.1A rule iii. |
| 4 | `6d09770e4` | `ec9a95861` | the legacy Alt test compares with the real base-character result | KEEP | Test correction. |
| 5 | `7e96afe46` | `9f476b148` | export the xterm-ghostty terminfo name and source | KEEP | TI-1, A2-8. |
| 6 | `91b06a537` | `f8aa86979` | mouse cells as given, and getters for the active mouse enums | KEEP | IN-9, ST-4, R-13. #14439 not merged. |
| 7 | `786cd9e7b` | `72d54d39e` | encode the typed replies to terminal queries | KEEP | EV-8. |
| 7 | `1148713f6` | `49e68f944` | query reply encoders check their enums, use unsigned positions and parse in tests | KEEP | EV-8. |
| 8 | `db6c206a8` | `41024b252` | report the OSC 52 selection and terminator on clipboard requests | KEEP | EV-3, EV-8. Context line only. |
| 8 | `8da81d4cb` | `cfcd2c21c` | the OSC 52 parser reads the whole selection | KEEP | Parser bug; upstream unchanged. |
| 9 | `fa481e17b` | `eafd0967b` | terminal data getters for modifyOtherKeys state 2 and XTSHIFTESCAPE | KEEP | E2-2. #13332 not merged. |
| 10 | `5aa1e708b` | `f0d70e3f1` | link libc on Linux so the static archive defines no allocator symbol | KEEP | P32. |
| 11 | `c13162d1d` | `1eb68104a` | the OSC 5522 write acknowledgement is written with the host's reply | KEEP | A13-1b. |
| 12 | `bed0c871a` | `d5bebc7e2` | the keypad equals key has an application keypad sequence | KEEP | IN-9; test kept under P35. |
| 13 | `c370ef4d9` | `3f8eb6810` | the snapshot decoder takes the host's Kitty image storage limit | KEEP | ST-6b, P34. |
| 14 | `779907e0e` | none | an OSC 5522 write over the transaction limit reaches the callback with its size | NEW | Audit A6, R-32 (below). |
| 14 | `0bfddc16f` | none | tests that ignored MIME types stay outside the over-limit size | NEW | Review finding F-A6-01, R-33 (below). |

No upstream change covers a patch, so nothing is dropped.

## Patch 14: the OSC 5522 over-limit report (audit A6, R-32)

**Before.** Above `kitty_clipboard_write_max_bytes` (64 MiB by default), `WriteState.data` returned `error.TooLarge`, and
the handler answered the program `status=EFBIG` itself and dropped the transaction. The clipboard callback never ran.
That broke EV-3 as A13 replaces it ("never silent"), the 9B row of `clipboard_bytes`, EV-7, and A13-1b (the worker is
the only acknowledger).

**After (through the C API).**

- `wrap()` sets the handler flag `kitty_clipboard_write_count_over_limit`. The Zig stream handler keeps it off, so the
  Ghostty application still answers EFBIG (its test `kitty clipboard oversized text write aborts with EFBIG` is
  unchanged).
- When data goes over the limit, the transaction frees its spool and keeps one counter. Later chunks are decoded only to
  count them, through the same streaming base64 decoder, with a fixed stack buffer (3 KiB). Validation is the same:
  invalid base64 is still EINVAL, and a payload that has padding before its end is still invalid (the counting loop
  checks the piece boundary).
- The commit still calls the callback. The sized `GhosttyClipboardWrite` has two new trailing fields: `too_large`
  (true) and `total_len`; `contents_len` is 0, which does not mean "clear" when `too_large` is set.
- **`total_len` is the decoded size of the whole transaction:** every decoded byte, including the data of a MIME type
  that a later chunk of the same type replaced; an alias adds nothing. This is the steward's reading of R-32's "full
  length" (2026-10-04, through the lead). For every other write, `total_len` is the sum of the contents' data lengths.
- The model sends no acknowledgement of its own. The callback's reply is the only one; a callback that does not reply is
  a denial, as for every write.

**Zig tests.**

- `clipboard_write.zig`: delivery at the limit with counting on; the decoded size with a replaced region and an alias
  (20 bytes, where the final contents would be 16), and the spool's capacity is 0 after the limit; base64 validation
  after the limit (padding inside a payload, invalid characters, padding at the end of the first counting piece of a
  long payload, missing padding at commit); a payload longer than one counting piece.
- `c/terminal.zig` "clipboard write over the max bytes reaches the callback with its length": with a limit of 5, a
  write of 5 bytes is delivered and the only bytes on the pty are the reply's `DONE`, written during the reply. A write
  whose chunks go over the limit answers nothing when it goes over; at the commit the callback gets `too_large`, no
  contents and `total_len` 11 (a replaced `text/plain` region included, the alias adds nothing; the final contents
  "Hi", "<b>" and the alias "Hi" would be 7), and the only bytes on the pty are the reply's `EIO`, written during the reply. The next transaction
  is delivered normally.

**Binding.** `on_clipboard_write` reads `too_large` and `total_len` when `size` covers them, and posts
`ClipboardWrite{contents: None, total_bytes: <decoded size>, too_large: true}` with the IO_ERROR reply. The public type
is unchanged, so the worker's use of it is unchanged.

**Core A14 (final33, contracts main `69327d5`).** A14-1 defines the decoded size (payload bytes that the model decodes,
replaced regions included, aliases excluded) and the contents size (each alias entry at its full length). A14-2
decides in two steps: the decoded size first (patch 14's report), then the contents size. A14-3 requires the model's
decode limit to equal `clipboard_bytes`, so the binding now sets option 39 (`CLIPBOARD_WRITE_MAX_BYTES`) from
`set_clipboard_limit`, wherever it creates a terminal (`Terminal::new` and the snapshot restore both call it). The
native memory part of A6 is implemented: the model holds at most `clipboard_bytes` of payload for one write.
**A6 status: implemented; verification pending** until the test evidence in "Test results" is complete and accepted.

**R-33 (contracts main `14c86ab`): ignored MIME types.** The model ignores the data of MIME types past its count limit
(64 per write) and never decodes it. That data is not in the decoded size and does not count against the limit; a
write whose kept types are within the bound is not `TooLarge` because of ignored types; their bytes stay in `Output`
(OU-12); invalid base64 in them is not counted. Patch 14 already behaves so (the ignored-type path returns before any
decode); review finding F-A6-01 asked for the opposite, and R-33 settles it. The second commit `0bfddc16f` adds the
tests: 64 kept types at the limit plus a valid and an invalid ignored type deliver the 64 kept types; over the limit,
ignored types add nothing (64 one-byte kept types report 64). The C API test also gained an alias, which adds nothing
to the decoded size.

## Test results

Raw logs and scripts are in `docs/stage1/ghostty-upstream-sync-20261004/` (its `README.md` lists each file).

### Mac (Apple Silicon, Zig 0.16.0 from mise, through `botsterq run --exclusive`)

- Synced stack `c370ef4d9`, 0 tracked changes (`mac-sync-c370ef4d9.log`): `zig build test-lib-vt --summary all`
  exit 0, and the library build with the binding's `GHOSTTY_BUILD_ARGS` exit 0 (`libghostty-vt.a`, 10189944 bytes).
  The log kept only the end of the test output, so it has no Build Summary line; the recorded run repeats it.
- PENDING (lead HOLD on all heavy jobs, 2026-10-04): the recorded runs of `0bfddc16f` and `c370ef4d9` with their Build
  Summary, and the Zig package list from an empty cache.

### Linux

PENDING (lead HOLD). The ghostty repository has no `ci/remote/job.sh`, so `botster-gate` cannot run it directly. The
plan: run `test-lib-vt` and the binding tests on Linux through the botster-core gate of the P2-crate PR, once the
fetch step has the new fork commit and the new `iterm2_themes` package.

## Pin candidate

- Fork branch: `botster/upstream-sync-20261004` on trybotster/ghostty (a new branch; no existing `botster/*` branch
  was changed: `botster/upstream-sync-20261002` is still `3f8eb6810`).
- Head: `0bfddc16fdf1e9b71f7662fbfa8314cd497fd92a`.
- Upstream base: `5dc28bb8eebaf57a6c793a406bfea8c632d4fa94`.

## What the P2-crate PR changes (branch `stage1/p2-fork-a6`)

1. The submodule `crates/botster-terminal-ghostty/vendor/ghostty` moves from `3f8eb6810` to `0bfddc16f`, and the
   `.gitmodules` branch from `botster/upstream-sync-20261002` to `botster/upstream-sync-20261004`.
2. `sys.rs`: `opt::QUERY` 46 and `opt::QUERY_MAX_BYTES` 47; `ClipboardWrite` gains `too_large` and `total_len`;
   `opt::CLIPBOARD_WRITE_MAX_BYTES` (39), which `set_clipboard_limit` sets in production (A14-3).
3. `events.rs`: the over-limit report (above). `lib.rs`: `set_clipboard_limit` also sets option 39 (A14-3).
4. `build_data.rs`: the new `iterm2_themes` hash.
5. `docs/stage1/libghostty-audit.md` revision 11. GHOSTSNP.md: no snapshot fact changed.
6. The lead records the new pin, the upstream SHA and this patch list in the plan and the state log (BUILD.md fork
   policy).

## Action audit

| Repository or host | Action |
|---|---|
| trybotster/ghostty | Two pushes of `git push origin botster/upstream-sync-20261004:refs/heads/botster/upstream-sync-20261004`: the new branch at `779907e0e`, then a fast-forward to `0bfddc16f`. An unpushed local commit for reading (b) was dropped before any push when R-33 chose reading (a). Each push followed `git remote get-url origin` showed `git@github.com:trybotster/ghostty.git`. No force-push. Afterwards `git ls-remote origin 'refs/heads/botster/*'` shows the earlier branches at their earlier heads: `upstream-sync-20261002` `3f8eb6810`, `vt-core-stage1-c` `ada251c5e`, `vt-core-stage1-b` `85a8d8eb1`, `vt-core-stage1` `c78b4beb4`, `p2-old-stack-backup` `1fd093bd3`. |
| ghostty-org/ghostty (upstream) | `git fetch ghostty-org` only. No `gh` command, no API call, no PR, issue, comment or reaction. |
| trybotster/botster-core | Branch `stage1/p2-fork-a6` (this PR). |
