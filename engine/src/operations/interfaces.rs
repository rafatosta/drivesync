//! Object-safe execution and composition boundaries, without default implementations.
use super::*;
use std::{future::Future, pin::Pin, sync::Arc};

/// One bounded asynchronous call. The future may borrow its scoped executor;
/// requests are owned and Send, so consumers can use `Arc<dyn Operator>` on Tokio.
pub type OperationFuture<'a, T> = Pin<Box<dyn Future<Output = OperatorResult<T>> + Send + 'a>>;

/// Public contract implemented by account-scoped provider specialists.
///
/// Implementations must validate scope, identities, destination and preconditions,
/// access remote services only through Providers, and preserve uncertain effects.
/// Every call binds its required ExecutionContext to the exact intent in the
/// Inventory journal before effects. Checkpoints do not authorize execution.
/// No method schedules other items, resolves conflicts or confirms a baseline.
/// The trait supplies no default operation bodies. The Google specialist
/// implements all allowed operations and rejects unavailable platform capabilities.
pub trait Operator: Send + Sync {
    /// Rechecks a creation's source, capabilities and free destination without
    /// creating a user resource. The executor repeats these checks before effects.
    fn verify_creation(
        &self,
        request: VerifyCreationRequest,
    ) -> OperationFuture<'_, VerifyCreationResult>;
    /// Returns live identity-bound source evidence, not an inferred baseline.
    fn verify_source(&self, request: VerifySourceRequest) -> OperationFuture<'_, VerifiedSource>;
    /// Constructs one specialist with an account-scoped Engine handle.
    /// The factory must resolve the persisted provider before selecting a type;
    /// construction must not execute operations or accept caller-supplied credentials.
    fn new(config: OperatorConfig) -> OperatorResult<Self>
    where
        Self: Sized;

    /// Creates a file at a verified free destination; never replaces a homonym.
    fn create_file(&self, request: CreateFileRequest) -> OperationFuture<'_, ItemResult>;
    /// Creates one directory; matching names never authorize merging folders.
    fn create_directory(&self, request: CreateDirectoryRequest) -> OperationFuture<'_, ItemResult>;
    /// Creates a remote replica of a local file, returning the created identity.
    fn upload(&self, request: UploadRequest) -> OperationFuture<'_, TransferResult>;
    /// Creates a local replica of a remote file, returning the created identity.
    fn download(&self, request: DownloadRequest) -> OperationFuture<'_, TransferResult>;
    /// Replaces content only at the explicitly identified existing target.
    fn update_content(&self, request: UpdateContentRequest) -> OperationFuture<'_, TransferResult>;
    /// Renames an identified resource without choosing or associating a homonym.
    fn rename(&self, request: RenameRequest) -> OperationFuture<'_, ItemResult>;
    /// Moves within one environment and compatible account/provider context.
    fn move_item(&self, request: MoveRequest) -> OperationFuture<'_, ItemResult>;
    /// Prohibited: implementations must warn and return PermanentDeletionProhibited
    /// with effect NotApplied, without I/O, authentication, mutation or false success.
    /// Never silently redirects to trash; the caller must request trash explicitly.
    fn delete(&self, request: DeleteRequest) -> OperationFuture<'_, DeleteResult>;
    /// Sends the identified resource to trash. Unsupported trash must fail safely;
    /// permanent deletion is never a fallback.
    fn trash(&self, request: TrashRequest) -> OperationFuture<'_, ItemResult>;
    /// Restores an identified recoverable resource, subject to destination checks.
    fn restore(&self, request: RestoreRequest) -> OperationFuture<'_, ItemResult>;
}

/// Composition boundary mirroring remote detector provider selection.
///
/// The implementation resolves config.scope.account_id via config.connector,
/// checks scope/account/provider compatibility and selects the specialist. The
/// provider is never inferred from names or supplied as an arbitrary selector.
/// `DefaultOperatorFactory` provides this composition for the Google specialist.
pub trait OperatorFactory: Send + Sync {
    /// Builds an account-scoped handle; unsupported providers must fail explicitly.
    fn create(&self, config: OperatorConfig) -> OperatorResult<Arc<dyn Operator>>;
}

/// Cooperative signal, independent of progress and of any GUI/runtime implementation.
pub trait CancellationSignal: Send + Sync {
    /// A request to stop is not proof that an already dispatched effect was undone.
    fn is_cancelled(&self) -> bool;
}

/// Per-call, nonblocking progress port. Implementations must not wait for the UI;
/// delivery failure must not change or roll back the operation's actual effect.
pub trait ProgressReporter: Send + Sync {
    /// Reports observed phases/bytes only; delivery is not operation completion.
    fn try_report(&self, progress: ExecutionProgress) -> ProgressDelivery;
}
