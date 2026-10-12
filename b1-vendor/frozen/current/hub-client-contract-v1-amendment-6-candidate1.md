# Hub→client contract, Amendment 6 (candidate 1): the installed package key, and a detached worktree head

The base is the Hub→client contract of `frozen/current` (manifest final31), with Amendments 1 to 5, which are unchanged except where A6-3 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the lb-cli-suite pair (CLI part), through the Stage 0 lead.

**Why.**
- **The install result.** CLI-PL3 prints `Installed <name> <version> (<source>)`, "with `<version>` from the version of the installed `package` row, read by `get(package, <key>)` after the operation", and treats a `staged` package as success. `install` is `create(package, {source, enable?})`, an accepted durable operation (HP PM-1). A.7b lists `package.install` with failure reasons only, and no result. The package key (`<name>`, or `<name>@<target_id>` for a target-scoped package, HP CP-8) is known only to the Hub, which reads the manifest. The CLI must not read the manifest (CLI-PL10), and a diff of `list(package)` before and after is not reliable while other operations run. So the client has no reliable way to learn the key.
- **The detached head.** The `worktree` row is `id`, `target_id`, `path`, `branch`, `state`, `active_sessions`, and `branch` is a string. A worktree whose HEAD is detached has no branch. The CLI spec (CLI-WT1) prints "a marker for a detached head", and ruling R-25 requires the marker in conformance. No row field carries this fact.

| # | Clause | Test |
|---|---|---|
| A6-1 | **`package.install` names the package it installed.** The A.7b row for `package.install` gains `result { package, version }`, in the same place as `package.upgrade`'s result: `package` is the package key (the `package` row's `id`), and `version` is the manifest `version` the Hub admitted (HP PD-1).<br>- The result is present on **every** end of the operation in which the Hub admitted a package: `succeeded`, and an end that leaves the package `staged` for approval (HP PM-8). A client then reads `get(package, <package>)` for the row.<br>- A `failed` end carries no result, as before. | `conf::a6_1_install_result_names_the_package_key_and_version`, `conf::a6_1_staged_install_carries_the_result` |
| A6-2 | **A worktree row shows a detached head.** In the `worktree` row, **`branch` is optional**: it is present, as the branch name, when the worktree's HEAD is on a branch, and **absent when HEAD is detached** (absent, not null, as the other optional fields of section 6). A new field **`head`** (string, always present) is the commit HEAD points at, as a full hexadecimal object name. A client shows a detached worktree from the absent `branch`. | `conf::a6_2_detached_worktree_has_no_branch_and_a_head`, `conf::a6_2_branch_worktree_has_branch_and_head` |

## A6-3. Text replaced

1. **A.7b, the `package.install` entry** (Hub→client Amendment 2): "`package.install` (`failed` with …)" reads "`package.install` (`result { package, version }`; `failed` with …)" (A6-1).
2. **Section 6, the `worktree` row:** "`id`, `target_id`, `path`, `branch`, `state`, `active_sessions`" reads "`id`, `target_id`, `path`, `branch?`, `head`, `state`, `active_sessions`" (A6-2).

Every other clause is unchanged.
