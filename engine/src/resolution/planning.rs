//! Same-parent rename candidates; classification is revalidated before execution.
use super::ResolutionChoice;
use crate::{
    local::{Evidence, Inventory},
    ConfirmedBase, DifferenceKind, InventoryEntry, InventoryKind, LocalObservation, PlanAction,
    PlanEntry,
};
use std::path::PathBuf;

pub(crate) struct Candidate {
    pub origin: PathBuf,
    pub target: PathBuf,
    pub fingerprint: String,
    pub choice: ResolutionChoice,
}

/// Select only proven continuous identities with independent/equal content and
/// compatible names. Moves, aliasing, ambiguous offline work and collisions
/// remain pending; a positive result still grants no filesystem/network effect.
pub(crate) fn candidate(
    item: &InventoryEntry,
    entry: &PlanEntry,
    base: &ConfirmedBase,
    local: &Inventory,
) -> Option<Candidate> {
    let PlanAction::Classified {
        decision,
        local_path,
    } = &entry.action
    else {
        return None;
    };
    if !matches!(
        decision.kind,
        DifferenceKind::LocalChanged
            | DifferenceKind::RemoteChanged
            | DifferenceKind::Conciliable
            | DifferenceKind::Equivalent
    ) || item.kind != InventoryKind::Binary
        || item.id != entry.remote_id
    {
        return None;
    }
    let location = base.location.as_ref()?;
    let (old_hash, device, inode) = crate::planner::fingerprint_parts(&base.local_fingerprint)?;
    let LocalObservation::Verified(fingerprint) = local.observe(local_path) else {
        return None;
    };
    let (hash, current_device, current_inode) = crate::planner::fingerprint_parts(&fingerprint)?;
    if (device, inode) != (current_device, current_inode)
        || item.parent_id != location.remote_parent
        || local_path.parent() != location.relative_path.parent()
        || entry.relative_path.as_ref()?.parent() != local_path.parent()
        || item.version? < base.remote_version
    {
        return None;
    }
    if local.entries.values().filter(|entry|matches!(entry,Evidence::Stable {stamp,..} if (stamp.device,stamp.inode)==(device,inode))).count()!=1 {return None}
    if local_path != &location.relative_path
        && (hash != old_hash || local.observe(&location.relative_path) != LocalObservation::Absent)
    {
        return None;
    }
    let local_name = local_path.file_name()?.to_str()?;
    let local_changed = local_name != location.remote_name;
    let remote_changed = item.name != location.remote_name;
    if (!local_changed && !remote_changed)
        || (local_changed && remote_changed && local_name != item.name)
        || (remote_changed && item.version == Some(base.remote_version))
    {
        return None;
    }
    let desired = if local_changed {
        local_name
    } else {
        &item.name
    };
    if !crate::planner::safe(desired) {
        return None;
    }
    let target = local_path.with_file_name(desired);
    if target != *local_path && local.observe(&target) != LocalObservation::Absent {
        return None;
    }
    let choice = if hash == old_hash {
        ResolutionChoice::KeepRemote
    } else {
        ResolutionChoice::KeepLocal
    };
    Some(Candidate {
        origin: local_path.clone(),
        target,
        fingerprint,
        choice,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AttributeChange as A, BaseEvidence, ContentComparison as C, DifferenceInput, SideDifference,
    };
    use std::collections::BTreeMap;
    fn setup(
        local_name: &str,
        remote_name: &str,
        edited: bool,
    ) -> (InventoryEntry, PlanEntry, ConfirmedBase, Inventory) {
        let old = "a".repeat(64);
        let hash = if edited { "b".repeat(64) } else { old.clone() };
        let local_fp = format!("{hash}:1:2");
        let base = ConfirmedBase {
            remote_version: 1,
            local_fingerprint: format!("{old}:1:2"),
            location: Some(crate::ConfirmedLocation {
                relative_path: "old.txt".into(),
                remote_name: "old.txt".into(),
                remote_parent: "root".into(),
            }),
        };
        let remote = InventoryEntry {
            id: "file".into(),
            parent_id: "root".into(),
            name: remote_name.into(),
            mime_type: "text/plain".into(),
            version: Some(2),
            size: Some(1),
            md5_checksum: Some("c".repeat(32)),
            web_view_link: None,
            kind: InventoryKind::Binary,
        };
        let local = Inventory {
            entries: BTreeMap::from([(
                local_name.into(),
                Evidence::Stable {
                    fingerprint: local_fp,
                    stamp: crate::local::Stamp {
                        device: 1,
                        inode: 2,
                        size: 1,
                        mtime: (1, 0),
                        ctime: (1, 0),
                    },
                },
            )]),
        };
        let attribute = |name: &str| {
            if name == "old.txt" {
                A::Unchanged
            } else {
                A::Changed(name.into())
            }
        };
        let decision = crate::classify_differences(&DifferenceInput {
            base: BaseEvidence::Confirmed,
            local: SideDifference::Present {
                content: if edited { C::Different } else { C::Equal },
                name: attribute(local_name),
                parent: A::Unchanged,
            },
            remote: SideDifference::Present {
                content: if edited { C::Equal } else { C::Different },
                name: attribute(remote_name),
                parent: A::Unchanged,
            },
            current_content: C::Different,
        });
        let entry = PlanEntry {
            remote_id: "file".into(),
            relative_path: Some(remote_name.into()),
            expected_remote_version: Some(2),
            expected_local: local.observe(std::path::Path::new(remote_name)),
            action: PlanAction::Classified {
                decision,
                local_path: local_name.into(),
            },
        };
        (remote, entry, base, local)
    }
    #[test]
    fn c09_both_directions_select_the_edited_content_and_the_independent_name() {
        for (local_name, remote_name, edited, choice) in [
            ("new.txt", "old.txt", false, ResolutionChoice::KeepRemote),
            ("old.txt", "new.txt", true, ResolutionChoice::KeepLocal),
        ] {
            let (remote, entry, base, local) = setup(local_name, remote_name, edited);
            let selected = candidate(&remote, &entry, &base, &local).unwrap();
            assert_eq!(selected.target, PathBuf::from("new.txt"));
            assert_eq!(selected.choice, choice);
        }
    }
    #[test]
    fn divergent_names_moves_aliases_and_destinations_are_never_auto_selected() {
        let (mut remote, entry, base, mut local) = setup("local.txt", "remote.txt", false);
        assert!(candidate(&remote, &entry, &base, &local).is_none());
        let (next, entry, base, next_local) = setup("old.txt", "new.txt", true);
        remote = next;
        local = next_local;
        remote.parent_id = "moved-parent".into();
        assert!(candidate(&remote, &entry, &base, &local).is_none());
        remote.parent_id = "root".into();
        local.entries.insert(
            "alias.txt".into(),
            local.entries[std::path::Path::new("old.txt")].clone(),
        );
        assert!(candidate(&remote, &entry, &base, &local).is_none());
        local.entries.remove(std::path::Path::new("alias.txt"));
        local
            .entries
            .insert("new.txt".into(), Evidence::Unsupported);
        assert!(candidate(&remote, &entry, &base, &local).is_none());
    }
}
