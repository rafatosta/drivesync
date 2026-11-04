//! Explicit, read-only remote detection diagnostic.
//!
//! Commands never transfer or modify user files. Event acknowledgement only
//! updates the detector's local delivery journal.
use drivesync_engine::{AccountId, DetectionCorpus, DetectionError, DriveSyncEngine};
use std::{error::Error, io};

const USAGE: &str = "\
Usage:
  drivesync-remote-diagnostic accounts
  drivesync-remote-diagnostic check ACCOUNT [--shared-drive DRIVE_ID]
  drivesync-remote-diagnostic events ACCOUNT ROOT [--shared-drive DRIVE_ID]
  drivesync-remote-diagnostic ack ACCOUNT ROOT EVENT_ID [--shared-drive DRIVE_ID]

Output includes account identifiers and remote metadata; handle it as private.
Events remain pending until explicitly acknowledged.
";

enum Command {
    Accounts,
    Check(AccountId, DetectionCorpus),
    Events(AccountId, String, DetectionCorpus),
    Ack(AccountId, String, i64, DetectionCorpus),
    Help,
}

fn parse(mut args: Vec<String>) -> Result<Command, &'static str> {
    if args.is_empty() || args == ["--help"] || args == ["-h"] {
        return Ok(Command::Help);
    }
    let corpus = if args.len() >= 2 && args[args.len() - 2] == "--shared-drive" {
        let id = args.pop().ok_or("Missing shared drive ID")?;
        args.pop();
        DetectionCorpus::SharedDrive(id)
    } else {
        DetectionCorpus::User
    };
    match args.as_slice() {
        [command] if command == "accounts" && corpus == DetectionCorpus::User => {
            Ok(Command::Accounts)
        }
        [command, account] if command == "check" => {
            Ok(Command::Check(AccountId(account.clone()), corpus))
        }
        [command, account, root] if command == "events" => Ok(Command::Events(
            AccountId(account.clone()),
            root.clone(),
            corpus,
        )),
        [command, account, root, id] if command == "ack" => {
            let id = id.parse::<i64>().map_err(|_| "Invalid event ID")?;
            if id <= 0 {
                return Err("Invalid event ID");
            }
            Ok(Command::Ack(
                AccountId(account.clone()),
                root.clone(),
                id,
                corpus,
            ))
        }
        _ => Err("Invalid arguments"),
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let command = parse(std::env::args().skip(1).collect())
        .map_err(|message| io::Error::new(io::ErrorKind::InvalidInput, message))?;
    let engine = DriveSyncEngine::new();
    match command {
        Command::Help => print!("{USAGE}"),
        Command::Accounts => println!("{}", serde_json::to_string(&engine.list_accounts()?)?),
        Command::Check(account, corpus) => println!(
            "{}",
            serde_json::to_string(&engine.check_remote_changes(&account, &corpus)?)?
        ),
        Command::Events(account, root, corpus) => println!(
            "{}",
            serde_json::to_string(&engine.remote_events(&account, &root, &corpus)?)?
        ),
        Command::Ack(account, root, id, corpus) => {
            let removed = engine.acknowledge_remote_event(&account, &root, &corpus, id)?;
            println!("{{\"removed\":{removed}}}");
        }
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        if let Some(detection) = error.downcast_ref::<DetectionError>() {
            eprintln!(
                "Remote diagnostic failed ({:?}): {detection}",
                detection.kind()
            );
        } else {
            eprintln!("Remote diagnostic failed: {error}");
        }
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|s| (*s).into()).collect()
    }

    #[test]
    fn parses_scoped_commands_without_implicit_acknowledgement() {
        assert!(matches!(
            parse(args(&["check", "google:a"])),
            Ok(Command::Check(_, DetectionCorpus::User))
        ));
        assert!(
            matches!(parse(args(&["events", "google:a", "root", "--shared-drive", "drive"])), Ok(Command::Events(_, _, DetectionCorpus::SharedDrive(id))) if id == "drive")
        );
        assert!(matches!(
            parse(args(&["ack", "google:a", "root", "1"])),
            Ok(Command::Ack(_, _, 1, DetectionCorpus::User))
        ));
        assert!(parse(args(&["ack", "google:a", "root", "0"])).is_err());
        assert!(parse(args(&["check", "google:a", "--shared-drive"])).is_err());
        assert!(parse(args(&["accounts", "--shared-drive", "drive"])).is_err());
    }
}
