//! Opt-in, scope-checked manual cycles for controlled live tests without GTK.
use drivesync_engine::{
    AuthError, Cancellation, DownloadStage, DriveSyncEngine, EngineError, FolderConfiguration,
};
use serde_json::json;
use std::{io::Write, path::PathBuf, process::ExitCode};

struct Options {
    account: String,
    remote: String,
    local: PathBuf,
    expected: Option<Vec<usize>>,
    cancel_after: Option<u64>,
    expected_error: Option<String>,
    hold: bool,
}
impl Options {
    fn parse(args: impl IntoIterator<Item = String>) -> Result<Self, String> {
        let mut args = args.into_iter();
        let mut options = Self {
            account: args.next().ok_or("Missing account email")?,
            remote: args.next().ok_or("Missing remote test root ID")?,
            local: args
                .next()
                .ok_or("Missing absolute local test root")?
                .into(),
            expected: None,
            cancel_after: None,
            expected_error: None,
            hold: false,
        };
        while let Some(flag) = args.next() {
            match flag.as_str() {
                "--expect" if options.expected.is_none() => {
                    let values = args
                        .next()
                        .ok_or("Missing expected counts")?
                        .split(',')
                        .map(str::parse)
                        .collect::<Result<Vec<usize>, _>>()
                        .map_err(|_| "Invalid counts")?;
                    if values.len() != 6 {
                        return Err("Expected six counts".into());
                    }
                    options.expected = Some(values);
                }
                "--cancel-after-bytes" if options.cancel_after.is_none() => {
                    let bytes = args
                        .next()
                        .ok_or("Missing byte threshold")?
                        .parse::<u64>()
                        .map_err(|_| "Invalid byte threshold")?;
                    if bytes == 0 {
                        return Err("Byte threshold must be positive".into());
                    }
                    options.cancel_after = Some(bytes);
                }
                "--expect-error" if options.expected_error.is_none() => {
                    let error = args.next().ok_or("Missing expected error")?;
                    if error != "cancelled" && error != "network" {
                        return Err("Expected error must be cancelled or network".into());
                    }
                    options.expected_error = Some(error);
                }
                "--hold-at-install" if !options.hold => options.hold = true,
                _ => return Err("Unknown or duplicate option".into()),
            }
        }
        if options.account.is_empty()
            || options.remote.is_empty()
            || options.remote == "root"
            || !options.local.is_absolute()
            || (options.hold && options.cancel_after.is_some())
            || (options.expected.is_some() && options.expected_error.is_some())
        {
            return Err("Invalid or incompatible test arguments".into());
        }
        Ok(options)
    }
    /// Test tools cannot silently choose a first account or reconfigure a real root.
    fn matches(&self, folder: &FolderConfiguration) -> bool {
        folder.remote.id == self.remote
            && folder.local_display == self.local
            && folder
                .remote
                .name
                .starts_with("DriveSync — Ensaio controlado")
    }
}
fn run() -> Result<(), String> {
    if std::env::var("DRIVESYNC_LIVE_GOOGLE_TEST").as_deref() != Ok("1") {
        return Err("Set DRIVESYNC_LIVE_GOOGLE_TEST=1 explicitly".into());
    }
    let mut options = Options::parse(std::env::args().skip(1))?;
    options.local =
        std::fs::canonicalize(&options.local).map_err(|_| "Local test root is unavailable")?;
    let engine = DriveSyncEngine::new();
    let accounts = engine.list_accounts().map_err(|e| e.to_string())?;
    let selected: Vec<_> = accounts
        .iter()
        .filter(|a| a.email.as_deref() == Some(options.account.as_str()))
        .collect();
    if selected.len() != 1 {
        return Err("Exactly one saved account must match the supplied email".into());
    }
    let id = &selected[0].id;
    let folder = engine
        .saved_folder(id)
        .map_err(|e| e.to_string())?
        .ok_or("No saved folder pair")?;
    if !options.matches(&folder) {
        return Err("Saved folder pair does not match the controlled test scope".into());
    }
    let cancel = Cancellation::new();
    let mut held = false;
    let mut media_attempts = 0usize;
    let result = engine.download_once(id, &cancel, |event| {
        if let Some(item) = event.current {
            if item.stage == DownloadStage::Downloading {
                if item.received == 0 {
                    media_attempts += 1;
                }
                if options
                    .cancel_after
                    .is_some_and(|limit| item.received >= limit)
                {
                    cancel.cancel();
                }
            }
            if options.hold && !held && item.stage == DownloadStage::Installing {
                held = true;
                println!("{}", json!({"event":"ready_for_install"}));
                std::io::stdout().flush().expect("flush test marker");
                std::thread::sleep(std::time::Duration::from_secs(30));
            }
        }
    });
    match result {
        Ok(report) => {
            let counts = vec![
                report.downloaded,
                report.updated,
                report.recovered,
                report.unchanged,
                report.issues.len(),
                report.preserved_copies.len(),
            ];
            println!(
                "{}",
                json!({"event":"completed","counts":counts,"media_attempts":media_attempts})
            );
            if options.expected_error.is_some() {
                return Err("Expected error did not occur".into());
            }
            if options
                .expected
                .as_ref()
                .is_some_and(|expected| expected != &counts)
            {
                return Err("Cycle counts did not match the oracle".into());
            }
        }
        Err(error) => {
            let code = match error {
                EngineError::Authentication(AuthError::Cancelled) => "cancelled",
                EngineError::Authentication(AuthError::Network) => "network",
                _ => "other",
            };
            println!(
                "{}",
                json!({"event":"failed","code":code,"media_attempts":media_attempts})
            );
            if options.expected_error.as_deref() != Some(code) {
                return Err(error.to_string());
            }
        }
    }
    Ok(())
}
fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_implicit_roots_and_incompatible_fault_injection() {
        for args in [
            vec![],
            vec!["a", "root", "/test"],
            vec!["a", "remote", "relative"],
            vec![
                "a",
                "remote",
                "/test",
                "--hold-at-install",
                "--cancel-after-bytes",
                "1",
            ],
            vec!["a", "remote", "/test", "--expect", "0,1"],
        ] {
            assert!(Options::parse(args.into_iter().map(String::from)).is_err());
        }
    }
    #[test]
    fn requires_exact_test_folder_pair() {
        let options = Options::parse(
            [
                "test@example.com",
                "remote",
                "/test",
                "--expect",
                "0,0,0,8,4,0",
            ]
            .map(String::from),
        )
        .unwrap();
        let mut folder = FolderConfiguration {
            account_id: drivesync_engine::AccountId("test".into()),
            remote: drivesync_engine::DriveFolder {
                id: "remote".into(),
                name: "DriveSync — Ensaio controlado — fixture".into(),
            },
            local: "/portal/access".into(),
            local_display: "/test".into(),
        };
        assert!(options.matches(&folder));
        folder.local_display = "/other".into();
        assert!(!options.matches(&folder));
        folder.local_display = "/test".into();
        folder.remote.id = "other".into();
        assert!(!options.matches(&folder));
        folder.remote.id = "remote".into();
        folder.remote.name = "Personal documents".into();
        assert!(!options.matches(&folder));
    }
}
