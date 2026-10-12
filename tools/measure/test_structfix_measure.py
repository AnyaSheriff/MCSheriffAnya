import unittest
from structfix_villages import local, measure

class VillageMarkers(unittest.TestCase):
    def test_inverse_rotation_uses_piece_origin_not_box_minimum(self):
        p={'origin':[12,60,-8],'rotation':'CLOCKWISE_90'}
        self.assertEqual(local(p,[9,62,-6]),(2,2,3))

    def test_entities_do_not_replace_explicit_baby_marker(self):
        house=dict(name='minecraft:village/taiga/houses/test',origin=[-16,60,-16],box=[-16,60,-16,-10,67,-10],rotation='NONE',projection='rigid')
        baby=dict(name='minecraft:village/taiga/villagers/baby',origin=[-14,61,-14],box=[-14,61,-14,-14,62,-14],rotation='NONE',projection='rigid')
        data={'starts':[dict(kind='village_taiga',chunk=[-1,-1],complete_bounds=True,pieces=[house,baby],block_entities=[],entities=[{'VillagerData':{'profession':'farmer'}}])]}
        asset,report=measure(data)
        self.assertEqual(asset['houses']['taiga/houses/test'],[((2,1,2,'baby'),)])
        self.assertEqual(report['markers']['baby'],1)

    def test_unattached_markers_are_reported_not_moved_to_a_house(self):
        baby=dict(name='minecraft:village/taiga/villagers/baby',origin=[0,61,0],box=[0,61,0,0,62,0],rotation='NONE',projection='rigid')
        asset,report=measure({'starts':[dict(kind='village_taiga',chunk=[0,0],complete_bounds=True,pieces=[baby],block_entities=[])]})
        self.assertFalse(asset['houses'])
        self.assertEqual(len(report['unattached']),1)

if __name__=='__main__':unittest.main()
