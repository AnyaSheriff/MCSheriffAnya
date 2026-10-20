import io
import unittest
from probe_player_actions_runtime import menu_slot,validate

class Tests(unittest.TestCase):
    def test_both_slot_spaces(self):
        self.assertEqual([menu_slot(i) for i in [0,8,9,35,36,39,40]],[36,44,9,35,8,5,45])
        for bad in [-1,41,127]:
            with self.assertRaises(ValueError):menu_slot(bad)
    def test_swap_preserves_both_counts_and_no_drop(self):
        inv={36:{'item':3,'count':5},45:{'item':2,'count':3}};names={'gold_ingot':3,'iron_ingot':2}
        self.assertEqual(validate('swap',[],inv,names,set()),{})
        with self.assertRaises(AssertionError):validate('swap',[],{36:inv[36]},names,set())
        with self.assertRaises(AssertionError):validate('swap',[{'name':'entity_metadata','entity':7,'slot':{'item':3,'count':1}}],inv,names,set())
    def test_drop_requires_exact_count_and_remaining_hand(self):
        event={'name':'entity_metadata','entity':7,'slot':{'item':2,'count':1}}
        inv={36:{'item':2,'count':6}};names={'stone':2,'cobblestone':4}
        validate('drop_one',[event],inv,names,set())
        for count in [0,2,7]:
            with self.assertRaises(AssertionError):validate('drop_one',[dict(event,slot={'item':2,'count':count})],inv,names,set())
        with self.assertRaises(AssertionError):validate('drop_one',[event],{36:None},names,set())
    def test_cancel_requires_authoritative_retained_target(self):
        validate('cancel',[{'name':'acknowledge_player_digging','sequence':0}],{}, {}, {'MCSA_ACTION_TARGET_RETAINED'})
        with self.assertRaises(AssertionError):validate('cancel',[],{}, {},set())
        with self.assertRaises(AssertionError):validate('cancel',[{'name':'acknowledge_player_digging','sequence':0},{'name':'block_change','position':[2,65,0],'state':0}],{}, {},{'MCSA_ACTION_TARGET_RETAINED'})

if __name__=='__main__':unittest.main()
