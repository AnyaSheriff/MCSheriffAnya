"""Офлайн проверки предикатов и запрета случайного запуска preview-сервера."""
import contextlib
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import probe_catalog_preview_777 as probe


class PreviewGuards(unittest.TestCase):
    def test_default_is_plan_without_process_socket_or_input_reads(self):
        output = io.StringIO()
        with patch('sys.argv', ['probe']), patch.object(probe.subprocess, 'Popen', side_effect=AssertionError('process forbidden')), \
             patch.object(probe.socket, 'socket', side_effect=AssertionError('socket forbidden')), \
             patch.object(probe, 'inputs', side_effect=AssertionError('inputs forbidden')), contextlib.redirect_stdout(output):
            probe.main()
        plan = json.loads(output.getvalue())
        self.assertFalse(plan['run'])
        self.assertEqual(plan['port'], 26026)
        self.assertEqual(plan['test'], 'catalog_preview_probe::serve_java_26_3_catalog_probe')

    def test_run_requires_explicit_binary_hash_data_and_fresh_output(self):
        with patch('sys.argv', ['probe', '--run']), contextlib.redirect_stderr(io.StringIO()), \
             patch.object(probe.subprocess, 'Popen', side_effect=AssertionError('process forbidden')):
            with self.assertRaises(SystemExit) as error:
                probe.main()
            self.assertEqual(error.exception.code, 2)

    def test_hash_mismatch_fails_before_data_socket_or_output(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            binary = root / 'self-authored-fake-binary'
            binary.write_bytes(b'not executed')
            output = root / 'must-not-exist'
            argv = ['probe', '--run', '--test-binary', str(binary), '--binary-sha256', '0' * 64,
                    '--data', str(root / 'data'), '--output', str(output)]
            with patch('sys.argv', argv), patch.object(probe, 'inputs', side_effect=AssertionError('inputs forbidden')), \
                 patch.object(probe.socket, 'socket', side_effect=AssertionError('socket forbidden')):
                with self.assertRaisesRegex(AssertionError, 'binary SHA mismatch'):
                    probe.main()
            self.assertFalse(output.exists())


class MiningEvidence(unittest.TestCase):
    def events(self, count):
        events = [{'name': 'block_change', 'position': list(probe.TARGET), 'state': 0}]
        if count:
            events.append({'name': 'entity_metadata', 'entity': 45, 'slot': {'item': 721, 'count': count}})
        return events

    def test_single_double_and_bare_hand_positive_controls(self):
        for count in (1, 2, 0):
            with self.subTest(count=count):
                inventory = {36: {'item': 1052, 'count': 1, 'damage': 1} if count else None}
                self.assertEqual(len(probe.validate_mining(self.events(count), inventory, 1052, 721, count)), bool(count))

    def test_no_air_wrong_drop_count_thrown_tool_and_wrong_wear_fail(self):
        held = {36: {'item': 1052, 'count': 1, 'damage': 1}}
        for events, inventory, expected in [
            (self.events(1)[1:], held, 1),
            (self.events(2), held, 1),
            ([*self.events(0), {'name': 'entity_metadata', 'entity': 45, 'slot': {'item': 1052, 'count': 1}}], held, 1),
            (self.events(1), {36: None}, 1),
            (self.events(1), {36: {'item': 1052, 'count': 1, 'damage': 2}}, 1),
            (self.events(1), {36: None}, 0),
        ]:
            with self.subTest(events=events, inventory=inventory, expected=expected), self.assertRaises(AssertionError):
                probe.validate_mining(events, inventory, 1052, 721, expected)


if __name__ == '__main__':
    unittest.main()
