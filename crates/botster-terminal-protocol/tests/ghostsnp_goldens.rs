#![allow(missing_docs)]

use std::path::PathBuf;

use sha2::{Digest, Sha256};

const HISTORY_READY_SHA: &str = "fbcdda31d682a61420251eed68f72e413485f057e3f374c57582955b0316bb6d";
const HISTORY_PAGE_SHA: &str = "b1b65d9d205f10a2cce4384ea15f0b6b20ee07bb3fda8e3bbdb8bd81dffb071f";
const HISTORY_FINISH_SHA: &str = "6e0bfa87315d3225b0dedaa88387eb37c5cb31922b7891741445114bf19a3085";
const BLANK_READY_SHA: &str = "06962b11d4a3acfb9b7c52b673a7b476904ddee2dd754b89b190ff82fdcfd0cc";
const BLANK_FINISH_SHA: &str = "a172e2380afec9ba9248735973f18965ee384ec2ae3440dbb4ddf4d5ced9d325";

#[test]
fn ghostsnp_goldens_keep_history_and_blank_identities() {
    let history_ready = golden("late-attach-history-ready-v2.ghostsnp");
    let history_page = golden("late-attach-history-page-v2.ghostsnp");
    let history_finish = golden("late-attach-history-finish-v2.ghostsnp");
    let blank_ready = golden("late-attach-blank-ready-v2.ghostsnp");
    let blank_finish = golden("late-attach-blank-finish-v2.ghostsnp");

    assert!(history_ready.starts_with(b"GHOSTSNP"));
    assert!(blank_ready.starts_with(b"GHOSTSNP"));
    assert_eq!(hex_sha256(&history_ready), HISTORY_READY_SHA);
    assert_eq!(hex_sha256(&history_page), HISTORY_PAGE_SHA);
    assert_eq!(hex_sha256(&history_finish), HISTORY_FINISH_SHA);
    assert_eq!(hex_sha256(&blank_ready), BLANK_READY_SHA);
    assert_eq!(hex_sha256(&blank_finish), BLANK_FINISH_SHA);
    assert_ne!(hex_sha256(&history_ready), hex_sha256(&blank_ready));
    assert_eq!(history_ready.len(), 2838);
    assert_eq!(blank_ready.len(), 1131);
}

fn golden(name: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("fixtures/ghostsnp")
        .join(name);
    std::fs::read(&path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn hex_sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

mod hex {
    pub fn encode(bytes: impl AsRef<[u8]>) -> String {
        bytes
            .as_ref()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }
}
