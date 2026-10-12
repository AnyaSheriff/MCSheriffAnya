"""Регрессии границ, повреждений и учёта кусочков сохранённого мира."""
import struct
import unittest
import zlib
from structures_audit2 import expected, decode, valid, box_chunks, piece_record


class Audit2Tests(unittest.TestCase):
    def test_halo_is_not_denominator(self):
        core, halo = expected(), expected(8)
        self.assertEqual(len(core), 49152)
        self.assertEqual(len(halo), 71936)
        self.assertTrue(core < halo)
        self.assertNotIn((64, 0), core)
        self.assertIn((64, 0), halo)

    def test_bounds_check_interior_not_only_corners(self):
        chunks = box_chunks([-17, -20, -1, 17, 30, 32])
        self.assertEqual(len(chunks), 16)
        full = chunks-{(0, 0)}
        self.assertFalse(chunks <= full)

    def test_truncated_and_missing_records(self):
        self.assertIsNone(decode(bytes(8192), 0))
        with self.assertRaises(ValueError): decode(b'', 0)
        data = bytearray(12288)
        data[:4] = b'\x00\x00\x02\x01'
        struct.pack_into('>I', data, 8192, 5000)
        with self.assertRaises(ValueError): decode(data, 0)
        raw = b'\x0a\x00\x00\x00'
        zipped = zlib.compress(raw)
        struct.pack_into('>I', data, 8192, len(zipped)+1)
        data[8196] = 2
        data[8197:8197+len(zipped)] = zipped
        self.assertEqual(decode(data, 0), {})

    def test_nonfull_or_wrong_coordinate_is_rejected(self):
        self.assertEqual(valid(None, 8, -524), 'missing')
        self.assertEqual(valid({'Status':'minecraft:features'}, 0, 0), 'status:minecraft:features')
        self.assertEqual(valid({'Status':'full','xPos':0,'zPos':1}, 0, 0), 'coordinate mismatch')

    def test_piece_rotation_and_ground_delta_survive(self):
        p = piece_record({'BB':[0,3,0,4,9,4], 'rotation':'CLOCKWISE_90', 'ground_level_delta':1,
                          'pool_element':{'location':'minecraft:village/taiga/houses/taiga_small_house_1', 'projection':'rigid'}})
        self.assertEqual(p['rotation'], 'CLOCKWISE_90')
        self.assertEqual(p['ground_delta'], 1)
        self.assertEqual(p['projection'], 'rigid')




class ReportTests(unittest.TestCase):
    def test_deferred_road_height_is_not_measured_as_world_zero(self):
        from structures_audit2_report import boxof, geometry_ours
        road={'name':'road','projection':'terrain_matching','box':[-5,0,-5,5,1,5]}
        house={'name':'house','projection':'rigid','box':[0,70,0,4,78,4]}
        hay={'name':'feature:pile_hay','box':[-2,0,-2,2,1,2]}
        start={'piece_records':[road,house,hay]}
        self.assertEqual(boxof(start),[-5,70,-5,5,78,5])
        self.assertEqual([p['datum_y'] for p in geometry_ours(start,{'samples':{}})],[70])

    def test_nn_uses_halo_and_censors_unobserved_neighbors(self):
        from structures_audit2_report import nearest
        rows=[{'window':'0,0','chunk':[63,0]}, {'window':None,'chunk':[65,0]}]
        self.assertEqual(nearest(rows)['median'],32)
        self.assertEqual(nearest(rows)['n'],1)
        self.assertEqual(nearest(rows[:1])['censored'],1)

    def test_window_bootstrap_is_reproducible_and_handles_zero(self):
        from structures_audit2_report import interval
        self.assertEqual(interval([0]*9,[4096]*9),[0,0])
        first=interval([4]*9,[4096]*9)
        self.assertEqual(first,interval([4]*9,[4096]*9))
        self.assertEqual(first[0],first[1])



class LootAssociationTests(unittest.TestCase):
    def test_pyramid_basement_is_not_lost_below_metadata_box(self):
        from structures_audit2 import attach_loot
        chest={'x':4,'y':56,'z':4,'LootTable':'minecraft:chests/desert_pyramid'}
        dungeon={'x':4,'y':30,'z':4,'LootTable':'minecraft:chests/simple_dungeon'}
        s={'kind':'desert_pyramid','box':[0,67,0,20,81,20],'block_entities':[], 'entities':[]}
        attach_loot({'starts':[s],'block_entities':[chest,dungeon]})
        self.assertEqual(s['compatible_loot'],[chest])
        self.assertEqual(s['loot_outside_saved_box'],[chest])

class IncompleteFullChunkTests(unittest.TestCase):
    def test_full_flag_does_not_hide_incomplete_sections(self):
        root={'Status':'full','DataVersion':4790,'xPos':8,'zPos':-524,'yPos':-4,
              'sections':[{'Y':252,'block_states':{'palette':[{'Name':'minecraft:stone'}]}}],
              'Heightmaps':{'OCEAN_FLOOR':[0]*37}}
        self.assertEqual(valid(root,8,-524),'missing block-state sections')
        root['sections']=[{'Y':y%256,'block_states':{'palette':[{'Name':'minecraft:stone'}]}} for y in range(-4,20)]
        self.assertIsNone(valid(root,8,-524))
        root['sections'][0]['block_states']['palette'].append({'Name':'minecraft:air'})
        self.assertEqual(valid(root,8,-524),'invalid block_states packed length')

if __name__ == '__main__': unittest.main()
