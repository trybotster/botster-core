//! The token proof of the hello (Core AD-6): how a peer shows that it holds the per-worker token.
//!
//! The proof is the SHA-256 of a fixed domain string, the token, the host epoch and the `InstanceId`. It binds the token to
//! one instance and one epoch, so a proof that was captured for one session or one host never opens another. The token itself
//! never travels on the link. Both ends call [`token_proof`]. The package that owns adoption (AD-6, P5) may replace the
//! function in one place; the hello codec treats the proof as opaque bytes.
//!
//! Hand-rolled reason: a keyed MAC over a challenge would need a second round trip; here the hello is the only message and the
//! link is a local socket that only the host's uid can reach (AD-6), so a bound hash proves possession without a new protocol.

use crate::hello::{TokenProof, PROOF_LEN};
use botster_core_contract::prelude::InstanceId;
use sha2::{Digest, Sha256};

/// The size of the per-worker token, in bytes.
pub const TOKEN_LEN: usize = 32;

const DOMAIN: &[u8] = b"botster-core-link/v1/hello-proof";

/// The proof of `token` for `instance` at `host_epoch`.
///
/// Clause: Core AD-6, Core DP-8.
pub fn token_proof(token: &[u8; TOKEN_LEN], instance: &InstanceId, host_epoch: u64) -> TokenProof {
    let mut hash = Sha256::new();
    hash.update(DOMAIN);
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
