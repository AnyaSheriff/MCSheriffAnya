"""Проверки отказов ARM64-упаковки без запуска бинарника."""
import hashlib
import json
import os
from pathlib import Path
import struct
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import package_linux_arm64 as packaging


def elf(machine=183, kind=3):
    return (b'\x7fELF\x02\x01\x01' + bytes(9)
            + struct.pack('<HHIQQQIHHHHHH', kind, machine, 1, 0, 0, 0, 0,
                          64, 0, 0, 0, 0, 0))


class PackageTests(unittest.TestCase):
    def test_elf_rejects_invalid_headers_and_truncated_tables(self):
        invalid = [b'', elf()[:63], elf(machine=62), elf(kind=1),
                   b'bad!' + elf()[4:], elf()[:4] + b'\x01' + elf()[5:],
                   elf()[:5] + b'\x02' + elf()[6:]]
        table = bytearray(elf())
        struct.pack_into('<Q', table, 32, 64)
        struct.pack_into('<HH', table, 54, 56, 1)
        invalid.append(bytes(table))
        for raw in invalid:
            with self.subTest(raw=raw):
                with self.assertRaises(ValueError):
                    packaging.elf_header(raw)
        self.assertEqual(packaging.elf_header(elf())['machine'], 183)

    def mock_command(self, args, root):
        if args[:2] == ['rustc', '-vV']:
            return 'rustc test\nhost: aarch64-unknown-linux-gnu\n'
        if args[:2] == ['git', 'rev-parse']:
            return 'a' * 40 + '\n'
        if args[0] == 'ldd':
            return 'libc.so.6 => /lib/aarch64-linux-gnu/libc.so.6 (0x1234)\n'
        if args[1] == '--version-info':
            return 'Name: GLIBC_2.9 Name: GLIBC_2.35 Name: GLIBC_2.17\n'
        if args[1] == '--dynamic':
            return '0x00000001 (NEEDED) Shared library: [libc.so.6]\n'
        if args[1] == '--program-headers':
            return ' [Requesting program interpreter: /lib/ld-linux-aarch64.so.1]\n'
        raise AssertionError(args)

    def fixture(self, root):
        binary = root / 'runtime'
        binary.write_bytes(elf())
        for filename in ('README.md', 'README_ru.md', 'LICENSE'):
            (root / filename).write_text(filename + '\n')
        return binary

    @patch.object(packaging.platform, 'system', return_value='Linux')
    @patch.object(packaging.platform, 'machine', return_value='aarch64')
    def test_checksums_archive_and_executable_permissions(self, *_):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {'RUNNER_ARCH': 'ARM64', 'GITHUB_REF_TYPE': 'branch'}):
            root = Path(temporary)
            binary = self.fixture(root)
            output = root / 'output'
            with patch.object(packaging, 'command', side_effect=self.mock_command):
                info = packaging.package(binary, output, root)
            self.assertEqual(info['glibc_required_max'], '2.35')
            self.assertEqual(info['dependencies'], ['libc.so.6'])
            self.assertEqual(info['runtime_sha256'], hashlib.sha256(binary.read_bytes()).hexdigest())
            self.assertEqual(json.loads((output / 'arm64-build-info.json').read_text()), info)
            bare = output / 'packages/mcsheriffanya-linux-arm64'
            self.assertEqual(bare.stat().st_mode & 0o777, 0o755)
            self.assertEqual(bare.read_bytes(), binary.read_bytes())
            lines = (output / 'packages/SHA256SUMS').read_text().splitlines()
            self.assertEqual(len(lines), 2)
            for line in lines:
                digest, name = line.split('  ')
                self.assertEqual(digest, packaging.sha(output / 'packages' / name))
            archive = next((output / 'packages').glob('*.tar.gz'))
            prefix = 'mcsheriffanya-dev-aaaaaaa-aarch64-unknown-linux-gnu'
            self.assertEqual(archive.name, prefix + '.tar.gz')
            with tarfile.open(archive) as tar:
                self.assertEqual(set(tar.getnames()), {prefix, *(prefix + '/' + filename for filename in ('mcsheriffanya', 'README.md', 'README_ru.md', 'LICENSE'))})
                member = tar.getmember(prefix + '/mcsheriffanya')
                self.assertEqual(member.mode, 0o755)
                self.assertEqual(tar.extractfile(member).read(), binary.read_bytes())

    def test_rejects_non_native_before_commands(self):
        with patch.object(packaging.platform, 'system', return_value='Linux'), patch.object(packaging.platform, 'machine', return_value='x86_64'), patch.object(packaging, 'command') as command, tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaisesRegex(ValueError, 'native ARM64'):
                packaging.package(root / 'binary', root / 'output', root)
            command.assert_not_called()

    @patch.object(packaging.platform, 'system', return_value='Linux')
    @patch.object(packaging.platform, 'machine', return_value='aarch64')
    def test_missing_dependency_never_publishes_packages(self, *_):
        def command(args, root):
            return 'libc.so.6 => not found\n' if args[0] == 'ldd' else self.mock_command(args, root)
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {'RUNNER_ARCH': 'ARM64'}):
            root = Path(temporary)
            binary = self.fixture(root)
            with patch.object(packaging, 'command', side_effect=command), self.assertRaisesRegex(ValueError, 'missing dynamic'):
                packaging.package(binary, root / 'output', root)
            self.assertFalse((root / 'output/packages').exists())
            self.assertIn('not found', (root / 'output/arm64-platform.log').read_text())

    @patch.object(packaging.platform, 'system', return_value='Linux')
    @patch.object(packaging.platform, 'machine', return_value='arm64')
    def test_rejects_runner_or_rustc_host_mismatch(self, *_):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            binary = self.fixture(root)
            with patch.dict(os.environ, {'RUNNER_ARCH': 'X64'}), self.assertRaisesRegex(ValueError, 'RUNNER_ARCH'):
                packaging.package(binary, root / 'output', root)
            with patch.dict(os.environ, {'RUNNER_ARCH': 'ARM64'}), patch.object(packaging, 'command', return_value='host: x86_64-unknown-linux-gnu\n'), self.assertRaisesRegex(ValueError, 'rustc host'):
                packaging.package(binary, root / 'output', root)

    @patch.object(packaging.platform, 'system', return_value='Linux')
    @patch.object(packaging.platform, 'machine', return_value='aarch64')
    def test_tag_name_is_used_and_cannot_escape_output(self, *_):
        with tempfile.TemporaryDirectory() as temporary, patch.dict(os.environ, {'RUNNER_ARCH': 'ARM64', 'GITHUB_REF_TYPE': 'tag', 'GITHUB_REF_NAME': 'v0.2.9-alpha'}), patch.object(packaging, 'command', side_effect=self.mock_command):
            root = Path(temporary)
            binary = self.fixture(root)
            packaging.package(binary, root / 'output', root)
            self.assertTrue((root / 'output/packages/mcsheriffanya-v0.2.9-alpha-aarch64-unknown-linux-gnu.tar.gz').is_file())
            with patch.dict(os.environ, {'GITHUB_REF_NAME': '../escape'}), self.assertRaisesRegex(ValueError, 'unsafe package tag'):
                packaging.package(binary, root / 'invalid', root)
            self.assertFalse((root / 'invalid/packages').exists())


if __name__ == '__main__':
    unittest.main()
