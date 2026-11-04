#!/usr/bin/env python3
"""Только разрешённые JSON: официальные версии и таблицы minecraft-data."""
import hashlib
import json
import urllib.request
from datetime import datetime, timezone

SOURCES = {
    'mojang_versions': 'https://piston-meta.mojang.com/mc/game/version_manifest_v2.json',
    'protocol_versions': 'https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/pc/common/protocolVersions.json',
    'data_paths': 'https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/dataPaths.json',
}


def main():
    documents, evidence = {}, {}
    for name, url in SOURCES.items():
        with urllib.request.urlopen(url, timeout=30) as response:
            content = response.read(8 * 1024 * 1024 + 1)
        if len(content) > 8 * 1024 * 1024:
            raise ValueError('oversized JSON: ' + name)
        documents[name] = json.loads(content)
        evidence[name] = {'url': url, 'sha256': hashlib.sha256(content).hexdigest()}
    versions = []
    for target in ('26.1.2', '26.2', '26.3'):
        official = [v for v in documents['mojang_versions']['versions'] if v['id'] == target]
        records = [v for v in documents['protocol_versions'] if v['minecraftVersion'] == target]
        if len(official) != 1 or len(records) != 1:
            raise ValueError('ambiguous/missing version: ' + target)
        record = records[0]
        available = documents['data_paths']['pc'].get(target)
        versions.append({'version': target, 'type': official[0]['type'],
                         'released': official[0]['releaseTime'], 'protocol': record['version'],
                         'data_version': record.get('dataVersion'),
                         'exact_data_paths': available,
                         'adapter_enabled': False})
    # Эта таблица никогда не включает поддержку автоматически.
    print(json.dumps({'checked_at': datetime.now(timezone.utc).isoformat(),
                      'latest_release': documents['mojang_versions']['latest']['release'],
                      'native_core': '26.1.2', 'sources': evidence, 'versions': versions},
                     ensure_ascii=False, indent=2))


if __name__ == '__main__':
    main()
