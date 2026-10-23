#!/usr/bin/env python3
"""Готовит opaque официальный клиент26.2/26.3 для probe_registry_client.py (Linux).

Скачивает клиент, библиотеки и ресурсы по официальному manifest с SHA1.
Не открывает JAR, mappings, код или датапаки. Не читает аккаунты лаунчера.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
import shutil
import time
import urllib.error
from pathlib import Path
import urllib.request


def data(url):
    for attempt in range(3):
        try:
            with urllib.request.urlopen(url, timeout=60) as response:
                return response.read()
        except (TimeoutError, urllib.error.URLError):
            if attempt == 2:
                raise
            time.sleep(1 + attempt)
    raise AssertionError('unreachable')


REUSE_ROOT = None
OUTPUT_ROOT = None
CLIENT_SHA1 = {"1.21.10": "d3bdf582a7fa723ce199f3665588dcfe6bf9aca8", "1.21.11": "ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd", "26.2": "2dc72797acbc1b63fc16a11c4ac393605f453754",
               "26.3": "e877b6a07acd633fb3bb475002175cec036e7b87"}


def output_file(path):
    """Не читать/перезаписывать ссылки и пути вне выбранного собственного cache."""
    if OUTPUT_ROOT is None or path.is_symlink():
        raise ValueError('Output cache file must not be a symlink')
    if not path.parent.resolve().is_relative_to(OUTPUT_ROOT):
        raise ValueError('Output cache path escapes its root')
    return path


def verify_file(path, expected, size):
    path = output_file(path)
    with path.open('rb') as source:
        actual = hashlib.file_digest(source, 'sha1').hexdigest()
    assert actual == expected and path.stat().st_size == size, str(path)
    return actual


def fetch(url, path, sha):
    path = output_file(path)
    if path.exists() and hashlib.sha1(path.read_bytes()).hexdigest() == sha:
        return
    if REUSE_ROOT is not None:
        cached = REUSE_ROOT / path.relative_to(OUTPUT_ROOT)
        if not cached.is_symlink() and cached.resolve().is_relative_to(REUSE_ROOT) and cached.is_file():
            with cached.open('rb') as source:
                matched = hashlib.file_digest(source, 'sha1').hexdigest() == sha
            if matched:
                path.parent.mkdir(parents=True, exist_ok=True)
                output_file(path)
                shutil.copyfile(cached, path)
                return
    raw = data(url)
    assert hashlib.sha1(raw).hexdigest() == sha, 'download checksum mismatch'
    path.parent.mkdir(parents=True, exist_ok=True)
    output_file(path).write_bytes(raw)


def batch(jobs):
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        list(pool.map(lambda job: fetch(*job), dict.fromkeys(jobs)))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--version', choices=tuple(CLIENT_SHA1), default='26.3')
    parser.add_argument('--metadata', type=Path)
    parser.add_argument('--metadata-sha1')
    parser.add_argument('--reuse-cache', type=Path, help='Только наш прежний opaque cache; копии с SHA1 проверкой')
    args = parser.parse_args()
    if args.output.is_symlink():
        raise ValueError('Output cache root must not be a symlink')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=True)
    global REUSE_ROOT, OUTPUT_ROOT
    OUTPUT_ROOT = root
    REUSE_ROOT = args.reuse_cache.resolve() if args.reuse_cache else None
    assert REUSE_ROOT != root
    os.sched_setaffinity(0, sorted(os.sched_getaffinity(0))[:2])
    if args.metadata:
        assert args.metadata_sha1, 'Saved metadata requires its official manifest SHA1'
        raw = args.metadata.read_bytes()
        expected_metadata_sha1 = args.metadata_sha1
    else:
        assert not args.metadata_sha1
        manifest = json.loads(data('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'))
        version = next(v for v in manifest['versions'] if v['id'] == args.version)
        raw = data(version['url'])
        expected_metadata_sha1 = version['sha1']
    assert hashlib.sha1(raw).hexdigest() == expected_metadata_sha1
    metadata = json.loads(raw)
    assert metadata['id'] == args.version
    assert metadata['javaVersion']['majorVersion'] == (21 if args.version in ('1.21.10','1.21.11') else 25)
    output_file(root / 'version.json').write_bytes(raw)
    client = metadata['downloads']['client']
    assert client['sha1'] == CLIENT_SHA1[args.version]
    fetch(client['url'], root / 'client.jar', client['sha1'])
    classpath, jobs = [], []
    verified_files = [(root / 'client.jar', client['sha1'], client['size'])]
    for library in metadata['libraries']:
        allowed = 'rules' not in library
        for rule in library.get('rules', []):
            if rule.get('os', {}).get('name', 'linux') == 'linux':
                allowed = rule['action'] == 'allow'
        artifact = library.get('downloads', {}).get('artifact')
        if allowed and artifact:
            path = root / 'libraries' / artifact['path']
            jobs.append((artifact['url'], path, artifact['sha1']))
            classpath.append(str(path))
            verified_files.append((path, artifact['sha1'], artifact['size']))
    batch(jobs)
    index = metadata['assetIndex']
    index_path = root / 'assets/indexes' / (index['id'] + '.json')
    fetch(index['url'], index_path, index['sha1'])
    verified_files.append((index_path, index['sha1'], index['size']))
    jobs = []
    for obj in json.loads(output_file(index_path).read_text())['objects'].values():
        digest = obj['hash']
        suffix = digest[:2] + '/' + digest
        jobs.append(('https://resources.download.minecraft.net/' + suffix,
                     root / 'assets/objects' / suffix, digest))
        verified_files.append((root / 'assets/objects' / suffix, digest, obj['size']))
    batch(jobs)
    output_file(root / 'classpath.txt').write_text(os.pathsep.join(classpath + [str(root / 'client.jar')]))
    checked = []
    for path, expected, size in dict.fromkeys(verified_files):
        actual = verify_file(path, expected, size)
        checked.append({'file': str(path.relative_to(root)), 'sha1': actual, 'size': size})
    summary = {'version': args.version, 'metadata_sha1': expected_metadata_sha1,
               'metadata_sha256': hashlib.sha256(raw).hexdigest(),
               'client_sha1': client['sha1'], 'libraries': len(classpath),
               'assets': len(jobs), 'unique_verified_files': len(checked),
               'asset_index': index['id'], 'executed': False, 'jar_unpacked': False,
               'reuse_cache': str(REUSE_ROOT) if REUSE_ROOT else None, 'files': checked}
    output_file(root / 'download-verification.json').write_text(json.dumps(summary, indent=2) + '\n')
    output_file(root / 'SHA1SUMS').write_text(''.join(f"{row['sha1']}  {row['file']}\n" for row in checked))
    print(json.dumps({k: v for k, v in summary.items() if k != 'files'}))


if __name__ == '__main__':
    main()
