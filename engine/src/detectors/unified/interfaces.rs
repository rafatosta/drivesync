//! Private source controls let the facade be tested without an Orchestrator or HTTP.
use super::*;
pub(super) trait Source: Send + Sync {
    fn start(&self, mode: DetectorStartMode) -> DetectorResult<()>;
    fn stop(&self, mode: DetectorStopMode) -> DetectorResult<()>;
    fn status(&self) -> DetectorSourceStatus;
    fn request_scan(&self, request: &DetectorScanRequest) -> DetectorResult<()>;
}
