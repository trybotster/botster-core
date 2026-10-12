# Core contract v1.17, erratum 4 (candidate 1): what a query reply's admission reserves

The base is the Core contract of `frozen/current` (manifest final21), which is unchanged except for the reading below. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the core-p4b reviewer, through the Stage 0 lead (EV-8(e) tests).

**Why.** EV-8(e): "Admission reserves the full encoded bytes (base64 expansion and framing included) against the session's input bounds". The clause names the parts of the cost but gives no rule that a test can compute. A test that can only check "at least the PTY reply length" cannot tell a worker that reserves the full cost from one that reserves less.

| # | Clause | Test |
|---|---|---|
| E4-1 | **The reserved cost C of a query reply's admission:**<br>- **A client reply** (typed or `Encoded`): C is the **size of the reply's `0x81 input` frame** as it arrived on the route. That is its type byte plus its JSON body (codec TS-3: "A frame's size counts the type byte plus the body"), including the base64 of `Encoded` bytes. The binding's own delimiting is not counted (TS-3).<br>- **The shadow fallback** (no frame): C is the **length of the reply bytes the worker writes** to the PTY.<br>C is reserved at the admission point (AM-2) and released when the reply's write completes. Every refusal and parking rule of EV-8(e) is unchanged. A test knows C exactly: it constructs the client's reply frame, and it reads the fallback's bytes at the program edge (or computes their length from the terminal oracle, R-7). | `conf::e4_1_client_reply_reserves_its_frame_size`, `conf::e4_1_fallback_reserves_its_reply_length`, `conf::e4_1_reply_one_byte_over_the_free_bound_is_parked` |

**Text replaced:** EV-8(e)'s "full encoded bytes (base64 expansion and framing included)" reads with E4-1. Every other clause is unchanged.
