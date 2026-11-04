#!/usr/bin/env python3
"""Непрозрачный клиент1.12.2; native-библиотеки из Debian, без распаковки JAR."""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path
import tarfile

from prepare_registry_client import batch, data, fetch

PACKAGES = [
    ('pool/main/j/jinput/libjinput-jni_20100502+dfsg-11_amd64.deb',
     'cd94d362fc753620195d6aad72ed5fdb3c1a60ab4503e9210c28d6b9052e5dd1', 'libjinput.so'),
    ('pool/main/l/lwjgl/liblwjgl-java-jni_2.9.3+dfsg-6_amd64.deb',
     '687d27720b6949a05fd10b60632dcd40fdb8c95746bd4ab61f8017a7756e6034', 'liblwjgl.so'),
]


def native_library(root, filename, digest, library):
    url = 'https://deb.debian.org/debian/' + filename
    blob = data(url)
    assert hashlib.sha256(blob).hexdigest() == digest and blob.startswith(b'!<arch>\n')
    offset, archive = 8, None
    while offset < len(blob):
        header = blob[offset:offset+60]
        assert len(header) == 60 and header[-2:] == b'`\n'
        name = header[:16].decode().strip().removesuffix('/')
        size = int(header[48:58])
        assert 0 <= size <= len(blob)-offset-60
        if name in ('data.tar.xz', 'data.tar.gz', 'data.tar.zst'):
            assert archive is None
            archive = blob[offset+60:offset+60+size]
        offset += 60 + size + size % 2
    assert archive is not None
    with tarfile.open(fileobj=io.BytesIO(archive), mode='r:*') as package:
        # Только один готовый ELF-файл. Не извлекаем пути/скрипты пакета,
        # не запускаем установку, исходников и JAR здесь нет.
        matches = [m for m in package.getmembers() if m.name.removeprefix('./') == 'usr/lib/jni/' + library]
        assert len(matches) == 1 and matches[0].isfile() and matches[0].size < 2*1024*1024
        contents = package.extractfile(matches[0]).read()
        assert contents.startswith(b'\x7fELF')
        target = root / 'natives' / library
        target.parent.mkdir(exist_ok=True)
        target.write_bytes(contents)
    return {'url': url, 'sha256': digest, 'library': library, 'library_sha256': hashlib.sha256(contents).hexdigest()}


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--output', type=Path, required=True)
    p.add_argument('--shared-assets', type=Path)
    a = p.parse_args()
    root = a.output.resolve(); root.mkdir(parents=True, exist_ok=True)
    manifest = json.loads(data('https://piston-meta.mojang.com/mc/game/version_manifest_v2.json'))
    version = next(v for v in manifest['versions'] if v['id'] == '1.12.2')
    raw = data(version['url']); assert hashlib.sha1(raw).hexdigest() == version['sha1']
    (root / 'version.json').write_bytes(raw); metadata = json.loads(raw)
    client = metadata['downloads']['client']
    assert client['sha1'] == '0f275bc1547d01fa5f56ba34bdc87d981ee12daf'
    fetch(client['url'], root / 'client.jar', client['sha1'])
    classpath, jobs = [], []
    for library in metadata['libraries']:
        allowed = 'rules' not in library
        for rule in library.get('rules', []):
            if rule.get('os', {}).get('name', 'linux') == 'linux':
                allowed = rule['action'] == 'allow'
        artifact = library.get('downloads', {}).get('artifact')
        if allowed and artifact:
            target = root / 'libraries' / artifact['path']
            jobs.append((artifact['url'], target, artifact['sha1']))
            classpath.append(str(target))
    batch(jobs)
    index = metadata['assetIndex']; target = root / 'assets/indexes' / (index['id'] + '.json')
    fetch(index['url'], target, index['sha1']); jobs = []
    for entry in json.loads(target.read_text())['objects'].values():
        digest = entry['hash']; suffix = digest[:2] + '/' + digest
        target = root / 'assets/objects' / suffix
        if a.shared_assets:
            source = a.shared_assets.resolve() / 'objects' / suffix
            if not target.exists() and source.is_file() and hashlib.sha1(source.read_bytes()).hexdigest() == digest:
                target.parent.mkdir(parents=True, exist_ok=True); target.symlink_to(source)
        jobs.append(('https://resources.download.minecraft.net/' + suffix, target, digest))
    batch(jobs)
    natives = [native_library(root, *package) for package in PACKAGES]
    for alias, target in [('liblwjgl64.so','liblwjgl.so'), ('libjinput-linux64.so','libjinput.so')]:
        path = root / 'natives' / alias
        if not path.exists(): path.symlink_to(target)
    openal = Path('/usr/lib/libopenal.so.1')
    if openal.exists():
        path = root / 'natives/libopenal64.so'
        if not path.exists(): path.symlink_to(openal)
    (root / 'classpath.txt').write_text(os.pathsep.join(classpath + [str(root / 'client.jar')] ))
    (root / 'native-sources.json').write_text(json.dumps(natives, indent=2) + '\n')
    print(json.dumps({'client_sha1': client['sha1'], 'libraries': len(classpath), 'assets': len(jobs), 'native_libraries': len(natives)}))


if __name__ == '__main__':
    main()
