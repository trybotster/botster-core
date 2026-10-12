# Hub→plugin Lua binding v1.1 r9, Amendment 2 (candidate 1): the Lua shape of tool, prompt and resource entries

The base is the Lua binding r9 of `frozen/current` (manifest final27), with Amendment 1, which are unchanged except where LBA2-2 names a replacement. Other contracts are cited by contract name and clause id only.

Author: the contract steward. Reviewer: the Amendment reviewer (Sol). Date: 2026-10-02. Found by the lb-cli-suite pair, through the Stage 0 lead (about 50 transcripts load plugins with tools).

**Why.** LB-10 says `botster.register{ tools, prompts?, resources?, handlers }` builds `Ready`, and that "`call` functions stay in the host's table by handler id". HP MT-1 gives the **wire** fields of a tool (`name`, `description`, `input_schema`, `handler`, and `callable_by` to opt in to plugin calls), and of a prompt (`name`, `description`, `arguments`, `handler`). No clause gives the **Lua** fields of a `tools`, `prompts` or `resources` entry. LBA1-3 makes an unknown handler-entry field fail the load.

| # | Clause | Test |
|---|---|---|
| LBA2-1 | **The Lua entries are flat, with `call` in place of the wire `handler`.** In `botster.register{ ... }`:<br>- a **`tools`** entry is `{ name, description, input_schema, call, id?, callable_by? }`;<br>- a **`prompts`** entry is `{ name, description, arguments, call, id? }`;<br>- a **`resources`** entry is `{ uri_template, name, mime_type?, call, id? }`.<br>`call` is the handler function. The host keeps it by handler id (IV-8) and sends that id on the wire as the entry's `handler` (HP MT-1), together with an `mcp_tool`, `mcp_prompt` or `mcp_resource` handler whose descriptor is the entry's descriptor fields (HP A2-5). `id` is the optional handler id, as in LBA1-3. The host makes one when it is omitted. **`handler` is not a Lua field**: the host fills it in. A Lua entry that sets `handler`, or any other unknown field, fails the load (`invalid_request`). A missing required field also fails the load. | `conf::lba2_1_tool_entry_is_name_description_input_schema_call`, `conf::lba2_1_handler_field_in_lua_fails_the_load`, `conf::lba2_1_prompt_and_resource_entries` |

## LBA2-2. Text replaced

1. **LB-10, `botster.register{ tools, prompts?, resources?, handlers }`:** the entries of `tools`, `prompts` and `resources` have the LBA2-1 shapes.

Every other clause is unchanged.
