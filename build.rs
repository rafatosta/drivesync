//! Embeds the source revision at build time without a runtime Git dependency.
use std::{env, process::Command};

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn main() {
    println!("cargo:rerun-if-env-changed=DRIVESYNC_BUILD_REVISION");
    // Watch package inputs and Git metadata, including commits with unchanged sources.
    for path in ["build.rs", "src", "Cargo.toml"] {
        println!("cargo:rerun-if-changed={path}");
    }
    if git(&["rev-parse", "--show-prefix"]).as_deref() == Some("") {
        if let Some(files) = git(&["ls-files"]) {
            for path in files.lines() {
                println!("cargo:rerun-if-changed={path}");
            }
        }
        for name in ["HEAD", "index", "packed-refs", "refs"] {
            if let Some(path) = git(&["rev-parse", "--git-path", name]) {
                if std::path::Path::new(&path).exists() {
                    println!("cargo:rerun-if-changed={path}");
                }
            }
        }
    }
    let revision = env::var("DRIVESYNC_BUILD_REVISION").ok().or_else(|| {
        // Never attribute an unpacked source tree to an unrelated parent repository.
        if !git(&["rev-parse", "--show-prefix"])?.is_empty() {
            return None;
        }
        let commit = git(&["rev-parse", "HEAD"])?;
        let status = git(&["status", "--porcelain", "--untracked-files=no"])?;
        Some(format!(
            "{commit}{}",
            if status.is_empty() { "" } else { "-modified" }
        ))
    });
    let revision = revision.unwrap_or_else(|| "unknown".into());
    let hash = revision.strip_suffix("-modified").unwrap_or(&revision);
    assert!(revision == "unknown" || ((hash.len() == 40 || hash.len() == 64)
        && hash.bytes().all(|byte| byte.is_ascii_hexdigit())),
        "DRIVESYNC_BUILD_REVISION must be a full Git hash, optionally ending in -modified, or unknown");
    let short = if revision == "unknown" {
        revision.clone()
    } else {
        format!(
            "{}{}",
            &hash[..7],
            if revision.ends_with("-modified") {
                "-modified"
            } else {
                ""
            }
        )
    };
    println!("cargo:rustc-env=DRIVESYNC_SOURCE_REVISION={revision}");
    println!(
        "cargo:rustc-env=DRIVESYNC_VERSION={} ({short})",
        env::var("CARGO_PKG_VERSION").unwrap()
    );
}
