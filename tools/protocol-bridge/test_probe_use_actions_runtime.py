import io
import sys
import unittest
from unittest.mock import patch
from probe_use_actions_runtime import lpvec,use_slot,validate,ActionProbe,menu_slot
from smoke import varint

class Tests(unittest.TestCase):
    names={'bow':1,'arrow':2,'iron_spear':3}
    def test_zero_velocity(self):self.assertEqual(lpvec(io.BytesIO(b'\0')),[0.,0.,0.])
    def test_truncated_velocity(self):
        with self.assertRaises(ValueError):lpvec(io.BytesIO(b'\1'))
    def test_offhand(self):self.assertEqual(menu_slot(40),45)
    def test_slots(self):
        raw=b''.join(varint(v) for v in [1,3,2,0,3,1,13,1,9,1])
        self.assertEqual(use_slot(io.BytesIO(raw)),{'item':3,'count':1,'damage':1,'enchantments':[[9,1]]})
    def test_unknown_component(self):
        with self.assertRaises(AssertionError):use_slot(io.BytesIO(bytes([1,3,1,0,22])))
    def test_bow(self):validate('bow-release',[{'name':'arrow','entity':8}],{36:{'item':1,'count':1,'damage':1},45:{'item':2,'count':2}},self.names,4)
    def test_duplicate_bow(self):
        with self.assertRaises(AssertionError):validate('bow-release',[{'name':'arrow','entity':8},{'name':'arrow','entity':9}],{36:{'item':1,'count':1,'damage':1},45:{'item':2,'count':2}},self.names,4)
    def test_spear(self):validate('spear-jab',[{'name':'entity_velocity','entity':4,'velocity':[.458,0.,0.]}],{36:{'item':3,'count':1,'damage':1},45:None},self.names,4)
    def test_other_entity_velocity(self):
        with self.assertRaises(AssertionError):validate('spear-jab',[{'name':'entity_velocity','entity':5,'velocity':[.458,0.,0.]}],{36:{'item':3,'count':1,'damage':1},45:None},self.names,4)
    def test_duplicate_impulse(self):
        with self.assertRaises(AssertionError):validate('spear-jab',[{'name':'entity_velocity','entity':4,'velocity':[.458,0.,0.]}]*2,{36:{'item':3,'count':1,'damage':1},45:None},self.names,4)
    def test_malformed_unchanged(self):validate('malformed-bow-release',[],{36:{'item':1,'count':1},45:{'item':2,'count':3}},self.names,4)
    def test_malformed_explicit_zero(self):validate('malformed-bow-release',[],{36:{'item':1,'count':1,'damage':0},45:{'item':2,'count':3}},self.names,4)
    def test_malformed_old_mutation(self):
        with self.assertRaises(AssertionError):validate('malformed-bow-release',[{'name':'arrow','entity':8}],{36:{'item':1,'count':1,'damage':1},45:{'item':2,'count':2}},self.names,4)
    def test_plan_no_socket(self):
        import probe_use_actions_runtime as p
        with patch.object(sys,'argv',['probe','--case','bow-release']),patch.object(p.socket,'socket',side_effect=AssertionError('socket forbidden')):p.main()

if __name__=='__main__':unittest.main()
