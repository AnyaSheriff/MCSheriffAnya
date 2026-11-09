"""Негативные контроли: согласие наблюдателей само по себе ещё не паритет."""
import copy
import unittest
from thorns_multi_sample import validate

class MultiThornsControls(unittest.TestCase):
    def row(self):return {'pieces':2,'reflections_by_observer':[1,1],'attacker_hurt_animations':1,'reflected_damage':3.,'wear':{5:3,6:3}}
    def test_observed_positive_controls(self):validate([self.row()])
    def test_rejects_duplicate_even_when_both_observers_agree(self):
        row=self.row();row['reflections_by_observer']=[2,2]
        with self.assertRaises(AssertionError):validate([row])
    def test_rejects_health_animation_and_wear_corruption(self):
        for key,value in [('reflections_by_observer',[2,1]),('attacker_hurt_animations',2),('reflected_damage',6.),('wear',{5:3,6:1})]:
            row=self.row();row[key]=value
            with self.assertRaises(AssertionError):validate([row])

if __name__=='__main__':unittest.main()
