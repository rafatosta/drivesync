//! Explicit first download into an already configured local folder.
use drivesync_engine::{AccountId, DriveSyncEngine};

fn main() {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let [account] = args.as_slice() else {
        eprintln!("Usage: drivesync-initial-download ACCOUNT");
        std::process::exit(2);
    };
    match DriveSyncEngine::new().initial_download(&AccountId(account.clone())) {
        Ok(report) => println!(
            "{} files downloaded, {} directories created, {} items already registered, {} unsupported items skipped.",
            report.files, report.directories, report.already_registered, report.unsupported
        ),
        Err(error) => {
            eprintln!("{error} Completed items may already be on disk.");
            std::process::exit(1);
        }
    }
}
