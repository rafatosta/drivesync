//! Source adapters retain typed failures while sharing one durable queue.
use super::*;
use crate::models::*;
pub(super) struct Local(pub LocalDetector);
pub(super) struct Remote(pub Arc<RemoteDetector>);
fn local_error(error: LocalDetectorFailure) -> DetectorFailure {
    DetectorFailure {
        kind: DetectorError::SourceUnavailable,
        retryable: error.retryable,
        source: Some(DetectorSourceError::Local(error.kind)),
        queue: None,
    }
}
fn remote_error(error: RemoteFailure) -> DetectorFailure {
    DetectorFailure {
        kind: DetectorError::SourceUnavailable,
        retryable: error.retryable,
        source: Some(DetectorSourceError::Remote(error.kind)),
        queue: None,
    }
}
impl Source for Local {
    fn start(&self, mode: DetectorStartMode) -> DetectorResult<()> {
        self.0
            .start(LocalStartRequest {
                mode: if mode == DetectorStartMode::Initial {
                    LocalStartMode::Initial
                } else {
                    LocalStartMode::Resume
                },
                requested_at: observed_at(),
            })
            .map(|_| ())
            .map_err(local_error)
    }
    fn stop(&self, _: DetectorStopMode) -> DetectorResult<()> {
        self.0
            .stop(LocalStopRequest {
                reason: LocalStopReason::UserRequest,
                requested_at: observed_at(),
            })
            .map(|_| ())
            .map_err(local_error)
    }
    fn status(&self) -> DetectorSourceStatus {
        match self.0.status(LocalStatusRequest {}) {
            Ok(status) => DetectorSourceStatus {
                state: match status.state {
                    LocalDetectorState::Stopped => DetectorState::Stopped,
                    LocalDetectorState::Starting => DetectorState::Starting,
                    LocalDetectorState::Running => DetectorState::Running,
                    LocalDetectorState::Degraded => DetectorState::Degraded,
                    LocalDetectorState::Stopping => DetectorState::Stopping,
                    LocalDetectorState::Failed => DetectorState::Failed,
                },
                last_error: status.last_error.map(DetectorSourceError::Local),
                local: Some(status),
                remote: None,
            },
            Err(error) => DetectorSourceStatus {
                state: DetectorState::Failed,
                last_error: Some(DetectorSourceError::Local(error.kind)),
                local: None,
                remote: None,
            },
        }
    }
    fn request_scan(&self, request: &DetectorScanRequest) -> DetectorResult<()> {
        let paths = match &request.scope {
            DetectorScanScope::All => vec![None],
            DetectorScanScope::Scopes(scopes) => scopes
                .iter()
                .filter_map(|scope| match scope {
                    FileScope::Local(path) => Some(Some(path.clone())),
                    FileScope::WholeSync => Some(None),
                    _ => None,
                })
                .collect(),
        };
        if paths.is_empty() {
            return Ok(());
        }
        self.0.request_observation(paths).map_err(local_error)
    }
}
impl Source for Remote {
    fn start(&self, mode: DetectorStartMode) -> DetectorResult<()> {
        self.0
            .start(RemoteStartRequest {
                mode: if mode == DetectorStartMode::Initial {
                    RemotePollMode::Initial
                } else {
                    RemotePollMode::Incremental
                },
            })
            .map(|_| ())
            .map_err(remote_error)
    }
    fn stop(&self, mode: DetectorStopMode) -> DetectorResult<()> {
        self.0
            .stop_mode(mode == DetectorStopMode::DrainInFlight)
            .map(|_| ())
            .map_err(remote_error)
    }
    fn status(&self) -> DetectorSourceStatus {
        match self.0.status(RemoteStatusRequest {}) {
            Ok(status) => DetectorSourceStatus {
                state: match status.state {
                    RemoteState::Stopped => DetectorState::Stopped,
                    RemoteState::Starting => DetectorState::Starting,
                    RemoteState::Running => DetectorState::Running,
                    RemoteState::Degraded => DetectorState::Degraded,
                    RemoteState::Stopping => DetectorState::Stopping,
                    RemoteState::Failed => DetectorState::Failed,
                },
                last_error: status.last_error.map(DetectorSourceError::Remote),
                local: None,
                remote: Some(status),
            },
            Err(error) => DetectorSourceStatus {
                state: DetectorState::Failed,
                last_error: Some(DetectorSourceError::Remote(error.kind)),
                local: None,
                remote: None,
            },
        }
    }
    fn request_scan(&self, request: &DetectorScanRequest) -> DetectorResult<()> {
        if matches!(&request.scope, DetectorScanScope::Scopes(scopes) if !scopes.iter().any(|scope| matches!(scope, FileScope::Remote(_) | FileScope::WholeSync)))
        {
            return Ok(());
        }
        self.0
            .request_observation(RemotePollMode::Recovery)
            .map_err(remote_error)
    }
}
