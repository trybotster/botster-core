//! The PD-6 resolution failures: the `detail` of each code, the kind it travels under, and the shapes that are refused (ruling R-40).

use botster_plugin_contract::prelude::*;
use serde_json::{json, Value};

fn detail(v: Value) -> Detail {
    serde_json::from_value(v).expect("a detail")
}

fn found(provider: &str, interface_version: &str) -> FoundProvider {
    FoundProvider {
        provider: provider.into(),
        interface_version: interface_version.into(),
    }
}

#[test]
fn pd_6_each_failure_is_its_code_and_the_fields_pd_6_names() {
    // PD-6: `failed{detail.code: requirement_missing, interface, range}`.
    let missing = ResolutionFailure::RequirementMissing {
        interface: "git@1".into(),
        range: "^1.2".into(),
    };
    // PD-6: `conflict{detail.code: requirement_conflict, interface, range, found}`; R-40: `found` is `[{provider, interface_version}]`.
    let conflict = ResolutionFailure::RequirementConflict {
        interface: "git@1".into(),
        range: "^1.2".into(),
        found: vec![found("git-a", "1.0.0"), found("git-b@t1", "2.0.0")],
    };
    // PD-6: `conflict{detail.code: ambiguous_provider, interface, candidates}`; the candidates are package keys.
    let ambiguous = ResolutionFailure::AmbiguousProvider {
        interface: "git@1".into(),
        candidates: vec!["git-a".into(), "git-b".into()],
    };
    let cases = [
        (
            &missing,
            ErrorKind::Failed,
            json!({"code": "requirement_missing", "interface": "git@1", "range": "^1.2"}),
        ),
        (
            &conflict,
            ErrorKind::Conflict,
            json!({"code": "requirement_conflict", "interface": "git@1", "range": "^1.2", "found": [
                {"provider": "git-a", "interface_version": "1.0.0"},
                {"provider": "git-b@t1", "interface_version": "2.0.0"},
            ]}),
        ),
        (
            &ambiguous,
            ErrorKind::Conflict,
            json!({"code": "ambiguous_provider", "interface": "git@1", "candidates": ["git-a", "git-b"]}),
        ),
    ];
    for (failure, kind, wire) in cases {
        assert_eq!(failure.kind(), kind, "{wire}");
        let d = failure.to_detail();
        assert_eq!(serde_json::to_value(&d).unwrap(), wire);
        assert_eq!(d.code.as_deref(), wire["code"].as_str());
        assert_eq!(
            &ResolutionFailure::from_detail(&detail(wire.clone())).unwrap(),
            failure
        );
    }
}

#[test]
fn pd_6_found_order_is_not_fixed() {
    // R-40: the order of `found` is not fixed, so a reader compares it as a set.
    let read = |v: Value| match ResolutionFailure::from_detail(&detail(v)).unwrap() {
        ResolutionFailure::RequirementConflict { found, .. } => {
            let mut f: Vec<_> = found
                .into_iter()
                .map(|p| (p.provider, p.interface_version))
                .collect();
            f.sort();
            f
        }
        other => panic!("{other:?}"),
    };
    let one = json!({"provider": "a", "interface_version": "1.0.0"});
    let two = json!({"provider": "b", "interface_version": "3.0.0"});
    let wire = |f: Value| json!({"code": "requirement_conflict", "interface": "i", "range": "^2", "found": f});
    assert_eq!(read(wire(json!([one, two]))), read(wire(json!([two, one]))));
}

#[test]
fn pd_6_refuses_a_detail_outside_the_three_forms() {
    let refused = [
        // R-40: a conflict has at least one provider, none in range; zero providers is `requirement_missing`.
        json!({"code": "requirement_conflict", "interface": "i", "range": "^1", "found": []}),
        // PD-6: "several providers in range": one candidate is not ambiguous.
        json!({"code": "ambiguous_provider", "interface": "i", "candidates": ["a"]}),
        // R-40: an entry is `{provider, interface_version}` only.
        json!({"code": "requirement_conflict", "interface": "i", "range": "^1", "found": [{"provider": "a"}]}),
        json!({"code": "requirement_conflict", "interface": "i", "range": "^1", "found": [
            {"provider": "a", "interface_version": "1.0.0", "provider_version": "1.0.0"}
        ]}),
        // A field that PD-6 does not name for the code, a missing field, and another code.
        json!({"code": "requirement_missing", "interface": "i", "range": "^1", "found": []}),
        json!({"code": "requirement_missing", "interface": "i"}),
        json!({"code": "requirement_unmet", "interface": "i"}),
        json!({"interface": "i", "range": "^1"}),
    ];
    for v in refused {
        assert!(
            ResolutionFailure::from_detail(&detail(v.clone())).is_err(),
            "{v}"
        );
    }
}
