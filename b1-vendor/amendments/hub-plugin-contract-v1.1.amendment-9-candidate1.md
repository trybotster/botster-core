# Hub→plugin contract v1.1, Amendment 9 (candidate 1): only the Hub uses the ICE configuration

The base is the Hub→plugin contract of `frozen/current` (manifest final35): v1.1 joint J4e, with Amendments 1 to 8, which are unchanged except where A9-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-09. Authority: **a user decision, relayed by the orchestrator on 2026-10-09:** WebRTC moves out of Core and into the Hub (Core Amendment 17; Hub→client Amendment 8). After the move, no session worker has a peer connection.

**Why.** WP-6 says that the stored STUN and TURN configuration is what "the **Hub and its workers use for their own peer connections**". Under Core A17-1 and Hub→client A8-1, a worker opens no network socket and no peer connection. Every peer connection of a client ends at the Hub.

| # | Clause | Test |
|---|---|---|
| A9-1 | **The Hub alone uses the ICE configuration.** The configuration that `web.set_ice_servers` stores (WP-6) is the one that the **Hub** uses for its own peer connections, which carry every channel of a client (Hub→client A8-1). No session worker uses it or receives it. The other rules of WP-6 are unchanged. | `conf::a9_1_hub_uses_the_ice_configuration` |

**Ids.** `conf::wp_6_hub_and_workers_use_the_configuration` is **withdrawn**, with authority A9-1, and `conf::a9_1_hub_uses_the_ice_configuration` replaces it. The other `wp_6_*` ids are unchanged.

## A9-2. Text replaced

1. **WP-6:** "the STUN and TURN configuration that the **Hub and its workers use for their own peer connections**" reads "the STUN and TURN configuration that the **Hub uses for its own peer connections** (A9-1)".

Every other clause is unchanged.
