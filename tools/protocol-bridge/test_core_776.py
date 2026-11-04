import unittest
import subprocess
import sys
from unittest.mock import patch
from probe_core_776 import Probe776, PORT, plan, stop_server, verify_stale_pickup
from smoke import varint


class Core776Tests(unittest.TestCase):
    def test_plan_keeps_client_and_native_distinct(self):
        value = plan()
        self.assertEqual((value['protocol'], value['native_protocol'], value['port']), (776, 775, 26028))
        self.assertEqual(PORT, 26028)
        self.assertIn('primary776', value['excluded'])

    def test_confirmation_has_only_varint(self):
        client = object.__new__(Probe776)
        client.acks = []
        with patch.object(client, 'send') as send:
            client.confirm_teleport(129, [1., 2., 3., 4., 5.])
            send.assert_called_once_with('teleport_confirm', varint(129))
        self.assertEqual(client.acks, [129])

    def test_ping_and_chat_observation_reuses_parent(self):
        client = object.__new__(Probe776)
        client.names = []; client.markers = []
        with patch.object(client, 'send') as send:
            client.observe_packet('ping', bytes([0, 0, 0, 7]))
            send.assert_called_once_with('pong', bytes([0, 0, 0, 7]))
        client.observe_packet('system_chat', b'own mcsa776-smoke')
        self.assertEqual(client.markers, ['mcsa776-smoke'])
        self.assertEqual(client.names, ['ping', 'system_chat'])


    def test_stop_closes_stdin_for_a_real_process_waiting_for_eof(self):
        # Настоящий дочерний процесс: stop принят, но выход требует EOF.
        code = "import sys; assert sys.stdin.readline() == 'stop\\n'; assert sys.stdin.read() == ''; print('saw-stop-and-eof')"
        child = subprocess.Popen([sys.executable, '-c', code], stdin=subprocess.PIPE,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, start_new_session=True)
        try:
            self.assertEqual(stop_server(child), 'stop-and-stdin-eof')
            self.assertEqual(child.returncode, 0)
            self.assertEqual(child.stdout.read().strip(), 'saw-stop-and-eof')
            self.assertEqual(child.stderr.read(), '')
        finally:
            if child.poll() is None:
                child.kill(); child.wait(timeout=5)
            child.stdout.close(); child.stderr.close()

    def test_stop_does_not_replace_an_existing_failure_exit(self):
        child = subprocess.Popen([sys.executable, '-c', 'raise SystemExit(7)'],
            stdin=subprocess.PIPE, text=True, start_new_session=True)
        child.wait(timeout=5)
        self.assertEqual(stop_server(child), 'already-exited')
        self.assertEqual(child.returncode, 7)



    def test_stale_pickup_matches_observed_ordinary776_and_conserves_items(self):
        inventory = {'window': 0, 'slots': [None] * 46, 'cursor': {'item': 1, 'count': 5}}
        verify_stale_pickup(inventory, 1)
        for slot36, cursor in [({'item': 1, 'count': 5}, None),
                               ({'item': 1, 'count': 5}, {'item': 1, 'count': 5}),
                               (None, {'item': 1, 'count': 4}),
                               (None, {'item': 2, 'count': 5})]:
            inventory['slots'][36], inventory['cursor'] = slot36, cursor
            with self.assertRaises(AssertionError):
                verify_stale_pickup(inventory, 1)
        inventory['slots'][36] = None; inventory['cursor'] = {'item': 1, 'count': 5}
        inventory['slots'][37] = {'item': 1, 'count': 1}
        with self.assertRaises(AssertionError):
            verify_stale_pickup(inventory, 1)


if __name__ == '__main__':
    unittest.main()
