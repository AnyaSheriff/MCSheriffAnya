#!/usr/bin/env python3
"""Скачать только разрешённые JSON по закреплённым URL и проверить SHA-256."""
import argparse
import hashlib
import json
from pathlib import Path
import re
import urllib.request

MANIFEST = Path(__file__).with_name('candidate-sources-26.3.json')
ALLOWED = {'version.json', 'protocol.json', 'blocks.json', 'items.json', 'entities.json',
           'foods.json', 'recipes.json', 'blockCollisionShapes.json',
           'protocol-26.1.json', 'entities-26.1.json'}
URL = re.compile(r'https://raw\.githubusercontent\.com/(?:DallasCarraher|PrismarineJS)/minecraft-data/[0-9a-f]{40}/data/pc/(?:26\.1|26\.3)/[A-Za-z]+\.json\Z')


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--directory', type=Path, required=True)
    args = parser.parse_args()
    manifest = json.loads(MANIFEST.read_text())
    records = manifest['files'] + manifest['baseline_files']
    assert {entry['file'] for entry in records} == ALLOWED
    assert len(records) == len(ALLOWED)
    args.directory.mkdir(parents=True, exist_ok=True)
    for entry in records:
        assert URL.fullmatch(entry['url']), 'only pinned permitted JSON URLs'
        assert re.fullmatch('[0-9a-f]{64}', entry['sha256'])
        destination = args.directory / entry['file']
        if destination.exists() and hashlib.sha256(destination.read_bytes()).hexdigest() == entry['sha256']:
            continue
        with urllib.request.urlopen(entry['url'], timeout=30) as response:
            data = response.read(32 * 1024 * 1024 + 1)
        assert len(data) == entry['bytes'] <= 32 * 1024 * 1024
        assert hashlib.sha256(data).hexdigest() == entry['sha256'], entry['file']
        json.loads(data)
        destination.write_bytes(data)
    (args.directory / 'sources.json').write_text(MANIFEST.read_text())
    print(f'Проверены {len(records)} JSON; исходный код не запрашивался.')


if __name__ == '__main__':
    main()
