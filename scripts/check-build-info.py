#!/usr/bin/env python3
"""Validate embedded revision generation with isolated, offline Git fixtures."""
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

root = Path(__file__).resolve().parents[1]


def run(args, cwd, **kwargs):
    return subprocess.run(args, cwd=cwd, check=True, capture_output=True,
                          text=True, **kwargs).stdout.strip()


with tempfile.TemporaryDirectory(prefix='drivesync-build-info-') as temporary:
    directory = Path(temporary)
    executable = directory / 'build-info'
    run(['rustc', '--edition=2021', '-D', 'warnings', str(root / 'build.rs'),
         '-o', str(executable)], directory)
    source = directory / 'source'
    source.mkdir()
    env = {key: value for key, value in os.environ.items()
           if not key.startswith('DRIVESYNC_BUILD_REVISION')}
    env['CARGO_PKG_VERSION'] = '0.1.0'

    def check(expected, settings=None, cwd=source):
        output = run([str(executable)], cwd, env={**env, **(settings or {})})
        assert f'cargo:rustc-env=DRIVESYNC_VERSION=0.1.0 ({expected})' in output, output
        return output

    check('unknown')
    check('unknown', {'PATH': ''})
    revision = 'abcdef1234567890abcdef1234567890abcdef12'
    check('abcdef1', {'DRIVESYNC_BUILD_REVISION': revision})
    check('abcdef1-modified', {'DRIVESYNC_BUILD_REVISION': revision + '-modified'})
    invalid = subprocess.run([str(executable)], cwd=source,
                             env={**env, 'DRIVESYNC_BUILD_REVISION': 'invalid'},
                             capture_output=True)
    assert invalid.returncode != 0
    run(['git', 'init', '-q'], source)
    (source / 'tracked').write_text('original')
    run(['git', 'add', 'tracked'], source)
    run(['git', '-c', 'user.name=Fixture', '-c', 'user.email=fixture@example.invalid',
         'commit', '-qm', 'Fixture'], source)
    commit = run(['git', 'rev-parse', 'HEAD'], source)
    output = check(commit[:7])
    assert f'DRIVESYNC_SOURCE_REVISION={commit}' in output
    (source / 'untracked').write_text('not included in tracked-change detection')
    check(commit[:7])
    (source / 'tracked').write_text('modified')
    check(commit[:7] + '-modified')
    run(['git', 'add', 'tracked'], source)
    check(commit[:7] + '-modified')
    nested = source / 'unpacked'
    nested.mkdir()
    check('unknown', cwd=nested)
    copied = directory / 'flatpak-copy'
    shutil.copytree(source, copied)
    check(commit[:7] + '-modified', cwd=copied)
print('Build identification checks passed (offline fixtures).')
