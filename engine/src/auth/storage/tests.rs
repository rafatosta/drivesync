//! Vault diagnostics and opt-in isolated Secret Service contract tests.
use super::*;
use dbus_secret_service::Error as ServiceError;

struct SensitiveError;
impl std::fmt::Debug for SensitiveError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        panic!("provider debug must never be formatted")
    }
}
impl std::fmt::Display for SensitiveError {
    fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        panic!("provider text must never be formatted")
    }
}
impl std::error::Error for SensitiveError {}

#[test]
fn classification_never_formats_provider_details() {
    let cases = [
        (
            keyring::Error::PlatformFailure(Box::new(SensitiveError)),
            VaultFailure::Platform,
        ),
        (
            keyring::Error::NoStorageAccess(Box::new(SensitiveError)),
            VaultFailure::Platform,
        ),
        (
            keyring::Error::PlatformFailure(Box::new(ServiceError::Crypto(Box::new(
                SensitiveError,
            )))),
            VaultFailure::Crypto,
        ),
        (
            keyring::Error::NoStorageAccess(Box::new(ServiceError::Locked)),
            VaultFailure::Locked,
        ),
        (
            keyring::Error::NoStorageAccess(Box::new(ServiceError::Prompt)),
            VaultFailure::PromptDismissed,
        ),
        (
            keyring::Error::PlatformFailure(Box::new(ServiceError::Unavailable)),
            VaultFailure::Unavailable,
        ),
        (
            keyring::Error::PlatformFailure(Box::new(ServiceError::Parse)),
            VaultFailure::Protocol,
        ),
        (
            keyring::Error::BadEncoding(b"private-token".to_vec()),
            VaultFailure::InvalidData,
        ),
        (
            keyring::Error::Invalid("private-account".into(), "private-token".into()),
            VaultFailure::InvalidAttribute,
        ),
        (
            keyring::Error::TooLong("private-account".into(), 5),
            VaultFailure::InvalidAttribute,
        ),
        (keyring::Error::Ambiguous(vec![]), VaultFailure::Ambiguous),
    ];
    for (error, expected) in cases {
        assert_eq!(classify(&error), expected);
        let public = keyring_failure(VaultOperation::Read, error);
        assert_eq!(
            public,
            AuthError::CredentialStorage {
                operation: VaultOperation::Read,
                failure: expected
            }
        );
        let text = format!("{public} {public:?}");
        assert!(!text.contains("private-"));
        assert_eq!(text.contains("Unlock"), expected == VaultFailure::Locked);
    }
}

#[test]
fn dbus_error_names_are_allowlisted_without_message_contents() {
    for (name, category) in [
        (
            "org.freedesktop.Secret.Error.NoSession",
            VaultFailure::InvalidSession,
        ),
        (
            "org.freedesktop.Secret.Error.IsLocked",
            VaultFailure::Locked,
        ),
        (
            "org.freedesktop.DBus.Error.AccessDenied",
            VaultFailure::AccessDenied,
        ),
        ("org.freedesktop.DBus.Error.NoReply", VaultFailure::Timeout),
        (
            "org.freedesktop.DBus.Error.ServiceUnknown",
            VaultFailure::Unavailable,
        ),
        ("org.example.PrivateToken", VaultFailure::Platform),
    ] {
        let backend = ServiceError::Dbus(dbus::Error::new_custom(name, "private-token"));
        assert_eq!(
            classify(&keyring::Error::PlatformFailure(Box::new(backend))),
            category
        );
    }
}

#[test]
fn operations_remain_distinct_and_generic_failure_does_not_claim_a_lock() {
    for operation in [
        VaultOperation::Entry,
        VaultOperation::Read,
        VaultOperation::Decode,
        VaultOperation::Encode,
        VaultOperation::Write,
        VaultOperation::Delete,
    ] {
        let error = report_failure(operation, VaultFailure::Platform);
        assert!(error
            .to_string()
            .contains(&format!("{operation:?}/Platform")));
        assert!(!error.to_string().contains("Unlock"));
    }
    assert!(!AuthError::SecureStorage.to_string().contains("Unlock"));
}

/// Requires the isolated runner; never run against a desktop's real vault.
#[test]
#[ignore = "requires scripts/check-vault-isolated.py and a private Secret Service"]
fn isolated_vault_roundtrip() {
    let root = std::env::var("DRIVESYNC_TEST_VAULT_ROOT").expect("use isolated runner");
    assert!(root.contains("drivesync-vault-"));
    assert_eq!(
        std::env::var("XDG_DATA_HOME").unwrap(),
        format!("{root}/data")
    );
    assert!(std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .unwrap()
        .contains(&root));
    let id = AccountId("synthetic-vault-account".into());
    let client = "synthetic-vault-client";
    let vault = DesktopVault;
    assert!(vault.load(client, &id).unwrap().is_none());
    let mut credential = Credential {
        subject: "synthetic-vault-account".into(),
        access_token: "synthetic-access".into(),
        refresh_token: "synthetic-refresh".into(),
        expires_at: 1000,
        write_access: true,
    };
    vault.save(client, &id, &credential).unwrap();
    for step in 0..500 {
        let result = vault.load(client, &id);
        let value = result
            .unwrap_or_else(|error| panic!("read {step}: {error}"))
            .unwrap();
        assert!(value.subject == credential.subject);
        assert!(value.access_token == credential.access_token);
        if step % 10 == 0 {
            credential.expires_at += 1;
            vault
                .save(client, &id, &credential)
                .unwrap_or_else(|error| panic!("write {step}: {error}"));
        }
    }
    vault.delete(client, &id).unwrap();
    vault.delete(client, &id).unwrap();
    assert!(vault.load(client, &id).unwrap().is_none());
}
