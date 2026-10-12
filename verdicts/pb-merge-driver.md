# PB item 0: list merge driver

Date: 2026-10-12. Round: 1.
Core head: `797f090ac8fa8f182b4201d524e3add74d8d60e5`.
Core base: `2683de7ccbb1360d124422b3b6a2dd196b046c7e`.
Tool directory: `~/botster-sessions/shared/tools/botster-list-merge/`.

VERDICT: NOT CLEAN (1 open)

This is a source review before READY. The implementer has not supplied the gate log yet.
The reviewer built nothing locally. The reviewer ran one Python reproduction to prove PB0-1.

## Scope and tier

The Core diff changes only `.gitattributes`. It names a driver for all nine current `conformance/*.txt` files.
`core-real-proofs.txt` uses the whole line as its key because one ID can have several proofs.
The other eight files use the ID as their key.

Plan revision 23co permits the tool outside Core under the xtask freeze.
The lead corrected LOW to STANDARD on 2026-10-12. The Core diff changes no listed HIGH path or gate decision.
The lead requires the pair review of the shared tool against conditions (a)-(d), with no integration review.

## PB0-1 — MEDIUM — A failed text merge can erase ours

Evidence: `botster-list-merge:164-186` and `194-211`.
`git_text_merge` returns stdout and a Boolean. It does not distinguish a text conflict from a command failure.
After a parse error, `resolve` accepts that stdout. `run` then replaces the ours file with those bytes.

The reproduction uses these inputs:

```python
base = b"conf::a\nconf::a\n"
ours = b"conf::a\nconf::a\n\x00\n"
theirs = b"conf::a\nconf::a\nconf::b\n"
```

The duplicate ID selects the documented text fallback. Git refuses the binary input with return code 255 and empty stdout.
The driver returns 1 but replaces ours with an empty file. It also discards Git's error message.
The output has neither the original records nor conflict markers.

This breaks the fallback's stated purpose: preserve Git's result and avoid a failed merge with no useful conflict content.
The duplicate-ID fallback is an explicit supported path, so it must preserve ours when Git cannot produce a merge result.

Required change:

1. Distinguish a command failure from an ordinary text conflict.
2. Preserve the ours file when Git fails to produce a merge result.
3. Report Git's error and return a failure status.
4. Add a regression test through `run` or a real Git merge. Assert that the failed fallback preserves ours.

## Other review results

- The record merge follows the required ID-set equation on clean, parsed inputs.
- The record merge keeps conflicting reasons unresolved.
- Whole-record comparison preserves the tested group comment after removal of its first ID.
- The in-place tool removes the stale-copy problem. The installed command uses Git when the script is missing.
- The test and README establish that `git merge-tree --write-tree` runs the installed driver.
- The three-way ID-set check remains the acceptance check.
- A clean text fallback must have the expected ID set. That check permits independent comment and reason edits.
- The replay log reports 80 two-sided file merges, with zero clean-result ID-set mismatches.

## Reviewed tool hashes

These hashes identify the unversioned files reviewed in this round.

| File | SHA256 |
|---|---|
| `botster-list-merge` | `dc2f1b2e9889622db9c1812b94e554eefed810c42ce6926e39c6527dd3cfb820` |
| `test_botster_list_merge.py` | `a2129cddf4825f18e41a719d41c336bb5a74cbce0f02a7b0f4e05194fa4eb447` |
| `README.md` | `25a4df1af86493c5c374cfc0d17d5d683318fb3948b59f148b417b8d2cc3f55c` |
| `replay-v1.py` | `419b996ef18149d653a1094876859aa77a4fc04da5ce63a9b35000e26d9fcf15` |
| `replay-v1-2683de7c.log` | `796f98b9e069937ba96f17b9d824a9180671d77ff9453ba7282874c5a6e2a79b` |
