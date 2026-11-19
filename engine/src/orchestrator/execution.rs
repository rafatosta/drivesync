//! Publishes analyzer lifecycle feedback and bounded per-file progress.

use crate::{
    analyzer::{
        AnalysisError, AnalysisFeedback, AnalysisResult, AnalysisStage, AnalysisStatus,
        AnalysisUpdate,
    },
    synchronization::{
        SynchronizationAction, SynchronizationActivity, SynchronizationCounters,
        SynchronizationEvent, SynchronizationProgress, SynchronizationProgressUnit,
        SynchronizationState, SynchronizationWork, SynchronizationWorkDetail,
    },
};
use std::sync::{Arc, Mutex};

/// Cached per-file analyzer feedback and bounded pass counters.
#[derive(Clone, Debug, Default)]
pub struct ExecutionStatus {
    /// Whether analyzer-selected physical operations are enabled by configuration.
    pub enabled: bool,
    /// Files that reached a completed analyzer outcome during the current process.
    pub completed: u64,
    /// Files intentionally skipped by the analyzer during the current process.
    pub skipped: u64,
    /// Files whose analysis returned an error during the current process.
    pub failed: u64,
    /// Detector identity associated with `last_result` or `last_error`.
    pub last_file_id: Option<crate::models::FileId>,
    /// File currently being analyzed, including its latest lifecycle status.
    pub current: Option<AnalysisUpdate>,
    /// Most recent successful terminal result.
    pub last_result: Option<AnalysisResult>,
    /// Category of the most recent analyzer error.
    pub last_error: Option<crate::analyzer::AnalysisErrorKind>,
    /// Current bounded batch's file progress.
    pub progress: Option<SynchronizationProgress>,
}

pub(super) struct ExecutionControl(
    Mutex<ExecutionStatus>,
    Mutex<Option<Arc<crate::synchronization::Publisher>>>,
    Mutex<Option<SynchronizationWork>>,
);

impl ExecutionControl {
    pub fn new(enabled: bool) -> Arc<Self> {
        Arc::new(Self(
            Mutex::new(ExecutionStatus {
                enabled,
                ..Default::default()
            }),
            Mutex::new(None),
            Mutex::new(None),
        ))
    }

    pub fn set_publisher(&self, publisher: Arc<crate::synchronization::Publisher>) {
        *self.1.lock().unwrap_or_else(|e| e.into_inner()) = Some(publisher);
    }

    pub fn snapshot(&self) -> ExecutionStatus {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn begin_batch(&self, total: u64) {
        let progress = SynchronizationProgress {
            completed: 0,
            total: Some(total),
            unit: SynchronizationProgressUnit::Items,
            phase: None,
        };
        let mut snapshot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        snapshot.progress = Some(progress);
        drop(snapshot);
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            publisher.update(|status| {
                status.counters = SynchronizationCounters {
                    analysis_items_processed: 0,
                    analysis_items_total: Some(total),
                    ..status.counters
                };
                status.progress = Some(progress);
            });
        }
    }

    pub fn advance_batch(&self, completed: u64, total: u64) {
        let progress = SynchronizationProgress {
            completed,
            total: Some(total),
            unit: SynchronizationProgressUnit::Items,
            phase: None,
        };
        self.0.lock().unwrap_or_else(|e| e.into_inner()).progress = Some(progress);
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            publisher.update(|status| {
                status.counters.analysis_items_processed = completed;
                status.counters.analysis_items_total = Some(total);
                status.progress = Some(progress);
            });
        }
    }

    pub fn finish_file(
        &self,
        file_id: crate::models::FileId,
        result: Result<AnalysisResult, AnalysisError>,
    ) {
        let mut snapshot = self.0.lock().unwrap_or_else(|e| e.into_inner());
        snapshot.current = None;
        snapshot.last_file_id = Some(file_id);
        match result {
            Ok(result @ AnalysisResult::Completed(_)) => {
                snapshot.completed = snapshot.completed.saturating_add(1);
                snapshot.last_result = Some(result);
                snapshot.last_error = None;
            }
            Ok(result @ AnalysisResult::Skipped(_)) => {
                snapshot.skipped = snapshot.skipped.saturating_add(1);
                snapshot.last_result = Some(result);
                snapshot.last_error = None;
            }
            Err(error) => {
                snapshot.failed = snapshot.failed.saturating_add(1);
                snapshot.last_result = None;
                snapshot.last_error = Some(error.kind);
            }
        }
    }
}

impl AnalysisFeedback for ExecutionControl {
    fn report(&self, update: AnalysisUpdate) {
        let file = &update.file;
        let item = super::flow::observation_item(file);
        let event = SynchronizationEvent {
            action: file.event.action,
            previous: file.event.previous.clone(),
            current: file.event.current.clone(),
            observed_at: file.observed_at,
        };
        let location = file
            .event
            .current
            .as_ref()
            .or(file.event.previous.as_ref())
            .map(super::flow::observation_location)
            .or_else(|| match &file.scope {
                crate::models::FileScope::Local(path) => Some(
                    crate::synchronization::SynchronizationLocation::Local(path.clone()),
                ),
                crate::models::FileScope::Remote(reference) => {
                    Some(crate::synchronization::SynchronizationLocation::Remote {
                        reference: reference.clone(),
                        parent_id: None,
                        name: None,
                    })
                }
                crate::models::FileScope::WholeSync => None,
            });
        let stage = match &update.status {
            AnalysisStatus::Started => AnalysisStage::Inspecting,
            AnalysisStatus::InProgress(stage) => *stage,
        };
        let work = SynchronizationWork {
            action: SynchronizationAction::Analyze,
            item,
            location,
            detail: SynchronizationWorkDetail::Analysis(stage),
            event: Some(event),
        };
        *self.2.lock().unwrap_or_else(|e| e.into_inner()) = Some(work.clone());
        self.0.lock().unwrap_or_else(|e| e.into_inner()).current = Some(update);
        if let Some(publisher) = self.1.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
            publisher.update(|status| {
                status.state = SynchronizationState::Running;
                status.activity = SynchronizationActivity::Analyzing;
                status.work = Some(work);
                status.progress = None;
            });
        }
    }
}
