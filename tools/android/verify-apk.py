"""Проверяет только наш собранный APK: ABI, точные байты runtime и наличие classes."""
import hashlib
import json
from pathlib import Path
import sys
import zipfile


def main(path, abi):
    repo = Path(__file__).resolve().parents[2]
    pin = json.loads((repo / 'android/runtime-pin.json').read_text())
    with zipfile.ZipFile(path) as archive:
        names = archive.namelist()
        assert len(names) == len(set(names)) and 'AndroidManifest.xml' in names and 'classes.dex' in names
        native = [name for name in names if name.startswith('lib/') and not name.endswith('/')]
        assert native == ['lib/' + abi + '/libmcsa.so'], native
        binary = archive.read(native[0])
        assert len(binary) == pin['runtimes'][abi]['bytes']
        assert hashlib.sha256(binary).hexdigest() == pin['runtimes'][abi]['sha256']
    result = {'apk': path.name, 'bytes': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'abi': abi, 'runtime_sha256': pin['runtimes'][abi]['sha256'], 'native_run': pin['run_id'], 'native_head': pin['head']}
    path.with_suffix('.receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main(Path(sys.argv[1]), sys.argv[2])
