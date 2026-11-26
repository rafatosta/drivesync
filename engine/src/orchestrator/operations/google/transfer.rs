//! Byte accounting for the existing resumable upload and verified download loops.
use super::*;
use std::io::{Read, Write};

impl GoogleOperator {
    pub(super) async fn upload(
        &self,
        session: &GoogleSession,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        metadata: GoogleFileWrite,
        etag: Option<&str>,
        journal: Option<&dyn ExecutionJournal>,
    ) -> Result<GoogleFile, OperatorError> {
        let path = r
            .operation
            .local_path
            .as_deref()
            .ok_or_else(filesystem::invalid)?;
        fs.check(&r.local)?;
        let input = fs.read(path)?;
        let stamp = filesystem::stamp(&input.metadata().map_err(filesystem::error)?);
        let total = stamp.2;
        let upload = activity::request(Phase::PrepareUpload, async {
            if r.operation.action == OperationAction::UpdateRemoteContent {
                self.client
                    .start_resumable_update_conditional(
                        session,
                        &FileId(
                            r.operation
                                .remote_id
                                .as_ref()
                                .ok_or_else(filesystem::invalid)?
                                .0
                                .clone(),
                        ),
                        metadata,
                        FileUpdateOptions {
                            fields: Some(FieldMask(FIELDS.into())),
                            supports_all_drives: Some(true),
                            ..Default::default()
                        },
                        Some(total),
                        etag.ok_or_else(filesystem::stale)?,
                    )
                    .await
            } else {
                self.client
                    .start_resumable_create(
                        session,
                        metadata,
                        FileCreateOptions {
                            fields: Some(FieldMask(FIELDS.into())),
                            supports_all_drives: Some(true),
                            ..Default::default()
                        },
                        Some(total),
                    )
                    .await
            }
            .map_err(mutation_error)
        })
        .await?;
        if matches!(
            r.operation.action,
            OperationAction::Upload | OperationAction::UpdateRemoteContent
        ) {
            journal
                .ok_or_else(filesystem::stale)?
                .checkpoint(RecoveryCheckpoint::Transfer(TransferReference {
                    session_uri: upload.session_uri.clone(),
                    total_bytes: total,
                    confirmed_offset: 0,
                }))?;
        }
        self.transfer(
            session,
            r,
            fs,
            UploadCursor {
                session: upload,
                offset: 0,
            },
            etag,
            journal,
        )
        .await
    }
    pub(super) async fn transfer(
        &self,
        session: &GoogleSession,
        r: &ExecuteOperationRequest,
        fs: &Filesystem,
        cursor: UploadCursor,
        etag: Option<&str>,
        journal: Option<&dyn ExecutionJournal>,
    ) -> Result<GoogleFile, OperatorError> {
        let _sample = crate::performance::Sample::begin("upload_transfer");
        use std::io::{Seek, SeekFrom};
        fs.check(&r.local).map_err(OperatorError::uncertain)?;
        let mut input = fs.read(
            r.operation
                .local_path
                .as_deref()
                .ok_or_else(filesystem::invalid)?,
        )?;
        let stamp = filesystem::stamp(&input.metadata().map_err(filesystem::error)?);
        let total = stamp.2;
        let upload = cursor.session;
        let mut offset = cursor.offset;
        if upload.total_bytes != Some(total) || offset > total {
            return Err(filesystem::stale().uncertain());
        }
        input
            .seek(SeekFrom::Start(offset))
            .map_err(filesystem::error)?;
        let mut span = Span::start(Phase::Upload, State::Running);
        let mut metrics = Bytes {
            total: Some(total),
            read: Some(0),
            submitted: Some(offset),
            submitted_this_attempt: Some(0),
            confirmed: Some(offset),
            ..Bytes::default()
        };
        span.progress(Progress::Bytes(metrics), true);
        let mut checkpoint_failed = false;
        let result = span
            .within(async {
                loop {
                    let remaining = (total - offset).min(256 * 1024) as usize;
                    let mut bytes = vec![0; remaining];
                    input
                        .read_exact(&mut bytes)
                        .map_err(|e| filesystem::error(e).uncertain())?;
                    metrics.read = Some(metrics.read.unwrap_or(0) + remaining as u64);
                    if filesystem::stamp(&input.metadata().map_err(filesystem::error)?) != stamp {
                        return Err(filesystem::stale().uncertain());
                    }
                    let chunk = GoogleUploadChunk {
                        start: offset,
                        bytes,
                        total_bytes: Some(total),
                    };
                    metrics.submitted = Some(offset + remaining as u64);
                    metrics.submitted_this_attempt = metrics.read;
                    span.progress(Progress::Bytes(metrics), false);
                    let progress = span
                        .response(Phase::Upload, async {
                            if let Some(etag) = etag {
                                self.client
                                    .upload_chunk_conditional(session, &upload, chunk, etag)
                                    .await
                            } else {
                                self.client.upload_chunk(session, &upload, chunk).await
                            }
                        })
                        .await
                        .map_err(mutation_error)?;
                    if filesystem::stamp(&input.metadata().map_err(filesystem::error)?) != stamp {
                        return Err(filesystem::stale().uncertain());
                    }
                    offset += remaining as u64;
                    match progress {
                        GoogleUploadProgress::Complete { file } if offset == total => {
                            metrics.confirmed = Some(offset);
                            return Ok(file);
                        }
                        GoogleUploadProgress::Incomplete {
                            received_range: Some(range),
                        } if range.start == 0
                            && range.end_inclusive == offset.checked_sub(1)
                            && offset < total =>
                        {
                            metrics.confirmed = Some(offset);
                            span.progress(Progress::Bytes(metrics), false);
                            if matches!(
                                r.operation.action,
                                OperationAction::Upload | OperationAction::UpdateRemoteContent
                            ) {
                                let checkpoint = journal.ok_or_else(filesystem::stale)?.checkpoint(
                                    RecoveryCheckpoint::Transfer(TransferReference {
                                        session_uri: upload.session_uri.clone(),
                                        total_bytes: total,
                                        confirmed_offset: offset,
                                    }),
                                );
                                checkpoint_failed = checkpoint.is_err();
                                checkpoint?;
                            }
                        }
                        _ => return Err(filesystem::stale().uncertain()),
                    }
                }
            })
            .await;
        span.progress(Progress::Bytes(metrics), true);
        if checkpoint_failed {
            span.finish(Outcome::Unconfirmed);
        } else {
            if result.is_ok() {
                span.effect(Effect::Applied, false);
            }
            span.result(&result);
        }
        let file = result?;
        if matches!(
            r.operation.action,
            OperationAction::Upload | OperationAction::UpdateRemoteContent
        ) {
            journal
                .ok_or_else(filesystem::stale)?
                .checkpoint(RecoveryCheckpoint::Transfer(TransferReference {
                    session_uri: upload.session_uri,
                    total_bytes: total,
                    confirmed_offset: offset,
                }))?;
        }
        Ok(file)
    }
}
/// Media must be completely written and verified before the caller can publish its stage.
pub(super) async fn receive_verified(
    stream: &mut GoogleByteStream,
    output: &mut (impl Write + Send),
    size: u64,
    expected: &str,
) -> Result<(), OperatorError> {
    let _sample = crate::performance::Sample::begin("download_transfer_and_verification");
    let mut download = Span::start(Phase::Download, State::Running);
    let mut metrics = Bytes {
        total: Some(size),
        received: Some(0),
        written: Some(0),
        ..Bytes::default()
    };
    let mut writing = Span::start(Phase::WriteLocal, State::Running);
    let mut hash = HashProgress::new(Phase::VerifyContent, Some(size), Algorithm::Sha256);
    let mut digest = Sha256::new();
    let mut count = 0u64;
    let mut exceeded = false;
    let mut write_failed = false;
    let transfer = async {
        while let Some(bytes) = download
            .response(Phase::Download, stream.next_chunk())
            .await
            .map_err(read_error)?
        {
            count = count
                .checked_add(bytes.len() as u64)
                .ok_or_else(filesystem::stale)?;
            metrics.received = Some(count);
            if count > size {
                exceeded = true;
                return Err(filesystem::stale());
            }
            let written = output.write_all(&bytes).map_err(filesystem::error);
            write_failed = written.is_err();
            written?;
            metrics.written = Some(count);
            digest.update(&bytes);
            hash.processed(bytes.len());
            download.progress(Progress::Bytes(metrics), false);
        }
        Ok(())
    }
    .await;
    download.progress(Progress::Bytes(metrics), true);
    writing.progress(
        Progress::Bytes(Bytes {
            total: metrics.total,
            written: metrics.written,
            ..Bytes::default()
        }),
        true,
    );
    writing.finish(if write_failed {
        Outcome::Failed
    } else if transfer.is_ok() {
        Outcome::Complete
    } else {
        Outcome::Cancelled
    });
    if exceeded || write_failed {
        download.finish(Outcome::Cancelled);
    } else {
        download.result(&transfer);
    }
    if exceeded {
        hash.finish(Outcome::Failed);
        return transfer;
    }
    if transfer.is_err() {
        hash.finish(Outcome::Cancelled);
        return transfer;
    }
    let verified =
        if count == size && format!("{:x}", digest.finalize()).eq_ignore_ascii_case(expected) {
            Ok(())
        } else {
            Err(filesystem::stale())
        };
    hash.result(&verified);
    verified
}
