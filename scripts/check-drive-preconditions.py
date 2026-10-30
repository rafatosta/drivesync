#!/usr/bin/env python3
"""Probe Drive conditional writes using newly created disposable files.

This experiment never enables production writes. It intentionally retains its
fixtures, including on failure, and never retries an uncertain mutation.
"""
import argparse
import getpass
import hashlib
import http.client
import json
import os
from pathlib import Path
import re
import sys
import urllib.error
import urllib.request
import uuid


class ProbeError(Exception):
    """A fixed, credential-free reason; provider bodies are never exceptions."""


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        return None


class Drive:
    """Bounded HTTPS transport with no redirects, proxy inheritance or retries."""

    def __init__(self, token):
        if not token or any(c.isspace() for c in token):
            raise ProbeError("invalid_token")
        self.token = token
        self.opener = urllib.request.build_opener(
            urllib.request.ProxyHandler({}), NoRedirect()
        )

    def request(self, method, path, body=None, content_type=None, etag=None, resumable=False):
        if not re.fullmatch(r"/(?:upload/)?drive/v[23]/files(?:/[A-Za-z0-9_-]+)?(?:\?[^\r\n]*)?", path):
            raise ProbeError("invalid_endpoint")
        headers = {"Authorization": "Bearer " + self.token}
        if content_type:
            headers["Content-Type"] = content_type
        if etag is not None:
            headers["If-Match"] = etag
        if resumable:
            headers["X-Upload-Content-Type"] = "application/octet-stream"
            headers["X-Upload-Content-Length"] = "17"
        request = urllib.request.Request(
            "https://www.googleapis.com" + path,
            data=body, headers=headers, method=method,
        )
        try:
            response = self.opener.open(request, timeout=30)
        except urllib.error.HTTPError as error:
            response = error
        except (OSError, ValueError, http.client.HTTPException):
            raise ProbeError("transport_unavailable_or_effect_unknown") from None
        try:
            with response:
                data = response.read(65537)
                if len(data) > 65536:
                    raise ProbeError("response_limit")
                return response.code, response.headers.get("Location" if resumable else "ETag"), data
        except (OSError, http.client.HTTPException):
            raise ProbeError("response_unavailable_or_effect_unknown") from None


def decode(data):
    try:
        value = json.loads(data)
    except (ValueError, UnicodeError):
        raise ProbeError("invalid_response") from None
    if not isinstance(value, dict):
        raise ProbeError("invalid_response")
    return value


def strong_etag(value):
    """Reject absent, weak, wildcard, combined or malformed entity tags."""
    if not isinstance(value, str) or not re.fullmatch(r'"[\x21\x23-\x7e]+"', value):
        raise ProbeError("strong_etag_unavailable")
    return value


class Probe:
    """Run positive controls and stale-write rejection oracles on one fixture."""

    def __init__(self, drive, record, write_api="v3"):
        self.drive = drive
        self.record = record
        self.nonce = uuid.uuid4().hex
        self.file_id = None
        self.write_api = write_api

    def endpoint(self, media=False):
        if self.file_id is None:
            raise ProbeError("fixture_not_created")
        if media:
            return "/drive/v3/files/" + self.file_id + "?alt=media"
        return (
            "/drive/v3/files/" + self.file_id
            + "?fields=id,name,version,trashed,appProperties"
        )

    def create(self):
        boundary = "drivesync-" + self.nonce
        metadata = json.dumps({
            "name": "DriveSync precondition experiment " + self.nonce,
            "mimeType": "application/octet-stream",
            "appProperties": {"drivesyncPreconditionProbe": self.nonce},
        }).encode()
        body = (
            b"--" + boundary.encode() + b"\r\nContent-Type: application/json\r\n\r\n"
            + metadata + b"\r\n--" + boundary.encode()
            + b"\r\nContent-Type: application/octet-stream\r\n\r\nbaseline\r\n--"
            + boundary.encode() + b"--\r\n"
        )
        code, _, data = self.drive.request(
            "POST", "/upload/drive/v3/files?uploadType=multipart&fields=id",
            body, "multipart/related; boundary=" + boundary,
        )
        if code not in (200, 201):
            raise ProbeError("create_not_confirmed")
        file_id = decode(data).get("id")
        if not isinstance(file_id, str) or not re.fullmatch(r"[A-Za-z0-9_-]{1,256}", file_id):
            raise ProbeError("invalid_created_identity")
        self.file_id = file_id
        self.record({"step": "created", "file_id": file_id})

    def metadata(self):
        code, etag, data = self.drive.request("GET", self.endpoint())
        if code != 200:
            raise ProbeError("metadata_unavailable")
        item = decode(data)
        if etag is None and self.write_api == "v3":
            # A projection must not be mistaken for endpoint-wide lack of a tag.
            code, etag, data = self.drive.request(
                "GET", "/drive/v3/files/" + self.file_id + "?fields=*"
            )
            if code != 200:
                raise ProbeError("full_metadata_unavailable")
            item = decode(data)
        item = {key: item.get(key) for key in (
            "id", "name", "version", "trashed", "appProperties"
        )}
        if (
            item.get("id") != self.file_id
            or item.get("trashed") is not False
            or item.get("appProperties", {}) != {"drivesyncPreconditionProbe": self.nonce}
            or not isinstance(item.get("name"), str)
            or not isinstance(item.get("version"), str)
            or not re.fullmatch(r"[1-9][0-9]*", item["version"])
        ):
            raise ProbeError("fixture_identity_or_version_changed")
        if self.write_api == "v2":
            code, _, data = self.drive.request(
                "GET", "/drive/v2/files/" + self.file_id + "?fields=id,title,version,etag"
            )
            legacy = decode(data)
            if (code != 200 or legacy.get("id") != item["id"]
                    or legacy.get("title") != item["name"]
                    or legacy.get("version") != item["version"]):
                self.record({"step": "cross_api_mismatch", "status": code,
                             "same_id": legacy.get("id") == item["id"],
                             "same_name": legacy.get("title") == item["name"],
                             "same_version": legacy.get("version") == item["version"]})
                raise ProbeError("cross_api_evidence_changed")
            etag = legacy.get("etag")
        return item, strong_etag(etag)

    def snapshot(self):
        before, etag = self.metadata()
        code, _, content = self.drive.request("GET", self.endpoint(media=True))
        after, after_etag = self.metadata()
        if code != 200 or before != after or etag != after_etag:
            self.record({"step": "snapshot_mismatch", "status": code,
                         "same_metadata": before == after, "same_etag": etag == after_etag,
                         "same_version": before["version"] == after["version"],
                         "before_version": before["version"], "after_version": after["version"]})
            raise ProbeError("unstable_snapshot")
        return before, etag, content

    def mutate(self, media, payload, etag):
        # The ID always comes from this process's successful create response.
        if media:
            path = "/upload/drive/" + self.write_api + "/files/" + self.file_id + "?uploadType=media"
            content_type = "application/octet-stream"
        else:
            path = "/drive/" + self.write_api + "/files/" + self.file_id
            content_type = "application/json"
            payload = json.dumps({"title" if self.write_api == "v2" else "name": payload}).encode()
        method = "PUT" if media and self.write_api == "v2" else "PATCH"
        return self.drive.request(method, path, payload, content_type, etag)[0]

    def settled_snapshot(self):
        # New objects can still be settling after creation/first read. Only
        # read-only observations may be retried; mutations never are. Callers
        # still check every expected effect after obtaining a stable snapshot.
        for attempt in range(3):
            try:
                return self.snapshot()
            except ProbeError as error:
                if str(error) not in ("unstable_snapshot", "cross_api_evidence_changed") or attempt == 2:
                    raise

    def run(self, media):
        self.create()
        initial = self.settled_snapshot()
        if initial[2] != b"baseline":
            raise ProbeError("unexpected_initial_bytes")
        self.record({"step": "baseline_verified", "version": initial[0]["version"]})
        current = initial
        for index in range(2):
            desired = (b"updated-bytes-" + str(index).encode()) if media else (
                "DriveSync probe updated " + self.nonce + "-" + str(index)
            )
            code = self.mutate(media, desired, current[1])
            self.record({"step": "current_write_response", "status": code,
                         "kind": "media" if media else "name", "iteration": index})
            if code != 200:
                raise ProbeError("current_precondition_not_accepted")
            updated = self.settled_snapshot()
            if (
                int(updated[0]["version"]) <= int(current[0]["version"])
                or updated[1] == current[1]
                or updated[0]["name"] != (current[0]["name"] if media else desired)
                or updated[2] != (desired if media else current[2])
            ):
                self.record({"step": "current_write_mismatch", "same_etag": updated[1] == current[1],
                             "version_advanced": int(updated[0]["version"]) > int(current[0]["version"]),
                             "expected_bytes": updated[2] == (desired if media else current[2]),
                             "expected_name": updated[0]["name"] == (current[0]["name"] if media else desired)})
                raise ProbeError("current_write_effect_not_verified")
            self.record({"step": "current_write_verified", "kind": "media" if media else "name",
                         "iteration": index, "version": updated[0]["version"],
                         "sha256": hashlib.sha256(updated[2]).hexdigest()})
            # Replay an obsolete write twice; a 412 alone is insufficient.
            for _ in range(2):
                stale = b"must-not-be-installed" if media else "must-not-be-renamed"
                code = self.mutate(media, stale, current[1])
                if code != 412:
                    raise ProbeError("stale_write_not_rejected")
                if self.snapshot() != updated:
                    raise ProbeError("stale_write_changed_fixture")
            self.record({"step": "stale_replay_rejected", "kind": "media" if media else "name",
                         "iteration": index, "status": 412})
            current = updated

    def begin_session(self, etag):
        """Return only a validated same-origin session path; never record it."""
        code, location, _ = self.drive.request(
            "PUT" if self.write_api == "v2" else "PATCH",
            "/upload/drive/" + self.write_api + "/files/" + self.file_id + "?uploadType=resumable",
            b"{}", "application/json", etag, resumable=True,
        )
        if code != 200:
            raise ProbeError("session_not_confirmed")
        prefix = "https://www.googleapis.com"
        expected = "/upload/drive/" + self.write_api + "/files/" + self.file_id + "?"
        if (not isinstance(location, str) or not location.startswith(prefix + expected)
                or "upload_id=" not in location or "\r" in location or "\n" in location):
            raise ProbeError("invalid_session_origin")
        return location[len(prefix):]

    def run_resumable_race(self):
        """Test the window between conditional session creation and publication."""
        self.create()
        initial = self.settled_snapshot()
        if initial[2] != b"baseline":
            raise ProbeError("unexpected_initial_bytes")
        session = self.begin_session(initial[1])
        self.record({"step": "conditional_session_created"})
        # A competing writer commits new content after session authorization.
        competitor = b"concurrent-writer"
        current = self.settled_snapshot()
        if self.mutate(True, competitor, current[1]) != 200:
            raise ProbeError("competitor_write_not_confirmed")
        competing = self.settled_snapshot()
        if competing[2] != competitor or competing[1] == initial[1]:
            raise ProbeError("competitor_effect_not_verified")
        self.record({"step": "competitor_verified", "version": competing[0]["version"]})
        # The original tag is also sent on the final session PUT. Acceptance
        # with replaced bytes disproves using initiation alone as the CAS gate.
        code, _, _ = self.drive.request(
            "PUT", session, b"stale-session-end",
            "application/octet-stream", initial[1],
        )
        after = self.settled_snapshot()
        self.record({"step": "stale_session_result", "status": code,
                     "competitor_bytes_preserved": after[2] == competitor,
                     "same_snapshot": after == competing})
        if code != 412 or after != competing:
            raise ProbeError("stale_session_not_safely_rejected")
        # A rejected stale completion is meaningful only if an equivalent
        # session with a current tag can actually publish bytes.
        current_session = self.begin_session(after[1])
        expected = b"current-session-x"
        code, _, _ = self.drive.request(
            "PUT", current_session, expected, "application/octet-stream", after[1],
        )
        installed = self.settled_snapshot()
        if (code != 200 or installed[2] != expected or installed[1] == after[1]
                or int(installed[0]["version"]) <= int(after[0]["version"])):
            raise ProbeError("current_session_effect_not_verified")
        self.record({"step": "current_session_verified", "status": code,
                     "version": installed[0]["version"]})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--run-real", action="store_true",
                        help="explicitly authorize creating disposable files in the test account")
    parser.add_argument("--evidence-dir", required=True,
                        help="new absolute directory outside any synchronized root")
    parser.add_argument("--token-stdin", action="store_true",
                        help="read the token from a private parent-process pipe instead of a terminal")
    parser.add_argument("--write-api", choices=("v3", "v2"), default="v3",
                        help="explicit endpoint experiment; never a production fallback")
    parser.add_argument("--kind", choices=("all", "name", "media", "resumable"), default="all",
                        help="select the independent endpoint experiment")
    args = parser.parse_args()
    if not args.run_real:
        parser.error("--run-real is required; use only a dedicated test account")
    directory = Path(args.evidence_dir)
    if not directory.is_absolute():
        parser.error("evidence directory must be absolute and new")
    # Exclusive creation prevents overwriting any previous evidence.
    directory.mkdir(mode=0o700, parents=False, exist_ok=False)
    path = directory / "evidence.jsonl"
    with path.open("x", encoding="utf-8") as output:
        def record(value):
            output.write(json.dumps(value, sort_keys=True) + "\n")
            output.flush()
            os.fsync(output.fileno())

        record({"step": "started", "experiment": "drive-v3-direct-if-match-v1",
                "write_api": args.write_api,
                "kind": args.kind,
                "production_writes_enabled": False})
        try:
            # No token in arguments, shell history, environment, evidence or errors.
            if args.token_stdin:
                if sys.stdin.isatty():
                    raise ProbeError("token_pipe_required")
                token = sys.stdin.readline(16385).removesuffix("\n")
                if len(token) > 16384:
                    raise ProbeError("invalid_token")
            else:
                token = getpass.getpass("Short-lived access token for the dedicated test account: ")
            drive = Drive(token)
            if args.kind == "resumable":
                Probe(drive, record, args.write_api).run_resumable_race()
            else:
                kinds = (False, True) if args.kind == "all" else (args.kind == "media",)
                for media in kinds:
                    Probe(drive, record, args.write_api).run(media)
        except (ProbeError, EOFError, KeyboardInterrupt) as error:
            reason = str(error) if isinstance(error, ProbeError) else "interrupted"
            record({"step": "incomplete", "reason": reason,
                    "production_writes_enabled": False})
            print("Probe incomplete; inspect private evidence. Fixtures are retained.")
            return 1
        record({"step": "selected_preconditions_observed", "kind": args.kind,
                "production_writes_enabled": False,
                "full_executor_validated": False})
    print("Selected preconditions observed. Production executor remains unvalidated; fixtures retained.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
