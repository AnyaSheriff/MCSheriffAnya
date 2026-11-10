"""Synthetic guard checks, not actual nativeGUI/core proof."""
import io
import struct
import unittest
from unittest.mock import Mock
from capture_headless_768 import Recording, valid_port, pose, MAX_PACKETS, MAX_BYTES

class Guards(unittest.TestCase):
    def test_protected_ports(self):
        for value in (25565,25566,25567,27000,True):
            with self.assertRaises(ValueError): valid_port(value)
    def test_pose_rejects_relative_nan_trailing_short(self):
        good=b'\x01'+struct.pack('>ddddddffI',1,2,3,0,0,0,0,0,0)
        self.assertEqual(pose(good),(1,(1.0,2.0,3.0,0.0,0.0)))
        for raw in (good[:-1],good+b'\0',good[:-4]+struct.pack('>I',1),b'\x01'+struct.pack('>ddddddffI',float('nan'),2,3,0,0,0,0,0,0)):
            with self.assertRaises((ValueError,EOFError)):pose(raw)
    def test_packet_and_byte_budget_before_observer(self):
        r=object.__new__(Recording);r.rows=[None]*MAX_PACKETS;r.bytes=0;r.observer=Mock()
        with self.assertRaises(ValueError):r.add('toClient',b'\0')
        r.rows=[];r.bytes=MAX_BYTES
        with self.assertRaises(ValueError):r.add('toClient',b'\0')
        r.observer.observe.assert_not_called()
    def test_readiness_requires_real_registry(self):
        r=object.__new__(Recording);r.observer=Mock();r.observer.complete.return_value=True;r.registries=[]
        self.assertFalse(r.ready())
        r.registries=['synthetic'];self.assertTrue(r.ready())
        r.observer.complete.return_value=False;self.assertFalse(r.ready())

if __name__=='__main__':unittest.main()
