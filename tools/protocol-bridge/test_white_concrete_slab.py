import io
import os
import json
import struct
import tempfile
import signal
import unittest
from pathlib import Path
from unittest.mock import patch
from probe_white_concrete_slab import Probe, changes, inputs, item_metadata, pack_position, slot, prepare_output, terminate_signal, cleanup_evidence, server_properties, diagnostic_commands, FINISH_STATUS_777
from smoke import varint

CACHE = Path(os.environ.get('MCSA_777_DATA', '~/.local/share/mcsa-java-26.3-probe/candidate-json')).expanduser()


class SlabTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tables = inputs(CACHE)

    def test_dig_bytes_match_pinned777_shape_and_observed_finish(self):
        schema = self.tables['protocol.json']['play']['toServer']['types']['packet_block_dig']
        self.assertEqual(schema, ['container', [
            {'name': 'status', 'type': 'varint'}, {'name': 'location', 'type': 'position'},
            {'name': 'face', 'type': 'i8'}, {'name': 'sequence', 'type': 'varint'}]])
        probe = Probe(None, self.tables['protocol.json'], 72)
        self.assertEqual(probe.outgoing['block_dig'], 41)
        self.assertEqual((varint(41) + varint(0) + pack_position((2, 65, 0)) + b'\1' + varint(5)).hex(),
                         '290000000080000000410105')
        self.assertEqual((varint(41) + varint(FINISH_STATUS_777) + pack_position((2, 65, 0)) + b'\1' + varint(6)).hex(),
                         '290300000080000000410106')

    def test_real777_observation_matches_actual_serialized_slab_action(self):
        evidence=json.loads((Path(__file__).parent / 'testdata/actual777-dig-observations.json').read_text())
        self.assertEqual(evidence['protocol'],777)
        self.assertEqual(evidence['source_sha256'],'2c9622a8ec5740665f4018175f54d8cf47d2ca88e6833b42f700d1bb6f151d21')
        slab=next(row for row in evidence['observations'] if row['case']=='slab')
        self.assertEqual([row['status'] for row in slab['dig']],[0,FINISH_STATUS_777])
        self.assertTrue(slab['air']);self.assertEqual(slab['drops'],[{'item':721,'count':1}])
        probe=Probe(None,self.tables['protocol.json'],72)
        probe.sequence=2
        with patch('probe_white_concrete_slab.Client.send') as send:
            probe.dig(0);probe.dig(FINISH_STATUS_777)
        self.assertEqual([call.args for call in send.call_args_list],
                         [('block_dig',bytes.fromhex(row['payload_hex'])) for row in slab['dig']])

    def test_diagnostic_commands_are_bounded_own_fields(self):
        commands = diagnostic_commands('slab-after')
        self.assertEqual(len(commands), 11)
        self.assertEqual(commands[:4], ['data get entity McsaProbe ' + field
            for field in ('Pos', 'Rotation', 'OnGround', 'SelectedItem')])
        self.assertEqual(sum('execute if block 2 65 0' in command for command in commands), 3)
        self.assertTrue(all('data get entity McsaProbe' not in command or
            command.rsplit(' ', 1)[1] in ('Pos', 'Rotation', 'OnGround', 'SelectedItem') for command in commands))
        with self.assertRaises(AssertionError):
            diagnostic_commands('arbitrary-command')

    def test_dedicated_properties_allow_offline_probe(self):
        props = dict(line.split('=', 1) for line in server_properties(26030).splitlines())
        self.assertEqual(props['white-list'], 'false')
        self.assertEqual(props['online-mode'], 'false')
        self.assertEqual(props['server-ip'], '127.0.0.1')
        self.assertEqual(props['server-port'], '26030')
        self.assertEqual(props['network-compression-threshold'], '-1')

    def test_grounded_fixture_uses_pinned_movement_flags(self):
        probe = Probe(None, self.tables['protocol.json'], 72)
        probe.position = [.5, 65., .5, 0., 0.]
        with patch('probe_white_concrete_slab.Client.send') as send:
            probe.send('tick_end')
            self.assertEqual(send.call_count, 1)
            send.reset_mock()
            probe.grounded_fixture = True
            probe.send('tick_end')
            self.assertEqual(send.call_args_list[0].args, ('position', struct.pack('>dddB', .5, 65., .5, 1)))
            self.assertEqual(send.call_args_list[1].args, ('tick_end', b''))
            probe.position[1] = 66.
            with self.assertRaises(AssertionError):
                probe.send('tick_end')

    def test_existing_output_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'output'
            prepare_output(path, True)
            (path / 'result.json').write_text('failure preserved')
            with self.assertRaises(FileExistsError):
                prepare_output(path, True)
            self.assertEqual((path / 'result.json').read_text(), 'failure preserved')
            prepare_output(path, False)

    def test_sigterm_raises_into_finally(self):
        cleaned = []
        with self.assertRaises(SystemExit) as raised:
            try:
                terminate_signal(signal.SIGTERM, None)
            finally:
                cleaned.append(True)
        self.assertEqual(raised.exception.code, 143)
        self.assertEqual(cleaned, [True])

    def test_cleanup_evidence_without_process_or_socket(self):
        with patch('probe_white_concrete_slab.os.killpg', side_effect=ProcessLookupError), \
             patch('probe_white_concrete_slab.socket.socket'):
            self.assertEqual(cleanup_evidence(123, 26030), {
                'remaining_own_group': False, 'port_released': True, 'errors': []})
        with patch('probe_white_concrete_slab.os.killpg'), \
             patch('probe_white_concrete_slab.socket.socket', side_effect=OSError('busy')):
            evidence = cleanup_evidence(123, 26030)
            self.assertTrue(evidence['remaining_own_group'])
            self.assertFalse(evidence['port_released'])
            self.assertEqual(len(evidence['errors']), 1)

    def test_six_exact_states(self):
        self.assertEqual(self.tables['states'], {'top/true': 18307, 'top/false': 18308,
            'bottom/true': 18309, 'bottom/false': 18310, 'double/true': 18311, 'double/false': 18312})

    def test_block_position_negative(self):
        payload = pack_position((-15, -64, -17)) + varint(18310)
        self.assertEqual(changes('block_change', payload), [([-15, -64, -17], 18310)])

    def test_multi_update(self):
        coords = ((-1 & 0x3fffff) << 42) | ((-2 & 0x3fffff) << 20) | (-4 & 0xfffff)
        payload = struct.pack('>Q', coords) + b'\1' + varint((18312 << 12) | (3 << 8) | (7 << 4) | 5)
        self.assertEqual(changes('multi_block_change', payload), [([-13, -59, -25], 18312)])

    def test_block_truncation_and_trailing(self):
        payload = pack_position((2, 65, 0)) + varint(18310)
        for cut in range(len(payload)):
            with self.assertRaises((ValueError, AssertionError, EOFError, IndexError)):
                changes('block_change', payload[:cut])
        with self.assertRaises(AssertionError):
            changes('block_change', payload + b'\0')

    def test_multi_count_bound(self):
        with self.assertRaises(AssertionError):
            changes('multi_block_change', bytes(8) + varint(4097))

    def test_item_plain_and_damage(self):
        self.assertEqual(slot(io.BytesIO(b'\2' + varint(721) + b'\0\0')), {'item': 721, 'count': 2})
        self.assertEqual(slot(io.BytesIO(b'\1' + varint(1047) + b'\1\0\3\2')),
                         {'item': 1047, 'count': 1, 'damage': 2})
        self.assertIsNone(slot(io.BytesIO(b'\0')))

    def test_item_unknown_component_and_bounds(self):
        for payload in (b'\1\1\1\0\6', b'\1\1\0\1', varint(128), b'\1\1\2\0'):
            with self.assertRaises(AssertionError):
                slot(io.BytesIO(payload))

    def test_item_metadata_exact(self):
        payload = b'\x07\x00\x00\x00\x06\x14\x00\x08\x07\x02' + varint(721) + b'\0\0\xff'
        self.assertEqual(item_metadata(payload), (7, {'item': 721, 'count': 2}))
        for cut in range(len(payload)):
            with self.assertRaises((ValueError, AssertionError, EOFError, IndexError)):
                item_metadata(payload[:cut])

    def test_item_metadata_unknown_duplicate_trailing(self):
        for payload in (b'\1\x09\0\0\xff', b'\1\0\0\0\0\0\0\xff', b'\1\xff\0', b'\1\2\6\1'):
            with self.assertRaises(AssertionError):
                item_metadata(payload)

    def test_untracked_metadata_is_not_parsed(self):
        probe = Probe(None, self.tables['protocol.json'], 72)
        probe.observe_packet('entity_metadata', b'\x7f\x7f')
        self.assertEqual(probe.events, [])
        probe.item_ids.add(127)
        with self.assertRaises((ValueError, EOFError, IndexError)):
            probe.observe_packet('entity_metadata', b'\x7f\x7f')

    def test_action_sequence_and_form(self):
        probe = Probe(None, self.tables['protocol.json'], 72)
        with patch.object(probe, 'send') as send:
            probe.place((2, 64, 0), 1, (.5, 1., .5))
            send.assert_called_with('block_place', b'\0' + pack_position((2, 64, 0)) + b'\1'
                                    + struct.pack('>fff', .5, 1., .5) + b'\0\0\1')
            probe.dig(0)
            send.assert_called_with('block_dig', b'\0' + pack_position((2, 65, 0)) + b'\1\2')

    def test_inventory_updates_and_limit(self):
        probe = Probe(None, self.tables['protocol.json'], 72)
        probe.observe_packet('set_player_inventory', b'\0\2' + varint(721) + b'\0\0')
        self.assertEqual(probe.inventory[36], {'item': 721, 'count': 2})
        probe.observe_packet('set_slot', b'\0\1\0\x24\0')
        self.assertIsNone(probe.inventory[36])
        probe.events = [None] * 2000
        with self.assertRaises(AssertionError):
            probe.observe_packet('keep_alive', bytes(8))


if __name__ == '__main__':
    unittest.main()
