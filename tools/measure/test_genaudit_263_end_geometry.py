"""Проверки гипотезы до новой контрольной серии; без Minecraft."""
import json
from pathlib import Path
import unittest

from genaudit_263_end_geometry import layer_mask, check_shape, validate_dataset

ROOT=Path(__file__).resolve().parents[2]
HYPOTHESIS=ROOT/'testdata/world-profile/genaudit-26.3/end-island-procedural-hypothesis.json'


class GeometryTests(unittest.TestCase):
    def setUp(self):
        self.hypothesis=json.loads(HYPOTHESIS.read_text())

    def test_all_old_cells_and_visible_sequences(self):
        path=ROOT/'testdata/world-profile/genaudit-26.3/end-island-shapes-observations-26.3.json'
        result=validate_dataset(json.loads(path.read_text()),self.hypothesis)
        self.assertTrue(result['passed'])
        self.assertEqual(result['measured_plots'],32)
        self.assertEqual(result['observed_layers'],160)
        self.assertFalse(any(r['stop_rule_tested'] for r in result['plots']))

    def test_integer_boundary_and_small_state_ambiguity(self):
        self.assertEqual(len(layer_mask(8)),77)
        self.assertEqual(len(layer_mask(10)),109)
        self.assertEqual(len(layer_mask(12)),145)
        self.assertEqual(layer_mask(1),layer_mask(2))
        self.assertIn((4,3),layer_mask(8))
        self.assertNotIn((5,0),layer_mask(8))
        self.assertNotIn((4,4),layer_mask(8))
        for q in [0,13,1.5]:
            with self.assertRaises(ValueError):layer_mask(q)

    def test_masks_and_counts_alone_do_not_pass_wrong_transitions(self):
        # q8→q4: обе маски допустимы, видимый шаг4 запрещён.
        cells=[(x,0,z) for x,z in layer_mask(8)]+[(x,-1,z) for x,z in layer_mask(4)]
        result=check_shape(cells,self.hypothesis)
        self.assertFalse(result['passed'])
        self.assertTrue(all(result['layer_q_candidates']))
        self.assertFalse(result['q_paths'])

    def test_holes_and_changed_boundary_are_rejected(self):
        cells=[(x,0,z) for x,z in layer_mask(8)]
        altered=[c for c in cells if c!=(-4,0,2)]+[(4,0,4)]
        self.assertFalse(check_shape(altered,self.hypothesis)['passed'])
        self.assertFalse(check_shape(cells+[(0,-2,0)],self.hypothesis)['passed'])
        self.assertFalse(check_shape(cells+[cells[0]],self.hypothesis)['passed'])

    def test_no_hidden_stop_rule_or_generated_layer_limit(self):
        for q_values in [[8],[12,11,10,9,8,7,6,5,4,3,2],[12,11,10,9,8,7,6,5,4,3,2,1]]:
            cells=[(x,-index,z) for index,q in enumerate(q_values) for x,z in layer_mask(q)]
            result=check_shape(cells,self.hypothesis)
            self.assertTrue(result['passed'])
            self.assertIn(q_values,result['q_paths'])
            self.assertFalse(result['stop_rule_tested'])

    def test_partial_or_duplicated_series_never_passes(self):
        cells=[(x,0,z) for x,z in layer_mask(8)]
        for ids in [[0],[0]*32,list(range(31))]:
            result=validate_dataset({'observations':[{'plot_index':i,'cells':cells} for i in ids]},self.hypothesis)
            self.assertFalse(result['complete'])
            self.assertFalse(result['passed'])


if __name__ == '__main__':
    unittest.main()
