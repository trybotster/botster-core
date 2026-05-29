//! Credential-store and non-exportable signing contract tests.

use botster_core::{
    CredentialRecord, CredentialStore, CredentialStoreError, NonExportableSigner, SignatureBytes,
    SigningError, SigningKeyHandle,
};
use std::collections::BTreeMap;

#[derive(Default)]
struct MemoryStore {
    records: BTreeMap<String, CredentialRecord>,
}

impl CredentialStore for MemoryStore {
    fn get(&self, key: &str) -> Result<Option<CredentialRecord>, CredentialStoreError> {
        Ok(self.records.get(key).cloned())
    }

    fn set(&mut self, key: &str, record: CredentialRecord) -> Result<(), CredentialStoreError> {
        self.records.insert(key.to_string(), record);
        Ok(())
    }

    fn delete(&mut self, key: &str) -> Result<(), CredentialStoreError> {
        self.records
            .remove(key)
            .map(|_| ())
            .ok_or(CredentialStoreError::NotFound)
    }
}

struct FakeSigner {
    handle_id: String,
}

impl NonExportableSigner for FakeSigner {
    fn sign(
        &self,
        handle: &SigningKeyHandle,
        message: &[u8],
    ) -> Result<SignatureBytes, SigningError> {
        if handle.id != self.handle_id {
            return Err(SigningError::UnknownHandle);
        }

        let mut signature = b"signature:".to_vec();
        signature.extend_from_slice(message);

        Ok(SignatureBytes(signature))
    }
}

#[test]
fn credential_store_contract_handles_get_set_and_delete() -> Result<(), CredentialStoreError> {
    let mut store = MemoryStore::default();
    let key = "device.identity";

    assert!(store.get(key)?.is_none());

    store.set(key, CredentialRecord::new(vec![1, 2, 3]))?;
    let record = store
        .get(key)?
        .expect("credential record should be present after set");
    assert_eq!(record.as_bytes(), &[1, 2, 3]);

    store.delete(key)?;
    assert!(store.get(key)?.is_none());
    assert_eq!(store.delete(key), Err(CredentialStoreError::NotFound));

    Ok(())
}

#[test]
fn signer_contract_signs_through_opaque_handle_without_private_material() {
    let handle = SigningKeyHandle::new("device-key");
    let signer = FakeSigner {
        handle_id: "device-key".to_string(),
    };

    assert_eq!(
        signer.sign(&handle, b"challenge"),
        Ok(SignatureBytes(b"signature:challenge".to_vec()))
    );

    assert_eq!(
        signer.sign(&SigningKeyHandle::new("missing"), b"challenge"),
        Err(SigningError::UnknownHandle)
    );
}
