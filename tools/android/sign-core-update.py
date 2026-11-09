"""Локальная подпись проверенных собственных native artifacts. Ключ не копируется."""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[2]
ABIS = ('armeabi-v7a', 'arm64-v8a', 'x86_64')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('native', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--revision', type=int, help='Defaults to the highest verified candidate in the pin')
    parser.add_argument('--tag', help='Defaults to v + the verified pin version')
    parser.add_argument('--notes', required=True)
    args = parser.parse_args()
    pin = json.loads((ROOT / 'android/runtime-pin.json').read_text())
    if args.tag is None:
        args.tag = 'v' + pin['version']
    if args.revision is None:
        args.revision = max(map(int, pin['updates']))
    receipt = json.loads((args.native / 'receipt.json').read_text())
    assert pin['format'] == receipt['format'] == 2
    assert receipt['source_head'] == pin['head']
    assert receipt['updates'] == pin['updates'] and receipt['runtimes'] == pin['runtimes']
    update = pin['updates'][str(args.revision)]
    assert args.revision > pin['core_revision'] and args.tag.replace('.', '').replace('-', '').isalnum()
    assert 0 < len(args.notes) <= 2000
    # Содержимое приватного ключа не читается Python и не выводится. Только OpenSSL sign.
    key = Path.home() / '.config/mcsheriffanya/android-ota/signing-key.pem'
    assert key.is_file() and key.stat().st_mode & 0o077 == 0
    public = ROOT / 'android/app/src/main/res/raw/core_update_public_key.der'
    actual_public = subprocess.check_output(['openssl', 'pkey', '-in', str(key), '-pubout', '-outform', 'DER'])
    assert actual_public == public.read_bytes(), 'Signing key does not match embedded public key'
    args.output.mkdir(parents=True, exist_ok=False)
    packages = {}
    for abi in ABIS:
        binary = (args.native / 'updates' / str(args.revision) / abi / 'libmcsa_core.so').read_bytes()
        expected = update['runtimes'][abi]
        digest = hashlib.sha256(binary).hexdigest()
        assert len(binary) == expected['bytes'] and digest == expected['sha256']
        name = f'mcsa-android-core-r{args.revision}-{pin["head"][:12]}-{digest[:12]}-{abi}.so'
        (args.output / name).write_bytes(binary)
        packages[abi] = {'url': f'https://github.com/AnyaSheriff/MCSheriffAnya/releases/download/{args.tag}/{name}', 'bytes': len(binary), 'sha256': digest}
    payload = {'schema': 1, 'channel': 'preview', 'game': '26.3', 'data_compat': 1,
               'revision': args.revision, 'min_app': 4, 'host_api': 1,
               'version': f'{pin["version"]} · Android #{args.revision}',
               'source': pin['head'], 'build_id': update['build_id'], 'notes': args.notes, 'packages': packages}
    raw = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(',', ':')).encode()
    with tempfile.TemporaryDirectory(prefix='mcsa-sign-') as temporary:
        directory = Path(temporary)
        (directory / 'payload').write_bytes(raw)
        signature = subprocess.check_output(['openssl', 'dgst', '-sha256', '-sign', str(key), str(directory / 'payload')])
        (directory / 'signature').write_bytes(signature)
        subprocess.run(['openssl', 'pkey', '-pubin', '-inform', 'DER', '-in', str(public), '-out', str(directory / 'public.pem')], check=True)
        subprocess.run(['openssl', 'dgst', '-sha256', '-verify', str(directory / 'public.pem'), '-signature', str(directory / 'signature'), str(directory / 'payload')], check=True)
    envelope = {'payload': base64.b64encode(raw).decode(), 'signature': base64.b64encode(signature).decode()}
    (args.output / 'preview.json').write_text(json.dumps(envelope, indent=2) + '\n')
    (args.output / 'payload-public.json').write_text(json.dumps(payload, ensure_ascii=False, indent=2) + '\n')
    (args.output / 'SHA256SUMS').write_text(''.join(f'{hashlib.sha256(f.read_bytes()).hexdigest()}  {f.name}\n' for f in sorted(args.output.iterdir()) if f.is_file()))
    print('PASS: three exact native hashes; embedded public key; local ECDSA signature verified')


if __name__ == '__main__':
    main()
