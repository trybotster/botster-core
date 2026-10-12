# Core contract v1.17, erratum 1 (candidate 1): the worker binary's file name is not fixed by Core

The base is the Core contract of `frozen/current` (v1.17 and Amendments 2 to 4), which is unchanged except for the one sentence below. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-01. Found by the Foundation lead.

**Why.** Section 13 says "a C embedder ships `botster-session-worker` and passes its path". The Hub→client contract's install layout (Amendment 2, RT-3) names the same binary `botster-worker`, and so does the CLI spec. Core receives the worker by **path**, so Core never needs a file name. Fixing one in Core text is a cross-contract conflict, and it puts a product name into a standalone library (compare SV-1: "Core never uses a product prefix").

| # | Clause | Test |
|---|---|---|
| E1-1 | **Section 13, the sentence** "a C embedder ships `botster-session-worker` and passes its path" becomes: "a C embedder ships the session worker binary built from `botster-core` and passes its path. **Core fixes no file name for it**; the embedder's install layout names it (Botster names it `botster-worker`, Hub→client RT-3)." Every other mention of the worker in Core means that binary. The name `botster-session-worker.rs` in the non-normative "Facts about today's code" is a historical source file name and is unchanged. | `conf::e1_1_worker_found_by_host_supplied_path` |
