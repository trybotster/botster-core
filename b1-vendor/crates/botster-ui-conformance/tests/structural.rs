//! UI-RZ-3b and steward ruling R-1.4: every `client` and `hub` transcript of `conformance/ui/` is structurally valid.

use botster_ui_conformance::{check, transcripts};

#[test]
fn every_ui_transcript_is_structurally_valid() {
    let mut problems = vec![];
    for t in transcripts() {
        if let Err(p) = check(&t) {
            problems.extend(p);
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_check_refuses_a_wrong_contract_and_a_wrong_subject() {
    use botster_conformance::{Contract, Subject};
    let hub = transcripts()
        .into_iter()
        .find(|t| t.subject == Subject::Hub)
        .expect("a hub transcript");
    assert!(
        check(&hub).is_ok(),
        "a hub transcript is checked in botster-hub-conformance"
    );
    let mut t = hub.clone();
    t.contract = Contract::Hc;
    assert!(check(&t).is_err(), "contract ui is required");
    let mut t = hub;
    t.subject = Subject::Core;
    assert!(
        check(&t).is_err(),
        "a core transcript is not a ui transcript"
    );
}
