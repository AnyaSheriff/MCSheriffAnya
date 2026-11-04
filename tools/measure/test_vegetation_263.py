"""Регрессии происхождения замеров и целостности четырёхствольной формы."""
import gzip
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

from vegetation_boundary_263 import measure as boundary_measure

from vegetation_shapes_263 import measure
from vegetation_263 import owned, compare, reference_side
from pack_crowns_263 import pack
from vegetation_selector_263 import shelf_models


class VegetationTests(unittest.TestCase):
    def test_density_reference_rejects_our_world_even_with_matching_seed(self):
        report = {'vanilla_level':{'seed':42,'version':'MCSA custom level.dat'}}
        with self.assertRaisesRegex(ValueError,'официальный мир'):
            reference_side(report,42)
        report['vanilla_level']['version'] = {'Name':'26.3','Id':5023}
        with self.assertRaisesRegex(ValueError,'Семя'):
            reference_side(report,43)

    def test_dappled_selector_preserves_empty_spruce_and_poplar_mushroom_states(self):
        root = Path(__file__).resolve().parents[2]/'testdata/world-profile/genaudit-26.3'
        observed = json.loads((root/'dappled-selector-v2.json').read_text())
        spruce = [p for p in observed['observations'] if p['base']=='spruce_log']
        self.assertEqual(len(spruce),4)
        self.assertTrue(all(not p['litter'] and not p['shelf'] for p in spruce))
        separate = json.loads((root/'spruce-litter-v2.json').read_text())
        self.assertEqual(separate['bases'],{'spruce_log':32})
        self.assertTrue(all(not p['litter'] for p in separate['observations']))
        models = json.loads((root/'poplar-shelf-models-v2.json').read_text())
        self.assertEqual(shelf_models(observed),models)
        self.assertEqual(len(models),96)
        self.assertEqual(sum(map(len,models)),61)
        self.assertEqual(sum(not m for m in models),61)
        self.assertEqual(max(map(len,models)),4)
        self.assertEqual({m[3] for model in models for m in model},{0,1})

    def test_empty_vegetation_keeps_reference_discrepancy(self):
        ours = {'biomes_3d':{'forest':10000},'features_by_biome':{}}
        vanilla = {'biomes_3d':{'forest':10000},'features_by_biome':{'forest':{'short_grass':100}}}
        rows = compare(ours,vanilla)
        self.assertEqual(len(rows),1)
        self.assertEqual(rows[0]['ours'],0)
        self.assertTrue(rows[0]['outside'])

    def test_census_keeps_bamboo_and_eyeblossoms(self):
        for name in ('bamboo','bamboo_sapling','open_eyeblossom','closed_eyeblossom'):
            self.assertTrue(owned(name),name)

    def test_our_world_cannot_be_used_as_official_reference(self):
        with tempfile.TemporaryDirectory() as folder:
            Path(folder,'level.dat').write_text('seed=42\nspawn=0 64 0\n')
            with self.assertRaisesRegex(ValueError,'официальный мир'):
                measure(Path(folder),{'seed':42},require_vanilla=True)
            with self.assertRaisesRegex(ValueError,'Семя'):
                measure(Path(folder),{'seed':43})

    def test_boundary_bands_use_halo_and_keep_interior_counts(self):
        class SyntheticReader:
            lookup = {'leaf_litter':1,'dark_oak_log':2,'yellow_poplar_leaves':3}

            def read(self,file,wanted):
                for cx,cz in sorted(wanted):
                    blocks = np.zeros((384,16,16),dtype=np.uint16)
                    if (cx,cz) == (0,0):
                        for x in (0,4,8,12):
                            blocks[140,8,x] = 1
                    bio = np.full((96,4,4),'forest' if cx < 0 else 'plains')
                    yield cx,cz,blocks,bio,None

        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'level.dat').write_text('seed=42\nspawn=0 64 0\n')
            (root/'region').mkdir()
            (root/'region/r.0.0.mcs').touch()
            plan = {'seed':42,'windows':{'overworld':[[-1,-1,1,1]]}}
            with patch('vegetation_boundary_263.Reader',SyntheticReader):
                result = boundary_measure(root,plan)['windows'][0]['pairs']['plains_litter']
            self.assertEqual(result['blocks'],{str(i):1 for i in range(4)})
            self.assertEqual(result['biome_blocks'],{str(i):24576 for i in range(4)})

    def test_dark_oak_models_preserve_every_observed_log_and_leaf_state(self):
        root = Path(__file__).resolve().parents[2]/'testdata/world-profile/genaudit-26.3'
        with gzip.open(root/'dark-oak-observed-v2.json.gz','rt') as stream:
            observations = json.load(stream)['dark_oak']
        models = json.loads((root/'dark-oak-models-v2.json').read_text())
        self.assertEqual(pack({'dark_oak':observations}),models)
        self.assertEqual(len(models),32)
        for observed,model in zip(observations,models,strict=True):
            self.assertEqual(model['trunk'],observed['trunk'])
            leaves = {(bit-8,y,z,d) for y,z,mask,d in model['rows']
                      for bit in range(17) if mask & (1<<bit)}
            self.assertEqual(leaves,{tuple(v) for v in observed['leaves']})
            logs = {(x,y,z,('x','y','z')[axis]) for x,y,z,axis in model['logs']}
            self.assertEqual(logs,{(x,y,z,axis or 'y') for x,y,z,axis in observed['logs']})
            for x,z in [(0,0),(1,0),(0,1),(1,1)]:
                self.assertIn((x,1-model['trunk'],z,'y'),logs)


if __name__ == '__main__':
    unittest.main()
