"""Own guard tests; synthetic inputs never count as reference capture."""
import io
import unittest
from unittest.mock import patch
from capture_native_768 import uint, empty_inventory, validate_ports, Observer, require_runtime, read_handshake

class Guards(unittest.TestCase):
    def test_execution_refused_without_approved_native_runtime(self):
        with self.assertRaisesRegex(RuntimeError, "BLOCKED_NATIVE_RUNTIME"): require_runtime()
    def test_native_exception_is_explicit_and_github_only(self):
        with patch.dict('os.environ', {}, clear=True):
            with self.assertRaisesRegex(RuntimeError, 'GitHub ONLY'): require_runtime(True)
        with patch.dict('os.environ', {'GITHUB_ACTIONS':'true'}, clear=True):
            require_runtime(True)
            for value in (False, None, 'true', 1):
                with self.assertRaisesRegex(RuntimeError, 'BLOCKED_NATIVE_RUNTIME'): require_runtime(value)
    def test_quickplay_status_is_distinct_from_login(self):
        from smoke import varint
        from probe_registry_client import string
        base=b'\0'+varint(768)+string('127.0.0.1')+(26269).to_bytes(2,'big')
        self.assertEqual(read_handshake(base+b'\1',26269),1)
        self.assertEqual(read_handshake(base+b'\2',26269),2)
        for raw in (base+b'\3',base+b'\2\0',base[:-1]+b'\0\2',base[:-1]):
            with self.assertRaises((ValueError,EOFError)):read_handshake(raw,26269)
    def test_varint_negative_overflow_truncated(self):
        for raw in (b'\xff\xff\xff\xff\x0f', b'\x80', b'\x80'*5):
            with self.assertRaises((ValueError, EOFError)): uint(io.BytesIO(raw))
    def test_ports_protected_duplicate_range(self):
        for a,b in ((25565,26269),(26268,26268),(26999,27000),(True,26269)):
            with self.assertRaises(ValueError): validate_ports(a,b)
        validate_ports(26268,26269)
    def test_inventory_exact_46_and_carried(self):
        raw=b'\0\0\x2e'+b'\0'*47
        self.assertEqual(empty_inventory(raw)['slots'],46)
        for altered in (raw[:-1], raw+b'\0', b'\1'+raw[1:], raw[:2]+b'\x2d'+raw[3:], raw[:-1]+b'\1'):
            with self.assertRaises((ValueError, EOFError)): empty_inventory(altered)
    def test_markers_not_ready(self):
        observer=object.__new__(Observer)
        observer.phase='play';observer.acks=set();observer.inventory=None
        observer.rows=[{'phase':'play','direction':'toClient','name':name} for name in ('login','player_info','map_chunk')]
        self.assertFalse(observer.complete())
        observer.acks={1};observer.inventory={'slots':46}
        self.assertFalse(observer.complete())
        observer.rows += [{'phase':'play','direction':'toServer','name':'chunk_batch_received'}]
        self.assertFalse(observer.complete())
        observer.rows += [{'phase':'play','direction':'toServer','name':'position'}]
        self.assertTrue(observer.complete())

if __name__=='__main__': unittest.main()
