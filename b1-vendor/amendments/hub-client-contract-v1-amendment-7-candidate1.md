# Hub→client contract, Amendment 7 (candidate 1): the upgrade preview carries the HP `breaking` list

The base is the Hub→client contract of `frozen/current` (manifest final35), with Amendments 1 to 6, which are unchanged except where A7-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. Found by the crate-catchup pair (Stage 0), in its audit of the contract crates against the final amendments.

**Why.**
- A.9's `package.preview_upgrade` row returns "`{ old_commit, new_commit, old_hash, new_hash, changed_files, manifest_diff, grant_diff, breaking?: [{ consumer, interface }] }` (HP PM-7 `preview`; …)". A.9's `hub.preview_package_upgrades` row returns "`{ previews: [{ package, …the preview_upgrade result, including breaking }], ambiguous: [{ consumer, interface, candidates }] }`".
- HP Amendment 8, A8-3, changes the cited clause HP PM-7. `preview_upgrade` returns "`breaking: [{consumer, interface, range, interface_version}]`", and "the list is empty when nothing breaks". `preview_package_upgrades` "returns the same `breaking` list for the whole set, next to `ambiguous`".
- The contract's **References** paragraph says that "an amendment on one side needs no change here unless a cited clause changes meaning (gate 5)". PM-7 is cited, and its meaning changed. So the two A.9 rows must carry the HP shape. A client must see the consumer's `range` and the new `interface_version` to decide what to name in `accept_breaking`, and the bulk upgrade is one resolution, so its preview needs one list for the whole set.
- The refusals already match HP A8-3: Amendment 2 lists `conflict { breaking_not_accepted }` with `detail { consumers }` for `package.upgrade` and `hub.upgrade_packages`. They are unchanged.

| # | Clause | Test |
|---|---|---|
| A7-1 | **The upgrade preview carries HP A8-3's `breaking` list.**<br>- **`package.preview_upgrade`:** the result's `breaking` is **always present**, a list of **`{ consumer, interface, range, interface_version }`**, with the entries and meaning of HP A8-3: one entry for each consumer requirement whose bound provider is the previewed package and whose `range` (the consumer's declared range, HP PD-5) does not contain the new `interface_version`. It is the empty list when nothing breaks.<br>- **`hub.preview_package_upgrades`:** the result gains a top-level **`breaking`**, always present, next to `ambiguous`. It is HP A8-3's list for the whole set, with entries of the same shape. Each entry of `previews` keeps its own `breaking`, as the `preview_upgrade` result.<br>- `accept_breaking` of `package.upgrade` and `hub.upgrade_packages` still names `{ consumer, interface }` pairs. A client builds it from the `consumer` and `interface` of the preview's entries. | `conf::a7_1_preview_upgrade_breaking_entries_carry_range_and_version`, `conf::a7_1_preview_upgrade_breaking_is_empty_when_nothing_breaks`, `conf::a7_1_bulk_preview_carries_a_set_breaking_list` |

## A7-2. Text replaced

1. **A.9, the `package.preview_upgrade` row, result:** "`breaking?: [{ consumer, interface }]`" reads "`breaking: [{ consumer, interface, range, interface_version }]`" (A7-1).
2. **A.9, the `hub.preview_package_upgrades` row, result:** "`{ previews: [{ package, …the preview_upgrade result, including breaking }], ambiguous: [{ consumer, interface, candidates }] }`" reads "`{ previews: [{ package, …the preview_upgrade result, including breaking }], ambiguous: [{ consumer, interface, candidates }], breaking: [{ consumer, interface, range, interface_version }] }`" (A7-1).

Every other clause is unchanged.
