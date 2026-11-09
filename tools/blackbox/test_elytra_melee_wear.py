"""Негативные контроли: нельзя принять потерю элитр за корректный износ."""
import unittest
from elytra_melee_wear import CASES,validate

class ElytraMeleeControls(unittest.TestCase):
    def rows(self):
        return [dict(trial=i,item=item,initial_damage=initial,unbreaking=level,
                     final_damage=1 if item=='diamond_chestplate' else initial,
                     broken_by_observer=[0,0],post_hit_item_echo=True)
                for i,(item,initial,level) in enumerate(CASES)]
    def test_positive_original_result(self):validate(self.rows())
    def test_rejects_extra_wear_break_and_missing_echo(self):
        for key,value in [('final_damage',1),('final_damage',None),('broken_by_observer',[1,1]),('post_hit_item_echo',False)]:
            rows=self.rows();rows[1][key]=value
            with self.assertRaises(AssertionError):validate(rows)
    def test_rejects_missing_case_or_wrong_input(self):
        for key,value in [('item','diamond_chestplate'),('initial_damage',1),('unbreaking',3),('trial',7)]:
            rows=self.rows();rows[1][key]=value
            with self.assertRaises(AssertionError):validate(rows)
        with self.assertRaises(AssertionError):validate(self.rows()[:-1])
