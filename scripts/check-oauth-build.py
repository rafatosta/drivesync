#!/usr/bin/env python3
"""Offline build-configuration checks; fixtures never contact Google or the vault."""
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]
env = os.environ.copy()
for name in ('DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE', 'DRIVESYNC_REQUIRE_GOOGLE_OAUTH'):
    env.pop(name, None)


def check(settings, succeeds):
    result = subprocess.run(
        ['cargo', 'check', '-p', 'drivesync-engine', '--offline', '--locked'],
        cwd=workspace, env={**env, **settings}, capture_output=True, text=True,
    )
    if (result.returncode == 0) != succeeds:
        raise AssertionError(f'Unexpected fixture build result:\n{result.stderr}')


with tempfile.TemporaryDirectory(prefix='drivesync-build-test-') as directory:
    # Isolate missing/default-file tests from the maintainer's real metadata.
    workspace = Path(directory)
    shutil.copy2(root / 'Cargo.toml', workspace / 'Cargo.toml')
    shutil.copytree(root / 'src', workspace / 'src')
    shutil.copy2(root / 'Cargo.lock', workspace / 'Cargo.lock')
    shutil.copytree(root / 'engine', workspace / 'engine',
                    ignore=shutil.ignore_patterns('target'))
    env['CARGO_TARGET_DIR'] = str(root / 'target' / 'oauth-build-checks')
    path = workspace / 'data/oauth/development.json'
    path.parent.mkdir(parents=True)
    check({}, True)
    check({'DRIVESYNC_REQUIRE_GOOGLE_OAUTH': '1'}, False)
    # Synthetic test ID: cannot authorize a Google user.
    client = json.dumps({'installed': {
        'client_id': 'build-test.apps.googleusercontent.com',
        'client_secret': 'public-test-compatibility',
        'refresh_token': 'must-not-be-embedded',
    }})
    path.write_text(client)
    check({'DRIVESYNC_REQUIRE_GOOGLE_OAUTH': '1'}, True)
    settings = {'DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE': 'data/oauth/development.json',
                'DRIVESYNC_REQUIRE_GOOGLE_OAUTH': '1'}
    check(settings, True)  # Manifest paths resolve from workspace, not engine/.
    check({**settings, 'DRIVESYNC_BUILD_GOOGLE_OAUTH_FILE': str(path)}, True)
    embedded = list(Path(env['CARGO_TARGET_DIR']).glob(
        'debug/build/drivesync-engine-*/out/google-oauth.json'))
    assert embedded and any(p.read_text() for p in embedded)
    assert all('must-not-be-embedded' not in p.read_text() for p in embedded)
    path.write_text('{"web":{}}')
    check(settings, False)  # Cargo must notice an edited input file.
    path.unlink()
    check(settings, False)  # Explicit missing inputs fail even in development.
    check({}, True)
    check({'DRIVESYNC_REQUIRE_GOOGLE_OAUTH': '1'}, False)
    check({}, True)
print('OAuth build configuration checks passed (no Google login performed).')
