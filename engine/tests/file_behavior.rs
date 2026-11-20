//! Behavioral specification for the new analyzer, independent of its implementation.
//! Each case owns an actual File and an explicitly authored expected outcome.
//! No automatic merge, destructive fallback, or external reads.
//! Run this corpus against the future analyzer via assert_behavior.
use drivesync_engine::file_contract::*;
use std::time::UNIX_EPOCH;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Expected {
    NoOp, Adopt, Upload, Download, UpdateRemote, UpdateLocal,
    RenameRemote, RenameLocal, MoveRemote, MoveLocal,
    TrashRemote, RemoveLocal, CreateRemoteDirectory, CreateLocalDirectory,
    CreateLocalShortcut, Conflict, Defer, Unsupported,
    UpdateAndRenameRemote, UpdateAndMoveLocal,
}

pub struct Case {
    pub name: &'static str,
    pub file: File,
    pub expected: Expected,
    pub rationale: &'static str,
}

fn k<T>(v: T) -> Fact<T> { Fact::Known(v) }
fn hashes(v: u8) -> Checksums {
    Checksums { md5: k([v;16]), sha1: k([v;20]), sha256: k([v;32]) }
}
fn local() -> LocalFile {
    LocalFile {
        identity: LocalIdentity { device_id: 1, inode: 2, mount_id: k(1), birth_time: k(UNIX_EPOCH) },
        path: "/sync/a.txt".into(), name: "a.txt".into(), parent: "/sync".into(),
        kind: LocalKind::Regular, size: k(10), created_at: k(UNIX_EPOCH),
        modified_at: k(UNIX_EPOCH), changed_at: k(UNIX_EPOCH),
        content_type: k("text/plain".into()), etag: Fact::NotApplicable,
        checksums: hashes(1), symlink_target: Fact::NotApplicable,
        mode: k(0o644), uid: k(1000), gid: k(1000), hard_link_count: k(1),
        capabilities: LocalCapabilities { can_read:k(true), can_write:k(true),
            can_rename:k(true), can_delete:k(true), can_trash:k(true), can_create_children:k(true) },
    }
}
fn remote() -> RemoteFile {
    RemoteFile {
        id:"r1".into(), name:"a.txt".into(), parents:k(vec!["root".into()]),
        drive_id:Fact::NotApplicable, resource_key:Fact::NotApplicable,
        mime_type:"text/plain".into(), size:k(10), checksums:hashes(1),
        version:k(1), head_revision_id:k("rev1".into()), created_at:k(UNIX_EPOCH),
        modified_at:k(UNIX_EPOCH), trashed:k(false), explicitly_trashed:k(false),
        shortcut:Fact::NotApplicable, web_view_link:k("https://drive.google.com/open?id=r1".into()),
        export_links:Fact::NotApplicable, content_restricted:k(false),
        capabilities:RemoteCapabilities { can_download:k(true), can_edit:k(true),
            can_modify_content:k(true), can_rename:k(true), can_move:k(true), can_trash:k(true),
            can_untrash:k(true), can_delete:k(true), can_add_children:k(true) },
    }
}
fn base() -> File {
    let l=local(); let r=remote();
    File {
        identity:FileIdentity { account_id:"account-a".into(), item_id:Some("item1".into()),
            remote_id:k(r.id.clone()), local_identity:k(l.identity.clone()) },
        local:FileState::Present(l.clone()), remote:FileState::Present(r.clone()),
        baseline:Baseline::Synced(Box::new(SyncedState {
            local:FileState::Present(l), remote:FileState::Present(r),
            confirmed_at:UNIX_EPOCH, projection:k(Projection::Native),
        })),
        observations:Observations { events:vec![], local_coverage_complete:k(true), remote_coverage_complete:k(true) },
        environment:Environment { local_root:"/sync".into(), remote_root_id:"root".into(),
            local_root_available:k(true), remote_available:k(true),
            destination_local_parent_available:k(true), destination_remote_parent_id:k("root".into()),
            destination_local_occupied:k(false), destination_remote_matches:k(vec![]), prior_conflict:k(false) },
    }
}
fn l(f:&mut File)->&mut LocalFile { match &mut f.local {FileState::Present(x)=>x,_=>panic!("local fixture absent")} }
fn r(f:&mut File)->&mut RemoteFile { match &mut f.remote {FileState::Present(x)=>x,_=>panic!("remote fixture absent")} }
fn lc(f:&mut File,v:u8) { l(f).checksums=hashes(v); }
fn rc(f:&mut File,v:u8) { let x=r(f); x.checksums=hashes(v); x.version=k(2); x.head_revision_id=k("rev2".into()); }
fn ln(f:&mut File) { let x=l(f); x.name="b.txt".into(); x.path="/sync/b.txt".into(); }
fn rn(f:&mut File) { let x=r(f); x.name="b.txt".into(); x.version=k(2); }
fn lm(f:&mut File) { let x=l(f); x.parent="/sync/sub".into(); x.path="/sync/sub/a.txt".into(); f.environment.destination_remote_parent_id=k("sub".into()); }
fn rm(f:&mut File) { let x=r(f); x.parents=k(vec!["sub".into()]); x.version=k(2); }
fn new_item(f:&mut File) { f.baseline=Baseline::NeverSynced; f.identity.item_id=None; }
fn no_local(f:&mut File) { f.local=FileState::Absent; f.identity.local_identity=Fact::NotApplicable; }
fn no_remote(f:&mut File) { f.remote=FileState::Absent; f.identity.remote_id=Fact::NotApplicable; }
fn event(origin:Origin,kind:EventKind)->Observation {
    Observation { origin,kind,observed_at:UNIX_EPOCH,old_path:None,new_path:None,remote_id:None,move_cookie:None }
}

pub fn cases()->Vec<Case> {
    use Expected::*;
    let mut out=Vec::new();
    macro_rules! c { ($n:literal,$e:ident,$why:literal,$f:ident => $body:block) => {{
        let mut $f=base(); $body
        out.push(Case{name:$n,file:$f,expected:$e,rationale:$why});
    }} }
    c!("unchanged",NoOp,"Both match baseline",f=>{});
    c!("new-local",Upload,"New local item",f=>{new_item(&mut f);no_remote(&mut f);});
    c!("new-remote",Download,"New remote item",f=>{new_item(&mut f);no_local(&mut f);});
    c!("initial-equal",Adopt,"Equal content can be associated if identity is unambiguous",f=>{new_item(&mut f);});
    c!("initial-different",Conflict,"No common baseline for unequal content",f=>{new_item(&mut f);lc(&mut f,2);});
    c!("initial-both-absent",NoOp,"Nothing exists",f=>{new_item(&mut f);no_local(&mut f);no_remote(&mut f);});
    c!("local-content",UpdateRemote,"Only local bytes changed",f=>{lc(&mut f,2);});
    c!("remote-content",UpdateLocal,"Only remote bytes changed",f=>{rc(&mut f,2);});
    c!("both-content-different",Conflict,"Concurrent divergent content",f=>{lc(&mut f,2);rc(&mut f,3);});
    c!("both-content-equal",Adopt,"Already converged; confirm new baseline",f=>{lc(&mut f,2);rc(&mut f,2);});
    c!("local-rename",RenameRemote,"Local name changed",f=>{ln(&mut f);});
    c!("remote-rename",RenameLocal,"Remote name changed",f=>{rn(&mut f);});
    c!("both-rename-equal",Adopt,"Names already converge",f=>{ln(&mut f);rn(&mut f);});
    c!("both-rename-different",Conflict,"Divergent names",f=>{ln(&mut f);r(&mut f).name="c.txt".into();});
    c!("local-move",MoveRemote,"Local parent changed",f=>{lm(&mut f);});
    c!("remote-move",MoveLocal,"Remote parent changed",f=>{rm(&mut f);});
    c!("both-move-equal",Adopt,"Parents already converge",f=>{lm(&mut f);rm(&mut f);});
    c!("both-move-different",Conflict,"Divergent parents",f=>{lm(&mut f);r(&mut f).parents=k(vec!["other".into()]);});
    c!("local-edit-rename",UpdateAndRenameRemote,"One side changed content and name",f=>{lc(&mut f,2);ln(&mut f);});
    c!("remote-edit-move",UpdateAndMoveLocal,"One side changed content and parent",f=>{rc(&mut f,2);rm(&mut f);});
    c!("local-edit-remote-rename",Conflict,"No automatic cross-side merge",f=>{lc(&mut f,2);rn(&mut f);});
    c!("local-move-remote-edit",Conflict,"No automatic cross-side merge",f=>{lm(&mut f);rc(&mut f,2);});
    c!("local-delete",TrashRemote,"Confirmed local removal of unchanged remote",f=>{f.local=FileState::Absent;});
    c!("remote-delete",RemoveLocal,"Confirmed remote removal of unchanged local",f=>{f.remote=FileState::Absent;});
    c!("both-delete",Adopt,"Confirm both absent",f=>{f.local=FileState::Absent;f.remote=FileState::Absent;});
    c!("local-delete-remote-edit",Conflict,"Deletion versus edit",f=>{f.local=FileState::Absent;rc(&mut f,2);});
    c!("remote-delete-local-edit",Conflict,"Deletion versus edit",f=>{f.remote=FileState::Absent;lc(&mut f,2);});
    c!("remote-trash",RemoveLocal,"Remote trashed, local unchanged",f=>{r(&mut f).trashed=k(true);});
    c!("remote-trash-local-edit",Conflict,"Trash versus local edit",f=>{r(&mut f).trashed=k(true);lc(&mut f,2);});
    c!("remote-restore",Download,"Previously confirmed trashed item restored",f=>{
        if let Baseline::Synced(b)=&mut f.baseline {b.local=FileState::Absent;if let FileState::Present(x)=&mut b.remote{x.trashed=k(true);}}
        f.local=FileState::Absent;
    });
    c!("remote-offline",Defer,"Unknown remote must not imply deletion",f=>{f.remote=FileState::Unavailable(UnavailableReason::Offline);});
    c!("remote-access-lost",Defer,"404/access loss is ambiguous",f=>{f.remote=FileState::Unavailable(UnavailableReason::NotFoundOrAccessLost);});
    c!("local-unreadable",Defer,"Unknown local must not imply deletion",f=>{f.local=FileState::Unavailable(UnavailableReason::PermissionDenied);});
    c!("baseline-unavailable",Defer,"Missing history is not a new item",f=>{f.baseline=Baseline::Unavailable(UnavailableReason::IoFailure);});
    c!("root-unmounted",Defer,"Unavailable local root prevents deletion inference",f=>{f.local=FileState::Absent;f.environment.local_root_available=k(false);});
    c!("upload-denied",Defer,"Remote content cannot be modified",f=>{lc(&mut f,2);r(&mut f).capabilities.can_modify_content=k(false);});
    c!("download-denied",Defer,"Remote download prohibited",f=>{rc(&mut f,2);r(&mut f).capabilities.can_download=k(false);});
    c!("local-write-denied",Defer,"Local write prohibited",f=>{rc(&mut f,2);l(&mut f).capabilities.can_write=k(false);});
    c!("trash-denied",Defer,"Do not replace trash with permanent deletion",f=>{f.local=FileState::Absent;r(&mut f).capabilities.can_trash=k(false);});
    c!("remote-name-collision",Conflict,"Destination has unrelated remote IDs",f=>{ln(&mut f);f.environment.destination_remote_matches=k(vec!["other".into()]);});
    c!("local-name-collision",Conflict,"Destination occupied by unrelated file",f=>{rn(&mut f);f.environment.destination_local_occupied=k(true);});
    c!("local-parent-missing",Defer,"Destination parent must be resolved",f=>{rm(&mut f);f.environment.destination_local_parent_available=k(false);});
    c!("remote-parent-unknown",Defer,"Destination parent must be resolved",f=>{lm(&mut f);f.environment.destination_remote_parent_id=Fact::Unknown(UnavailableReason::NotObserved);});
    c!("identity-ambiguous",Defer,"Do not guess correlation",f=>{f.identity.local_identity=Fact::Unknown(UnavailableReason::AmbiguousIdentity);});
    c!("prior-conflict",Conflict,"Existing conflict stays unresolved",f=>{f.environment.prior_conflict=k(true);});
    c!("new-local-directory",CreateRemoteDirectory,"Directory is not binary upload",f=>{new_item(&mut f);no_remote(&mut f);l(&mut f).kind=LocalKind::Directory;});
    c!("new-remote-directory",CreateLocalDirectory,"Directory is not binary download",f=>{new_item(&mut f);no_local(&mut f);r(&mut f).mime_type="application/vnd.google-apps.folder".into();});
    c!("native-document",CreateLocalShortcut,"Native document uses web projection",f=>{new_item(&mut f);no_local(&mut f);let x=r(&mut f);x.mime_type="application/vnd.google-apps.document".into();x.checksums=Checksums{md5:Fact::NotApplicable,sha1:Fact::NotApplicable,sha256:Fact::NotApplicable};x.size=Fact::NotApplicable;x.head_revision_id=Fact::NotApplicable;});
    c!("local-symlink",Unsupported,"Symlink policy not implemented; do not follow target",f=>{new_item(&mut f);no_remote(&mut f);l(&mut f).kind=LocalKind::Symlink;l(&mut f).symlink_target=k("/outside".into());});
    c!("remote-outside-scope",Defer,"Scope exit policy requires explicit definition",f=>{let x=remote();f.remote=FileState::OutsideScope(x);});
    c!("local-outside-scope",Defer,"Scope exit must not silently destroy remote",f=>{let mut x=local();x.path="/outside/a.txt".into();x.parent="/outside".into();f.local=FileState::OutsideScope(x);});
    c!("checksum-unknown",Defer,"No reliable content comparison",f=>{l(&mut f).checksums=Checksums{md5:Fact::Unknown(UnavailableReason::IoFailure),sha1:Fact::NotApplicable,sha256:Fact::NotApplicable};});
    c!("metadata-only-version",NoOp,"Version increment is not content modification",f=>{r(&mut f).version=k(2);});
    c!("duplicate-events",UpdateRemote,"Duplicate events do not duplicate decision",f=>{lc(&mut f,2);let e=event(Origin::Local,EventKind::Modified);f.observations.events=vec![e.clone(),e];});
    c!("event-with-no-change",NoOp,"Events are evidence, not commands",f=>{f.observations.events=vec![event(Origin::Remote,EventKind::Modified)];});
    c!("overflow-with-unknown-state",Defer,"Lost events plus unknown state cannot justify deletion",f=>{f.local=FileState::Unavailable(UnavailableReason::NotObserved);f.observations.local_coverage_complete=k(false);f.observations.events=vec![event(Origin::Local,EventKind::Overflow)];});
    c!("other-account",UpdateRemote,"Same item ID in a different account remains isolated",f=>{f.identity.account_id="account-b".into();lc(&mut f,2);});
    out
}

/// Future analyzer adapter: compare each actual outcome with the literal oracle.
/// Not called until the new analyzer exists.
pub fn assert_behavior(mut analyze:impl FnMut(File)->Expected) {
    for case in cases() {
        assert_eq!(analyze(case.file),case.expected,"{}: {}",case.name,case.rationale);
    }
}

#[test]
fn corpus_has_unique_names_and_documented_expectations() {
    let cases=cases();
    let mut names=std::collections::HashSet::new();
    assert!(cases.len()>=50);
    for case in cases {assert!(names.insert(case.name));assert!(!case.rationale.is_empty());}
}
