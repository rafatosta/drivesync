//! Bounded three-way candidate generation for plain UTF-8 text.
//! This pure operation neither authenticates its inputs nor authorizes installation.

/// Why a candidate cannot be produced without guessing alignment or content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextMergeRefusal {
    UnsupportedFormat,
    MissingBase,
    TooLarge,
    InvalidText,
    ChangedLineStructure,
    OverlappingChanges,
}

/// A candidate remains subject to review, version preservation and revalidation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextMergeOutcome {
    Candidate(Vec<u8>),
    Preserve(TextMergeRefusal),
}

/// Combines nonoverlapping line replacements relative to a supplied common base.
///
/// Only explicit `text/plain`, UTF-8 without NUL, and at most 1 MiB per version
/// are supported. All versions must have the same number of lines and identical
/// per-line terminators (LF, CRLF, or no final newline). Different line counts,
/// line-ending changes and differing replacements of one line are refused.
/// Compensating insertions/deletions cannot be distinguished from replacements;
/// this positional algorithm requires candidate review even on success.
/// Inputs are borrowed and untouched; no partial candidate escapes on refusal.
///
/// The caller must establish that `base` is the confirmed common ancestor of
/// this exact local/remote identity. A candidate is syntactic, not semantic
/// validation, and never grants permission to resolve a saved conflict.
///
/// ```
/// use drivesync_engine::{merge_plain_text, TextMergeOutcome};
/// assert_eq!(
///     merge_plain_text("text/plain", Some(b"a\nb\n"), b"A\nb\n", b"a\nB\n"),
///     TextMergeOutcome::Candidate(b"A\nB\n".to_vec())
/// );
/// ```
pub fn merge_plain_text(
    mime_type: &str,
    base: Option<&[u8]>,
    local: &[u8],
    remote: &[u8],
) -> TextMergeOutcome {
    use TextMergeRefusal::*;
    let refuse = TextMergeOutcome::Preserve;
    if mime_type != "text/plain" {
        return refuse(UnsupportedFormat);
    }
    let Some(base) = base else {
        return refuse(MissingBase);
    };
    let mut versions = Vec::with_capacity(3);
    for bytes in [base, local, remote] {
        if bytes.len() > 1024 * 1024 {
            return refuse(TooLarge);
        }
        let Ok(text) = std::str::from_utf8(bytes) else {
            return refuse(InvalidText);
        };
        if text.contains('\0') {
            return refuse(InvalidText);
        }
        versions.push(text.split_inclusive('\n').collect::<Vec<_>>());
    }
    let [base, local, remote] = versions.as_slice() else {
        unreachable!()
    };
    if base.len() != local.len() || base.len() != remote.len() {
        return refuse(ChangedLineStructure);
    }
    let mut merged = Vec::new();
    for ((b, l), r) in base.iter().zip(local).zip(remote) {
        if terminator(b) != terminator(l) || terminator(b) != terminator(r) {
            return refuse(ChangedLineStructure);
        }
        let line = if l == r || r == b {
            l
        } else if l == b {
            r
        } else {
            return refuse(OverlappingChanges);
        };
        merged.extend_from_slice(line.as_bytes());
        if merged.len() > 1024 * 1024 {
            return refuse(TooLarge);
        }
    }
    TextMergeOutcome::Candidate(merged)
}

fn terminator(line: &str) -> &str {
    if line.ends_with("\r\n") {
        "\r\n"
    } else if line.ends_with('\n') {
        "\n"
    } else {
        ""
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p05_independent_replacements_and_identical_edits() {
        for (base, local, remote, expected) in [
            ("a\nb\n", "A\nb\n", "a\nB\n", "A\nB\n"),
            ("a\nb", "A\nb", "A\nb", "A\nb"),
            ("a\r\nb", "á\r\nb", "a\r\nβ", "á\r\nβ"),
            ("", "", "", ""),
        ] {
            assert_eq!(
                merge_plain_text(
                    "text/plain",
                    Some(base.as_bytes()),
                    local.as_bytes(),
                    remote.as_bytes()
                ),
                TextMergeOutcome::Candidate(expected.as_bytes().to_vec())
            );
        }
    }

    #[test]
    fn p05_refuses_unknown_base_format_binary_overlap_and_structure() {
        use TextMergeRefusal::*;
        for (mime, base, local, remote, reason) in [
            (
                "text/plain",
                None,
                b"a".as_slice(),
                b"b".as_slice(),
                MissingBase,
            ),
            (
                "application/json",
                Some(b"a".as_slice()),
                b"a",
                b"a",
                UnsupportedFormat,
            ),
            (
                "text/plain",
                Some(b"a".as_slice()),
                b"\xff",
                b"a",
                InvalidText,
            ),
            (
                "text/plain",
                Some(b"a".as_slice()),
                b"a\0",
                b"a",
                InvalidText,
            ),
            (
                "text/plain",
                Some(b"a".as_slice()),
                b"b",
                b"c",
                OverlappingChanges,
            ),
            (
                "text/plain",
                Some(b"a\n".as_slice()),
                b"a\nb\n",
                b"a\n",
                ChangedLineStructure,
            ),
            (
                "text/plain",
                Some(b"a\n".as_slice()),
                b"",
                b"a\n",
                ChangedLineStructure,
            ),
            (
                "text/plain",
                Some(b"a\n".as_slice()),
                b"a\r\n",
                b"a\n",
                ChangedLineStructure,
            ),
            (
                "text/plain",
                Some(b"a\n".as_slice()),
                b"a",
                b"a\n",
                ChangedLineStructure,
            ),
        ] {
            assert_eq!(
                merge_plain_text(mime, base, local, remote),
                TextMergeOutcome::Preserve(reason)
            );
        }
        let large = vec![b'a'; 1024 * 1024 + 1];
        for (b, l, r) in [
            (large.as_slice(), b"".as_slice(), b"".as_slice()),
            (b"", &large, b""),
            (b"", b"", &large),
        ] {
            assert_eq!(
                merge_plain_text("text/plain", Some(b), l, r),
                TextMergeOutcome::Preserve(TooLarge)
            );
        }
    }

    #[test]
    fn p05_bounds_output_and_never_returns_partial_candidate() {
        let limit = vec![b'a'; 1024 * 1024];
        assert_eq!(
            merge_plain_text("text/plain", Some(&limit), &limit, &limit),
            TextMergeOutcome::Candidate(limit.clone())
        );
        let half = "x".repeat(600_000);
        let local = format!("{half}\nb\n");
        let remote = format!("a\n{half}\n");
        assert_eq!(
            merge_plain_text(
                "text/plain",
                Some(b"a\nb\n"),
                local.as_bytes(),
                remote.as_bytes()
            ),
            TextMergeOutcome::Preserve(TextMergeRefusal::TooLarge)
        );
        assert_eq!(
            merge_plain_text("text/plain", Some(b"a\nb\n"), b"A\nB\n", b"a\nC\n"),
            TextMergeOutcome::Preserve(TextMergeRefusal::OverlappingChanges)
        );
    }

    #[test]
    fn p05_repeat_parallel_and_fresh_invocations_preserve_all_inputs() {
        let base = b"a\nb\n".to_vec();
        let local = b"A\nb\n".to_vec();
        let remote = b"a\nB\n".to_vec();
        let originals = (base.clone(), local.clone(), remote.clone());
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    for _ in 0..10 {
                        assert_eq!(
                            merge_plain_text("text/plain", Some(&base), &local, &remote),
                            TextMergeOutcome::Candidate(b"A\nB\n".to_vec())
                        );
                    }
                });
            }
        });
        assert_eq!((base, local, remote), originals);
    }
}
