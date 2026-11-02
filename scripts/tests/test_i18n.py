"""Check the gettext catalog and locale fallback without starting the application."""

import os
from pathlib import Path
import subprocess
import tempfile
import unittest


ROOT = Path(__file__).resolve().parents[2]
CATALOG = ROOT / "po" / "pt_BR.po"


class InternationalizationTests(unittest.TestCase):
    def test_catalog_covers_marked_messages(self):
        sources = sorted(str(path.relative_to(ROOT)) for path in (ROOT / "src").rglob("*.rs"))
        with tempfile.TemporaryDirectory() as directory:
            template = Path(directory) / "messages.pot"
            subprocess.run(
                ["xgettext", "--language=Rust", "--from-code=UTF-8", "--keyword=gettext",
                 "--output", str(template), *sources],
                cwd=ROOT, check=True,
            )
            subprocess.run(["msgcmp", str(CATALOG), str(template)], check=True)

    def test_compiled_catalog_and_english_fallback(self):
        with tempfile.TemporaryDirectory() as directory:
            destination = Path(directory) / "pt_BR" / "LC_MESSAGES" / "drivesync.mo"
            destination.parent.mkdir(parents=True)
            subprocess.run(["msgfmt", "--check", "-o", str(destination), str(CATALOG)],
                           check=True)
            env = os.environ.copy()
            env.update(LC_ALL="pt_BR.UTF-8", TEXTDOMAINDIR=directory)
            for language, expected in (
                ("pt_BR", "Boas-vindas ao DriveSync"),
                ("en", "Welcome to DriveSync"),
                ("fr", "Welcome to DriveSync"),
            ):
                env["LANGUAGE"] = language
                result = subprocess.run(
                    ["gettext", "drivesync", "Welcome to DriveSync"],
                    env=env, check=True, capture_output=True, text=True,
                )
                self.assertEqual(result.stdout, expected)


if __name__ == "__main__":
    unittest.main()
