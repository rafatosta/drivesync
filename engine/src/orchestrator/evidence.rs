//! Pure convergence predicates shared by planning and execution verification.
use crate::{
    inventory::models::{LocalState, RemoteState},
    models::*,
};

pub(super) fn equivalent(local: &LocalState, remote: &RemoteState) -> bool {
    if crate::projections::workspace(remote) {
        return crate::projections::equivalent(local, remote);
    }
    if local.state != FileState::Present
        || remote.state != FileState::Present
        || remote.trashed != Evidence::Known(false)
        || !matches!(local.identity, Evidence::Known(_))
        || !matches!((&local.name,&remote.name),(Evidence::Known(a),Evidence::Known(b)) if a == b)
        || local.path.as_ref().is_none_or(|path| {
            path.is_absolute()
                || path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .map(|name| Evidence::Known(name.to_owned()))
                    != Some(local.name.clone())
        })
    {
        return false;
    }
    match (&local.kind, &remote.mime_type) {
        (Evidence::Known(FileKind::Directory), Evidence::Known(mime))
            if mime == "application/vnd.google-apps.folder" =>
        {
            true
        }
        (Evidence::Known(FileKind::Regular), Evidence::Known(mime))
            if !mime.starts_with("application/vnd.google-apps.") =>
        {
            matches!((&local.size,&remote.size),(Evidence::Known(a),Evidence::Known(b)) if a == b)
                && matches!((&local.checksum,&remote.checksum),(Evidence::Known(a),Evidence::Known(b)) if a == b && a.algorithm == "sha256" && !a.value.is_empty())
        }
        _ => false,
    }
}
