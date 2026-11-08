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
        expected={'libmcsa.so':pin['runtimes'][abi]['host'],'libmcsa_core.so':pin['runtimes'][abi]['core']} if pin.get('format')==2 else {'libmcsa.so':pin['runtimes'][abi]}
        assert set(native)=={'lib/'+abi+'/'+name for name in expected},native
        for name,info in expected.items():
            binary=archive.read('lib/'+abi+'/'+name)
            assert len(binary)==info['bytes'] and hashlib.sha256(binary).hexdigest()==info['sha256']
    result = {'apk': path.name, 'bytes': path.stat().st_size, 'sha256': hashlib.sha256(path.read_bytes()).hexdigest(), 'abi': abi, 'runtime_hashes':{name:info['sha256'] for name,info in expected.items()}, 'native_run': pin['run_id'], 'native_head': pin['head']}
    path.with_suffix('.receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result))


if __name__ == '__main__':
    main(Path(sys.argv[1]), sys.argv[2])
