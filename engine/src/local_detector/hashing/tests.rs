use super::*;
use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "drivesync-local-hash-{}",
            oauth2::CsrfToken::new_random().secret()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str, data: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, data).unwrap();
        path
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn opened(path: &Path) -> File {
    File::open(path).unwrap()
}

#[test]
fn hashes_expected_bytes_without_changing_content_or_version() {
    let directory = TestDirectory::new();
    let path = directory.file("file", b"abc");
    let before = fs::metadata(&path).unwrap();
    let result = hash_local_regular_file(opened(&path), 3).unwrap();
    assert_eq!(
        result.sha256,
        [
            0xba, 0x78, 0x16, 0xbf, 0x8f, 0x01, 0xcf, 0xea, 0x41, 0x41, 0x40, 0xde, 0x5d, 0xae,
            0x22, 0x23, 0xb0, 0x03, 0x61, 0xa3, 0x96, 0x17, 0x7a, 0x9c, 0xb4, 0x10, 0xff, 0x61,
            0xf2, 0x00, 0x15, 0xad,
        ]
    );
    assert_eq!(result.metadata, local_metadata(&before));
    assert!(same_version(&before, &fs::metadata(&path).unwrap()));
    assert_eq!(fs::read(path).unwrap(), b"abc");
}

#[test]
fn zero_and_exact_limits_and_non_regular_descriptors() {
    let directory = TestDirectory::new();
    let empty = directory.file("empty", b"");
    let file = directory.file("file", b"abc");
    assert!(hash_local_regular_file(opened(&empty), 0).is_ok());
    assert!(hash_local_regular_file(opened(&file), 3).is_ok());
    assert!(matches!(
        hash_local_regular_file(opened(&file), 2),
        Err(LocalHashError::LimitExceeded)
    ));
    assert!(matches!(
        hash_local_regular_file(opened(&directory.0), 100),
        Err(LocalHashError::NotRegularFile)
    ));
}

#[test]
fn hashes_from_start_even_when_descriptor_has_been_read() {
    let directory = TestDirectory::new();
    let path = directory.file("file", b"abc");
    let mut file = opened(&path);
    file.seek(SeekFrom::Start(2)).unwrap();
    assert_eq!(
        hash_local_regular_file(file, 3).unwrap(),
        hash_local_regular_file(opened(&path), 3).unwrap()
    );
}

#[test]
fn concurrent_content_change_rejects_digest() {
    let directory = TestDirectory::new();
    let path = directory.file("file", b"abc");
    let file = opened(&path);
    let result = hash_with(file, 100, || {
        let mut writer = OpenOptions::new().append(true).open(&path).unwrap();
        writer.write_all(b"changed").unwrap();
    });
    assert!(matches!(result, Err(LocalHashError::Changed)));
}

#[test]
fn same_size_rewrite_rejects_digest() {
    let directory = TestDirectory::new();
    let path = directory.file("file", b"abc");
    let result = hash_with(opened(&path), 3, || {
        fs::write(&path, b"xyz").unwrap();
    });
    assert!(matches!(result, Err(LocalHashError::Changed)));
}

#[test]
fn multi_buffer_file_is_hashed_completely() {
    let directory = TestDirectory::new();
    let content = vec![0x5a; 131_123];
    let path = directory.file("large", &content);
    let result = hash_local_regular_file(opened(&path), content.len() as u64).unwrap();
    assert_eq!(
        result.sha256.as_slice(),
        Sha256::digest(&content).as_slice()
    );
    assert_eq!(result.metadata.size, content.len() as u64);
}

#[test]
fn replacement_does_not_change_open_file_identity() {
    let directory = TestDirectory::new();
    let path = directory.file("file", b"old");
    let original = opened(&path);
    let replacement = directory.file("replacement", b"new");
    let result = hash_with(original, 100, || {
        fs::rename(&replacement, &path).unwrap();
    });
    // Replacement changes the original inode's link count or ctime; callers
    // must independently verify path membership even if metadata is stable.
    assert!(matches!(result, Err(LocalHashError::Changed)));
    assert_eq!(fs::read(path).unwrap(), b"new");
}
