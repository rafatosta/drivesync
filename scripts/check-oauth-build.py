#!/usr/bin/env python3
"""Offline build-configuration checks; fixtures never contact Google or the vault."""
import json
import os
from pathlib import Path
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
env = os.environ.copy()
for name in ('DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE', 'DRIVESYNC_REQUIRE_GOOGLE_OAUTH'):
    env.pop(name, None)


def check(settings, succeeds):
    result = subprocess.run(
        ['cargo', 'check', '-p', 'drivesync-engine', '--offline', '--locked'],
        cwd=root, env={**env, **settings}, capture_output=True, text=True,
    )
    if (result.returncode == 0) != succeeds:
        raise AssertionError('Unexpected build configuration result (output withheld)')


with tempfile.TemporaryDirectory(prefix='drivesync-build-test-') as directory:
    path = Path(directory) / 'desktop.json'
    # Synthetic test ID: cannot authorize a Google user.
    path.write_text(json.dumps({'installed': {
        'client_id': 'build-test.apps.googleusercontent.com',
        'client_secret': 'public-test-compatibility',
        'refresh_token': 'must-not-be-embedded',
    }}))
    settings = {'DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE': str(path),
                'DRIVESYNC_REQUIRE_GOOGLE_OAUTH': '1'}
    check(settings, True)
    path.write_text('{"web":{}}')
    check(settings, False)  # Cargo must notice an edited input file.
    path.unlink()
    check(settings, False)  # Explicit missing inputs fail even in development.

# Without a staged official file: development works, publication fails.
if not (root / 'data/google-oauth-client.json').exists():
    check({}, True)
    check({'DRIVESYNC_REQUIRE_GOOGLE_OAUTH': '1'}, False)
    check({}, True)  # Restore the ordinary unconfigured development build.
print('OAuth build configuration checks passed (no Google login performed).')
