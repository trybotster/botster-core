//! The token proof of the hello (Core AD-6): how a peer shows that it holds the per-worker token.
//!
//! The proof is the SHA-256 of a fixed domain string, the role of the sender, the token, the host epoch and the
//! `InstanceId`. It binds the token to one instance and one epoch, so a proof that was captured for one session or one host
//! never opens another. The token itself never travels on the link. The hello codec treats the proof as opaque bytes.
//!
//! **Roles.** A worker proves itself with [`token_proof`] and a host with [`host_proof`]; each end checks the role of the
//! other. So neither end can send back the proof that it received: in an adoption the host speaks first (P5), and an impostor
//! at the worker's endpoint could otherwise return the host's own proof.
//!
//! **Rule (AD-4).** The proof rule (the domain, the role byte and the order of the hashed fields) and the five fields of the
//! hello never change between worker protocol numbers. A worker of protocol N - 1 must be able to answer a host of protocol
//! N; otherwise it would be `Lost(WorkerUnreachable)` instead of adopted or `Lost(WorkerVersion)`.
//!
//! Hand-rolled reason: a keyed MAC over a challenge would need a second round trip; here the hello is the only message and the
//! link is a local socket that only the host's uid can reach (AD-6), so a bound hash proves possession without a new protocol.

use crate::hello::{TokenProof, PROOF_LEN};
use botster_core_contract::prelude::InstanceId;
use sha2::{Digest, Sha256};

/// The size of the per-worker token, in bytes.
pub const TOKEN_LEN: usize = 32;

const DOMAIN: &[u8] = b"botster-core-link/v1/hello-proof";

/// The role byte of a worker's proof.
const WORKER: u8 = b'w';
/// The role byte of a host's proof.
const HOST: u8 = b'h';

/// A token as 64 lowercase hex digits: its form in the worker's environment and in the registry row (AD-6).
pub fn token_hex(token: &[u8; TOKEN_LEN]) -> String {
    to_hex(token)
}

/// The token that `text` holds as 64 lowercase hex digits, or `None` for any other text.
pub fn token_from_hex(text: &str) -> Option<[u8; TOKEN_LEN]> {
    from_hex(text)
}

/// `bytes` as lowercase hex digits, two for each byte: the one hex form of the link (tokens and proofs).
pub(crate) fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// The `N` bytes that `text` holds as `2 * N` lowercase hex digits, or `None` for any other text.
pub(crate) fn from_hex<const N: usize>(text: &str) -> Option<[u8; N]> {
    let digits = text.as_bytes();
    if digits.len() != N * 2 {
        return None;
    }
    let nibble = |d: u8| match d {
        b'0'..=b'9' => Some(d - b'0'),
        b'a'..=b'f' => Some(d - b'a' + 10),
        _ => None,
    };
    let mut out = [0u8; N];
    for (byte, pair) in out.iter_mut().zip(digits.chunks(2)) {
        *byte = nibble(pair[0])? * 16 + nibble(pair[1])?;
    }
    Some(out)
}

/// The proof of `token` for `instance` at `host_epoch`.
///
/// Clause: Core AD-6, Core DP-8.
/// The proof that a worker sends in its hello (AD-6).
pub fn token_proof(token: &[u8; TOKEN_LEN], instance: &InstanceId, host_epoch: u64) -> TokenProof {
    proof(WORKER, token, instance, host_epoch)
}

/// The proof that a host sends in its hello (AD-6). It differs from the worker's proof of the same inputs.
pub fn host_proof(token: &[u8; TOKEN_LEN], instance: &InstanceId, host_epoch: u64) -> TokenProof {
    proof(HOST, token, instance, host_epoch)
}

fn proof(role: u8, token: &[u8; TOKEN_LEN], instance: &InstanceId, host_epoch: u64) -> TokenProof {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
    hash.update([0]);
    hash.update([role]);
    hash.update([0]);
    hash.update(token);
    hash.update([0]);
    hash.update(host_epoch.to_be_bytes());
    hash.update([0]);
    hash.update(instance.0.as_bytes());
    let digest: [u8; PROOF_LEN] = hash.finalize().into();
    TokenProof(digest)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// AD-6: a token survives its hex form; every hex digit decodes to its bytes; a non-digit, an upper-case digit or a short
    /// text is not a token.
    #[test]
    fn a_token_survives_its_hex_form_and_other_text_is_not_a_token() {
        let token: [u8; TOKEN_LEN] = std::array::from_fn(|i| ((i * 7 + 9) % 256) as u8);
        assert_eq!(token_from_hex(&token_hex(&token)), Some(token));
        assert_eq!(
            token_from_hex(&"09".repeat(TOKEN_LEN)),
            Some([9u8; TOKEN_LEN])
        );
        assert_eq!(
            token_from_hex(&"90".repeat(TOKEN_LEN)),
            Some([0x90u8; TOKEN_LEN])
        );
        assert_eq!(
            token_from_hex(&"af".repeat(TOKEN_LEN)),
            Some([0xAFu8; TOKEN_LEN])
        );
        assert_eq!(token_from_hex(&"0g".repeat(TOKEN_LEN)), None);
        assert_eq!(token_from_hex(&"AF".repeat(TOKEN_LEN)), None);
        assert_eq!(token_from_hex("00"), None);
    }

    fn instance(text: &str) -> InstanceId {
        InstanceId(text.to_string())
    }

    #[test]
    fn the_same_inputs_give_the_same_proof() {
        let token = [7u8; TOKEN_LEN];
        assert_eq!(
            token_proof(&token, &instance("i"), 3),
            token_proof(&token, &instance("i"), 3)
        );
    }

    /// AD-6: the proof is bound to the token, the instance and the epoch; changing any one changes it.
    #[test]
    fn the_proof_depends_on_the_token_the_instance_and_the_epoch() {
        let token = [7u8; TOKEN_LEN];
        let base = token_proof(&token, &instance("i"), 3);
        assert_ne!(base, token_proof(&[8u8; TOKEN_LEN], &instance("i"), 3));
        assert_ne!(base, token_proof(&token, &instance("j"), 3));
        assert_ne!(base, token_proof(&token, &instance("i"), 4));
    }

    /// AD-6: a host's proof and a worker's proof of the same inputs differ, so neither end can return the proof it received.
    /// Each proof also follows its own token, instance and epoch.
    #[test]
    fn the_host_and_the_worker_prove_with_different_roles() {
        let token = [7u8; TOKEN_LEN];
        let worker = token_proof(&token, &instance("i"), 3);
        let host = host_proof(&token, &instance("i"), 3);
        assert_ne!(worker, host);
        assert_eq!(host, host_proof(&token, &instance("i"), 3));
        assert_ne!(host, host_proof(&[8u8; TOKEN_LEN], &instance("i"), 3));
        assert_ne!(host, host_proof(&token, &instance("j"), 3));
        assert_ne!(host, host_proof(&token, &instance("i"), 4));
    }

    /// AD-4 rule: the proof is the SHA-256 of the documented layout (domain, role byte, token, epoch, instance, each after a
    /// zero byte). The hash is computed here from that layout, so a change of the rule fails this test.
    #[test]
    fn the_proof_follows_the_documented_layout() {
        let token = [3u8; TOKEN_LEN];
        let layout = |role: u8| {
            let mut bytes = b"botster-core-link/v1/hello-proof".to_vec();
            bytes.extend([0, role, 0]);
            bytes.extend(token);
            bytes.push(0);
            bytes.extend(9u64.to_be_bytes());
            bytes.push(0);
            bytes.extend(b"4-2");
            TokenProof(Sha256::digest(&bytes).into())
        };
        assert_eq!(token_proof(&token, &instance("4-2"), 9), layout(b'w'));
        assert_eq!(host_proof(&token, &instance("4-2"), 9), layout(b'h'));
    }

    /// The domain bytes and the separators keep two different field splits apart.
    #[test]
    fn adjacent_fields_do_not_run_together() {
        let token = [1u8; TOKEN_LEN];
        assert_ne!(
            token_proof(&token, &instance("1"), 0x0102),
            token_proof(&token, &instance("\u{1}\u{2}1"), 0)
        );
    }
}
