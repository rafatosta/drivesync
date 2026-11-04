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

#[test]
fn backend_encoding_read_retries_once_and_preserves_persistent_failures() {
    let error = AuthError::CredentialStorage {
        operation: VaultOperation::Read,
        failure: VaultFailure::InvalidData,
    };
    let mut calls = 0;
    assert_eq!(
        recover_read(|| {
            calls += 1;
            if calls == 1 {
                Err(error)
            } else {
                Ok(42)
            }
        }),
        Ok(42)
    );
    assert_eq!(calls, 2);
    calls = 0;
    assert_eq!(
        recover_read::<()>(|| {
            calls += 1;
            Err(error)
        }),
        Err(error)
    );
    assert_eq!(calls, 2);
    let decode = AuthError::CredentialStorage {
        operation: VaultOperation::Decode,
        failure: VaultFailure::InvalidData,
    };
    calls = 0;
    assert_eq!(
        recover_read::<()>(|| {
            calls += 1;
            Err(decode)
        }),
        Err(decode)
    );
    assert_eq!(calls, 1);
}

#[test]
fn backend_encoding_message_does_not_claim_stored_json_corruption() {
    let read = AuthError::CredentialStorage {
        operation: VaultOperation::Read,
        failure: VaultFailure::InvalidData,
    }
    .to_string();
    let decode = AuthError::CredentialStorage {
        operation: VaultOperation::Decode,
        failure: VaultFailure::InvalidData,
    }
    .to_string();
    assert!(read.contains("vault returned unreadable data"));
    assert!(read.contains("Read/InvalidData"));
    assert!(!read.contains("saved credential could not be decoded"));
    assert!(decode.contains("saved credential could not be decoded"));
    assert!(decode.contains("Decode/InvalidData"));
}

#[test]
fn fresh_session_retry_is_bounded_and_only_for_eligible_backend_reads() {
    let transient = AuthError::CredentialStorage {
        operation: VaultOperation::Read,
        failure: VaultFailure::Crypto,
    };
    let mut calls = 0;
    let recovered = recover_read(|| {
        calls += 1;
        if calls == 1 {
            Err(transient)
        } else {
            Ok(42)
        }
    });
    assert_eq!(recovered, Ok(42));
    assert_eq!(calls, 2);
    calls = 0;
    assert_eq!(
        recover_read::<()>(|| {
            calls += 1;
            Err(transient)
        }),
        Err(transient)
    );
    assert_eq!(calls, 2);
    for operation in [
        VaultOperation::Read,
        VaultOperation::Entry,
        VaultOperation::Decode,
        VaultOperation::Write,
        VaultOperation::Delete,
    ] {
        for failure in [
            VaultFailure::Crypto,
            VaultFailure::Locked,
            VaultFailure::AccessDenied,
            VaultFailure::InvalidData,
            VaultFailure::Ambiguous,
            VaultFailure::Platform,
            VaultFailure::Unavailable,
            VaultFailure::InvalidSession,
        ] {
            if operation == VaultOperation::Read
                && matches!(failure, VaultFailure::Crypto | VaultFailure::InvalidData)
            {
                continue;
            }
            let error = AuthError::CredentialStorage { operation, failure };
            calls = 0;
            assert_eq!(
                recover_read::<()>(|| {
                    calls += 1;
                    Err(error)
                }),
                Err(error)
            );
            assert_eq!(calls, 1);
        }
    }
    calls = 0;
    assert_eq!(
        recover_read::<Option<()>>(|| {
            calls += 1;
            Ok(None)
        }),
        Ok(None)
    );
    assert_eq!(calls, 1);
}

/// Requires the isolated runner; never run against a desktop's real vault.
#[test]
#[ignore = "requires scripts/check-vault-isolated.py and a private Secret Service"]
fn isolated_vault_roundtrip() {
    require_isolated_vault();
    let id = AccountId("synthetic-vault-account".into());
    let client = "synthetic-vault-client";
    let vault = DesktopVault;
    assert!(vault.load(client, &id).unwrap().is_none());
    let mut credential = Credential {
        subject: "synthetic-vault-account".into(),
        access_token: "synthetic-access".into(),
        refresh_token: "synthetic-refresh".into(),
        expires_at: 1000,
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
    let entry = DesktopVault::entry(client, &id).unwrap();
    entry.set_password("synthetic-invalid-json").unwrap();
    assert!(matches!(
        vault.load(client, &id),
        Err(AuthError::CredentialStorage {
            operation: VaultOperation::Decode,
            failure: VaultFailure::InvalidData
        })
    ));
    assert!(entry.get_password().unwrap() == "synthetic-invalid-json");
    vault.delete(client, &id).unwrap();
    vault.delete(client, &id).unwrap();
    assert!(vault.load(client, &id).unwrap().is_none());
}

fn require_isolated_vault() {
    let root = std::env::var("DRIVESYNC_TEST_VAULT_ROOT").expect("use isolated runner");
    assert!(root.contains("drivesync-vault-"));
    assert_eq!(
        std::env::var("XDG_DATA_HOME").unwrap(),
        format!("{root}/data")
    );
    assert!(std::env::var("DBUS_SESSION_BUS_ADDRESS")
        .unwrap()
        .contains(&root));
}

/// Uses only a private synthetic entry, never the desktop OAuth vault.
#[test]
#[ignore = "requires the isolated runner and a private Secret Service"]
fn isolated_vault_invalid_encoding_is_preserved() {
    require_isolated_vault();
    let client = "synthetic-binary-vault-client";
    let id = AccountId("synthetic-binary-vault-account".into());
    let vault = DesktopVault;
    let entry = DesktopVault::entry(client, &id).unwrap();
    let binary = [0xff, 0xfe, 0xfd];
    entry.set_secret(&binary).unwrap();
    assert!(matches!(
        vault.load(client, &id),
        Err(AuthError::CredentialStorage {
            operation: VaultOperation::Read,
            failure: VaultFailure::InvalidData
        })
    ));
    assert_eq!(
        recover_read(|| entry
            .get_secret()
            .map_err(|e| keyring_failure(VaultOperation::Read, e)))
        .unwrap(),
        binary
    );
    vault.delete(client, &id).unwrap();
}
