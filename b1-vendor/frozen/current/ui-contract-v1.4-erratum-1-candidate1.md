# ui-contract 1.4, erratum 1 (candidate 1): the error tone is `danger`

Owner: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-09-30. **This is an erratum in the post-freeze amendment path.** It adds no behavior, no feature and no version: `ui_contract_view` stays `"1.4"`, and the feature stays `ui_1_4`. The base is the ui-contract of manifest `final9` (1.0 to 1.4), which is unchanged. Found by the Foundation build's ui-amendments review (steward ruling R-10).

**Why.** UI 1.3 UI-AC-7(a) and UI 1.4 UI-NV-3 and UI-NV-5 require an "`error`-tone `text`". The backticks make `error` read as a tone value, but UI-CF-4's tone set is closed: `neutral|info|success|warning|danger`. A `tone: "error"` would be an unknown tone (the `unknown_tone` diagnostic, UI-NV-4). The vectors fix one token for every renderer (UI section 16), so the text must name it.

| # | Clause | Test |
|---|---|---|
| UI-NV-8 | **The error tone is `danger`.** Wherever UI-AC-7(a), UI-NV-3 or UI-NV-5 says "`error`-tone", the realized node carries `tone: "danger"`, a member of UI-CF-4's closed set. There is no tone value `error`. `error` stays a role segment (UI-RZ-2) and a prop name (UI-NV-2). It is never a tone. | `conf::ui_nv_8_error_tone_is_danger` |
