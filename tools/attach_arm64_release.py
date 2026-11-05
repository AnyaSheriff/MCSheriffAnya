"""Добавляет проверенные ARM64-файлы в существующий релиз без замены его тега."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import urllib.parse
import urllib.request

REPO = 'AnyaSheriff/MCSheriffAnya'
TAG = 'v0.2.9-alpha'
RELEASE_ID = 421646884
FILES = {
    'mcsheriffanya-linux-arm64': '39b18ee03b06983cfb0654ec098865cd895c963d6ede6407734b97ce3a6f757b',
    'mcsheriffanya-dev-bed8f06-aarch64-unknown-linux-gnu.tar.gz': 'fcc420dad5a491923eda69c441d1fa3dba59f2d46dccd44738ef316ff006cc90',
    'mcsheriffanya-android-arm64': 'e79693d3aa2608f937ea2a0d7da672a1de70983956582c3ac7d8df98a2db012b',
}
BEGIN = '<!-- mcsa-arm64-26.3-begin -->'
END = '<!-- mcsa-arm64-26.3-end -->'
SECTION = '''### Дополнительные сборки ARM64 — Minecraft Java 26.3

- `mcsheriffanya-linux-arm64`: обычный Linux ARM64, glibc 2.35+; плагины WASM включены.
- `mcsheriffanya-android-arm64`: Android/Termux ARM64, API24+; плагины WASM включены.
- Рядом приложены архивы, `SHA256SUMS-ARM64.txt` и инструкция `ARM64-README.txt`.

Эти дополнительные файлы собраны после первоначального релиза: Linux из
[`bed8f062`](https://github.com/AnyaSheriff/MCSheriffAnya/commit/bed8f062b3537214645dddaef838ccaa8fb1b625),
Android из [`9d6fb8e`](https://github.com/AnyaSheriff/MCSheriffAnya/commit/9d6fb8eadf37d1461e3d9cf3537bd11aaa2633c8).
Архивы Source code по тегу относятся к прежнему снимку исходников.

Linux прошёл полный прогон на ARM64. Для Android проверены сборка и HTTPS-клиент
в эмуляторе Android35 x86_64; сам ARM64-сервер на настоящем телефоне ещё не запускался.
'''
README = '''MCSheriffAnya — дополнительные ARM64-сборки Minecraft Java 26.3, протокол 777.
Linux: glibc 2.35+, например Ubuntu 22.04+ или Debian 12+.
Android/Termux: ARM64/Bionic, API24+. Плагины WASM включены в обе сборки.

В отдельной папке сервера:
chmod +x mcsheriffanya-linux-arm64
./mcsheriffanya-linux-arm64

Для Termux используйте mcsheriffanya-android-arm64 и аналогичные команды.
Скопируйте файл в домашнюю папку Termux; не запускайте его из /sdcard.
Проверка скачанных файлов: sha256sum -c SHA256SUMS-ARM64.txt

Linux source: bed8f062b3537214645dddaef838ccaa8fb1b625
Android source: 9d6fb8eadf37d1461e3d9cf3537bd11aaa2633c8
Эти файлы добавлены к v0.2.9-alpha позднее; снимок Source code по тегу прежний.
Linux full native checks: https://github.com/AnyaSheriff/MCSheriffAnya/actions/runs/41963915495
Android HTTPS check: https://github.com/AnyaSheriff/MCSheriffAnya/actions/runs/41967060238
Сам ARM64-сервер на настоящем Android-телефоне ещё не запускался.
'''


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def unique(root, name):
    paths = [p for p in root.rglob(name) if p.is_file() and not p.is_symlink()]
    if len(paths) != 1:
        raise ValueError(f'Expected exactly one {name}, got {len(paths)}')
    return paths[0]


def prepare(linux, android, output):
    output.mkdir(parents=True, exist_ok=True)
    manifest = dict(FILES)
    android_tar = 'mcsheriffanya-android-arm64-9d6fb8e.tar.gz'
    sums = unique(android, 'SHA256SUMS') if (android / 'SHA256SUMS').is_file() else android / 'android-dist/SHA256SUMS'
    entries = dict(line.split('  ', 1)[::-1] for line in sums.read_text().splitlines())
    manifest[android_tar] = entries[android_tar]
    # Сверяем архив с уже проверенным бинарником, а не только с его manifest.
    import tarfile
    archive = unique(android, android_tar)
    with tarfile.open(archive) as tar:
        members = [m for m in tar.getmembers() if m.isfile() and Path(m.name).name == 'mcsheriffanya']
        assert len(members) == 1
        assert sha(tar.extractfile(members[0]).read()) == FILES['mcsheriffanya-android-arm64']
    for name, digest in manifest.items():
        root = android if 'android' in name else linux
        raw = unique(root, name).read_bytes()
        assert sha(raw) == digest, f'SHA mismatch: {name}'
        (output / name).write_bytes(raw)
    (output / 'ARM64-README.txt').write_text(README)
    manifest['ARM64-README.txt'] = sha((output / 'ARM64-README.txt').read_bytes())
    sums = ''.join(f'{digest}  {name}\n' for name, digest in sorted(manifest.items()))
    (output / 'SHA256SUMS-ARM64.txt').write_text(sums)
    manifest['SHA256SUMS-ARM64.txt'] = sha(sums.encode())
    return manifest


def publish(output, manifest):
    token = os.environ['GH_TOKEN']
    api = f'https://api.github.com/repos/{REPO}'

    def request(path, method='GET', data=None, binary=False):
        headers = {'Authorization': 'Bearer ' + token, 'Accept': 'application/vnd.github+json',
                   'User-Agent': 'MCSheriffAnya-release-publication'}
        if data is not None:
            headers['Content-Type'] = 'application/octet-stream' if binary else 'application/json'
            if not binary:
                data = json.dumps(data).encode()
        url = path if path.startswith('https://uploads.github.com/') else api + path
        with urllib.request.urlopen(urllib.request.Request(url, data=data, headers=headers, method=method), timeout=180) as response:
            return json.load(response)

    release = request(f'/releases/tags/{TAG}')
    tag = request(f'/git/ref/tags/{TAG}')
    assert release['id'] == RELEASE_ID and not release.get('immutable') and not release['draft']
    (output / 'release-before.json').write_text(json.dumps(release, indent=2))
    (output / 'tag-before.json').write_text(json.dumps(tag, indent=2))
    existing = {a['name']: a for a in release['assets']}
    for name, digest in manifest.items():
        if name in existing:
            assert existing[name]['digest'] == 'sha256:' + digest, f'Existing asset differs: {name}'
    for name, digest in manifest.items():
        if name in existing:
            continue
        data = (output / name).read_bytes()
        url = f'https://uploads.github.com/repos/{REPO}/releases/{RELEASE_ID}/assets?name=' + urllib.parse.quote(name)
        asset = request(url, 'POST', data, True)
        assert asset['digest'] == 'sha256:' + digest and asset['size'] == len(data)
        print('Uploaded:', name)
    latest = request(f'/releases/{RELEASE_ID}')
    body = latest.get('body') or ''
    block = BEGIN + '\n' + SECTION + END
    if BEGIN in body:
        assert END in body
        body = body[:body.index(BEGIN)] + block + body[body.index(END) + len(END):]
    else:
        body += '\n\n' + block
    if body != latest.get('body'):
        request(f'/releases/{RELEASE_ID}', 'PATCH', {'body': body})
    final = request(f'/releases/{RELEASE_ID}')
    final_tag = request(f'/git/ref/tags/{TAG}')
    assert final_tag['object'] == tag['object'], 'Tag changed'
    assets = {a['name']: a for a in final['assets']}
    for name, old in existing.items():
        assert all(assets[name][k] == old[k] for k in ('id', 'size', 'digest')), name
    verified = []
    for name, digest in manifest.items():
        asset = assets[name]
        assert asset['digest'] == 'sha256:' + digest
        expected = f'https://github.com/{REPO}/releases/download/{TAG}/' + urllib.parse.quote(name)
        assert asset['browser_download_url'] == expected
        # Публичное скачивание без передачи API-ключа через redirect.
        with urllib.request.urlopen(expected, timeout=180) as response:
            raw = response.read(asset['size'] + 1)
        assert len(raw) == asset['size'] and sha(raw) == digest, name
        verified.append({'name': name, 'id': asset['id'], 'sha256': digest, 'url': expected})
    result = {'verified': True, 'release_id': RELEASE_ID, 'tag': TAG, 'tag_unchanged': True,
              'original_assets_preserved': len(existing), 'uploaded_assets': verified}
    (output / 'publication-receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    print('VERIFIED: all six assets downloaded and SHA256 checked; original assets/tag preserved')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('linux', 'android', 'output'):
        parser.add_argument('--' + name, required=True, type=Path)
    parser.add_argument('--prepare-only', action='store_true')
    args = parser.parse_args()
    files = prepare(args.linux, args.android, args.output)
    print('Prepared:', ', '.join(files))
    if not args.prepare_only:
        publish(args.output, files)
