//! Read-only remote observations and isolated, durable detection state.
mod detector;
mod journal;
mod provider;

use crate::{configuration::valid_id, AccountId};
use std::path::PathBuf;

use detector::Limits;
pub use detector::{
    Check as DetectionCheck, DeliveredEvent as RemoteDelivery, DetectionError,
    DetectionFailureKind, Event as RemoteEvent, EventKind as RemoteEventKind,
    Membership as RemoteMembership,
};
pub(crate) use provider::GoogleDrive;
pub use provider::{
    Corpus as DetectionCorpus, File as RemoteFileMetadata,
    FileCapabilities as RemoteFileCapabilities,
};

fn scope(
    client: &str,
    account: &AccountId,
    root: &str,
    corpus: &DetectionCorpus,
) -> Result<journal::Scope, DetectionError> {
    if client.is_empty() || account.0.is_empty() || !valid_id(root) {
        return Err(DetectionError::Intervention);
    }
    let corpus = match corpus {
        DetectionCorpus::User => "user".to_owned(),
        DetectionCorpus::SharedDrive(id) if valid_id(id) => format!("drive:{id}"),
        DetectionCorpus::SharedDrive(_) => return Err(DetectionError::Intervention),
    };
    Ok(journal::Scope {
        client: client.to_owned(),
        account: account.0.clone(),
        root: root.to_owned(),
        corpus,
    })
}

fn journal() -> Result<journal::Journal, DetectionError> {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .filter(|p| p.is_absolute())
                .map(|p| p.join(".local/share"))
        })
        .ok_or(DetectionError::Intervention)?;
    journal::Journal::open(&base.join("drivesync/remote-detector")).map_err(Into::into)
}

pub(crate) fn configured_corpus(
    provider: &GoogleDrive<'_>,
    root: &str,
    client: &str,
    account: &AccountId,
) -> Result<DetectionCorpus, DetectionError> {
    match journal()?
        .known_corpus(client, &account.0, root)?
        .as_deref()
    {
        Some("user") => Ok(DetectionCorpus::User),
        Some(key) if key.strip_prefix("drive:").is_some_and(valid_id) => {
            Ok(DetectionCorpus::SharedDrive(key["drive:".len()..].into()))
        }
        _ => Ok(provider
            .file(root)?
            .drive_id
            .map_or(DetectionCorpus::User, DetectionCorpus::SharedDrive)),
    }
}

pub(crate) fn check(
    provider: &GoogleDrive<'_>,
    client: &str,
    account: &AccountId,
    root: &str,
    corpus: &DetectionCorpus,
    incremental: bool,
) -> Result<DetectionCheck, DetectionError> {
    let scope = scope(client, account, root, corpus)?;
    detector::check(
        provider,
        &journal()?,
        &scope,
        corpus,
        Limits::default(),
        incremental,
    )
}

pub(crate) fn pending(
    client: &str,
    account: &AccountId,
    root: &str,
    corpus: &DetectionCorpus,
) -> Result<Vec<RemoteDelivery>, DetectionError> {
    detector::pending(&journal()?, &scope(client, account, root, corpus)?)
}

pub(crate) fn acknowledge(
    client: &str,
    account: &AccountId,
    root: &str,
    corpus: &DetectionCorpus,
    event_id: i64,
) -> Result<bool, DetectionError> {
    if event_id <= 0 {
        return Err(DetectionError::Intervention);
    }
    journal()?
        .acknowledge(&scope(client, account, root, corpus)?, event_id)
        .map_err(Into::into)
}

pub(crate) fn acknowledge_many(
    client: &str,
    account: &AccountId,
    root: &str,
    corpus: &DetectionCorpus,
    ids: &[i64],
) -> Result<(), DetectionError> {
    if ids.is_empty() {
        return Ok(());
    }
    journal()?
        .acknowledge_many(&scope(client, account, root, corpus)?, ids)
        .map_err(Into::into)
}
