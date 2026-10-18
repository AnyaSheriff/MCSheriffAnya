#!/usr/bin/env python3
"""Готовит opaque официальный клиент26.3 для probe_registry_client.py (Linux).

Скачивает клиент, библиотеки и ресурсы по официальному manifest с SHA1.
Не открывает JAR, mappings, код или датапаки. Не читает аккаунты лаунчера.
"""
import argparse
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import urllib.request


def data(url):
    with urllib.request.urlopen(url, timeout=60) as response:
        return response.read()


def fetch(url, path, sha):
    if path.exists() and hashlib.sha1(path.read_bytes()).hexdigest() == sha:
        return
    raw = data(url)
    assert hashlib.sha1(raw).hexdigest() == sha, 'download checksum mismatch'
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.is_symlink():
        path.unlink()  # Не перезаписывать ресурс другого лаунчера через ссылку кэша.
    path.write_bytes(raw)


def batch(jobs):
    with concurrent.futures.ThreadPoolExecutor(max_workers=6) as pool:
        list(pool.map(lambda job: fetch(*job), jobs))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(data('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'))
    version = next(v for v in manifest['versions'] if v['id'] == '26.3')
    raw = data(version['url'])
    assert hashlib.sha1(raw).hexdigest() == version['sha1']
    metadata = json.loads(raw)
    (root / 'version.json').write_bytes(raw)
    client = metadata['downloads']['client']
    assert client['sha1'] == 'e877b6a07acd633fb3bb475002175cec036e7b87'
    fetch(client['url'], root / 'client.jar', client['sha1'])
    classpath, jobs = [], []
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
    batch(jobs)
    index = metadata['assetIndex']
    index_path = root / 'assets/indexes' / (index['id'] + '.json')
    fetch(index['url'], index_path, index['sha1'])
    jobs = []
    for obj in json.loads(index_path.read_text())['objects'].values():
        digest = obj['hash']
        suffix = digest[:2] + '/' + digest
        jobs.append(('https://resources.download.minecraft.net/' + suffix,
                     root / 'assets/objects' / suffix, digest))
    batch(jobs)
    (root / 'classpath.txt').write_text(os.pathsep.join(classpath + [str(root / 'client.jar')]))
    print(json.dumps({'client_sha1': client['sha1'], 'libraries': len(classpath), 'assets': len(jobs)}))


if __name__ == '__main__':
    main()
