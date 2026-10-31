//! Durable effect ordering with independent remote and real-filesystem oracles.
use super::{local, Intent, Plan, RemoteVersion, ResolutionChoice, Stage};
use crate::{
    download::{
        filesystem::{self, Root},
        stream,
    },
    state::Store,
    Cancellation, DownloadPlan, EngineError,
};
use std::{fs::File, io::Read};

/// Conditional provider operations. Implementations must check account/root,
/// permissions and configuration immediately before every mutation.
pub(crate) trait Remote {
    fn configured(&mut self) -> Result<(), EngineError>;
    fn now(&self) -> Result<u64, EngineError> {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|v| v.as_secs())
            .map_err(|_| EngineError::StateStorage)
    }
    fn observe(&mut self, id: &str) -> Result<RemoteVersion, EngineError>;
    fn media(&mut self, version: &RemoteVersion) -> Result<Box<dyn Read>, EngineError>;
    fn generate(&mut self) -> Result<String, EngineError>;
    fn copy(&mut self, plan: &Plan, bytes: File, blob: &super::Blob) -> Result<(), EngineError>;
    fn update(
        &mut self,
        plan: &Plan,
        bytes: Option<File>,
        blob: &super::Blob,
    ) -> Result<(), EngineError>;
}
pub(crate) struct Context<'a> {
    pub client: &'a str,
    pub scope: &'a DownloadPlan,
    pub root: &'a Root,
    pub store: &'a Store,
    pub cancel: &'a Cancellation,
}

/// Verified effects at completion, not a statement about future file availability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionReport {
    pub operation_id: String,
    pub path: std::path::PathBuf,
    pub preserved_paths: Vec<std::path::PathBuf>,
    pub additional_path: Option<std::path::PathBuf>,
}
fn report(intent: &Intent) -> ResolutionReport {
    ResolutionReport {
        operation_id: intent.id.clone(),
        path: intent.plan.target.clone(),
        preserved_paths: vec![
            intent
                .plan
                .origin
                .with_file_name(local::remote_name(intent)),
            intent.plan.origin.with_file_name(intent.backup()),
        ],
        additional_path: intent
            .plan
            .copy_name
            .as_ref()
            .map(|v| intent.plan.target.with_file_name(v)),
    }
}
fn save(ctx: &Context<'_>, old: &mut Intent, next: Intent) -> Result<(), EngineError> {
    ctx.root.validate()?;
    ctx.store
        .save_resolution(ctx.client, ctx.scope, Some(old), &next)?;
    *old = next;
    Ok(())
}
fn stage(ctx: &Context<'_>, intent: &mut Intent, next: Stage) -> Result<(), EngineError> {
    let mut value = intent.clone();
    value.stage = next;
    save(ctx, intent, value)
}
fn check(ctx: &Context<'_>, remote: &mut impl Remote, intent: &Intent) -> Result<(), EngineError> {
    crate::local::check(ctx.cancel)?;
    local::parent(ctx.root, intent)?;
    remote.configured()
}

/// Verify remote bytes bracketed by exact metadata and reobserve after streaming.
/// Metadata and checksums alone never establish equality with selected bytes.
fn content(
    ctx: &Context<'_>,
    remote: &mut impl Remote,
    version: &RemoteVersion,
) -> Result<String, EngineError> {
    if !version.valid() {
        return Err(EngineError::InvalidPlan);
    }
    let mut media = remote.media(version)?;
    let sha = stream::copy(
        &mut media,
        &mut std::io::sink(),
        version.item.size.ok_or(EngineError::InvalidPlan)?,
        version
            .item
            .md5_checksum
            .as_deref()
            .ok_or(EngineError::InvalidPlan)?,
        ctx.cancel,
        |_| {},
        || ctx.root.validate(),
    )?;
    if remote.observe(&version.item.id)? != *version {
        return Err(EngineError::RemoteChanged);
    }
    Ok(sha)
}
fn desired(
    ctx: &Context<'_>,
    remote: &mut impl Remote,
    intent: &Intent,
    copy: bool,
) -> Result<Option<RemoteVersion>, EngineError> {
    let id = if copy {
        intent
            .plan
            .copy_id
            .as_deref()
            .ok_or(EngineError::InvalidPlan)?
    } else {
        &intent.plan.remote.item.id
    };
    let version = remote.observe(id)?;
    let blob = if copy {
        intent.remote.as_ref().ok_or(EngineError::StateStorage)?
    } else {
        intent.winner()?
    };
    let name = if copy {
        intent
            .plan
            .copy_name
            .as_deref()
            .ok_or(EngineError::InvalidPlan)?
    } else {
        &intent.plan.desired_name
    };
    if version.item.parent_id != intent.plan.remote.item.parent_id
        || version.item.name != name
        || version.item.size != Some(blob.size)
        || version.item.md5_checksum.as_deref() != Some(&blob.md5)
    {
        return Ok(None);
    }
    if content(ctx, remote, &version)? != blob.sha256 {
        return Ok(None);
    }
    Ok(Some(version))
}

/// Advances one previously persisted decision. Interrupted mutations are first
/// reconciled by ID and verified content; an uncertain result is never assumed
/// absent and a new remote identity is never allocated during replay.
pub(crate) fn run(
    ctx: &Context<'_>,
    remote: &mut impl Remote,
    mut intent: Intent,
) -> Result<ResolutionReport, EngineError> {
    if !intent.valid() || ctx.scope.local_root != ctx.root.path() {
        return Err(EngineError::InvalidPlan);
    }
    super::guard::check(ctx)?;
    if intent.stage == Stage::Confirmed {
        super::guard::retire(ctx, &intent)?;
        return Ok(report(&intent));
    }
    if intent.stage == Stage::Aborted {
        return Err(EngineError::RemoteChanged);
    }
    check(ctx, remote, &intent)?;
    if intent.stage >= Stage::CopyPending {
        super::guard::ensure(ctx, &intent)?;
    }
    if intent.stage == Stage::Preparing {
        let preserved = (|| {
            if remote.observe(&intent.plan.remote.item.id)? != intent.plan.remote {
                return Err(EngineError::RemoteChanged);
            }
            let local = local::snapshot_local(ctx.root, &intent, ctx.cancel)?;
            let version = &intent.plan.remote;
            let mut bytes = remote.media(version)?;
            let saved = local::capture(
                ctx.root,
                &intent,
                &local::remote_name(&intent),
                &mut bytes,
                local::Expected {
                    size: version.item.size.ok_or(EngineError::InvalidPlan)?,
                    md5: version
                        .item
                        .md5_checksum
                        .as_deref()
                        .ok_or(EngineError::InvalidPlan)?,
                    sha256: None,
                },
                ctx.cancel,
            )?;
            if remote.observe(&version.item.id)? != *version {
                return Err(EngineError::RemoteChanged);
            }
            local::original(ctx.root, &intent, ctx.cancel)?;
            Ok((local, saved))
        })();
        match preserved {
            Ok((local, observed)) => {
                let mut next = intent.clone();
                next.local = Some(local);
                next.remote = Some(observed);
                next.stage = Stage::Preserved;
                save(ctx, &mut intent, next)?;
            }
            Err(error) => {
                // No remote mutation or local cutover has started. Aborting is
                // safe; snapshots remain and a new comparison is required.
                stage(ctx, &mut intent, Stage::Aborted)?;
                return Err(error);
            }
        }
    }
    let mut writable = true;
    if intent.stage <= Stage::RemotePending {
        let mut next = intent.clone();
        writable = next.retry.reserve(
            remote.now()?,
            u64::from_str_radix(&intent.id[..8], 16).map_err(|_| EngineError::StateStorage)?,
        )?;
        save(ctx, &mut intent, next)?;
    }
    if intent.stage == Stage::Preserved {
        check(ctx, remote, &intent)?;
        local::original(ctx.root, &intent, ctx.cancel)?;
        if remote.observe(&intent.plan.remote.item.id)? != intent.plan.remote {
            stage(ctx, &mut intent, Stage::Aborted)?;
            return Err(EngineError::RemoteChanged);
        }
        let next = if intent.plan.choice == ResolutionChoice::KeepBoth {
            Stage::CopyPending
        } else {
            Stage::RemotePending
        };
        stage(ctx, &mut intent, next)?;
    }
    if intent.stage == Stage::CopyPending {
        super::guard::ensure(ctx, &intent)?;
        check(ctx, remote, &intent)?;
        let existing = match desired(ctx, remote, &intent, true) {
            Ok(value) => value,
            Err(EngineError::RemoteFolderUnavailable) => None,
            Err(error) => return Err(error),
        };
        let copy = if let Some(existing) = existing {
            existing
        } else {
            if !writable {
                return Err(EngineError::UploadRetryExhausted);
            }
            local::original(ctx.root, &intent, ctx.cancel)?;
            if remote.observe(&intent.plan.remote.item.id)? != intent.plan.remote {
                return Err(EngineError::RemoteChanged);
            }
            remote.copy(
                &intent.plan,
                local::snapshot_file(ctx.root, &intent, true, ctx.cancel)?,
                intent.remote.as_ref().ok_or(EngineError::StateStorage)?,
            )?;
            desired(ctx, remote, &intent, true)?.ok_or(EngineError::RemoteChanged)?
        };
        let mut next = intent.clone();
        next.copy_result = Some(copy);
        next.stage = Stage::RemotePending;
        save(ctx, &mut intent, next)?;
    }
    if intent.stage == Stage::RemotePending {
        super::guard::ensure(ctx, &intent)?;
        check(ctx, remote, &intent)?;
        let current = remote.observe(&intent.plan.remote.item.id)?;
        let result = if let Some(version) = desired(ctx, remote, &intent, false)? {
            version
        } else {
            if current != intent.plan.remote {
                return Err(EngineError::RemoteChanged);
            }
            if !writable {
                return Err(EngineError::UploadRetryExhausted);
            }
            local::original(ctx.root, &intent, ctx.cancel)?;
            local::target_available(ctx.root, &intent.plan.origin, &intent.plan.target)?;
            let source = if intent.winner()?.sha256
                != intent
                    .remote
                    .as_ref()
                    .ok_or(EngineError::StateStorage)?
                    .sha256
            {
                Some(local::snapshot_file(ctx.root, &intent, false, ctx.cancel)?)
            } else {
                None
            };
            remote.update(&intent.plan, source, intent.winner()?)?;
            desired(ctx, remote, &intent, false)?.ok_or(EngineError::RemoteChanged)?
        };
        let mut next = intent.clone();
        next.result = Some(result);
        next.stage = Stage::RemoteVerified;
        save(ctx, &mut intent, next)?;
    }
    if intent.stage == Stage::RemoteVerified {
        check(ctx, remote, &intent)?;
        if desired(ctx, remote, &intent, false)?.is_none() {
            return Err(EngineError::RemoteChanged);
        }
        local::original(ctx.root, &intent, ctx.cancel)?;
        local::target_available(ctx.root, &intent.plan.origin, &intent.plan.target)?;
        let result = local::installation(ctx.root, &intent, false, ctx.cancel)?;
        let copy = if intent.plan.choice == ResolutionChoice::KeepBoth {
            Some(local::installation(ctx.root, &intent, true, ctx.cancel)?)
        } else {
            None
        };
        let mut next = intent.clone();
        next.installed = Some(result.identity);
        next.copy_installed = copy.map(|v| v.identity);
        next.stage = Stage::LocalPending;
        save(ctx, &mut intent, next)?;
    }
    if intent.stage == Stage::LocalPending {
        check(ctx, remote, &intent)?;
        // A later remote edit cannot authorize applying the old local plan.
        if desired(ctx, remote, &intent, false)?.is_none() {
            return Err(EngineError::RemoteChanged);
        }
        if intent.plan.choice == ResolutionChoice::KeepBoth {
            if desired(ctx, remote, &intent, true)?.is_none() {
                return Err(EngineError::RemoteChanged);
            }
            local::install(ctx.root, &intent, true, ctx.cancel)?;
        }
        local::install(ctx.root, &intent, false, ctx.cancel)?;
        stage(ctx, &mut intent, Stage::Verified)?;
    }
    if intent.stage == Stage::Verified {
        check(ctx, remote, &intent)?;
        local::verify_preserved(ctx.root, &intent, ctx.cancel)?;
        let result = desired(ctx, remote, &intent, false)?.ok_or(EngineError::RemoteChanged)?;
        if !local::verify_installed(ctx.root, &intent, false, ctx.cancel)? {
            return Err(EngineError::DownloadConflict);
        }
        let mut next = intent.clone();
        next.result = Some(result);
        if intent.plan.choice == ResolutionChoice::KeepBoth {
            if !local::verify_installed(ctx.root, &intent, true, ctx.cancel)? {
                return Err(EngineError::DownloadConflict);
            }
            next.copy_result =
                Some(desired(ctx, remote, &intent, true)?.ok_or(EngineError::RemoteChanged)?);
        }
        save(ctx, &mut intent, next)?;
        crate::local::check(ctx.cancel)?;
        intent = ctx
            .store
            .confirm_resolution(ctx.client, ctx.scope, &intent, intent.parent)?;
        super::guard::retire(ctx, &intent)?;
    }
    Ok(report(&intent))
}

/// Publishes the chosen exact plan before creating any snapshot or mutation.
pub(crate) fn begin(ctx: &Context<'_>, plan: Plan, id: String) -> Result<Intent, EngineError> {
    crate::local::check(ctx.cancel)?;
    super::guard::check(ctx)?;
    let old = ctx
        .store
        .resolutions(ctx.client, ctx.scope)?
        .into_iter()
        .find(|v| v.plan.remote.item.id == plan.remote.item.id);
    if old
        .as_ref()
        .is_some_and(|v| !matches!(v.stage, Stage::Confirmed | Stage::Aborted))
    {
        return Err(EngineError::SyncBusy);
    }
    local::target_available(ctx.root, &plan.origin, &plan.target)?;
    let intent = Intent {
        id,
        root: filesystem::identity(&ctx.root.file)?,
        parent: filesystem::identity(&ctx.root.parent(&plan.origin)?.0)?,
        plan,
        stage: Stage::Preparing,
        local: None,
        remote: None,
        result: None,
        copy_result: None,
        installed: None,
        copy_installed: None,
        retry: Default::default(),
    };
    ctx.store
        .save_resolution(ctx.client, ctx.scope, old.as_ref(), &intent)?;
    Ok(intent)
}
