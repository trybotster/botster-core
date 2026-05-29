//! Device public metadata tests.

use botster_core::{
    device_fingerprint, verify_device_fingerprint, DeviceFingerprint, DevicePublicMetadata,
    PublicSigningKeyBytes,
};

#[test]
fn fingerprint_is_deterministic_and_verifiable_from_public_key() {
    let public_key = b"public verifying key bytes";
    let fingerprint = device_fingerprint(public_key);

    assert_eq!(fingerprint, device_fingerprint(public_key));
    assert!(verify_device_fingerprint(public_key, &fingerprint));
    assert!(!verify_device_fingerprint(
        b"different public key bytes",
        &fingerprint
    ));
}

#[test]
fn fingerprint_uses_colon_separated_lower_hex_format() {
    let fingerprint = device_fingerprint(b"public verifying key bytes");
    let parts = fingerprint.0.split(':').collect::<Vec<_>>();

    assert_eq!(parts.len(), 8);
    assert!(parts
        .iter()
        .all(|part| part.len() == 2 && part.chars().all(|ch| ch.is_ascii_hexdigit())));
    assert_eq!(fingerprint.0, fingerprint.0.to_lowercase());
}

#[test]
fn public_metadata_serialization_excludes_private_material() -> Result<(), serde_json::Error> {
    let metadata = DevicePublicMetadata::new(
        Some("local device".to_string()),
        PublicSigningKeyBytes(vec![1, 2, 3, 4]),
    );
    let serialized = serde_json::to_string(&metadata)?;

    assert!(serialized.contains("verifying_key"));
    assert!(serialized.contains("fingerprint"));
    assert!(!serialized.contains("private_key"));
    assert!(!serialized.contains("signing_key"));
    assert!(!serialized.contains("secret"));
    assert!(!serialized.contains("token"));
    assert!(!serialized.contains("credential"));

    Ok(())
}

#[test]
fn public_metadata_can_preserve_existing_fingerprint() {
    let metadata = DevicePublicMetadata {
        name: None,
        verifying_key: PublicSigningKeyBytes(vec![9, 8, 7]),
        fingerprint: DeviceFingerprint("aa:bb:cc:dd:ee:ff:00:11".to_string()),
    };

    assert_eq!(metadata.fingerprint.to_string(), "aa:bb:cc:dd:ee:ff:00:11");
    assert!(!metadata.fingerprint_matches_key());
}

#[test]
fn public_metadata_can_verify_its_fingerprint_trust_boundary() {
    let metadata = DevicePublicMetadata::new(None, PublicSigningKeyBytes(vec![9, 8, 7]));

    assert!(metadata.fingerprint_matches_key());
}
