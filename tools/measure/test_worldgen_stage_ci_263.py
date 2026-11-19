"""Новый helper обязан связывать именно новый build/trace/source, не старый ELF."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, call, patch

import numpy as np

import worldgen_stage_ci_263 as ci


class StageCiTests(unittest.TestCase):
    def test_console_compares_decoded_names_and_requires_one_positive_signal(self):
        blocks = np.zeros((384, 16, 16), dtype=np.uint16)
        blocks[64, 0, 3] = 1
        blocks[65, 0, 3] = 2
        blocks[65, 1, 4] = 3
        reader = Mock()
        reader.names = ['air', 'sulfur', 'water', 'example:rock']
        reader.read.return_value = iter([(512, 513, blocks, [['plains'] * 64] * 24, {})])
        server = Mock()

        def reply(command, *, timeout):
            marker = command.split()[-1]
            response = f'[Server thread/INFO]: command echo: {command}\n'
            if marker in {'MCSA_WORLDGEN_BLOCK_0_IF_END', 'MCSA_WORLDGEN_BLOCK_1_UNLESS_END',
                          'MCSA_WORLDGEN_BLOCK_3_IF_END', 'MCSA_WORLDGEN_BLOCK_3_UNLESS_END',
                          'MCSA_WORLDGEN_BLOCK_4_IF_END', 'MCSA_WORLDGEN_BLOCK_5_IF_END'}:
                response += f'[Server thread/INFO]: [Server] {marker}\n'
            return response + '[Server thread/INFO]: [Server] MCSA_EXACT_SYNC_1_END\n'

        server.command.side_effect = reply
        with tempfile.TemporaryDirectory() as name:
            evidence = Path(name)
            region = evidence / 'region'
            with patch.object(ci.exact, 'Reader', return_value=reader), \
                    patch.object(ci.exact, 'region_dir', return_value=region):
                ci.check_vanilla_console(server, evidence / 'world', evidence)
            result = json.loads((evidence / 'vanilla-console-blocks.json').read_text())
        reader.read.assert_called_once_with(region / 'r.16.16.mca', {(512, 513)})
        expected = [([8195, 0, 8208], 'minecraft:sulfur', 'match'),
                    ([8195, 1, 8208], 'minecraft:water', 'mismatch'),
                    ([8195, 2, 8208], 'minecraft:air', 'inconclusive'),
                    ([8195, 3, 8208], 'minecraft:air', 'inconclusive'),
                    ([8195, 64, 8208], 'minecraft:air', 'match'),
                    ([8196, 1, 8209], 'example:rock', 'match')]
        self.assertEqual(result['seed'], 100554032945340)
        self.assertFalse(result['all_match'])
        self.assertEqual([(r['position'], r['decoded_block'], r['status']) for r in result['checks']], expected)
        commands = []
        for i, (position, block, _) in enumerate(expected):
            pos = ' '.join(map(str, position))
            for condition in ('if', 'unless'):
                marker = f'MCSA_WORLDGEN_BLOCK_{i}_{condition.upper()}_END'
                commands.append(call(f'execute in minecraft:overworld if loaded {pos} '
                                     f'{condition} block {pos} {block} run say {marker}', timeout=60))
        self.assertEqual(server.command.call_args_list, commands)

    def test_build_receipt_binds_source_binary_trace_and_lock(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name)
            (root / 'Cargo.lock').write_bytes(b'fixed-lock')
            elf, trace = root / 'server', root / 'trace.gz'
            elf.write_bytes(b'\x7fELFown-new-source'); trace.write_bytes(b'own-stage-data')
            receipt = {'source': 'a' * 40, 'elf_sha256': ci.sha(elf), 'trace_sha256': ci.sha(trace),
                       'cargo_lock_sha256': ci.sha(root / 'Cargo.lock')}
            with patch.object(ci, 'ROOT', root):
                ci.validate_build(receipt, 'a' * 40, elf, trace)
                for field in receipt:
                    invalid = {**receipt, field: 'wrong'}
                    with self.subTest(field=field), self.assertRaises(ValueError):
                        ci.validate_build(invalid, 'a' * 40, elf, trace)
                elf.write_bytes(b'old-or-replaced-binary')
                with self.assertRaises(ValueError):
                    ci.validate_build(receipt, 'a' * 40, elf, trace)
                receipt['elf_sha256'] = ci.sha(elf)
                with self.assertRaises(ValueError):
                    ci.validate_build(receipt, 'a' * 40, elf, trace)

    def test_old_baseline_guard_is_not_reused_or_weakened(self):
        self.assertEqual(ci.baseline.SOURCE, '177fd36c55d9ebad4ebbedb0564e2ad3f6dfe3ff')
        with self.assertRaises(ValueError):
            ci.baseline.validate_source_diff('M\tsrc/world/mod.rs\n')
        self.assertNotIn('src/world/mod.rs', ci.baseline.ALLOWED_ADDITIONS)


if __name__ == '__main__':
    unittest.main()
