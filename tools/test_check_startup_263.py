"""Guards for the smoke harness; no Minecraft process or network traffic."""
import io
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import check_startup_263 as smoke


class SmokeGuards(unittest.TestCase):
    def test_varint_boundaries(self):
        for value in [0, 127, 128, 777, 65535, 2147483647]:
            self.assertEqual(smoke.read_varint(io.BytesIO(smoke.varint(value))), value)

    def test_truncated_and_oversized_varints_fail(self):
        for value, error in [(b'\x80', EOFError), (b'\x80' * 5, ValueError)]:
            with self.assertRaises(error):
                smoke.read_varint(io.BytesIO(value))

    def test_snapshot_detects_world_rewrite_and_added_file(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            marker = root / 'old-world'
            marker.write_bytes(b'old')
            before = smoke.snapshot(root)
            marker.write_bytes(b'new')
            self.assertNotEqual(before, smoke.snapshot(root))
            marker.write_bytes(b'old')
            (root / 'session.lock').touch()
            self.assertNotEqual(before, smoke.snapshot(root))

    def test_host_network_rejected_before_process_launch(self):
        with tempfile.TemporaryDirectory() as temporary:
            with patch('sys.argv', ['check', '--binary', '/does-not-exist', '--output', temporary]), \
                 patch.object(smoke.socket, 'if_nameindex', return_value=[(1, 'lo'), (2, 'eth0')]), \
                 patch.object(smoke.subprocess, 'Popen') as process:
                with self.assertRaisesRegex(AssertionError, 'unshare'):
                    smoke.main()
                process.assert_not_called()


if __name__ == '__main__':
    unittest.main()
