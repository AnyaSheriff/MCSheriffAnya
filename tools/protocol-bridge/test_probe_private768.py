"""Bounded own inventory assertion parser; no game/runtime execution."""
import io
import unittest
from probe_private768 import CoreObserver,stone_slot

class Guards(unittest.TestCase):
    def test_stone_slot_is_typed_and_rejects_unknown_component_or_item(self):
        self.assertIsNone(stone_slot(io.BytesIO(b'\0'),1))
        self.assertEqual(stone_slot(io.BytesIO(b'\5\1\0\0'),1),{'item':1,'count':5})
        for raw in (b'\5\2\0\0',b'\5\1\1\0',b'\5\1\0\1',b'\x41\1\0\0',b'\5'):
            with self.assertRaises((ValueError,EOFError)):stone_slot(io.BytesIO(raw),1)
    def test_46_slot_inventory_and_carried_boundaries(self):
        observer=object.__new__(CoreObserver);observer.stone=1
        raw=b'\0\0\x2e'+b'\0'*46+b'\0'
        self.assertEqual(observer.decode_inventory(raw)['slots'],46)
        self.assertEqual(sum(s['count'] for s in observer.slots if s),0)
        stone=raw[:3]+b'\0'*36+b'\5\1\0\0'+b'\0'*10
        observer.decode_inventory(stone);self.assertEqual(observer.slots[36],{'item':1,'count':5})
        for altered in (raw[:-1],raw+b'\0',b'\1'+raw[1:],raw[:2]+b'\x2d'+raw[3:]):
            with self.assertRaises((ValueError,EOFError)):observer.decode_inventory(altered)

if __name__=='__main__':unittest.main()
