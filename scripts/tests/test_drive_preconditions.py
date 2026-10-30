"""Deterministic oracles for the opt-in Drive experiment; no Google access."""
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch


spec = importlib.util.spec_from_file_location(
    "probe", Path(__file__).resolve().parents[1] / "check-drive-preconditions.py"
)
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class SimulatedDrive:
    """Model current/stale preconditions independently of the probe oracles."""

    def __init__(self, behavior="conditional"):
        self.behavior = behavior
        self.item = None
        self.content = b"baseline"
        self.version = 1
        self.calls = []

    def request(self, method, path, body=None, content_type=None, etag=None, resumable=False):
        self.calls.append((method, path, etag))
        current = '"revision-' + str(self.version) + '"'
        if method == "POST":
            metadata = json.loads(body.split(b"\r\n\r\n")[1].split(b"\r\n--")[0])
            self.item = dict(metadata, id="created-id", version="1", trashed=False)
            return 200, current, b'{"id":"created-id"}'
        if resumable:
            return 200, "https://www.googleapis.com/upload/drive/v2/files/created-id?upload_id=private", b""
        if "upload_id=" in path:
            if self.behavior == "rejects_every_session":
                return 412, current, b""
            if self.behavior == "ignores_session_precondition" or etag == current:
                self.content = body
                self.version += 1
                self.item["version"] = str(self.version)
                return 200, current, b"{}"
            return 412, current, b""
        if method in ("PATCH", "PUT"):
            if etag != current and self.behavior != "ignores_header":
                if self.behavior == "mutates_on_412":
                    self.content = b"corrupted"
                return 412, current, b"provider body must never be logged"
            if self.behavior == "rejects_every_write":
                return 412, current, b""
            if self.behavior != "acknowledges_without_effect":
                if "uploadType=media" in path:
                    self.content = body
                else:
                    value = json.loads(body)
                    self.item["name"] = value.get("name", value.get("title"))
                self.version += 1
                self.item["version"] = str(self.version)
            return 200, current, b"{}"
        if "alt=media" in path:
            return 200, None, self.content
        if self.behavior == "missing_etag":
            current = None
        if self.behavior == "weak_etag":
            current = "W/" + current
        item = dict(self.item)
        if self.behavior == "foreign_identity":
            item["id"] = "someone-elses-file"
        if self.behavior == "racing_read":
            self.version += 1
            item["version"] = str(self.version)
        if "/drive/v2/" in path:
            item = {"id": item["id"], "title": item["name"],
                    "version": item["version"], "etag": current}
        return 200, current, json.dumps(item).encode()


class ProbeTests(unittest.TestCase):
    def run_probe(self, behavior="conditional", media=False, write_api="v3"):
        drive = SimulatedDrive(behavior)
        records = []
        instance = probe.Probe(drive, records.append, write_api)
        instance.run(media)
        return drive, records

    def test_current_and_repeated_stale_writes_for_both_endpoints(self):
        for media in (False, True):
            with self.subTest(media=media):
                drive, records = self.run_probe(media=media)
                self.assertEqual(8, len(records))
                self.assertEqual(6, sum(call[0] == "PATCH" for call in drive.calls))
                self.assertEqual(3, drive.version)
                self.assertEqual("created", records[0]["step"])
                self.assertNotIn("revision-", json.dumps(records))

    def test_fail_closed_oracles(self):
        cases = {
            "missing_etag": "strong_etag_unavailable",
            "weak_etag": "strong_etag_unavailable",
            "foreign_identity": "fixture_identity_or_version_changed",
            "racing_read": "unstable_snapshot",
            "ignores_header": "stale_write_not_rejected",
            "mutates_on_412": "stale_write_changed_fixture",
            "rejects_every_write": "current_precondition_not_accepted",
            "acknowledges_without_effect": "current_write_effect_not_verified",
        }
        for behavior, reason in cases.items():
            for media in (False, True):
                with self.subTest(behavior=behavior, media=media):
                    with self.assertRaisesRegex(probe.ProbeError, "^" + reason + "$"):
                        self.run_probe(behavior, media)

    def test_v2_mode_is_explicit_and_checks_both_write_endpoints(self):
        for media in (False, True):
            drive, records = self.run_probe(media=media, write_api="v2")
            writes = [call for call in drive.calls if call[0] in ("PUT", "PATCH")]
            self.assertEqual(6, len(writes))
            self.assertTrue(all("/v2/" in call[1] for call in writes))
            self.assertTrue(all(call[0] == ("PUT" if media else "PATCH") for call in writes))
            self.assertEqual(8, len(records))
            for behavior, reason in (("ignores_header", "stale_write_not_rejected"),
                                     ("mutates_on_412", "stale_write_changed_fixture")):
                with self.subTest(media=media, behavior=behavior):
                    with self.assertRaisesRegex(probe.ProbeError, reason):
                        self.run_probe(behavior, media, "v2")

    def test_resumable_publication_race_requires_rejection_without_effect(self):
        records = []
        drive = SimulatedDrive()
        instance = probe.Probe(drive, records.append, "v2")
        instance.run_resumable_race()
        self.assertEqual(b"current-session-x", drive.content)
        self.assertTrue(records[-2]["competitor_bytes_preserved"])
        self.assertEqual("current_session_verified", records[-1]["step"])
        self.assertNotIn("upload_id", json.dumps(records))
        records = []
        drive = SimulatedDrive("ignores_session_precondition")
        instance = probe.Probe(drive, records.append, "v2")
        with self.assertRaisesRegex(probe.ProbeError, "^stale_session_not_safely_rejected$"):
            instance.run_resumable_race()
        self.assertFalse(records[-1]["competitor_bytes_preserved"])
        drive = SimulatedDrive("rejects_every_session")
        instance = probe.Probe(drive, lambda _: None, "v2")
        with self.assertRaisesRegex(probe.ProbeError, "^current_session_effect_not_verified$"):
            instance.run_resumable_race()

    def test_live_writes_require_explicit_opt_in(self):
        with patch("sys.argv", ["probe", "--evidence-dir", "/unused"]):
            with patch.object(probe.Path, "mkdir") as mkdir:
                with patch("sys.stderr"), self.assertRaises(SystemExit) as error:
                    probe.main()
                self.assertEqual(2, error.exception.code)
                mkdir.assert_not_called()

    def test_invalid_entity_tags(self):
        for value in (None, "", "*", 'W/"weak"', '"a", "b"', '"a\r\nb"', '""'):
            with self.subTest(value=value), self.assertRaises(probe.ProbeError):
                probe.strong_etag(value)
        self.assertEqual('"opaque"', probe.strong_etag('"opaque"'))

    def test_no_mutation_before_creation(self):
        drive = SimulatedDrive()
        instance = probe.Probe(drive, lambda _: None)
        with self.assertRaises(probe.ProbeError):
            instance.endpoint()
        self.assertEqual([], drive.calls)

    def test_transport_sanitizes_errors_and_does_not_retry(self):
        drive = probe.Drive("test-secret")
        with patch.object(drive.opener, "open", side_effect=OSError("test-secret")) as call:
            with self.assertRaisesRegex(probe.ProbeError, "^transport_unavailable_or_effect_unknown$"):
                drive.request("PATCH", "/drive/v3/files/created-id", b"{}")
            self.assertEqual(1, call.call_count)

    def test_redirect_is_never_followed(self):
        self.assertIsNone(probe.NoRedirect().redirect_request(
            None, None, 307, "redirect", {}, "https://other.example/"
        ))

    def test_refuses_unexpected_endpoints(self):
        drive = probe.Drive("test-secret")
        with self.assertRaisesRegex(probe.ProbeError, "^invalid_endpoint$"):
            drive.request("GET", "https://other.example/")

    def test_response_limit(self):
        drive = probe.Drive("test-secret")
        class Response:
            def __enter__(self):
                return self
            def __exit__(self, *args):
                pass
            def read(self, size):
                return b"x" * size
        with patch.object(drive.opener, "open", return_value=Response()):
            with self.assertRaisesRegex(probe.ProbeError, "^response_limit$"):
                drive.request("GET", "/drive/v3/files/created-id")

    def test_invalid_response(self):
        for data in (b"not-json", b"[]", b"null", b"\xff"):
            with self.subTest(data=data), self.assertRaises(probe.ProbeError):
                probe.decode(data)


if __name__ == "__main__":
    unittest.main()
