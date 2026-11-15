//! Compile-time checks of public consumer ports; deliberately no executor implementation.
use drivesync_engine::operations::*;
use std::sync::Arc;

#[test]
fn analyzer_can_hold_object_safe_send_sync_ports() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<Arc<dyn Operator>>();
    send_sync::<Arc<dyn OperatorFactory>>();
    send_sync::<Arc<dyn CancellationSignal>>();
    send_sync::<Arc<dyn ProgressReporter>>();
    send_sync::<OperatorConfig>();
}

#[test]
fn all_ten_owned_requests_return_send_futures_through_the_public_interface() {
    fn send_future<T>(_: OperationFuture<'_, T>) {}
    // Type-check the complete consumer boundary without constructing an executor
    // or calling this closure. No mock/default implementation can hide missing methods.
    let _consumer = |operator: &dyn Operator,
                     requests: (
        CreateFileRequest,
        CreateDirectoryRequest,
        UploadRequest,
        DownloadRequest,
        UpdateContentRequest,
        RenameRequest,
        MoveRequest,
        DeleteRequest,
        TrashRequest,
        RestoreRequest,
    )| {
        send_future(operator.create_file(requests.0));
        send_future(operator.create_directory(requests.1));
        send_future(operator.upload(requests.2));
        send_future(operator.download(requests.3));
        send_future(operator.update_content(requests.4));
        send_future(operator.rename(requests.5));
        send_future(operator.move_item(requests.6));
        send_future(operator.delete(requests.7));
        send_future(operator.trash(requests.8));
        send_future(operator.restore(requests.9));
    };
}
