"""Получить закреплённый локально проверенный Cargo.lock для сборки тега в CI."""
import hashlib
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile
import time

root = Path(__file__).resolve().parents[1]
tag = os.environ['GITHUB_REF_NAME']
assert re.fullmatch(r'v[0-9]+\.[0-9]+\.[0-9]+-alpha', tag)
expected = (root / '.github/release-lock.sha256').read_text().strip()
assert re.fullmatch('[0-9a-f]{64}', expected)
assert not (root / 'Cargo.lock').exists(), 'Tagged checkout must be clean'
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
    lock = Path(directory) / 'Cargo.lock'
    assert hashlib.sha256(lock.read_bytes()).hexdigest() == expected, 'Release lock SHA256 mismatch'
    shutil.copy2(lock, root / 'Cargo.lock')
print('PASS: exact locally verified release Cargo.lock')
