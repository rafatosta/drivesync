//! Exercises the actual diagnostic process against disposable filesystem activity.
use serde_json::Value;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::PathBuf,
    process::{Child, Command, Stdio},
    sync::mpsc::{self, Receiver},
    time::{Duration, Instant},
};

struct Fixture {
    root: PathBuf,
    child: Child,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.root);
    }
}
fn start() -> (Fixture, Receiver<Value>) {
    let root = std::env::temp_dir().join(format!(
        "drivesync-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir(&root).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_drivesync-local-diagnostic"))
        .args([
            "--client",
            "diagnostic",
            "--account",
            "disposable",
            "--seconds",
            "2",
            "--root",
        ])
        .arg(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let reader = BufReader::new(child.stdout.take().unwrap());
    let (tx, rx) = mpsc::channel();
    std::thread::spawn(move || {
        for line in reader.lines() {
            let Ok(line) = line else {
                break;
            };
            let Ok(value) = serde_json::from_str(&line) else {
                break;
            };
            if tx.send(value).is_err() {
                break;
            }
        }
    });
    (Fixture { root, child }, rx)
}
fn wait_for(rx: &Receiver<Value>, mut matches: impl FnMut(&Value) -> bool) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let value = rx
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .expect("diagnostic output");
        if matches(&value) {
            return value;
        }
    }
}
fn has(batch: &Value, kind: &str, path: &str) -> bool {
    batch["notices"].as_array().is_some_and(|notices| {
        notices
            .iter()
            .any(|n| n["kind"] == kind && n["path"]["utf8"] == path)
    })
}

#[test]
fn diagnostic_observes_real_actions_and_exits_without_mutating_content() {
    let (mut fixture, rx) = start();
    let first = wait_for(&rx, |v| v["type"] == "activity");
    assert_eq!(first["state"], "Observing");
    assert!(first["notices"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["reason"] == "Startup"));
    fs::write(fixture.root.join("file"), b"preserved").unwrap();
    wait_for(&rx, |v| has(v, "written", "file"));
    fs::rename(fixture.root.join("file"), fixture.root.join("renamed")).unwrap();
    let moved = wait_for(&rx, |v| has(v, "moved", "renamed"));
    assert!(moved["notices"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["kind"] == "moved" && n["from"]["utf8"] == "file"));
    fs::create_dir(fixture.root.join("folder")).unwrap();
    wait_for(&rx, |v| has(v, "created", "folder"));
    fs::write(fixture.root.join("folder/nested"), b"nested").unwrap();
    wait_for(&rx, |v| has(v, "written", "folder/nested"));
    fs::remove_file(fixture.root.join("renamed")).unwrap();
    wait_for(&rx, |v| has(v, "removed", "renamed"));
    let end = wait_for(&rx, |v| v["type"] == "session_ended");
    assert_eq!(end["state"], "Incomplete");
    assert_eq!(end["requires_recheck_on_restart"], true);
    assert!(fixture.child.wait().unwrap().success());
    assert_eq!(
        fs::read(fixture.root.join("folder/nested")).unwrap(),
        b"nested"
    );
}

#[test]
fn diagnostic_stops_on_root_loss_with_nonzero_exit() {
    let (mut fixture, rx) = start();
    wait_for(&rx, |v| v["type"] == "activity");
    fs::remove_dir(&fixture.root).unwrap();
    let last = wait_for(&rx, |v| v["state"] == "Stopped");
    assert!(last["notices"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["reason"] == "RootChanged"));
    assert_eq!(fixture.child.wait().unwrap().code(), Some(2));
}
