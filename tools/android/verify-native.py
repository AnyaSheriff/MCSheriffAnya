"""Проверка собственных Android ELF, без исполнения и чужих исходников."""
import hashlib
import json
from pathlib import Path
import re
import struct
import sys


def inspect(binary, machine):
    bits = 32 if machine == 40 else 64
    assert binary[:6] == b'\x7fELF' + bytes([1 if bits == 32 else 2, 1]), 'wrong ELF class or endianness'
    assert struct.unpack_from('<H', binary, 16)[0] == 3, 'PIE required'
    assert struct.unpack_from('<H', binary, 18)[0] == machine, 'wrong ABI'
    offset = struct.unpack_from('<I' if bits == 32 else '<Q', binary, 28 if bits == 32 else 32)[0]
    size, count = struct.unpack_from('<HH', binary, 42 if bits == 32 else 54)
    loads = []
    interpreters = []
    for index in range(count):
        if bits == 32:
            kind, start, va, pa, filesz, memsz, flags, alignment = struct.unpack_from('<IIIIIIII', binary, offset + index * size)
        else:
            kind, flags, start, va, pa, filesz, memsz, alignment = struct.unpack_from('<IIQQQQQQ', binary, offset + index * size)
        if kind == 1:
            assert alignment >= 16384 and start % 16384 == va % 16384, 'ELF is not 16K aligned'
            loads.append(alignment)
        if kind == 3:
            interpreters.append(binary[start:start + filesz].rstrip(b'\0').decode())
    assert loads and interpreters == ['/system/bin/linker' if bits == 32 else '/system/bin/linker64']
    return {'bytes': len(binary), 'sha256': hashlib.sha256(binary).hexdigest(), 'bits': bits, 'machine': machine, 'load_alignments': loads, 'interpreter': interpreters[0]}


def main(root):
    result = {'source_head': (root / 'source-head.txt').read_text().strip(), 'api': 24, 'runtimes': {}}
    assert re.fullmatch('[0-9a-f]{40}', result['source_head'])
    for abi, machine in [('armeabi-v7a', 40), ('arm64-v8a', 183), ('x86_64', 62)]:
        result['runtimes'][abi] = inspect((root / abi / 'libmcsa.so').read_bytes(), machine)
        log = (root / f'elf-{abi}.txt').read_text()
        assert 'GLIBC_' not in log
        needed = re.findall(r'\(NEEDED\).*?\[([^\]]+)\]', log)
        assert 'libc.so' in needed and set(needed) <= {'libc.so', 'libm.so', 'libdl.so', 'liblog.so', 'libandroid.so'}
        result['runtimes'][abi]['features'] = 'default, plugins enabled' if abi == 'arm64-v8a' else 'no-default, plugins disabled'
    (root / 'receipt.json').write_text(json.dumps(result, indent=2) + '\n')
    print(json.dumps(result, indent=2))


if __name__ == '__main__':
    main(Path(sys.argv[1]))
