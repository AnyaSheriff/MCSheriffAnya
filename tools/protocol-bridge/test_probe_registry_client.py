"""Детерминированные проверки probe без Java, сокетов, загрузок и игровых данных."""
import contextlib
import io
import json
from pathlib import Path
import struct
import tempfile
import types
import unittest
from unittest.mock import MagicMock, patch

import probe_registry_client as probe


def pack_list(packs):
    return bytes(probe.varint(len(packs))) + b''.join(
        probe.string(part) for pack in packs for part in pack)


def fixture(version='26.2'):
    config = probe.CONFIGURATIONS[version]
    count = 25 if version == '26.2' else 27
    # Здесь проверяются только конверт/порядок. Реальное NBT проверяет Rust/client.
    return [b'\x01', bytes(probe.varint(config['known'])) + pack_list([('minecraft', 'core', version)])] + [b'\x07'] * count + [bytes(probe.varint(config['tags'])), b'\x03']


def encoded(bodies):
    return json.dumps([body.hex() for body in bodies]).encode()


class MemoryConnection(io.BytesIO):
    def __init__(self, bodies):
        super().__init__(b''.join(bytes(probe.varint(len(body))) + body for body in bodies))
        self.timeouts = []

    def settimeout(self, value):
        self.timeouts.append(value)


class ProbeTests(unittest.TestCase):
    def arguments(self, extra=()):
        return ['--client-dir', '/unused/cache', '--packets', '/unused/fixture', '--output', '/unused/result', *extra]

    def report(self):
        return {'received': [], 'finish_ack': False}

    def test_default_and_explicit_version(self):
        self.assertEqual(probe.parse_args(self.arguments()).version, '26.3')
        self.assertEqual(probe.parse_args(self.arguments(['--version', '26.2'])).version, '26.2')

    def test_776_rejects_each_play_flag_before_launch(self):
        for flags in [('--play-packets', 'play.json'), ('--particle-probe',), ('--entity-probe',), ('--extra-play-packets', 'extra.json')]:
            with self.subTest(flags=flags), patch.object(probe, 'launch') as launch, contextlib.redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit) as error:
                    probe.main(self.arguments(['--version', '26.2', *flags]))
                self.assertEqual(error.exception.code, 2)
                launch.assert_not_called()

    def test_invalid_version_and_ports(self):
        for flags in [('--version', '26.1.2'), ('--port', '0'), ('--port', '65536')]:
            with self.subTest(flags=flags), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                probe.parse_args(self.arguments(flags))

    def test_777_play_dependencies_and_valid_flags(self):
        for flags in [('--entity-probe',), ('--extra-play-packets', 'e'), ('--play-packets', 'p', '--particle-probe')]:
            with self.subTest(flags=flags), contextlib.redirect_stderr(io.StringIO()), self.assertRaises(SystemExit):
                probe.parse_args(self.arguments(flags))
        args = probe.parse_args(self.arguments(['--play-packets', 'p', '--entity-probe', '--particle-probe', '--extra-play-packets', 'e']))
        self.assertTrue(args.entity_probe and args.particle_probe)

    def test_fixture_version_and_empty_mode(self):
        for version in ['26.2', '26.3']:
            for empty in [False, True]:
                with self.subTest(version=version, empty=empty):
                    original = fixture(version)
                    bodies, count = probe.configuration_fixture(encoded(original), version, empty)
                    self.assertEqual(count, 25 if version == '26.2' else 27)
                    self.assertEqual(bodies[:1] + bodies[2:], original[:1] + original[2:])
                    body = io.BytesIO(bodies[1])
                    self.assertEqual(probe.integer(body), probe.CONFIGURATIONS[version]['known'])
                    self.assertEqual(probe.known_packs(body), [] if empty else [('minecraft', 'core', version)])

    def test_fixture_wrong_version_and_count(self):
        with self.assertRaises(ValueError):
            probe.configuration_fixture(encoded(fixture('26.3')), '26.2', False)
        bodies = fixture(); bodies.insert(3, b'\x07')
        with self.assertRaises(ValueError):
            probe.configuration_fixture(encoded(bodies), '26.2', False)

    def test_fixture_rejects_empty_wrong_order_and_trailing_finish(self):
        for index, body in [(0, b''), (2, b'\x08'), (-2, b'\x0e'), (-1, b'\x03\x00')]:
            bodies = fixture(); bodies[index] = body
            with self.subTest(index=index), self.assertRaises(ValueError):
                probe.configuration_fixture(encoded(bodies), '26.2', False)

    def test_fixture_checks_full_core_tuple_even_in_empty_mode(self):
        for packs in [[], [('minecraft', 'core', '26.3')], [('other', 'core', '26.2')], [('minecraft', 'other', '26.2')]]:
            bodies = fixture(); bodies[1] = b'\x0e' + pack_list(packs)
            with self.subTest(packs=packs), self.assertRaises(ValueError):
                probe.configuration_fixture(encoded(bodies), '26.2', True)

    def test_known_pack_bounded_strings_count_and_trailing(self):
        for data in [bytes(probe.varint(1025)), b'\x01' + bytes(probe.varint(32768)), b'\x00\x01', b'\x01\x01\xff']:
            with self.subTest(data=data), self.assertRaises((ValueError, EOFError)):
                probe.known_packs(io.BytesIO(data))

    def test_known_pack_reply_matches_all_fields(self):
        packs = [('minecraft', 'core', '26.2')]
        report = self.report(); conn = MemoryConnection([b'\x00', b'\x07' + pack_list(packs)])
        probe.configuration_reply(conn, report, packs)
        self.assertEqual(report['received'], [0, 7])
        self.assertEqual(report['known_packs'], packs)
        self.assertEqual(report['known_packs_count'], 1)
        self.assertFalse(report['finish_ack'])
        self.assertTrue(all(0 < value <= 60 for value in conn.timeouts))

    def test_empty_known_pack_reply(self):
        report = self.report()
        probe.configuration_reply(MemoryConnection([b'\x07\x00']), report, [])
        self.assertEqual(report['known_packs'], [])
        self.assertEqual(report['known_packs_count'], 0)

    def test_known_pack_reply_rejects_wrong_tuple_and_preserves_observation(self):
        wanted = [('minecraft', 'core', '26.2')]
        for actual in [[], [('minecraft', 'core', '26.3')], [('other', 'core', '26.2')], wanted * 2]:
            report = self.report()
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                probe.configuration_reply(MemoryConnection([b'\x07' + pack_list(actual)]), report, wanted)
            self.assertEqual(report['known_packs'], actual)
            self.assertFalse(report['finish_ack'])

    def test_empty_mode_rejects_nonempty_reply(self):
        with self.assertRaises(ValueError):
            probe.configuration_reply(MemoryConnection([b'\x07' + pack_list([('minecraft', 'core', '26.2')])]), self.report(), [])

    def test_finish_ack_requires_empty_payload(self):
        report = self.report()
        probe.configuration_reply(MemoryConnection([b'\x00', b'\x03']), report)
        self.assertTrue(report['finish_ack'])
        report = self.report()
        with self.assertRaises(ValueError):
            probe.configuration_reply(MemoryConnection([b'\x03\x00']), report)
        self.assertFalse(report['finish_ack'])

    def test_out_of_order_replies_fail(self):
        for body, expected in [(b'\x03', []), (b'\x07\x00', None)]:
            report = self.report()
            with self.subTest(body=body), self.assertRaises(ValueError):
                probe.configuration_reply(MemoryConnection([body]), report, expected)
            self.assertFalse(report['finish_ack'])

    def test_truncated_and_disconnected_reply_fail(self):
        for bodies in [[], [b'\x07\x01']]:
            with self.subTest(bodies=bodies), self.assertRaises(EOFError):
                probe.configuration_reply(MemoryConnection(bodies), self.report(), [])

    def test_configuration_packet_budget(self):
        report = self.report()
        with self.assertRaises(RuntimeError):
            probe.configuration_reply(MemoryConnection([b'\x00'] * 33), report)
        self.assertEqual(len(report['received']), 32)
        self.assertFalse(report['finish_ack'])

    def test_configuration_deadline(self):
        with patch.object(probe.time, 'monotonic', side_effect=[10, 12]), self.assertRaises(TimeoutError):
            probe.configuration_reply(MemoryConnection([]), self.report(), timeout=1)

    def main_case(self, version, empty=False, *, bad_finish=False, launch_failure=False):
        with tempfile.TemporaryDirectory(prefix='mcsa-probe-offline-') as directory:
            directory = Path(directory)
            fixture_path = directory / 'packets.json'; fixture_path.write_bytes(encoded(fixture(version)))
            output = directory / 'output'
            wanted = [] if empty else [('minecraft', 'core', version)]
            protocol = probe.CONFIGURATIONS[version]['protocol']
            replies = [b'\x00' + bytes(probe.varint(protocol)) + probe.string('localhost') + struct.pack('>H', 26012) + b'\x02',
                       b'\x00' + probe.string('McsaProbe') + bytes(16), b'\x03',
                       b'\x07' + pack_list(wanted), b'\x03' + (b'\x00' if bad_finish else b'')]
            listener = MagicMock(); listener.__enter__.return_value = listener
            connection = MagicMock(); connection.__enter__.return_value = connection
            listener.accept.return_value = (connection, None)
            class FakeSocket:
                def __new__(cls):
                    return listener
            client = types.SimpleNamespace(returncode=0)
            flags = ['--client-dir', str(directory / 'unused-cache'), '--packets', str(fixture_path), '--output', str(output), '--version', version]
            if empty:
                flags.append('--empty-known-packs')
            with patch.object(probe.socket, 'socket', FakeSocket), patch.object(probe, 'packet', side_effect=[io.BytesIO(body) for body in replies]), patch.object(probe, 'launch', return_value=client) as launch, patch.object(probe, 'stop') as stop, patch.object(probe.time, 'sleep'), contextlib.redirect_stdout(io.StringIO()):
                if launch_failure:
                    launch.side_effect = RuntimeError('mock launcher failure')
                if bad_finish or launch_failure:
                    with self.assertRaises((ValueError, RuntimeError)):
                        probe.main(flags)
                else:
                    probe.main(flags)
                self.assertEqual(launch.call_args.kwargs, {'version': version})
                if launch_failure:
                    stop.assert_not_called()
                else:
                    stop.assert_called_once_with(client)
            report = json.loads((output / 'result.json').read_text())
            self.assertEqual(report['protocol'], protocol)
            self.assertEqual(report['client'], version)
            self.assertEqual(report['finish_ack'], not (bad_finish or launch_failure))
            if bad_finish or launch_failure:
                self.assertIn('error', report)
            else:
                self.assertEqual(report['known_packs'], [list(pack) for pack in wanted])
                self.assertEqual(report['client_exit'], 0)
                self.assertEqual(len(connection.sendall.call_args_list), 31 if version == '26.2' else 33)

    def test_main_776_known_mode_mocked(self):
        self.main_case('26.2')

    def test_main_776_empty_mode_mocked(self):
        self.main_case('26.2', True)

    def test_main_777_unchanged_default_fixture_mocked(self):
        self.main_case('26.3')

    def test_main_failed_ack_preserves_report_mocked(self):
        self.main_case('26.2', bad_finish=True)

    def test_main_failed_launcher_preserves_report_mocked(self):
        self.main_case('26.2', launch_failure=True)


if __name__ == '__main__':
    unittest.main()
