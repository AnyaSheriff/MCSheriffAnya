#!/usr/bin/env python3
"""Проверяет платформу и упаковывает собственную нативную ARM64-сборку."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import tarfile
import tempfile

TARGET = 'aarch64-unknown-linux-gnu'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def elf_header(raw):
    require(len(raw) >= 64, 'truncated ELF64 header')
    require(raw[:7] == b'\x7fELF\x02\x01\x01', 'expected ELF64 little-endian version 1')
    fields = struct.unpack('<HHIQQQIHHHHHH', raw[16:64])
    kind, machine, version = fields[:3]
    require(kind in (2, 3), 'expected ELF executable or PIE')
    require(machine == 183, 'expected AArch64 ELF machine 183')
    require(version == 1 and fields[7] == 64, 'invalid ELF header version/size')
    # Не принимаем заголовок с таблицами за концом файла.
    for offset, size, count, expected in ((fields[4], fields[8], fields[9], 56),
                                           (fields[5], fields[10], fields[11], 64)):
        if count:
            require(offset >= 64 and size == expected and offset + size * count <= len(raw),
                    'invalid or truncated ELF table')
    return {'class': 64, 'endianness': 'little', 'type': kind, 'machine': machine}


def command(args, root):
    return subprocess.check_output(args, cwd=root, text=True, stderr=subprocess.STDOUT,
                                   env={**os.environ, 'LC_ALL': 'C'})


def package(binary, output, root):
    binary, output, root = Path(binary).resolve(), Path(output).resolve(), Path(root).resolve()
    output.mkdir(parents=True, exist_ok=True)
    require(platform.system() == 'Linux', 'native Linux required')
    machine = platform.machine().lower()
    require(machine in ('aarch64', 'arm64'), 'native ARM64 required')
    runner_arch = os.environ.get('RUNNER_ARCH', '')
    require(not runner_arch or runner_arch == 'ARM64', 'RUNNER_ARCH must be ARM64')
    rustc = command(['rustc', '-vV'], root)
    hosts = re.findall(r'^host: (.+)$', rustc, re.M)
    require(hosts == [TARGET], 'native GNU ARM64 rustc host required')
    raw = binary.read_bytes()
    elf = elf_header(raw)
    head = command(['git', 'rev-parse', 'HEAD'], root).strip()
    require(re.fullmatch(r'[0-9a-f]{40}', head), 'invalid git HEAD')
    logs = [('rustc -vV', rustc)]
    captures = {}
    try:
        for name, args in (
                ('ldd', ['ldd', str(binary)]),
                ('versions', ['readelf', '--version-info', str(binary)]),
                ('dynamic', ['readelf', '--dynamic', str(binary)]),
                ('program_headers', ['readelf', '--program-headers', '--wide', str(binary)])):
            try:
                value = command(args, root)
            except subprocess.CalledProcessError as error:
                logs.append((' '.join(args), error.output or ''))
                raise
            logs.append((' '.join(args), value))
            captures[name] = value
        require('not found' not in captures['ldd'], 'missing dynamic dependency')
        versions = re.findall(r'\bGLIBC_(\d+(?:\.\d+)+)\b', captures['versions'])
        require(versions, 'GNU runtime must declare GLIBC requirements')
        glibc = max(set(versions), key=lambda value: tuple(map(int, value.split('.'))))
        dependencies = re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', captures['dynamic'])
        interpreters = re.findall(r'Requesting program interpreter: ([^\]]+)\]', captures['program_headers'])
        require(len(interpreters) == 1 and dependencies, 'missing GNU interpreter/dependencies')
    finally:
        (output / 'arm64-platform.log').write_text(''.join(f'$ {name}\n{value}\n' for name, value in logs))
    tag = f'dev-{head[:7]}'
    if os.environ.get('GITHUB_REF_TYPE') == 'tag':
        tag = os.environ.get('GITHUB_REF_NAME', '')
    require(re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9._-]*', tag), 'unsafe package tag')
    name = f'mcsheriffanya-{tag}-{TARGET}'
    packages = output / 'packages'
    packages.mkdir(exist_ok=True)
    bare = packages / 'mcsheriffanya-linux-arm64'
    shutil.copyfile(binary, bare)
    bare.chmod(0o755)
    archive = packages / f'{name}.tar.gz'
    with tempfile.TemporaryDirectory() as temporary:
        folder = Path(temporary) / name
        folder.mkdir()
        shutil.copyfile(bare, folder / 'mcsheriffanya')
        (folder / 'mcsheriffanya').chmod(0o755)
        for filename in ('README.md', 'README_ru.md', 'LICENSE'):
            shutil.copyfile(root / filename, folder / filename)
        with tarfile.open(archive, 'w:gz') as tar:
            tar.add(folder, arcname=name)
    hashes = {p.name: sha(p) for p in (bare, archive)}
    (packages / 'SHA256SUMS').write_text(''.join(f'{digest}  {name}\n' for name, digest in hashes.items()))
    info = {
        'head': head, 'runtime_sha256': hashlib.sha256(raw).hexdigest(),
        'runtime_bytes': len(raw), 'host': hosts[0], 'runner_arch': runner_arch,
        'machine': machine, 'elf': elf, 'glibc_required_max': glibc,
        'dependencies': dependencies, 'interpreter': interpreters[0],
        'package_sha256': hashes,
    }
    (output / 'arm64-build-info.json').write_text(json.dumps(info, indent=2) + '\n')
    return info


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--binary', required=True, type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    package(args.binary, args.output, Path(__file__).resolve().parent.parent)


if __name__ == '__main__':
    main()
