//! Pure model oracles; no filesystem, network, clock or credentials are used.
use super::*;

#[test]
fn relative_paths_preserve_unix_bytes_and_policy_reserved_names() {
    // MOD-PTH-01/03/04: these are literals, not normalized expectations.
    for bytes in [
        b"dir/file".as_slice(),
        b".hidden",
        b"..backup",
        b"a..b",
        b"with space/line\nbreak",
        b"back\\slash",
        b"invalid-\xff/name",
        b".drivesync/file",
    ] {
        let path = LocalRelativePath::new(bytes.to_vec()).unwrap();
        let borrowed: &[u8] = path.as_bytes();
        assert_eq!(borrowed, bytes);
        assert_eq!(borrowed.as_ptr(), path.as_bytes().as_ptr());
    }
}

#[test]
fn relative_paths_reject_root_escape_and_noncanonical_components() {
    // MOD-PTH-02: validate bytes before any physical path resolution.
    for bytes in [
        b"".as_slice(),
        b"/",
        b"/file",
        b"a/",
        b"a//b",
        b".",
        b"..",
        b"a/./b",
        b"a/../b",
        b"\0a",
        b"a\0b",
        b"a\0",
    ] {
        assert_eq!(
            LocalRelativePath::new(bytes.to_vec()),
            Err(ModelError::InvalidRelativePath)
        );
    }
}

#[test]
fn local_timestamps_preserve_negative_seconds_and_nanosecond_boundaries() {
    // MOD-TIM-01: no timezone conversion, rounding or wall-clock fallback.
    for seconds in [i64::MIN, -1, 0, 42, i64::MAX] {
        for nanos in [0, 999_999_999] {
            let timestamp = LocalTimestamp::new(seconds, nanos).unwrap();
            assert_eq!(timestamp.unix_seconds, seconds);
            assert_eq!(timestamp.nanoseconds, nanos);
        }
    }
}

#[test]
fn local_timestamps_reject_overflowing_nanoseconds_without_carry() {
    // MOD-TIM-02.
    for nanos in [1_000_000_000, u32::MAX] {
        assert_eq!(
            LocalTimestamp::new(0, nanos),
            Err(ModelError::InvalidTimestamp)
        );
    }
}

#[test]
fn metadata_states_do_not_collapse_observed_zero_false_or_empty_values() {
    // MOD-LOC-15.
    let states = [
        MetadataValue::Known(0),
        MetadataValue::NotCollected,
        MetadataValue::NotApplicable,
        MetadataValue::Unsupported,
        MetadataValue::PermissionDenied,
        MetadataValue::Unavailable,
    ];
    for (index, state) in states.iter().enumerate() {
        assert_eq!(*state, state.clone());
        for other in states.iter().skip(index + 1) {
            assert_ne!(state, other);
        }
    }
    assert_ne!(MetadataValue::Known(false), MetadataValue::NotCollected);
    assert_ne!(
        MetadataValue::Known(Vec::<u8>::new()),
        MetadataValue::Unavailable
    );
}

#[test]
fn existing_account_provider_and_timestamp_serialization_remains_compatible() {
    // MOD-ID-03/04: moving the module must preserve existing wire values.
    let account: crate::AccountId = AccountId("account-1".into());
    let provider: crate::ProviderId = ProviderId("google".into());
    let timestamp: crate::Timestamp = Timestamp { unix_millis: -1234 };
    assert_eq!(serde_json::to_string(&account).unwrap(), "\"account-1\"");
    assert_eq!(serde_json::to_string(&provider).unwrap(), "\"google\"");
    assert_eq!(
        serde_json::to_string(&timestamp).unwrap(),
        "{\"unix_millis\":-1234}"
    );
    assert_eq!(
        serde_json::from_str::<AccountId>("\"account-1\"").unwrap(),
        account
    );
}
