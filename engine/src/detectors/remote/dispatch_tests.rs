//! Public construction resolves the provider from real, isolated Accounts metadata.
use super::*;
use crate::{models::*, Engine, EngineConfig};
use std::{
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::{Duration, Instant},
};
struct Profile(PathBuf);
impl Profile {
    fn new(provider: &str) -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let base = std::env::temp_dir().join(format!(
            "remote-dispatch-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir(&base).unwrap();
        let accounts = base.join("accounts");
        std::fs::create_dir(&accounts).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&accounts, std::fs::Permissions::from_mode(0o700)).unwrap();
        let file = accounts.join("accounts.json");
        std::fs::write(&file, serde_json::to_vec(&serde_json::json!({
            "version":1, "application":"org.example.RemoteDispatch", "intent":null,
            "records":{"account": {"account": {"id":"account", "provider":provider,
                "provider_user_id":"synthetic-subject", "email":"fixture@example.invalid", "display_name":null,
                "state":"ReauthorizationRequired", "created_at":{"unix_millis":0}, "updated_at":{"unix_millis":0}},
                "key":format!("{provider}:account:synthetic")}}
        })).unwrap()).unwrap();
        std::fs::set_permissions(&file, std::fs::Permissions::from_mode(0o600)).unwrap();
        Self(base)
    }
    fn request(&self, account: &str) -> RemoteDetectorCreateRequest {
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let connector = runtime
            .block_on(Engine::open(EngineConfig {
                data_directory: self.0.join("accounts"),
                application_id: "org.example.RemoteDispatch".into(),
                google_oauth_file: None,
            }))
            .unwrap();
        RemoteDetectorCreateRequest {
            connector,
            scope: RemoteDetectorScope {
                account_id: AccountId(account.into()),
                sync_id: SyncId("sync".into()),
                remote_root_id: "google-looking-root".into(),
            },
            settings: RemoteDetectorSettings::default(),
            data_directory: self.0.join("queues"),
        }
    }
}
impl Drop for Profile {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn registered_provider_is_selected_from_accounts_without_authentication_at_construction() {
    let profile = Profile::new("google");
    let detector = RemoteDetector::new(profile.request("account")).unwrap();
    assert_eq!(
        detector.status(RemoteStatusRequest {}).unwrap().state,
        RemoteState::Stopped
    );
    assert_eq!(detector.diagnostics().queue.detected, 0);
    detector
        .start(RemoteStartRequest {
            mode: RemotePollMode::Initial,
        })
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(3);
    while detector.diagnostics().status.last_error.is_none() {
        assert!(Instant::now() < deadline);
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        detector.diagnostics().status.last_error,
        Some(RemoteError::AuthorizationRequired)
    );
    detector.stop(RemoteStopRequest {}).unwrap();
}
#[test]
fn unsupported_account_provider_never_falls_back_to_google_or_creates_storage() {
    let profile = Profile::new("unsupported");
    let error = RemoteDetector::new(profile.request("account"))
        .err()
        .unwrap();
    assert_eq!(error.kind, RemoteError::UnsupportedProvider);
    assert!(!profile.0.join("queues").exists());
}
#[test]
fn missing_account_is_rejected_before_provider_construction_and_storage() {
    let profile = Profile::new("google");
    let error = RemoteDetector::new(profile.request("another-account"))
        .err()
        .unwrap();
    assert_eq!(error.kind, RemoteError::AuthorizationRequired);
    assert!(!profile.0.join("queues").exists());
}
