"""Синтетические профили; maximum не зависит от первых 20 примеров."""
import copy
import gzip
import hashlib
import json
import struct
import tempfile
from pathlib import Path
from unittest.mock import patch
import unittest

import numpy as np

import worldgen_stage_263 as stage


def arrays():
    return [np.full((384, 16, 16), 'air', dtype=object),
            np.full((24, 4, 4, 4), 'plains', dtype=object), np.full((16, 16), -65)]


def snapshot():
    return {'stage': 'final', 'state_names': {'0': 'air'},
            'columns': [{'local': [x, z], 'runs': [[-64, 319, 0]]} for x in range(16) for z in range(16)],
            'biome_ids': [1] * 1536, 'biomes': ['plains'] * 1536,
            'blockstates_sha256': hashlib.sha256(b'mcsa-stage-blockstates-v1:y,z,x:i32be\n' + bytes(98304 * 4)).hexdigest(),
            'biome_ids_sha256': hashlib.sha256(b'mcsa-stage-biomes-v1:qy,qz,qx:u16be\n' + struct.pack('>1536H', *([1] * 1536))).hexdigest()}


class StageTests(unittest.TestCase):
    def test_argmax_after_many_smaller_examples_and_includes_complete_column(self):
        ours, ref = {(512, 512): arrays()}, {(512, 512): arrays()}
        ours[512, 512][2][:] = -64
        ours[512, 512][0][0] = 'sand'
        ours[512, 512][0][65, 15, 15] = 'oak_leaves'
        ours[512, 512][2][15, 15] = 1
        report = stage.height_argmax_diagnostics(ours, ref)
        self.assertEqual(report['abs_error_max'], 66)
        self.assertEqual(report['selected_column'], [8207, 8207])
        self.assertEqual(report['tie_count'], 1)
        self.assertEqual(report['compared_columns'], 256)
        col = next(c for c in report['neighbors'] if c['column'] == report['selected_column'])
        self.assertEqual(col['ours']['top_name'], 'oak_leaves')
        self.assertEqual(col['ours']['top_substrate'], -64)
        self.assertEqual(col['ours']['profile_y_inclusive_name'][-1], [2, 319, 'air'])
        self.assertEqual(sum(c['in_plan'] for c in report['neighbors']), 4)

    def test_tie_rule_world_x_then_z_including_negative_chunks(self):
        ours = {(-1, 0): arrays(), (-1, -1): arrays()}
        ref = {coord: arrays() for coord in ours}
        for a in ours.values():
            a[2][:] = -64
            a[0][0] = 'stone'
        result = stage.height_argmax_diagnostics(ours, ref)
        self.assertEqual(result['selected_column'], [-16, -16])
        self.assertEqual(result['tie_count'], 512)
        self.assertEqual(result['abs_error_sum'], 512)
        selected = next(c for c in result['neighbors'] if c['column'] == [-16, -16])
        self.assertEqual(selected['ours']['top_non_air'], -64)

    def test_empty_height_zero_error_ties_all_columns(self):
        a = {(0, 0): arrays()}
        r = stage.height_argmax_diagnostics(a, a)
        self.assertEqual((r['abs_error_max'], r['tie_count'], r['selected_column']), (0, 256, [0, 0]))
        self.assertEqual(stage.column_record(a[0, 0][0][:, 0, 0])['top_name'], None)

    def test_height_includes_water_plants_and_leaves(self):
        values = np.full(384, 'air', dtype=object)
        for y, name in [(0, 'raw_iron_block'), (62, 'water'), (63, 'seagrass'), (127, 'oak_leaves')]:
            values[y + 64] = name
        r = stage.column_record(values)
        self.assertEqual((r['top_non_air'], r['top_water'], r['top_substrate']), (127, 62, 0))

    def test_full_trace_decode_and_native_hashes(self):
        a, b = stage.decode_stage(snapshot())
        self.assertEqual(a.shape, (384, 16, 16))
        self.assertTrue(np.all(a == 'air'))
        self.assertTrue(np.all(b == 'plains'))

    def test_trace_rejects_bad_coverage_hash_or_unknown_name(self):
        for mutate in [lambda s: s['columns'].pop(),
                       lambda s: s['columns'].append(s['columns'][0]),
                       lambda s: s['columns'][0]['runs'].__setitem__(0, [-63, 319, 0]),
                       lambda s: s['columns'][0]['runs'].__setitem__(0, [-64, 318, 0]),
                       lambda s: s.__setitem__('blockstates_sha256', '0' * 64),
                       lambda s: s.__setitem__('biome_ids_sha256', '0' * 64),
                       lambda s: s['state_names'].__setitem__('0', '?'),
                       lambda s: s['biome_ids'].pop(),
                       lambda s: s['biomes'].__setitem__(0, 'forest')]:
            with self.subTest(mutate=mutate):
                value = snapshot(); mutate(value)
                with self.assertRaises(ValueError):
                    stage.decode_stage(value)

    def test_trace_final_saved_full_chunk_gate(self):
        value = snapshot()
        record = {'chunk': [0, 0], 'stages': [value],
                  'diagnostic_columns': [{'column': [0, 0], 'grid_top': -65}]}
        saved = arrays()
        result = stage.link_trace(record, saved, [[0, 0]])
        self.assertTrue(result['final_saved_names_equal'])
        self.assertEqual(result['columns'][0]['own_stages'][0]['top_non_air'], -65)
        bad = copy.deepcopy(saved); bad[0][100, 15, 15] = 'stone'
        with self.assertRaises(ValueError):
            stage.link_trace(record, bad, [[0, 0]])
        bad = copy.deepcopy(saved); bad[1][23, 3, 3, 3] = 'forest'
        with self.assertRaises(ValueError):
            stage.link_trace(record, bad, [[0, 0]])

    def test_trace_manifest_rejects_missing_reordered_or_nonidentical_records(self):
        records = []
        for seed in stage.SEEDS:
            for chunk in stage.TRACE_CHUNKS:
                records.append({'schema': 1, 'version': '26.3', 'seed': seed, 'chunk': chunk,
                                'density_router': False, 'holdout': seed in stage.SEEDS[2:],
                                'stages': [{'stage': name, 'blockstates_sha256': 'blocks',
                                            'biome_ids_sha256': 'biomes'} for name in stage.STAGES],
                                'identity': {'equal_blockstates': True, 'equal_biomes': True,
                                             'compared_blockstates': 98304, 'compared_biomes': 1536,
                                             'ordinary_blockstates_sha256': 'blocks',
                                             'ordinary_biome_ids_sha256': 'biomes'}})
        def read(values):
            with tempfile.TemporaryDirectory() as folder:
                path = Path(folder) / 'trace.gz'
                with gzip.open(path, 'wt') as out:
                    for record in values:
                        out.write(json.dumps(record) + '\n')
                with patch.object(stage, 'decode_stage'):
                    return stage.read_traces(path)
        self.assertEqual(len(read(records)), 8)
        for mutate in [lambda r: r.pop(), lambda r: r.append(r[0]),
                       lambda r: r[0].__setitem__('density_router', True),
                       lambda r: r[-1].__setitem__('holdout', False),
                       lambda r: r[0]['stages'].reverse(),
                       lambda r: r[0]['identity'].__setitem__('equal_blockstates', False),
                       lambda r: r[0]['identity'].__setitem__('compared_biomes', 1535),
                       lambda r: r[0]['identity'].__setitem__('ordinary_biome_ids_sha256', 'bad')]:
            with self.subTest(mutate=mutate):
                bad = copy.deepcopy(records); mutate(bad)
                with self.assertRaises(ValueError):
                    read(bad)

    def test_holdouts_and_stage_order_are_fixed_before_results(self):
        self.assertEqual(stage.SEEDS[2:], (42, -20261116))
        self.assertEqual(stage.STAGES[0], 'base_underground')
        self.assertLess(stage.STAGES.index('trees'), stage.STAGES.index('vegetation'))
        self.assertLess(stage.STAGES.index('vegetation'), stage.STAGES.index('structure_terrain'))
        self.assertLess(stage.STAGES.index('ocean'), stage.STAGES.index('monument'))


if __name__ == '__main__':
    unittest.main()
