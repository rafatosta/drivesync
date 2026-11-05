//! Read-only activity diagnostic. Scope labels do not access accounts or credentials.
use drivesync_engine::{
    LocalActivity, LocalBatch, LocalConnectivity, LocalNotice, LocalScope, LocalStateStore,
    LocalWatchOptions, LocalWatchState, LocalWatcher,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    ffi::OsString,
    io::{self, Write},
    os::unix::ffi::OsStrExt,
    path::{Path, PathBuf},
    process::ExitCode,
    time::{Duration, Instant},
};

const USAGE: &str = "Usage: drivesync-local-diagnostic --client LABEL --account LABEL --root ABSOLUTE_PATH [--seconds N] [--poll-ms N] [--max-watches N] [--max-queued N] [--state-dir DIRECTORY] [--pending]\nOutputs JSON lines with byte-exact paths. No synchronization or persistent delivery. Ctrl+C stops observation.";

struct Arguments {
    scope: LocalScope,
    options: LocalWatchOptions,
    duration: Option<Duration>,
    interval: Duration,
    state_dir: Option<PathBuf>,
    pending_only: bool,
}
fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Arguments, &'static str> {
    let mut values = HashMap::new();
    let mut pending_only = false;
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let flag = flag.into_string().map_err(|_| USAGE)?;
        if flag == "--pending" {
            if pending_only {
                return Err(USAGE);
            }
            pending_only = true;
            continue;
        }
        if ![
            "--client",
            "--account",
            "--root",
            "--seconds",
            "--poll-ms",
            "--max-watches",
            "--max-queued",
            "--state-dir",
        ]
        .contains(&flag.as_str())
        {
            return Err(USAGE);
        }
        let value = args.next().ok_or(USAGE)?;
        if values.insert(flag, value).is_some() {
            return Err(USAGE);
        }
    }
    let client = values
        .get("--client")
        .and_then(|v| v.to_str())
        .ok_or(USAGE)?;
    let account = values
        .get("--account")
        .and_then(|v| v.to_str())
        .ok_or(USAGE)?;
    let root = PathBuf::from(values.get("--root").ok_or(USAGE)?);
    let scope = LocalScope::new(client, account, &root).map_err(|_| USAGE)?;
    let number = |key: &str| -> Result<Option<u64>, &'static str> {
        values
            .get(key)
            .map(|v| {
                v.to_str()
                    .and_then(|s| s.parse::<u64>().ok())
                    .filter(|n| *n > 0)
                    .ok_or(USAGE)
            })
            .transpose()
    };
    let seconds = number("--seconds")?;
    if seconds.is_some_and(|n| n > 86400) {
        return Err(USAGE);
    }
    let interval = number("--poll-ms")?.unwrap_or(25);
    if interval > 1000 {
        return Err(USAGE);
    }
    let mut options = LocalWatchOptions::default();
    if let Some(n) = number("--max-watches")? {
        options.max_watches = usize::try_from(n).map_err(|_| USAGE)?;
    }
    if let Some(n) = number("--max-queued")? {
        options.max_queued = usize::try_from(n).map_err(|_| USAGE)?;
    }
    let state_dir = values.get("--state-dir").map(PathBuf::from);
    if pending_only && state_dir.is_none() {
        return Err(USAGE);
    }
    Ok(Arguments {
        state_dir,
        pending_only,
        scope,
        options,
        duration: seconds.map(Duration::from_secs),
        interval: Duration::from_millis(interval),
    })
}

/// Hex is authoritative; the optional UTF-8 value is only for human readability.
fn path_json(path: &Path) -> Value {
    let bytes = path.as_os_str().as_bytes();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    json!({"hex": hex, "utf8": path.to_str()})
}
fn notice_json(notice: &LocalNotice) -> Value {
    let (kind, from, reason) = match &notice.activity {
        LocalActivity::Created => ("created", None, None),
        LocalActivity::Removed => ("removed", None, None),
        LocalActivity::Written => ("written", None, None),
        LocalActivity::ClosedAfterWrite => ("closed_after_write", None, None),
        LocalActivity::MetadataChanged => ("metadata_changed", None, None),
        LocalActivity::MovedIn => ("moved_in", None, None),
        LocalActivity::MovedOut => ("moved_out", None, None),
        LocalActivity::Moved { from } => ("moved", Some(path_json(from)), None),
        LocalActivity::Recheck(reason) => ("recheck", None, Some(reason)),
    };
    json!({"path": path_json(&notice.path), "object_type": notice.object_type,
        "kind": kind, "from": from, "reason": reason})
}
fn batch_json(batch: &LocalBatch) -> Value {
    json!({"type": "activity", "scope": {"client": batch.scope.client(),
        "account": batch.scope.account(), "root": path_json(batch.scope.root())},
        "state": batch.state, "notices": batch.notices.iter().map(notice_json).collect::<Vec<_>>()})
}
fn line(output: &mut impl Write, value: &Value) -> io::Result<()> {
    serde_json::to_writer(&mut *output, value)?;
    writeln!(output)?;
    output.flush()
}
fn run(args: Arguments) -> Result<bool, Box<dyn std::error::Error>> {
    let state = args
        .state_dir
        .as_ref()
        .map(|directory| LocalStateStore::open(directory))
        .transpose()?;
    let mut output = io::stdout().lock();
    if args.pending_only {
        let checkpoint = state
            .as_ref()
            .expect("validated state directory")
            .load(&args.scope)?;
        let value = match checkpoint {
            Some(checkpoint) => json!({"type":"pending_state","generation":checkpoint.generation,
                "items":checkpoint.items,"pending":checkpoint.pending}),
            None => json!({"type":"pending_state","generation":null,"items":[],"pending":[]}),
        };
        line(&mut output, &value)?;
        return Ok(true);
    }
    let mut watcher = LocalWatcher::start(args.scope, args.options)?;
    let started = Instant::now();
    loop {
        let batch = watcher.poll();
        if !batch.notices.is_empty() {
            if let Some(state) = &state {
                state.ingest(&batch, LocalConnectivity::Unknown)?;
            }
            line(&mut output, &batch_json(&batch))?;
        }
        if batch.state == LocalWatchState::Stopped {
            return Ok(false);
        }
        if args
            .duration
            .is_some_and(|duration| started.elapsed() >= duration)
        {
            line(
                &mut output,
                &json!({"type": "session_ended", "state": batch.state,
                "requires_recheck_on_restart": true}),
            )?;
            return Ok(true);
        }
        std::thread::sleep(args.interval);
    }
}
fn main() -> ExitCode {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args == [OsString::from("--help")] {
        println!("{USAGE}");
        return ExitCode::SUCCESS;
    }
    let args = match parse(args) {
        Ok(args) => args,
        Err(message) => {
            eprintln!("{message}");
            return ExitCode::from(2);
        }
    };
    match run(args) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => {
            eprintln!("Observation stopped: start a new session and investigate the coverage gap.");
            ExitCode::from(2)
        }
        Err(_) => {
            eprintln!("Local diagnostic failed; observation or output is unavailable.");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use drivesync_engine::LocalItemKind;
    #[test]
    fn parses_labels_limits_and_rejects_ambiguous_arguments() {
        let base = [
            "--client",
            "test",
            "--account",
            "test",
            "--root",
            "/tmp/test",
        ];
        assert!(parse(base.map(OsString::from)).is_ok());
        for extra in [
            vec!["--seconds", "0"],
            vec!["--poll-ms", "1001"],
            vec!["--unknown", "x"],
            vec!["--root", "/other"],
            vec!["--max-queued", "0"],
        ] {
            assert!(parse(base.iter().copied().chain(extra).map(OsString::from)).is_err());
        }
    }
    #[test]
    fn diagnostic_preserves_non_utf8_paths_and_move_sources() {
        use std::os::unix::ffi::OsStringExt;
        let path = PathBuf::from(OsString::from_vec(vec![0xff, b'\n']));
        let notice = LocalNotice {
            path: path.clone(),
            object_type: LocalItemKind::Unknown,
            activity: LocalActivity::Moved { from: path },
        };
        let value = notice_json(&notice);
        assert_eq!(value["path"]["hex"], "ff0a");
        assert_eq!(value["from"]["hex"], "ff0a");
        assert!(value["path"]["utf8"].is_null());
        assert!(serde_json::to_string(&value).is_ok());
    }
}
