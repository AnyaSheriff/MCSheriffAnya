"""Самостоятельные синтетические координаты и настоящие read-only форматы."""
import copy
import ctypes
import ctypes.util
import gzip
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch
import zlib

import numpy as np

import worldgen_exact_263 as exact


class ExactTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.ours, self.vanilla = [self.root / label for label in ('ours', 'vanilla')]
        for world, suffix in [(self.ours, 'mcs'), (self.vanilla, 'mca')]:
            (world / 'region').mkdir(parents=True)
            (world / 'region' / ('r.0.0.' + suffix)).write_bytes(b'synthetic-read-guard')
            (world / 'level.dat').write_text('seed=42\n')
        (self.ours / 'mcsa-world.json').write_text(json.dumps(dict(schema=1, gameplay_version='26.3', registry_id='mcsa-catalog-v1:sha256:' + 'f' * 64)))
        self.plan = dict(version='26.3', seed=42, dimension='overworld', chunks=[[0, 0]])
        def record():
            return (0, 0, np.zeros((384, 16, 16), dtype=np.uint16), [['plains'] * 64 for _ in range(24)], {})
        self.records = {'ours': [record()], 'vanilla': [record()]}
        self.metadata = {'ours': dict(seed=42, version='MCSA custom level.dat'),
                         'vanilla': dict(seed=42, version={'Name': '26.3'}, data_version=5023)}
        self.names = ['air', 'stone', 'water', 'cave_air', 'void_air']

    def run_compare(self, limit=20):
        test = self
        class FakeReader:
            names = test.names
            def read(self, file, wanted):
                return iter(test.records[file.parent.parent.name])
        with patch.object(exact, 'Reader', FakeReader), patch.object(exact, 'level', lambda path: self.metadata[path.name]):
            return exact.compare_worlds(self.ours, self.vanilla, self.plan, limit)

    def invalid(self):
        result = self.run_compare()
        self.assertFalse(result['recording_valid'])
        self.assertFalse(result['parity'])
        self.assertTrue(result['errors'])

    def test_identical_complete_chunks_and_empty_heights(self):
        r = self.run_compare()
        self.assertTrue(r['recording_valid'] and r['parity'])
        self.assertTrue(r['props_not_compared'])
        self.assertEqual(r['totals']['block_names'], dict(compared=98304, mismatches=0, exact_cells=98304))
        self.assertEqual(r['totals']['biome_cells']['compared'], 1536)
        self.assertEqual(r['chunks'][0]['ours'], r['chunks'][0]['vanilla'])
        self.assertEqual(r['comparison_on_union_nonair']['compared'], 0)

    def test_exact_block_position_pair_height_and_union(self):
        self.records['ours'][0][2][64, 2, 3] = 1
        r = self.run_compare()
        self.assertTrue(r['recording_valid']); self.assertFalse(r['parity'])
        self.assertEqual(r['totals']['block_names']['mismatches'], 1)
        self.assertEqual(r['examples']['block_names'], [dict(block=[3, 0, 2], ours='stone', vanilla='air')])
        self.assertEqual(r['examples']['heights'], [dict(column=[3, 2], ours=0, vanilla=-65)])
        self.assertEqual(r['block_name_pairs'], [dict(ours='stone', vanilla='air', count=1)])
        self.assertEqual(r['comparison_on_union_nonair']['exact_cells'], 0)

    def test_quart_coordinates_and_negative_chunk(self):
        self.plan['chunks'] = [[-1, -1]]
        for label, world in [('ours', self.ours), ('vanilla', self.vanilla)]:
            old = self.records[label][0]
            self.records[label][0] = (-1, -1, *old[2:])
            path = next((world / 'region').iterdir())
            path.rename(path.with_name('r.-1.-1' + path.suffix))
        self.records['ours'][0][3][0][0] = 'forest'
        r = self.run_compare()
        self.assertTrue(r['recording_valid'])
        self.assertEqual(r['examples']['biome_cells'], [dict(quart=[-4, -16, -4], block_origin=[-16, -64, -16], ours='forest', vanilla='plains')])
        self.assertEqual(r['totals']['biome_cells']['mismatches'], 1)

    def test_air_variants_are_not_equivalent(self):
        self.records['ours'][0][2][0, 0, 0] = 3
        r = self.run_compare()
        self.assertFalse(r['parity'])
        self.assertEqual(r['comparison_on_union_nonair']['compared'], 1)
        self.assertEqual(r['totals']['top_non_air_heights']['mismatches'], 0)

    def test_height_includes_water_and_leaves_definition(self):
        self.records['ours'][0][2][383, 0, 0] = 2
        r = self.run_compare()
        self.assertEqual(r['examples']['heights'][0]['ours'], 319)
        self.assertIn('includes fluids', r['height_definition'])

    def test_limited_examples_do_not_limit_mismatch_counts(self):
        self.records['ours'][0][2][:] = 1
        r = self.run_compare(3)
        self.assertEqual(r['totals']['block_names']['mismatches'], 98304)
        self.assertEqual(len(r['examples']['block_names']), 3)
        self.assertEqual(r['block_name_pairs'][0]['count'], 98304)
        self.assertTrue(self.run_compare(0)['recording_valid'])

    def test_seed_and_version_guards(self):
        for side, field, value in [('ours', 'seed', 43), ('vanilla', 'seed', True), ('vanilla', 'version', {'Name': '26.1.2'}), ('vanilla', 'data_version', 0)]:
            with self.subTest(side=side, field=field):
                before = copy.deepcopy(self.metadata)
                self.metadata[side][field] = value
                self.invalid()
                self.metadata = before

    def test_saved_candidate_profile_required(self):
        path = self.ours / 'mcsa-world.json'
        path.write_text(json.dumps(dict(schema=1, gameplay_version='26.1.2', registry_id='synthetic')))
        self.invalid()
        path.unlink(); self.invalid()

    def test_missing_and_duplicate_chunk_records_rejected(self):
        records = self.records['vanilla']
        self.records['vanilla'] = []; self.invalid()
        self.records['vanilla'] = records * 2; self.invalid()

    def test_unexpected_chunk_record_rejected(self):
        old = self.records['vanilla'][0]
        self.records['vanilla'][0] = (1, 0, *old[2:]); self.invalid()

    def test_malformed_or_duplicate_plan_rejected(self):
        original = copy.deepcopy(self.plan)
        for field, value in [('version', '26.1.2'), ('seed', True), ('seed', 2 ** 63), ('dimension', None), ('chunks', []), ('chunks', [[0, 0], [0, 0]]), ('chunks', [[0, True]])]:
            self.plan = {**original, field: value}; self.invalid()

    def test_unknown_block_and_biome_rejected(self):
        self.records['ours'][0][2][0, 0, 0] = 65535; self.invalid()
        self.records['ours'][0][2][0, 0, 0] = 0
        self.records['ours'][0][3][0][0] = 'unknown'; self.invalid()

    def test_incomplete_array_rejected(self):
        old = self.records['ours'][0]
        self.records['ours'][0] = (0, 0, old[2][:383], old[3], {}); self.invalid()
        self.records['ours'][0] = (0, 0, old[2], old[3][:23], {}); self.invalid()

    def test_hashes_use_names_not_internal_codes(self):
        self.records['ours'][0][2][0, 0, 0] = 1
        before = self.run_compare()['chunks'][0]['ours']
        self.names = ['stone', 'air', 'water', 'cave_air', 'void_air']
        for label in ('ours', 'vanilla'):
            blocks = self.records[label][0][2]
            blocks[:] = np.where(blocks == 0, 1, 0)
        self.assertEqual(self.run_compare()['chunks'][0]['ours'], before)

    def test_input_mutation_rejected(self):
        real = exact.fingerprints
        calls = 0
        def changed(paths):
            nonlocal calls
            calls += 1
            result = real(paths)
            if calls == 2:
                return {**result, 'changed': 'bad'}
            return result
        with patch.object(exact, 'fingerprints', changed):
            self.invalid()

    def test_cli_baseline_and_strict_parity(self):
        plan = self.root / 'plan.json'; plan.write_text(json.dumps(self.plan))
        output = self.root / 'result.json'
        args = ['worldgen-exact', '--ours', str(self.ours), '--vanilla', str(self.vanilla), '--plan', str(plan), '--output', str(output)]
        result = self.run_compare()
        result['parity'] = False
        with patch.object(exact, 'compare_worlds', return_value=result):
            with patch('sys.argv', args): self.assertEqual(exact.main(), 0)
            with patch('sys.argv', args + ['--require-match']): self.assertEqual(exact.main(), 1)
            result['recording_valid'] = False
            with patch('sys.argv', args): self.assertEqual(exact.main(), 1)

    def actual_fixture(self, status='minecraft:full', separate_seed=None):
        def text(value):
            value = value.encode(); return struct.pack('>H', len(value)) + value
        def tag(kind, name, payload): return bytes([kind]) + text(name) + payload
        def compound(*tags): return b''.join(tags) + b'\0'
        def string(name, value): return tag(8, name, text(value))
        def number(name, value): return tag(3, name, struct.pack('>i', value))
        sections = []
        for y in range(-4, 20):
            blocks = tag(10, 'block_states', compound(tag(9, 'palette', b'\x0a' + struct.pack('>i', 1) + compound(string('Name', 'minecraft:air')))))
            bios = tag(10, 'biomes', compound(tag(9, 'palette', b'\x08' + struct.pack('>i', 1) + text('minecraft:plains'))))
            sections.append(compound(tag(1, 'Y', struct.pack('>b', y)), blocks, bios))
        chunk = b'\x0a\0\0' + compound(string('Status', status), number('xPos', 0), number('zPos', 0), tag(9, 'sections', b'\x0a' + struct.pack('>i', 24) + b''.join(sections)))
        compressed = zlib.compress(chunk)
        record = struct.pack('>I', len(compressed) + 1) + b'\x02' + compressed
        header = bytearray(8192); header[:4] = bytes([0, 0, 2, 1])
        (self.vanilla / 'region/r.0.0.mca').write_bytes(header + record.ljust(4096, b'\0'))
        metadata = b'\x0a\0\0' + compound(tag(10, 'Data', compound(tag(10, 'WorldGenSettings', compound(tag(4, 'seed', struct.pack('>q', 42)))), tag(10, 'Version', compound(string('Name', '26.3'))), number('DataVersion', 5023))))
        (self.vanilla / 'level.dat').write_bytes(gzip.compress(metadata))
        if separate_seed is not None:
            metadata = b'\x0a\0\0' + compound(tag(10, 'Data', compound(tag(10, 'Version', compound(string('Name', '26.3'))), number('DataVersion', 5023))))
            (self.vanilla / 'level.dat').write_bytes(gzip.compress(metadata))
            folder = self.vanilla / 'data/minecraft'; folder.mkdir(parents=True, exist_ok=True)
            seed_data = b'\x0a\0\0' + compound(tag(10, 'data', compound(tag(4, 'seed', struct.pack('>q', separate_seed)))), number('DataVersion', 5023))
            (folder / 'world_gen_settings.dat').write_bytes(gzip.compress(seed_data))
        reader = exact.Reader()
        raw = b'\x04' + b'\0' * 24 + (b'\x01' + struct.pack('>H', reader.biomes.index('plains'))) * 24
        lib = ctypes.CDLL(ctypes.util.find_library('zstd'))
        lib.ZSTD_compress.argtypes = [ctypes.c_void_p, ctypes.c_size_t, ctypes.c_void_p, ctypes.c_size_t, ctypes.c_int]
        lib.ZSTD_compress.restype = ctypes.c_size_t
        buffer = ctypes.create_string_buffer(len(raw) + 1024)
        size = lib.ZSTD_compress(buffer, len(buffer), raw, len(raw), 1)
        compressed = buffer.raw[:size]
        record = struct.pack('>II', zlib.crc32(compressed), len(compressed)) + compressed
        header = bytearray(33 * 256); header[:5] = b'MCS\x01\x01'; header[8:16] = struct.pack('>II', 33, len(record))
        (self.ours / 'region/r.0.0.mcs').write_bytes(header + record)
        return exact.compare_worlds(self.ours, self.vanilla, self.plan)

    def test_actual_reader_synthetic_mcs_and_anvil_equal(self):
        result = self.actual_fixture()
        self.assertTrue(result['recording_valid'], result['errors'])
        self.assertTrue(result['parity'])

    def test_actual_reader_rejects_incomplete_anvil_status(self):
        result = self.actual_fixture('minecraft:empty')
        self.assertFalse(result['recording_valid'])
        self.assertIn('incomplete coverage', result['errors'][0])

    def test_actual_saved_separate_seed_without_world_file_changes(self):
        result = self.actual_fixture(separate_seed=42)
        self.assertTrue(result['recording_valid'], result['errors'])
        self.assertTrue(result['parity'])
        self.assertEqual(result['seed_sources']['vanilla'], 'data/minecraft/world_gen_settings.dat:data.seed')
        path = self.vanilla / 'data/minecraft/world_gen_settings.dat'
        self.assertIn(str(path), result['input_sha256'])
        self.assertEqual(result['input_sha256'][str(path)], exact.fingerprints([path])[str(path)])

    def test_conflicting_actual_saved_seed_never_uses_expected_plan(self):
        result = self.actual_fixture(separate_seed=43)
        self.assertFalse(result['recording_valid'])
        self.assertIn('saved seed differs', result['errors'][0])
        metadata = dict(seed=42)
        with self.assertRaisesRegex(ValueError, 'Conflicting'):
            exact.saved_seed(self.vanilla, metadata, 42)

    def test_missing_saved_seed_not_filled_from_plan_or_console_sidecar(self):
        with self.assertRaisesRegex(ValueError, 'saved seed differs'):
            exact.saved_seed(self.vanilla, dict(seed=None), 42)
        (self.vanilla.parent / 'observed-seed.json').write_text(json.dumps(dict(seed=42)))
        with self.assertRaisesRegex(ValueError, 'sidecar'):
            exact.saved_seed(self.vanilla, dict(seed=42), 42)

    def test_wrong_region_format_rejected(self):
        path = self.ours / 'region/r.0.0.mcs'
        path.rename(path.with_suffix('.mca'))
        self.invalid()

    def test_other_dimensions_explicitly_not_supported(self):
        for dim in ('the_nether', 'the_end'):
            self.plan['dimension'] = dim
            self.invalid()


if __name__ == '__main__':
    unittest.main()
