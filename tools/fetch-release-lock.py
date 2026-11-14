"""Проверить закреплённый Cargo.lock для воспроизводимой сборки выпуска."""
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time
import tomllib


def install(root):
    expected = (root / '.github/release-lock.sha256').read_text(encoding='utf-8').strip()
    if not re.fullmatch('[0-9a-f]{64}', expected):
        raise RuntimeError('Invalid release lock SHA256 pin')
    destination = root / 'Cargo.lock'
    if destination.exists():
        raise RuntimeError('Release checkout must not already contain Cargo.lock')
    bundled = root / '.github/release.lock'
    if bundled.exists():
        verify_and_copy(root, bundled, destination, expected)
    else:
        # Совместимость со старыми тегами, где lock публиковался отдельно.
        tag = os.environ['GITHUB_REF_NAME']
        if not re.fullmatch(r'v[0-9]+\.[0-9]+\.[0-9]+-alpha', tag):
            raise RuntimeError('Invalid release tag')
        with tempfile.TemporaryDirectory(prefix='mcsa-release-lock-') as directory:
            for attempt in range(60):
                result = subprocess.run(['gh', 'release', 'download', tag, '--repo',
                    'AnyaSheriff/MCSheriffAnya', '--pattern', 'Cargo.lock', '--dir', directory],
                    capture_output=True)
                if result.returncode == 0:
                    break
                print('Waiting for the pinned release lock...', flush=True)
                time.sleep(5)
            else:
                raise RuntimeError('Pinned release Cargo.lock is unavailable')
            verify_and_copy(root, Path(directory) / 'Cargo.lock', destination, expected)
    print('PASS: exact locally verified release Cargo.lock')


def verify_and_copy(root, lock, destination, expected):
    data = lock.read_bytes()
    if hashlib.sha256(data).hexdigest() != expected:
        raise RuntimeError('Release lock SHA256 mismatch')
    manifest = tomllib.loads((root / 'Cargo.toml').read_text(encoding='utf-8'))['package']
    packages = tomllib.loads(data.decode('utf-8'))['package']
    own = [package for package in packages if package['name'] == manifest['name']]
    if len(own) != 1 or own[0]['version'] != manifest['version']:
        raise RuntimeError('Release lock package version differs from Cargo.toml')
    tag = os.environ.get('GITHUB_REF_NAME', '')
    if os.environ.get('GITHUB_REF_TYPE') == 'tag':
        if not re.fullmatch(r'v[0-9]+\.[0-9]+\.[0-9]+-alpha', tag) or tag != 'v' + manifest['version']:
            raise RuntimeError('Release tag differs from Cargo.toml')
    shutil.copyfile(lock, destination)


if __name__ == '__main__':
    install(Path(__file__).resolve().parents[1])
