use super::*;
use crate::detectors::observation::scanner::tests::Sandbox;
fn kind(observation: &Observation) -> ObservationKind {
    match observation {
        Observation::Local { data, .. } => data.event.as_ref().unwrap().kind,
        _ => unreachable!(),
    }
}
#[test]
fn initial_snapshot_and_continuous_edits_removals_and_renames_are_facts_only() {
    let s = Sandbox::new();
    std::fs::write(s.0.join("file"), b"original").unwrap();
    let mut observer =
        LocalObserver::new(Scanner::open(s.0.clone(), SyncRootId("scope".into())).unwrap());
    let token = CancellationToken::new();
    let (first, _) = observer.poll(&token).unwrap();
    assert_eq!(kind(&first[0]), ObservationKind::Snapshot);
    assert!(observer.poll(&token).unwrap().0.is_empty());
    std::fs::write(s.0.join("file"), b"changed").unwrap();
    assert_eq!(
        kind(&observer.poll(&token).unwrap().0[0]),
        ObservationKind::Modified
    );
    std::fs::rename(s.0.join("file"), s.0.join("renamed")).unwrap();
    let (changes, coverage) = observer.poll(&token).unwrap();
    assert_eq!(coverage, ObservationCoverage::Complete);
    assert_eq!(changes.len(), 2);
    assert!(changes.iter().any(|o| kind(o) == ObservationKind::Removed));
    assert!(changes.iter().any(|o| kind(o) == ObservationKind::Created));
}
#[test]
fn failed_round_keeps_previous_snapshot_and_recovery_republishes_evidence() {
    let s = Sandbox::new();
    std::fs::write(s.0.join("file"), b"original").unwrap();
    let mut observer =
        LocalObserver::new(Scanner::open(s.0.clone(), SyncRootId("scope".into())).unwrap());
    let token = CancellationToken::new();
    observer.poll(&token).unwrap();
    let cancelled = CancellationToken::new();
    cancelled.cancel();
    assert!(observer.poll(&cancelled).is_err());
    assert_eq!(observer.unavailable().len(), 1);
    let (again, _) = observer.poll(&token).unwrap();
    assert_eq!(kind(&again[0]), ObservationKind::RecheckRequired);
}
#[test]
fn absence_is_emitted_only_after_a_complete_recovery_round() {
    let s = Sandbox::new();
    std::fs::write(s.0.join("file"), b"original").unwrap();
    let mut observer =
        LocalObserver::new(Scanner::open(s.0.clone(), SyncRootId("scope".into())).unwrap());
    let token = CancellationToken::new();
    observer.poll(&token).unwrap();
    let failed = CancellationToken::new();
    failed.cancel();
    assert!(observer.poll(&failed).is_err());
    std::fs::remove_file(s.0.join("file")).unwrap();
    assert!(observer.unavailable().iter().all(|observation|matches!(observation,Observation::Local {data,..} if data.state==FileState::Unavailable)));
    let (recovered, coverage) = observer.poll(&token).unwrap();
    assert_eq!(coverage, ObservationCoverage::Complete);
    assert_eq!(recovered.len(), 1);
    assert_eq!(kind(&recovered[0]), ObservationKind::Removed);
}
