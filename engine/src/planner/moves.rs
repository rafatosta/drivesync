//! Pure reconciliation of concurrent moves (C10, C12, C17, C19).
//!
//! Moves are judged by item and parent IDs, never by paths or names. The result
//! is a candidate only: it grants no filesystem or network permission and must be
//! revalidated by the executor. Ambiguous or unsafe cases are never resolved by
//! choosing a winner.
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Kind of a tree node; only folders can be destinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeKind {
    File,
    Folder,
}

/// One observed item in an ID tree.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TreeNode {
    pub parent_id: String,
    pub kind: NodeKind,
    /// False for trashed, inaccessible or otherwise unusable items.
    pub available: bool,
}

/// An ID tree for one side (local mapped to IDs, or remote) below one root.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct IdTree {
    pub root_id: String,
    pub nodes: BTreeMap<String, TreeNode>,
}

/// Whether the destination parent is the same object the move was aimed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DestinationIdentity {
    Continuous,
    /// The destination path/ID now refers to a different object.
    Replaced,
    Unknown,
}

/// Move evidence for one side relative to the confirmed base parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveEvidence {
    Unchanged,
    MovedTo {
        parent_id: String,
        identity: DestinationIdentity,
    },
    Unknown,
}

/// Content change of one side relative to the base; `Unknown` suspends combining.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentChange {
    Unchanged,
    Edited,
    Unknown,
}

/// Normalized inputs for one item whose identity is continuous on both sides.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveInput {
    pub item_id: String,
    pub local: MoveEvidence,
    pub remote: MoveEvidence,
    pub local_content: ContentChange,
    pub remote_content: ContentChange,
    /// Both sides edited and the current bytes differ.
    pub content_conflict: bool,
    pub local_tree: IdTree,
    pub remote_tree: IdTree,
}

/// Direction of a candidate move propagation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveDirection {
    LocalToRemote,
    RemoteToLocal,
}

/// Why a move cannot be applied automatically.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveBlock {
    /// C10/C17: both sides moved the item to different parents.
    DivergentDestinations,
    /// C19: the destination does not exist in the tree.
    DestinationMissing,
    /// C19: the destination is unavailable (trashed or inaccessible).
    DestinationUnavailable,
    /// C19: the destination is not a folder.
    DestinationNotFolder,
    /// C19: the destination identity changed or cannot be proven.
    DestinationIdentity,
    /// The move would make a folder its own ancestor.
    Cycle,
    /// The destination chain does not reach the root.
    Orphan,
    /// Both sides edited the content differently; the move is not combined.
    ContentConflict,
    /// Evidence is missing; nothing may be applied.
    IncompleteEvidence,
}

/// Candidate handling for a move; never an executable permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveDecision {
    NoMove,
    /// Both sides already agree on the destination (convergent move).
    Equivalent,
    /// One side moved; `combined_edit` is true when the other side edited (C12).
    Apply {
        direction: MoveDirection,
        parent_id: String,
        combined_edit: bool,
    },
    Preserve(MoveBlock),
}

/// Checks a destination parent against one tree with the item re-parented.
fn validate(tree: &IdTree, item: &str, parent: &str) -> Result<(), MoveBlock> {
    if parent == item {
        return Err(MoveBlock::Cycle);
    }
    if parent != tree.root_id {
        let node = tree
            .nodes
            .get(parent)
            .ok_or(MoveBlock::DestinationMissing)?;
        if !node.available {
            return Err(MoveBlock::DestinationUnavailable);
        }
        if node.kind != NodeKind::Folder {
            return Err(MoveBlock::DestinationNotFolder);
        }
    }
    let mut current = parent;
    for _ in 0..=tree.nodes.len() {
        if current == tree.root_id {
            return Ok(());
        }
        if current == item {
            return Err(MoveBlock::Cycle);
        }
        match tree.nodes.get(current) {
            Some(node) => current = &node.parent_id,
            None => return Err(MoveBlock::Orphan),
        }
    }
    // A loop that never reaches the root is a cycle among other nodes.
    Err(MoveBlock::Cycle)
}

fn apply(input: &MoveInput, from: &MoveEvidence, direction: MoveDirection) -> MoveDecision {
    let MoveEvidence::MovedTo {
        parent_id,
        identity,
    } = from
    else {
        return MoveDecision::Preserve(MoveBlock::IncompleteEvidence);
    };
    if *identity != DestinationIdentity::Continuous {
        return MoveDecision::Preserve(MoveBlock::DestinationIdentity);
    }
    if input.content_conflict {
        return MoveDecision::Preserve(MoveBlock::ContentConflict);
    }
    let other_content = match direction {
        MoveDirection::LocalToRemote => input.remote_content,
        MoveDirection::RemoteToLocal => input.local_content,
    };
    if other_content == ContentChange::Unknown {
        return MoveDecision::Preserve(MoveBlock::IncompleteEvidence);
    }
    // Validate in both trees so concurrent moves of ancestors cannot form a cycle.
    for tree in [&input.local_tree, &input.remote_tree] {
        if let Err(block) = validate(tree, &input.item_id, parent_id) {
            return MoveDecision::Preserve(block);
        }
    }
    MoveDecision::Apply {
        direction,
        parent_id: parent_id.clone(),
        combined_edit: other_content == ContentChange::Edited,
    }
}

/// Decides one item's concurrent move without I/O; repeated inputs are idempotent.
///
/// Different destinations (C10/C17) are always preserved. A single move, with or
/// without an edit on the other side (C12), is a candidate only if the
/// destination is a continuous, available folder and the resulting tree is
/// acyclic and rooted in both observed trees (C19).
pub fn decide_move(input: &MoveInput) -> MoveDecision {
    use MoveEvidence::{MovedTo, Unchanged, Unknown};
    match (&input.local, &input.remote) {
        (Unknown, _) | (_, Unknown) => MoveDecision::Preserve(MoveBlock::IncompleteEvidence),
        (Unchanged, Unchanged) => MoveDecision::NoMove,
        (MovedTo { .. }, Unchanged) => apply(input, &input.local, MoveDirection::LocalToRemote),
        (Unchanged, MovedTo { .. }) => apply(input, &input.remote, MoveDirection::RemoteToLocal),
        (MovedTo { parent_id: l, .. }, MovedTo { parent_id: r, .. }) if l != r => {
            MoveDecision::Preserve(MoveBlock::DivergentDestinations)
        }
        (MovedTo { identity: li, .. }, MovedTo { identity: ri, .. }) => {
            if *li != DestinationIdentity::Continuous || *ri != DestinationIdentity::Continuous {
                MoveDecision::Preserve(MoveBlock::DestinationIdentity)
            } else {
                MoveDecision::Equivalent
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder(parent: &str) -> TreeNode {
        TreeNode {
            parent_id: parent.into(),
            kind: NodeKind::Folder,
            available: true,
        }
    }
    fn tree(extra: &[(&str, &str)]) -> IdTree {
        let mut nodes = BTreeMap::new();
        for (id, parent) in [
            ("projects", "root"),
            ("archive", "root"),
            ("item", "archive"),
        ]
        .iter()
        .chain(extra.iter())
        {
            nodes.insert((*id).to_string(), folder(parent));
        }
        IdTree {
            root_id: "root".into(),
            nodes,
        }
    }
    fn moved(parent: &str) -> MoveEvidence {
        MoveEvidence::MovedTo {
            parent_id: parent.into(),
            identity: DestinationIdentity::Continuous,
        }
    }
    fn input(local: MoveEvidence, remote: MoveEvidence) -> MoveInput {
        MoveInput {
            item_id: "item".into(),
            local,
            remote,
            local_content: ContentChange::Unchanged,
            remote_content: ContentChange::Unchanged,
            content_conflict: false,
            local_tree: tree(&[]),
            remote_tree: tree(&[]),
        }
    }

    #[test]
    fn c10_c17_divergent_destinations_are_preserved_and_idempotent() {
        let i = input(moved("projects"), moved("archive"));
        let expected = MoveDecision::Preserve(MoveBlock::DivergentDestinations);
        assert_eq!(decide_move(&i), expected);
        assert_eq!(decide_move(&i), expected);
    }

    #[test]
    fn convergent_and_single_moves_are_candidates_without_a_winner_choice() {
        assert_eq!(
            decide_move(&input(moved("projects"), moved("projects"))),
            MoveDecision::Equivalent
        );
        assert_eq!(
            decide_move(&input(moved("projects"), MoveEvidence::Unchanged)),
            MoveDecision::Apply {
                direction: MoveDirection::LocalToRemote,
                parent_id: "projects".into(),
                combined_edit: false
            }
        );
        assert_eq!(
            decide_move(&input(MoveEvidence::Unchanged, MoveEvidence::Unchanged)),
            MoveDecision::NoMove
        );
    }

    #[test]
    fn c12_move_plus_edit_combines_only_without_a_content_conflict() {
        let mut i = input(moved("projects"), MoveEvidence::Unchanged);
        i.remote_content = ContentChange::Edited;
        assert_eq!(
            decide_move(&i),
            MoveDecision::Apply {
                direction: MoveDirection::LocalToRemote,
                parent_id: "projects".into(),
                combined_edit: true
            }
        );
        let mut i = input(MoveEvidence::Unchanged, moved("projects"));
        i.local_content = ContentChange::Edited;
        assert!(matches!(
            decide_move(&i),
            MoveDecision::Apply {
                direction: MoveDirection::RemoteToLocal,
                combined_edit: true,
                ..
            }
        ));
        i.content_conflict = true;
        assert_eq!(
            decide_move(&i),
            MoveDecision::Preserve(MoveBlock::ContentConflict)
        );
        i.content_conflict = false;
        i.local_content = ContentChange::Unknown;
        assert_eq!(
            decide_move(&i),
            MoveDecision::Preserve(MoveBlock::IncompleteEvidence)
        );
    }

    #[test]
    fn c19_missing_unavailable_replaced_or_non_folder_destinations_block() {
        let i = input(moved("gone"), MoveEvidence::Unchanged);
        assert_eq!(
            decide_move(&i),
            MoveDecision::Preserve(MoveBlock::DestinationMissing)
        );
        let mut i = input(moved("projects"), MoveEvidence::Unchanged);
        i.remote_tree.nodes.get_mut("projects").unwrap().available = false;
        assert_eq!(
            decide_move(&i),
            MoveDecision::Preserve(MoveBlock::DestinationUnavailable)
        );
        let mut i = input(moved("projects"), MoveEvidence::Unchanged);
        i.local_tree.nodes.get_mut("projects").unwrap().kind = NodeKind::File;
        assert_eq!(
            decide_move(&i),
            MoveDecision::Preserve(MoveBlock::DestinationNotFolder)
        );
        for identity in [DestinationIdentity::Replaced, DestinationIdentity::Unknown] {
            let i = input(
                MoveEvidence::MovedTo {
                    parent_id: "projects".into(),
                    identity,
                },
                MoveEvidence::Unchanged,
            );
            assert_eq!(
                decide_move(&i),
                MoveDecision::Preserve(MoveBlock::DestinationIdentity)
            );
        }
    }

    #[test]
    fn cycles_and_orphans_are_blocked_across_both_trees() {
        let i = input(moved("item"), MoveEvidence::Unchanged);
        assert_eq!(decide_move(&i), MoveDecision::Preserve(MoveBlock::Cycle));
        // Local moves "item" under its remote descendant "child".
        let mut i = input(moved("child"), MoveEvidence::Unchanged);
        i.remote_tree = tree(&[("child", "item")]);
        i.local_tree = tree(&[("child", "projects")]);
        assert_eq!(decide_move(&i), MoveDecision::Preserve(MoveBlock::Cycle));
        let mut i = input(moved("projects"), MoveEvidence::Unchanged);
        i.remote_tree.nodes.get_mut("projects").unwrap().parent_id = "missing".into();
        assert_eq!(decide_move(&i), MoveDecision::Preserve(MoveBlock::Orphan));
        let mut i = input(moved("a"), MoveEvidence::Unchanged);
        i.remote_tree = tree(&[("a", "b"), ("b", "a")]);
        i.local_tree = tree(&[("a", "root"), ("b", "a")]);
        assert_eq!(decide_move(&i), MoveDecision::Preserve(MoveBlock::Cycle));
    }

    #[test]
    fn unknown_evidence_suspends_the_decision() {
        for (l, r) in [
            (MoveEvidence::Unknown, MoveEvidence::Unchanged),
            (moved("projects"), MoveEvidence::Unknown),
        ] {
            assert_eq!(
                decide_move(&input(l, r)),
                MoveDecision::Preserve(MoveBlock::IncompleteEvidence)
            );
        }
    }
}
