# Hub→client contract v1: Amendment 1 (candidate 5): the package source path

Owner: plan auditor. Reviewer: Sol Hub A. Date: 2026-09-30. **This file is the amendment, in the post-freeze amendment path.** Candidate 5 replaces candidates 1 to 4 (the last was `4e52e355…0f17`). Candidate 2 has Sol Hub A's design acceptance; candidates 3 to 5 keep that design, align the code names and bounds with HP, and add PM-4's package-root containment. Sol Hub A found that a symlink escape cannot be detected before the fetch, so PKC-1 checks the path in two stages. It uses the error codes and bounds of HP Amendment 1 candidate 5 (`5fe279bb…a1e9`); the client `reason` is the HP `detail.code`. The frozen base is `hub-client-contract-v1-candidate-parity45.md` (sha256 `19a2ade453cd2597ac2c73586245694b9d78a4d72346842aee26d2d950173b34`), which stays immutable. The joint manifest lists the frozen base and this amendment together. Until the amendment is accepted, the base stands alone.

**Why:** Hub→plugin Amendment 1 (PK-1 to PK-9) names the package manifest file (`botster-plugin.json` at the package root) and adds an optional `path` inside a `git` source and a catalog entry, so that one repository can hold several plugins. The client's package and catalog shapes must match. This amendment cites the Hub→plugin contract by contract name and clause id only.

## Edits to existing clauses

| Clause or row | The edit |
|---|---|
| Section 6, `package` row, `source` | `source` is `{ kind: local, path }`, `{ kind: git, url, ref?, path? }`, or the catalog entry it came from (HP PM-1, PK-5). For `git`, `path` is the package root inside the repository (HP PK-2). For `local`, `path` is the directory itself, and no separate root path exists. The lock gains `source_path` (HP PM-3, PK-5). |
| Section 6, `catalog` row, `entries` | Each entry's `source` is `{ git url, ref?, path? }`, and `kind` names the package kind of HP PK-6 (`plugin`, `agent_definition`, `accessory`). Two entries may name the same `url` and `ref` with different `path`s (HP PK-7). |
| A.9, `package` `create` (install) | The `git` source is `{ kind: "git", url, ref?, path? }`. |
| A.9, install refusals | Install also ends `failed` with the HP PK-3 reasons: `manifest_missing` (the error says that `path` exists when the repository root has no manifest), `manifest_ambiguous`, `manifest_malformed { path? }` (only a file that is not UTF-8, not valid JSON, not one object, or oversize, or a schema failure with no specific code). **The existing specific admission codes still apply** (for example `manifest_unknown_key`, `manifest_bad_version`, `manifest_bad_scope`, `manifest_bad_allow`). Install also ends `failed` with `path_not_found` or `path_not_contained` (PKC-1). |

## New clause

| # | Clause | Test |
|---|---|---|
| PKC-1 | **Package source path, checked in two stages.** A client sends `path` only for a `git` source, or picks it from a catalog entry.
- **Before any fetch, syntax only:** `path` is relative, uses `/`, has no `.`, `..` or empty segment, and is not absolute. Its length is bounded by the frame limit `max_frame_bytes`, with no separate path limit (HP PK-2). A violation is refused at once with `invalid_arguments { field_errors }` carrying `detail.code: path_invalid`, with `effect: none`, and nothing is fetched. **A client must not promise that every bad path fails before the fetch:** some fail only after it (next item).
- **After the fetch, before static admission and before any code (containment):** `path` must name a directory inside the fetched tree, and neither it nor anything under it may be a symlink that leaves the tree (HP PK-2, PM-4). A violation ends the install operation `failed { reason: path_not_found }` (not a directory) or `failed { reason: path_not_contained }` (it resolves outside the tree). Nothing is staged, admitted, locked or run.
- **Package-root containment (HP PM-4) still applies on top:** at unpack, a file or symlink under the package root that resolves outside the **package root** (not only outside the fetched tree) is refused, as PM-4 already states. So a package never reaches into another package of the same repository.
- A client shows each package's `path` next to its `url`, so two plugins from one repository are distinguishable.
| `conf::pkc_1_git_source_path_round_trips`, `conf::pkc_1_bad_path_syntax_refused_before_fetch`, `conf::pkc_1_symlink_escape_fails_after_fetch_before_admission`, `conf::pkc_1_missing_directory_fails_path_not_found`, `conf::pkc_1_two_packages_one_repo_distinct_rows` |
