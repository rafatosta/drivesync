#!/usr/bin/env python3
"""Run opt-in cancel/retry and process-crash checks against controlled fixtures."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import selectors
import signal
import stat
import subprocess
import time


def fingerprint(path):
    """Hash a regular non-symlink file and reject concurrent metadata changes."""
    fd = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(fd, "rb") as stream:
        before = os.fstat(stream.fileno())
        if not stat.S_ISREG(before.st_mode):
            raise RuntimeError("Fixture must be a regular file")
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
        after = os.fstat(stream.fileno())
    signature = lambda s: (s.st_dev, s.st_ino, s.st_size, s.st_mtime_ns, s.st_ctime_ns)
    if signature(before) != signature(after):
        raise RuntimeError("Fixture changed while hashing")
    return digest


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for flag in ["binary", "account", "remote-root", "local-root", "transfer-file", "evidence-dir"]:
        parser.add_argument("--" + flag, required=True)
    parser.add_argument("--items", required=True, type=int)
    parser.add_argument("--issues", required=True, type=int)
    args = parser.parse_args()
    if os.environ.get("DRIVESYNC_LIVE_GOOGLE_TEST") != "1":
        parser.error("Set DRIVESYNC_LIVE_GOOGLE_TEST=1 explicitly")
    root = Path(args.local_root)
    relative = Path(args.transfer_file)
    if not root.is_absolute() or root.is_symlink() or not root.is_dir():
        parser.error("Local root must be an existing absolute directory without a symlink")
    root = root.resolve()
    if relative.is_absolute() or not relative.parts or any(p in (".", "..") for p in relative.parts):
        parser.error("Fixture must be a safe relative path")
    fixture = root / relative
    for part in [fixture, *fixture.parents]:
        if part.is_symlink():
            parser.error("Fixture path must not contain symlinks")
        if part == root:
            break
    if args.items < 1 or args.issues < 0 or not 0 < fixture.stat().st_size <= 128 * 1024 * 1024:
        parser.error("Invalid counts or fixture size (maximum 128 MiB)")
    evidence = Path(args.evidence_dir)
    if not evidence.is_absolute() or evidence.resolve().is_relative_to(root):
        parser.error("Evidence must be a new absolute directory outside the sync root")
    evidence.mkdir(mode=0o700, parents=False, exist_ok=False)
    expected_hash = fingerprint(fixture)
    root_identity = (root.stat().st_dev, root.stat().st_ino)
    command = [str(Path(args.binary).resolve()), args.account, args.remote_root, str(root)]
    saved = None

    def record(step, result):
        data = {"step": step, "result": result}
        with (evidence / "steps.jsonl").open("a") as output:
            output.write(json.dumps(data) + "\n")
        print(json.dumps(data), flush=True)

    def validate_root():
        if root.is_symlink() or (root.stat().st_dev, root.stat().st_ino) != root_identity:
            raise RuntimeError("Local root identity changed")

    def preserve(name):
        nonlocal saved
        validate_root()
        if fingerprint(fixture) != expected_hash:
            raise RuntimeError("Fixture differs from its baseline")
        target = evidence / name
        # Evidence is newly created and private; rename fails on cross-filesystem moves.
        if target.exists():
            raise RuntimeError("Evidence name collision")
        os.rename(fixture, target)
        saved = target

    def run(step, *flags):
        validate_root()
        result = subprocess.run(command + list(flags), capture_output=True, text=True, timeout=180, check=False)
        if result.returncode:
            raise RuntimeError(f"{step}: {result.stderr.strip() or 'worker failed'}")
        events = [json.loads(line) for line in result.stdout.splitlines() if line.strip()]
        if not events:
            raise RuntimeError("Worker returned no report")
        record(step, events[-1])
        return events[-1]

    def expect_counts(downloaded=0, recovered=0):
        return f"{downloaded},0,{recovered},{args.items-downloaded-recovered},{args.issues},0"

    try:
        baseline = run("baseline", "--expect", expect_counts())
        if baseline["media_attempts"]:
            raise RuntimeError("Baseline unexpectedly transferred content")
        preserve("before-cancellation.dat")
        cancelled = run("cancel", "--cancel-after-bytes", "1", "--expect-error", "cancelled")
        if cancelled.get("code") != "cancelled" or fixture.exists():
            raise RuntimeError("Cancellation exposed an unexpected final file")
        run("retry", "--expect", expect_counts(downloaded=1))
        if fingerprint(fixture) != expected_hash:
            raise RuntimeError("Retry bytes differ from original")
        preserve("before-crash.dat")
        worker = subprocess.Popen(command + ["--hold-at-install"], stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        ready = False
        try:
            with selectors.DefaultSelector() as selector:
                selector.register(worker.stdout, selectors.EVENT_READ)
                pending = b""
                deadline = time.monotonic() + 180
                while worker.poll() is None and time.monotonic() < deadline:
                    if not selector.select(timeout=1):
                        continue
                    chunk = os.read(worker.stdout.fileno(), 4096)
                    if not chunk:
                        break
                    pending += chunk
                    while b"\n" in pending:
                        line, pending = pending.split(b"\n", 1)
                        if json.loads(line).get("event") == "ready_for_install":
                            ready = True
                            break
                    if ready:
                        break
            if not ready:
                raise RuntimeError("Worker did not reach durable Ready within the timeout")
            worker.kill()
            if worker.wait(timeout=10) != -signal.SIGKILL:
                raise RuntimeError("Worker was not interrupted at the crash boundary")
            record("crash", {"durable_ready_marker": True, "signal": "SIGKILL"})
        finally:
            if worker.poll() is None:
                worker.kill()
            worker.communicate(timeout=10)
        if fixture.exists():
            raise RuntimeError("Unexpected final file before journal recovery")
        recovered = run("recover", "--expect", expect_counts(recovered=1))
        if recovered["media_attempts"] != 0 or fingerprint(fixture) != expected_hash:
            raise RuntimeError("Recovery redownloaded content or produced different bytes")
        final = run("final-restart", "--expect", expect_counts())
        if final["media_attempts"]:
            raise RuntimeError("Final restart unexpectedly transferred content")
        record("suite", {"passed": True, "sha256": expected_hash})
    finally:
        # On failure, restore only an absent name. Never overwrite a competing entry.
        validate_root()
        if saved is not None and not os.path.lexists(fixture):
            try:
                os.link(saved, fixture, follow_symlinks=False)
            except OSError as error:
                raise RuntimeError("Original fixture remains preserved in evidence; restore manually") from error
            record("failure-restoration", {"restored_missing_path": True})


if __name__ == "__main__":
    main()
