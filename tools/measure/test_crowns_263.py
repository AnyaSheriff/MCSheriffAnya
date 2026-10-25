"""Проверки измерителя на малых наблюдаемых формах."""
import unittest
from crowns_263 import sample
from crown_statistics import metrics
from pack_crowns_263 import pack

class Crowns(unittest.TestCase):
    def test_default_axis_and_trunk_reference(self):
        blocks={(0,y,0):('poplar_log',{}) for y in range(200,207)}
        blocks[1,206,0]=('yellow_poplar_leaves',{'distance':'1'})
        tree=sample(blocks,0,0)
        self.assertEqual(tree['trunk'],7)
        self.assertEqual(tree['leaves'],[[1,0,0,1]])

    def test_zero_layers_count_as_zero(self):
        tree={'trunk':2,'logs':[[0,-1,0,'y'],[0,0,0,'y']], 'leaves':[[0,1,0,1]]}
        m=metrics(tree)
        self.assertEqual(m['layer_count:-4'],0)
        self.assertEqual(m['layer_corner:1'],0)
        self.assertEqual(m['layer_count:1'],1)

    def test_mask_keeps_holes_and_negative_coordinates(self):
        tree={'trunk':7,'logs':[[0,0,0,None]],'leaves':[[-8,-4,-2,6],[0,-4,-2,1],[8,-4,-2,6]]}
        raw={n:[tree] for n in ['acacia','birch','fancy_oak','natural_poplar','oak','poplar','spruce','swamp_oak']}
        model=pack(raw)[0]
        self.assertEqual(model['rows'],[[-4,-2,1<<8,1],[-4,-2,(1<<16)|1,6]])
        self.assertEqual(model['logs'],[[0,0,0,1]])

if __name__=='__main__':unittest.main()
