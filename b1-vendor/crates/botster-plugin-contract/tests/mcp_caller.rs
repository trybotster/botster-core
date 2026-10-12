//! The caller that the Hub forwards to a proxied MCP server (HP MP-5): `_meta` key `botster/caller`, value `{kind, session?}`.

use botster_plugin_contract::prelude::*;
use serde_json::{json, Value};

#[test]
fn mp_5_the_meta_is_the_caller_kind_and_only_a_session_ref() {
    let session = SessionRef {
        hub_id: "h1".into(),
        session_id: "s1".into(),
    };
    let callers = [
        Caller::Operator {
            credential_id: Some("cred-1".into()),
            hub_id: Some("h1".into()),
        },
        Caller::Plugin {
            plugin_key: "p".into(),
            instance: "i1".into(),
        },
        Caller::Session {
            session: session.clone(),
        },
        Caller::System,
    ];
    let expected = [
        json!({"kind": "operator"}),
        json!({"kind": "plugin"}),
        json!({"kind": "session", "session": {"hub_id": "h1", "session_id": "s1"}}),
        json!({"kind": "system"}),
    ];
    assert_eq!(MCP_CALLER_META_KEY, "botster/caller");
    for (caller, wire) in callers.iter().zip(expected) {
        let meta = McpCallerMeta::from(caller);
        // The kind is the serde tag of the Caller, so the two never drift.
        assert_eq!(
            Value::String(meta.kind.clone()),
            serde_json::to_value(caller).unwrap()["kind"]
        );
        // MP-5: never a credential: `credential_id`, `plugin_key` and `instance` are not forwarded.
        assert_eq!(serde_json::to_value(&meta).unwrap(), wire);
        assert_eq!(serde_json::from_value::<McpCallerMeta>(wire).unwrap(), meta);
    }
}

#[test]
fn mp_5_the_meta_refuses_another_field() {
    let v = json!({"kind": "operator", "credential_id": "cred-1"});
    assert!(serde_json::from_value::<McpCallerMeta>(v).is_err());
}
