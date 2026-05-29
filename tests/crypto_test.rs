//! AES-GCM envelope tests.

use botster_core::{decrypt_aes_gcm, encrypt_aes_gcm, AesGcmEnvelope, AesGcmKey, CryptoError};
use serde_json::Value;

fn key(byte: u8) -> AesGcmKey {
    AesGcmKey::new([byte; 32])
}

#[test]
fn aes_gcm_encrypts_and_decrypts_round_trip() -> Result<(), Box<dyn std::error::Error>> {
    let plaintext = b"botster core envelope";
    let envelope = encrypt_aes_gcm(&key(7), plaintext, 1)?;
    let decrypted = decrypt_aes_gcm(&key(7), &envelope)?;

    assert_eq!(decrypted, plaintext);

    Ok(())
}

#[test]
fn aes_gcm_rejects_wrong_key() -> Result<(), Box<dyn std::error::Error>> {
    let envelope = encrypt_aes_gcm(&key(7), b"authenticated payload", 1)?;
    let error = decrypt_aes_gcm(&key(8), &envelope)
        .expect_err("wrong key must fail authenticated decryption");

    assert_eq!(error, CryptoError::DecryptFailed);

    Ok(())
}

#[test]
fn aes_gcm_envelope_round_trips_through_json() -> Result<(), Box<dyn std::error::Error>> {
    let envelope = encrypt_aes_gcm(&key(7), b"serialized payload", 42)?;
    let serialized = serde_json::to_string(&envelope)?;
    let decoded: AesGcmEnvelope = serde_json::from_str(&serialized)?;

    assert_eq!(decoded.version, 42);
    assert_eq!(decoded.nonce, envelope.nonce);
    assert_eq!(decoded.ciphertext, envelope.ciphertext);
    assert_eq!(decrypt_aes_gcm(&key(7), &decoded)?, b"serialized payload");

    Ok(())
}

#[test]
fn aes_gcm_json_shape_is_nonce_ciphertext_and_version() -> Result<(), Box<dyn std::error::Error>> {
    let envelope = encrypt_aes_gcm(&key(7), b"shape payload", 3)?;
    let json = serde_json::to_value(&envelope)?;
    let object = json
        .as_object()
        .expect("serialized envelope must be a JSON object");

    assert_eq!(object.len(), 3);
    assert!(matches!(object.get("nonce"), Some(Value::String(_))));
    assert!(matches!(object.get("ciphertext"), Some(Value::String(_))));
    assert_eq!(object.get("version"), Some(&Value::from(3)));

    Ok(())
}

#[test]
fn aes_gcm_generates_distinct_internal_nonces() -> Result<(), Box<dyn std::error::Error>> {
    let first = encrypt_aes_gcm(&key(7), b"same payload", 1)?;
    let second = encrypt_aes_gcm(&key(7), b"same payload", 1)?;

    assert_ne!(first.nonce, second.nonce);

    Ok(())
}

#[test]
fn aes_gcm_rejects_malformed_base64_and_nonce_lengths() {
    let malformed = AesGcmEnvelope {
        nonce: "not base64".to_string(),
        ciphertext: "also not base64".to_string(),
        version: 1,
    };

    assert_eq!(
        decrypt_aes_gcm(&key(7), &malformed),
        Err(CryptoError::DecodeFailed { field: "nonce" })
    );

    let wrong_nonce_len = AesGcmEnvelope {
        nonce: "AA==".to_string(),
        ciphertext: "AA==".to_string(),
        version: 1,
    };

    assert_eq!(
        decrypt_aes_gcm(&key(7), &wrong_nonce_len),
        Err(CryptoError::InvalidNonceLength {
            expected: 12,
            actual: 1
        })
    );
}
