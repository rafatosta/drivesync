//! Durable checkpoints for incomplete binary downloads.
use super::*;
use reqwest::header::CONTENT_RANGE;
use serde::Serialize;

const CHECKPOINT_BYTES: u64 = 4 * 1024 * 1024;
const ATTEMPTS: u32 = 3;

#[derive(Deserialize, Serialize)]
struct Checkpoint {
    id: String,
    version: String,
    total: u64,
    sha256: Option<String>,
    durable: u64,
}

pub(super) struct PartialDownload<'a> {
    pub(super) file: File,
    pub(super) path: String,
    sidecar: String,
    directory: &'a File,
    checkpoint: Checkpoint,
}

impl<'a> PartialDownload<'a> {
    pub(super) fn open(pending: &'a PendingFile<'_>, total: u64) -> Result<Self, EngineError> {
        let entry = pending.entry;
        let key = Sha256::digest(entry.id.as_bytes());
        let path = format!(".drivesync-download-{key:x}.partial");
        let sidecar = format!(".drivesync-download-{key:x}.checkpoint");
        let file = File::from(
            fs::openat2(
                pending.directory,
                path.as_str(),
                OFlags::CREATE
                    | OFlags::RDWR
                    | OFlags::NOFOLLOW
                    | OFlags::NONBLOCK
                    | OFlags::CLOEXEC,
                Mode::RUSR | Mode::WUSR,
                ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_XDEV,
            )
            .map_err(local_error)?,
        );
        let stat = fs::fstat(&file).map_err(local_error)?;
        if fs::FileType::from_raw_mode(stat.st_mode) != fs::FileType::RegularFile
            || stat.st_nlink != 1
        {
            return Err(EngineError::DownloadConflict);
        }
        let saved = pending
            .open(&sidecar)
            .ok()
            .and_then(|file| serde_json::from_reader::<_, Checkpoint>(file.take(4096)).ok());
        let mut partial = Self {
            file,
            path,
            sidecar,
            directory: pending.directory,
            checkpoint: Checkpoint {
                id: entry.id.clone(),
                version: entry.version.clone(),
                total,
                sha256: entry.sha256_checksum.clone(),
                durable: 0,
            },
        };
        if let Some(saved) = saved.filter(|saved| {
            saved.id == entry.id
                && saved.version == entry.version
                && saved.total == total
                && saved.sha256 == entry.sha256_checksum
                && saved.durable <= total
                && saved.durable <= stat.st_size as u64
        }) {
            partial.checkpoint = saved;
        }
        // A checksum is required before trusting bytes obtained across requests.
        if entry.sha256_checksum.is_none() {
            partial.checkpoint.durable = 0;
        }
        partial.rewind_to_checkpoint()?;
        Ok(partial)
    }

    fn rewind_to_checkpoint(&mut self) -> Result<(), EngineError> {
        let offset = self.checkpoint.durable;
        self.file
            .set_len(offset)
            .and_then(|()| self.file.seek(io::SeekFrom::Start(offset)))
            .map_err(|_| EngineError::DownloadLocal)?;
        Ok(())
    }

    pub(super) fn invalidate(&mut self) -> Result<(), EngineError> {
        self.clear_checkpoint()?;
        self.directory
            .sync_all()
            .map_err(|_| EngineError::DownloadLocal)?;
        self.checkpoint.durable = 0;
        self.rewind_to_checkpoint()
    }

    pub(super) fn clear_checkpoint(&self) -> Result<(), EngineError> {
        match fs::unlinkat(self.directory, self.sidecar.as_str(), fs::AtFlags::empty()) {
            Ok(()) | Err(rustix::io::Errno::NOENT) => Ok(()),
            Err(error) => Err(local_error(error)),
        }
    }

    fn save(&mut self, bytes: u64) -> Result<(), EngineError> {
        self.file
            .sync_data()
            .map_err(|_| EngineError::DownloadLocal)?;
        self.checkpoint.durable = bytes;
        let temporary = format!(
            ".drivesync-download-{}.checkpoint-tmp",
            oauth2::CsrfToken::new_random().secret()
        );
        let result = (|| {
            let mut proof = File::from(
                fs::openat(
                    self.directory,
                    temporary.as_str(),
                    OFlags::CREATE | OFlags::EXCL | OFlags::WRONLY | OFlags::CLOEXEC,
                    Mode::RUSR | Mode::WUSR,
                )
                .map_err(local_error)?,
            );
            serde_json::to_writer(&mut proof, &self.checkpoint)
                .map_err(|_| EngineError::DownloadLocal)?;
            proof.sync_all().map_err(|_| EngineError::DownloadLocal)?;
            fs::renameat(
                self.directory,
                temporary.as_str(),
                self.directory,
                self.sidecar.as_str(),
            )
            .map_err(local_error)?;
            self.directory
                .sync_all()
                .map_err(|_| EngineError::DownloadLocal)
        })();
        if result.is_err() {
            let _ = fs::unlinkat(self.directory, temporary.as_str(), fs::AtFlags::empty());
        }
        result
    }

    pub(super) fn receive(
        &mut self,
        token: &str,
        entry: &Entry,
        notify: &impl Fn(DownloadProgress),
    ) -> Result<(), EngineError> {
        let total = self.checkpoint.total;
        notify(DownloadProgress::Started { total });
        if self.checkpoint.durable > 0 {
            notify(DownloadProgress::Receiving {
                received: self.checkpoint.durable,
                total,
            });
        }
        for attempt in 0..ATTEMPTS {
            match self.receive_once(token, entry, notify) {
                Ok(()) => return Ok(()),
                Err(error) if attempt + 1 < ATTEMPTS && retryable(&error) => {
                    if entry.sha256_checksum.is_none() {
                        self.invalidate()?;
                    }
                    std::thread::sleep(Duration::from_secs(1 << attempt));
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!("at least one attempt")
    }

    fn receive_once(
        &mut self,
        token: &str,
        entry: &Entry,
        notify: &impl Fn(DownloadProgress),
    ) -> Result<(), EngineError> {
        self.rewind_to_checkpoint()?;
        let mut offset = self.checkpoint.durable;
        let total = self.checkpoint.total;
        if offset > 0 {
            if !entry.same_download_version(&metadata(token, &entry.id)?) {
                self.invalidate()?;
                return Err(EngineError::DownloadIncomplete);
            }
            if offset == total {
                return Ok(());
            }
        } else {
            // Discard any checkpoint for a different version before writing.
            self.invalidate()?;
        }
        let mut response = remote::stream(
            token,
            &format!("{FILES}/{}", entry.id),
            &[("alt", "media"), ("supportsAllDrives", "true")],
            Duration::from_secs(600),
            (offset > 0).then_some(offset),
        )?;
        match response.status().as_u16() {
            200 => {
                if offset > 0 {
                    self.invalidate()?;
                    offset = 0;
                    notify(DownloadProgress::Started { total });
                }
            }
            206 if offset > 0
                && valid_range(
                    response
                        .headers()
                        .get(CONTENT_RANGE)
                        .and_then(|value| value.to_str().ok()),
                    offset,
                    total,
                ) => {}
            206 | 416 => {
                self.invalidate()?;
                return Err(EngineError::DownloadIncomplete);
            }
            status => {
                remote::response_status(status, &[])?;
                return Err(EngineError::DownloadIncomplete);
            }
        }
        while offset < total {
            let chunk = (total - offset).min(CHECKPOINT_BYTES);
            let result = copy_with_progress(
                (&mut response).take(chunk),
                &mut self.file,
                chunk,
                |progress| {
                    if let DownloadProgress::Receiving { received, .. } = progress {
                        notify(DownloadProgress::Receiving {
                            received: offset + received,
                            total,
                        });
                    }
                },
            );
            let written = self
                .file
                .stream_position()
                .map_err(|_| EngineError::DownloadLocal)?;
            if written > offset && written < total {
                self.save(written)?;
            }
            let copied = result?;
            if copied != chunk {
                return Err(EngineError::Authentication(crate::AuthError::Network));
            }
            offset = written;
        }
        let mut extra = [0];
        if response
            .read(&mut extra)
            .map_err(|_| EngineError::Authentication(crate::AuthError::Network))?
            != 0
        {
            self.invalidate()?;
            return Err(EngineError::DownloadIncomplete);
        }
        self.save(total)?;
        Ok(())
    }
}

fn valid_range(header: Option<&str>, offset: u64, total: u64) -> bool {
    total > offset && header == Some(format!("bytes {offset}-{}/{total}", total - 1).as_str())
}

fn retryable(error: &EngineError) -> bool {
    matches!(
        error,
        EngineError::RemoteRateLimited
            | EngineError::Authentication(
                crate::AuthError::Network
                    | crate::AuthError::ProviderUnavailable
                    | crate::AuthError::TimedOut
            )
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reopening_uses_only_durable_bytes_and_discards_a_changed_version() {
        let nonce = oauth2::CsrfToken::new_random();
        let path = std::env::temp_dir().join(format!("drivesync-partial-{}", nonce.secret()));
        std::fs::create_dir(&path).unwrap();
        let directory = File::open(&path).unwrap();
        let mut entry = Entry {
            id: "test-id".into(),
            name: "test".into(),
            mime_type: "application/octet-stream".into(),
            version: "1".into(),
            size: Some("10".into()),
            sha256_checksum: Some("test-checksum".into()),
            modified_time: None,
            trashed: false,
            explicitly_trashed: false,
            parents: vec![],
        };
        {
            let pending = PendingFile::new(&entry, &directory);
            let mut partial = PartialDownload::open(&pending, 10).unwrap();
            partial.file.write_all(b"0123").unwrap();
            partial.save(4).unwrap();
            partial.file.write_all(b"unconfirmed").unwrap();
        }
        {
            let pending = PendingFile::new(&entry, &directory);
            let mut partial = PartialDownload::open(&pending, 10).unwrap();
            assert_eq!(partial.file.metadata().unwrap().len(), 4);
            assert_eq!(partial.file.stream_position().unwrap(), 4);
        }
        entry.version = "2".into();
        let pending = PendingFile::new(&entry, &directory);
        let partial = PartialDownload::open(&pending, 10).unwrap();
        assert_eq!(partial.file.metadata().unwrap().len(), 0);
        assert_eq!(partial.checkpoint.durable, 0);
        std::fs::remove_dir_all(path).unwrap();
    }

    #[test]
    fn validates_the_exact_remaining_range() {
        assert!(valid_range(Some("bytes 4-9/10"), 4, 10));
        for header in [
            None,
            Some("bytes 0-9/10"),
            Some("bytes 4-8/10"),
            Some("bytes 4-9/11"),
        ] {
            assert!(!valid_range(header, 4, 10));
        }
    }
}
