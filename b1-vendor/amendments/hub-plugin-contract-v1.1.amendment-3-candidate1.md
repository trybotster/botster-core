# Hub→plugin contract v1.1, Amendment 3 (candidate 1): the scripted services are the real Hub's injected edges; one non-claim test withdrawn

The base is the Hub→plugin contract of `frozen/current` (manifest final22): v1.1 joint J4e, with Amendments 1 and 2, which are unchanged except where A3-3 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the hp-events pair, through the Stage 0 lead.

**Why.**
- **The scripted services.** NT-10, PM-14 and MP-10 say "`FakeHub` provides" a scripted I/O service, a scripted git source and catalog, and a scripted MCP server. Their proofs test Hub behavior: the `net` rules (NT-1 to NT-9: redirects, private addresses, cancellation on unload), the package manager (PM-1 to PM-13), and the MCP proxy (MP-1 to MP-9).
  - Under the adopted test design (BUILD.md; Core A5 and HC A5, by the same decision), the plugin-side `FakeHub` of section 19 is a **frame-script player** for plugin authors. Steward ruling R-1.3 forbids it to invent semantic Hub behavior.
  - So a "FakeHub" that follows redirects, refuses private addresses or proxies MCP would be a second implementation of the Hub.
  - The real Hub already takes these worlds through injected I/O edges.
- **The non-claim test.** WP-3's last sentence ("browser code that the plugin serves … is trusted code …, and this contract does not claim otherwise") states that the contract makes **no** claim. There is no Hub behavior to stimulate or observe, so its test has no subject.

| # | Clause | Test |
|---|---|---|
| A3-1 | **The scripted services are the real Hub's injected edges.**<br>- The scripted I/O service of NT-10, the scripted git source and catalog of PM-14, and the scripted MCP server of MP-10 are the **real Hub's injected I/O edges** in the Hub's test harness: deterministic endpoints and no real network, as each clause lists.<br>- Their tests run against the **real Hub** with those edges. This includes the Telegram-shaped run and the unload mid-request of NT-10, the install, update, pin and rollback runs of PM-14, and the list, call, cancel and unload run of MP-10.<br>- **The plugin-side `FakeHub` (section 19) implements none of these services.** For a plugin author's test, it answers the plugin's `HostCall`s for these calls with **scripted `HostResult`s** (R-1.3), and it adds no network, package or MCP semantics.<br>- The "real run" parts of these clauses are unchanged. | `conf::nt_10_fake_io_is_scripted_and_offline`, `conf::nt_10_telegram_shaped_long_poll_and_send`, `conf::nt_10_unload_mid_request_cancels_and_leaks_nothing`, `conf::pm_14_fake_git_and_catalog_are_scripted_and_offline`, `conf::mp_10_fake_mcp_server_is_scripted_and_offline` (each now run against the real Hub with its injected edge), `conf::a3_1_plugin_side_fakehub_scripts_host_results_only` |
| A3-2 | **WP-3's non-claim has no test.** `conf::wp_3_served_code_is_trusted_not_claimed_otherwise` is **withdrawn**: the sentence it cites states that the contract makes no claim, so nothing can be observed. WP-3's other three tests are unchanged, and they cover the relay role. | (withdrawal; no replacement is needed) |

## A3-3. Text replaced

1. **NT-10, PM-14 and MP-10:** "`FakeHub` provides" reads "the Hub's test harness provides, as injected I/O edges of the real Hub (A3-1)".
2. **Section 19, the `FakeHub` bullet:** it reads with A3-1. The plugin-side `FakeHub` scripts `HostResult`s, and it provides no network, package or MCP service.
3. **WP-3:** its test `conf::wp_3_served_code_is_trusted_not_claimed_otherwise` is withdrawn (A3-2).

Every other clause is unchanged.
