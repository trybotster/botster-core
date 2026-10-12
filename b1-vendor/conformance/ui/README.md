# UI transcripts for the renderer and the Hub

The ids of ui-contract that only a renderer or the Hub can prove. One JSON file per id.

- `"subject": "client"`: steps `client_load`, `client_act`, `client_expect` (`botster_conformance::client`, schema `schemas/client-step.schema.json`).
- `"subject": "hub"`: the HC step vocabulary.
- Phase 0 checks them structurally (R-1.4): `cargo test -p botster-ui-conformance`.

The vectors of the `realizer` subject are in `crates/botster-ui-contract/vectors/`.
