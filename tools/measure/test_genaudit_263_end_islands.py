"""Независимые проверки геометрии/цензурирования; без Minecraft и Cargo."""
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import numpy as np

import genaudit_263_end_islands as islands


def sample_cells(radius=4):
    # Измеренная верхняя маска; два слоя дают легко проверяемый объём154.
    top = [(x, z) for x in range(-radius, radius+1) for z in range(-radius, radius+1)
           if x*x+z*z <= (radius+1)**2]
    return [(x, y, z) for x, z in top for y in [-1, 0]]


def sample_bank():
    return islands.bank_from_data({'schema': 1, 'observations': [
        {'plot_index': 7, 'radius': 4, 'cells': sample_cells()}]})


class IslandTests(unittest.TestCase):
    def maps(self):
        top = np.full((64,64),-1,np.int16)
        return top, top.copy(), np.full(top.shape,'small_end_islands',object)

    def test_edge_clipped_connected_large_and_height_exclusions(self):
        top, bottom, biomes = self.maps()
        top[0,2:5] = bottom[0,2:5] = 60  # Край окна.
        top[10,5:25] = bottom[10,5:25] = 60  # Длинная связная полоса.
        top[20,20] = bottom[20,20] = 54
        top[30,30] = bottom[30,30] = 60
        result = islands.component_candidates(top,bottom,biomes,(-2,-2,1,1))
        self.assertEqual(result['components'],4)
        self.assertEqual(result['excluded_components'],{'edge_clipped':1,'large_or_connected':1,'top_outside55_70':1})
        self.assertEqual(result['candidates'][0]['center_proxy'],[-2,60,-2])

    def test_diagonal_contacts_are_separate(self):
        top,bottom,bios=self.maps()
        for z,x in [(20,20),(21,21)]:top[z,x]=bottom[z,x]=60
        result=islands.component_candidates(top,bottom,bios,(-2,-2,1,1))
        self.assertEqual(result['components'],2)
        self.assertEqual(len(result['candidates']),2)

    def test_full_mask_and_exact_cells_not_only_equal_counts(self):
        bank=sample_bank();cells=sample_cells()
        result=islands.classify_cells(cells,(0,0,0),bank)
        self.assertEqual(result['volume'],154)
        self.assertTrue(result['matches_observed_shape_properties'])
        self.assertEqual(result['exact_placed_model_plots'],[7])
        self.assertNotIn('cells',result)
        altered=[c for c in cells if c != (-4,0,2)] + [(4,0,4)]
        other=islands.classify_cells(altered,(0,0,0),bank)
        self.assertEqual(other['volume'],154)
        self.assertFalse(other['top_mask_matches_placed_class'])
        self.assertFalse(other['exact_placed_model_match'])
        self.assertNotEqual(other['relative_cells_sha256'],result['relative_cells_sha256'])

    def test_discontinuous_column_rejected_despite_matching_top(self):
        bank=sample_bank();cells=sample_cells()+[(0,-3,0)]
        result=islands.classify_cells(cells,(0,0,0),bank)
        self.assertTrue(result['top_mask_matches_placed_class'])
        self.assertFalse(result['all_columns_contiguous_to_top'])
        self.assertFalse(result['matches_observed_shape_properties'])
        with self.assertRaises(ValueError):islands.classify_cells(cells+[(0,-3,0)],(0,0,0),bank)
        with self.assertRaises(ValueError):islands.classify_cells([], (0,0,0),bank)

    def test_negative_translation_preserves_shape_hash_and_cells(self):
        bank=sample_bank();original=islands.classify_cells(sample_cells(),(0,0,0),bank)
        translated=[(x-17,y+60,z-1) for x,y,z in sample_cells()]
        moved=islands.classify_cells(translated,(-17,60,-1),bank,include_cells=True)
        self.assertEqual(original['relative_cells_sha256'],moved['relative_cells_sha256'])
        self.assertEqual(moved['exact_placed_model_plots'],[7])
        self.assertEqual({tuple(c) for c in moved['cells']},set(translated))
        self.assertEqual({(x//16,z//16) for x,y,z in moved['cells']},{(-2,-1),(-2,0),(-1,-1),(-1,0)})

    def test_proxy_rate_uses_chunk_center_for_both_numerator_and_denominator(self):
        top,bottom,bios=self.maps()
        top[30,30]=bottom[30,30]=60
        bios[30,30]='end_barrens'  # Биом центра самой компоненты иной.
        result=islands.component_candidates(top,bottom,bios,(-2,-2,1,1))
        candidate=result['candidates'][0]
        self.assertEqual(candidate['center_biome_y64'],'end_barrens')
        self.assertEqual(candidate['center_chunk_center_biome_y64'],'small_end_islands')
        candidate['matches_observed_shape_properties']=True
        rates=islands.proxy_rates([candidate,candidate],result['eligible_proxy_halo_chunks_by_center_biome_y64'])
        self.assertEqual(rates,[{'center_chunk_center_biome_y64':'small_end_islands','chunk_denominator':4,
            'isolated_candidate_count':2,'shape_property_candidate_count':2,
            'distinct_center_chunks_with_property_candidate':1,'property_candidates_per_chunk':0.5}])
        candidate['center_chunk_has_full_halo']=False
        self.assertEqual(islands.proxy_rates([candidate],{'small_end_islands':4})[0]['isolated_candidate_count'],0)

    def test_no_bank_and_empty_windows_do_not_imply_zero_observed_matches(self):
        result=islands.classify_cells(sample_cells(),(0,0,0))
        self.assertIsNone(result['matches_observed_shape_properties'])
        self.assertIsNone(result['exact_placed_model_match'])
        self.assertIsNone(islands.proxy_rates([],{'small_end_islands':4},False)[0]['shape_property_candidate_count'])
        self.assertEqual(islands.window_summary([],True)['property_matches'],0)
        self.assertIsNone(islands.window_summary([],False)['property_matches'])

    def test_bank_columns_and_saved_cells_agree(self):
        data={'schema':1,'models':[{'radius':4,'plot_index':7,'columns':
            [[x,z,-1,0] for x,z in sorted({(x,z) for x,y,z in sample_cells()})]}]}
        columns=islands.bank_from_data(data)
        self.assertEqual(columns.exact,sample_bank().exact)
        self.assertEqual(columns.top_masks,sample_bank().top_masks)
        data['models'][0]['columns'].append([0,0,-1,0])
        with self.assertRaises(ValueError):islands.bank_from_data(data)

    def test_malformed_maps_bounds_and_bank_rejected(self):
        for bounds in [(1,0,0,0),(0,0,1),(0,0,1.5,2)]:
            with self.assertRaises(ValueError):islands.checked_bounds(bounds)
        top,bottom,bios=self.maps()
        with self.assertRaises(ValueError):islands.component_candidates(top,bottom,bios,(0,0,0,0))
        top[20,20],bottom[20,20]=60,61
        with self.assertRaises(ValueError):islands.component_candidates(top,bottom,bios,(-2,-2,1,1))
        with self.assertRaises(ValueError):islands.bank_from_data({'schema':1,'models':[]})
        with self.assertRaises(ValueError):islands.bank_from_data({'schema':1,'observations':[{'radius':5,'cells':sample_cells()}]})

    def test_overlapping_windows_rejected_before_io(self):
        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaisesRegex(ValueError,'Перекрывающиеся'):
                islands.analyze_world(Path(tmp),[(0,0,2,2),(2,2,4,4)])

    def test_missing_and_duplicate_saved_chunks_fail_closed(self):
        class FakeReader:
            lookup={'end_stone':1}
            def read(self,file,wanted):
                blocks=np.zeros((384,16,16),np.uint16)
                bios=np.full((96,4,4),'small_end_islands',object)
                yield 0,0,blocks,bios,{}
                yield 0,0,blocks,bios,{}
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp)
            with patch('genaudit_263_io.region_dir',return_value=folder):
                with self.assertRaisesRegex(ValueError,'Неполное покрытие'):
                    islands.analyze_window(folder,(0,0,0,0),FakeReader())
                (folder/'r.0.0.mcs').write_bytes(b'fake test region')
                with self.assertRaisesRegex(ValueError,'Повтор сохранённого'):
                    islands.analyze_window(folder,(0,0,0,0),FakeReader())

    def test_committed_observations_reconstruct_all_32_exact(self):
        bank_path=Path(__file__).resolve().parents[2]/'testdata/world-profile/genaudit-26.3/end-island-shapes-observations-26.3.json'
        bank=islands.load_bank(bank_path)
        self.assertEqual(bank.metadata['models'],32)
        self.assertEqual(bank.metadata['distinct_forms'],26)
        for cells in bank.exact:
            result=islands.classify_cells(cells,(0,0,0),bank)
            self.assertTrue(result['exact_placed_model_match'])
            self.assertTrue(result['matches_observed_shape_properties'])


if __name__ == '__main__':
    unittest.main()
