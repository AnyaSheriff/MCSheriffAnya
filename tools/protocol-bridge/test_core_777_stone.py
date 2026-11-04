import copy
import unittest
from probe_core_777_stone import validate_stone

class Tests(unittest.TestCase):
    def setUp(self):
        self.events=[{'name':'block_change','position':[2,65,0],'state':0},
                     {'name':'entity_metadata','entity':1,'slot':{'item':62,'count':1}}]
        self.inventory={36:{'item':1052,'count':1,'damage':1}}
    def test_actual_semantics_require_air_tool_damage_one_cobble(self):
        self.assertEqual(validate_stone(self.events,self.inventory,1052,62),{1:{'item':62,'count':1}})
    def test_before_passthrough_tool_throw_is_failure(self):
        events=[{'name':'entity_metadata','entity':1,'slot':{'item':1052,'count':1}}]
        with self.assertRaises(AssertionError):validate_stone(events,{36:None},1052,62)
        events=self.events+[{'name':'entity_metadata','entity':2,'slot':{'item':1052,'count':1}}]
        with self.assertRaises(AssertionError):validate_stone(events,self.inventory,1052,62)
    def test_missing_air_wrong_tool_damage_or_drop_count_is_failure(self):
        for events,inventory in [(self.events[1:],self.inventory),
            (self.events,{36:{'item':1052,'count':1}}),
            (self.events,{36:{'item':1052,'count':1,'damage':2}}),
            (self.events+[{'name':'entity_metadata','entity':2,'slot':{'item':62,'count':1}}],self.inventory)]:
            with self.assertRaises(AssertionError):validate_stone(events,inventory,1052,62)
    def test_duplicate_metadata_for_one_drop_is_not_double_counted(self):
        self.assertEqual(validate_stone(self.events+[copy.deepcopy(self.events[1])],self.inventory,1052,62),
                         {1:{'item':62,'count':1}})

if __name__=='__main__':unittest.main()
