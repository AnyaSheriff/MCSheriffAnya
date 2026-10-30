"""Отрицательные проверки свежего полного bot-пути; без Minecraft/npm/сети."""
import copy
import json
import subprocess
from unittest.mock import Mock
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import attr_phase_ci as ci
import via_harness as harness


def capture(name='pvp-weakness'):
    return {'scenario': name, 'lane': 1, 'events': [{'kind': 'do'}, {'kind': 'sv', 'text': 'Attributes'}],
            'window': {'first': 100, 'last': 190},
            'attribute_phase': {'players': [0, 0], 'base_tick': 100, 'attempts': 1}}


def populate(root):
    rows = []
    for name in ci.NAMES:
        for repeat in range(ci.REPEAT):
            rows.append({'scenario': name, 'repeat': repeat, 'exits': {'official': 0, 'ours': 0},
                         'compare_exit': 0, 'counts': [12, 0, 0, 0], 'warnings': [], 'status': 'match'})
            (root / f'{name}-{repeat}-compare.log').write_text('fixture comparison')
            for side in ('official', 'ours'):
                ci.write_json(root / f'{name}-{repeat}-{side}.json', capture(name))
                (root / f'{name}-{repeat}-{side}.log').write_text('fixture robot')
    ci.write_json(root / 'summary.json', rows)
    sources = json.loads((ci.CONFIG / 'sources.json').read_text())
    ci.write_json(root / 'manifest.json', {
        'primary': '26.3', 'scenarios': ci.NAMES, 'repeat': ci.REPEAT, 'translation': True,
        'official_recordings': None, 'port': 26170, 'ours_port': 26270,
        'binary': {'sha256': 'a' * 64}, 'jar': {'sha256': sources['official_jar']['sha256']},
        'proxy': {'sha256': sources['proxy']['sha256']},
        'scripts': {name: ci.sha(ci.ROOT / 'tools/blackbox' / name)
                    for name in ('bot.js', 'attr-phase.js', 'compare.js', 'via_ticks.js')}})
    servers = [{'pid': i + 100, 'identity': {}} for i in range(16)]
    lifecycle = {'servers': servers, 'cleanup_errors': [],
                 'commands': [{'pid': s['pid'], 'argv': ['node' if i >= 4 else 'java']}
                              for i, s in enumerate(servers)],
                 'cleanup': [{'pid': s['pid'], 'group_gone': True, 'exit_code': 0} for s in servers],
                 'ports_free': {str(p): True for p in ci.PORTS}}
    ci.write_json(root / 'lifecycle.json', lifecycle)
    return rows, lifecycle


class CaptureTests(unittest.TestCase):
    def test_final_equal_phases_and_window_are_required(self):
        self.assertEqual(ci.validate_capture(capture(), ci.NAMES[0])['players'], [0, 0])
        no_respawn = capture(); no_respawn['attribute_phase']['attempts'] = 0
        self.assertEqual(ci.validate_capture(no_respawn, ci.NAMES[0])['attempts'], 0)
        for phase in [None, {}, {'players': [0, 1], 'base_tick': 100, 'attempts': 1},
                      {'players': [1, 1], 'base_tick': 100, 'attempts': 1},
                      {'players': [False, False], 'base_tick': 100, 'attempts': 1},
                      {'players': [0], 'base_tick': 100, 'attempts': 1},
                      {'players': [0, 0], 'base_tick': 100, 'attempts': 9},
                      {'players': [0, 0], 'base_tick': 100, 'attempts': -1},
                      {'players': [0, 0], 'base_tick': 102, 'attempts': 1}]:
            with self.subTest(phase=phase), self.assertRaises(ValueError):
                ci.validate_capture({**capture(), 'attribute_phase': phase}, ci.NAMES[0])

    def test_empty_commands_only_and_wrong_identity_fail(self):
        for change in [{'events': []}, {'events': [{'kind': 'do'}]}, {'events': [{}]},
                       {'scenario': 'different'}, {'lane': 2}, {'window': {'first': 100, 'last': 100}}]:
            with self.subTest(change=change), self.assertRaises(ValueError):
                ci.validate_capture({**capture(), **change}, ci.NAMES[0])


class AuditTests(unittest.TestCase):
    def test_complete_fresh_matrix_has_twelve_validated_captures(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            populate(root)
            result = ci.audit(root, 'a' * 64)
            self.assertEqual(result['status'], 'match', result)
            self.assertEqual(len(result['captures']), 12)

    def test_stale_runtime_sources_and_scope_manifest_fail(self):
        for key, value in [('binary', {'sha256': 'b' * 64}), ('jar', {'sha256': 'b' * 64}),
                           ('proxy', {'sha256': 'b' * 64}), ('repeat', 3), ('translation', False),
                           ('official_recordings', 'saved/reference'), ('scenarios', ci.NAMES[:-1]),
                           ('scripts', {})]:
            with self.subTest(key=key), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                populate(root)
                path = root / 'manifest.json'
                manifest = json.loads(path.read_text()); manifest[key] = value
                ci.write_json(path, manifest)
                self.assertEqual(ci.audit(root, 'a' * 64)['status'], 'failed')

    def test_missing_duplicate_and_failed_comparisons_fail(self):
        changes = [('missing', lambda rows: rows.pop()),
                   ('duplicate', lambda rows: rows.__setitem__(0, rows[1])),
                   ('zero', lambda rows: rows[0].update(counts=[0, 0, 0, 0])),
                   ('compare-exit', lambda rows: rows[0].update(compare_exit=1)),
                   ('capture-exit', lambda rows: rows[0].update(exits={'official': 0, 'ours': 1})),
                   ('shift', lambda rows: rows[0].update(counts=[12, 1, 0, 0])),
                   ('warning', lambda rows: rows[0].update(warnings=['warning'])),
                   ('badcounts', lambda rows: rows[0].update(counts=[True, 0, 0, 0]))]
        for name, change in changes:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                rows, _ = populate(root)
                change(rows)
                ci.write_json(root / 'summary.json', rows)
                self.assertEqual(ci.audit(root, 'a' * 64)['status'], 'failed')

    def test_missing_raw_wrong_phase_and_symlink_fail(self):
        for failure in ('missing', 'phase', 'symlink'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                populate(root)
                path = root / f'{ci.NAMES[0]}-0-official.json'
                if failure == 'missing':
                    path.unlink()
                elif failure == 'phase':
                    raw = capture(); raw['attribute_phase']['players'] = [0, 1]
                    ci.write_json(path, raw)
                else:
                    path.rename(root / 'saved.json')
                    path.symlink_to(root / 'saved.json')
                self.assertEqual(ci.audit(root, 'a' * 64)['status'], 'failed')

    def test_incomplete_cleanup_occupied_ports_and_bot_failure_fail(self):
        for failure in ('missing-cleanup', 'group', 'port', 'bot-exit', 'cleanup-error'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                _, lifecycle = populate(root)
                if failure == 'missing-cleanup': lifecycle['cleanup'].pop()
                elif failure == 'group': lifecycle['cleanup'][0]['group_gone'] = False
                elif failure == 'port': lifecycle['ports_free'][str(ci.PORTS[0])] = False
                elif failure == 'bot-exit': lifecycle['cleanup'][-1]['exit_code'] = 124
                else: lifecycle['cleanup_errors'].append('error')
                ci.write_json(root / 'lifecycle.json', lifecycle)
                self.assertEqual(ci.audit(root, 'a' * 64)['status'], 'failed')


class HarnessTests(unittest.TestCase):
    def exercise(self, compare_exit=0, identity_failure=False):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            opaque = root / 'opaque'; opaque.write_bytes(b'opaque fixture, never executed')
            processes = []
            def spawn(argv, **kwargs):
                proc = Mock(pid=1000 + len(processes), returncode=0)
                proc.poll.return_value = None
                proc.wait.return_value = 0
                processes.append((proc, argv, kwargs))
                return proc
            def stop(proc, **kwargs):
                return {'pid': proc.pid, 'exit_code': 0, 'group_gone': True, 'method': 'fixture'}
            argv = ['via_harness.py', '--output', str(root / 'results'), '--binary', str(opaque),
                    '--jar', str(opaque), '--proxy', str(opaque), '--java', '/verified/java25',
                    '--repeat', '1', 'pvp-weakness']
            result = subprocess.CompletedProcess([], compare_exit,
                'Совпало: 12. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.\n', '')
            with patch('sys.argv', argv), patch.object(harness.subprocess, 'Popen', side_effect=spawn), \
                    patch.object(harness.subprocess, 'run', return_value=result), \
                    patch.object(harness.socket, 'socket'), patch.object(harness.socket, 'create_connection'), \
                    patch.object(harness, 'process_identity', side_effect=ValueError('identity fixture') if identity_failure else lambda pid: {'pid': pid}), \
                    patch.object(harness, 'stop_process', side_effect=stop) as stopped, \
                    patch.object(harness, 'ports_free', return_value={str(p): True for p in ci.PORTS}):
                if identity_failure:
                    with self.assertRaises(ValueError): harness.main()
                    self.assertEqual(stopped.call_count, 1)
                    self.assertIsNone(stopped.call_args.kwargs['identity'])
                else:
                    code = harness.main()
                    self.assertEqual(code, 0 if compare_exit == 0 else 1)
                    self.assertEqual(stopped.call_count, 6)
                    self.assertTrue(all(kwargs['start_new_session'] for _, _, kwargs in processes))
                    self.assertEqual(sum(command[0] == '/verified/java25' for _, command, _ in processes[:4]), 3)
                    lifecycle = json.loads((root / 'results/lifecycle.json').read_text())
                    self.assertEqual(len(lifecycle['cleanup']), 6)
                    summary = json.loads((root / 'results/summary.json').read_text())
                    self.assertEqual(summary[0]['compare_exit'], compare_exit)

    def test_portable_java_and_all_owned_groups_are_recorded_and_cleaned(self):
        self.exercise()

    def test_nonzero_comparator_cannot_pass_matching_counters(self):
        self.exercise(compare_exit=1)

    def test_identity_failure_after_spawn_still_cleans_held_child(self):
        self.exercise(identity_failure=True)


class EvidenceTests(unittest.TestCase):
    def test_collection_excludes_runtime_jars_worlds_and_preserves_failures(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            output, results = root / 'output', root / 'results'
            output.mkdir(); results.mkdir()
            populate(results)
            (results / 'proxy.jar').write_bytes(b'opaque fixture, not a JAR')
            (results / 'world').mkdir()
            (results / 'world' / 'saved.json').write_text('world fixture')
            ci.collect(output, results)
            paths = json.loads((output / 'files.json').read_text())
            self.assertTrue(any(p['path'].endswith('official.json') for p in paths))
            self.assertFalse(any('world' in p['path'] or p['path'].endswith('.jar') for p in paths))
            for row in paths:
                self.assertEqual(ci.sha(output / row['path']), row['sha256'])

    def test_existing_work_does_not_reuse_previous_pass(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            with self.assertRaises(ValueError):
                ci.execute(root, root, root, 'not-executed', root)

    def test_dependency_metadata_is_exact_and_scripts_are_disabled_in_workflow(self):
        package = json.loads((ci.CONFIG / 'package.json').read_text())
        lock = json.loads((ci.CONFIG / 'package-lock.json').read_text())
        self.assertEqual(package['dependencies'], lock['packages']['']['dependencies'])
        for name, version in package['dependencies'].items():
            entry = lock['packages']['node_modules/' + name]
            self.assertEqual(entry['version'], version)
            self.assertTrue(entry['integrity'].startswith('sha512-'))
            self.assertTrue(entry['resolved'].startswith('https://registry.npmjs.org/'))
        workflow = (ci.ROOT / '.github/workflows/attr-phase-checks.yml').read_text()
        self.assertIn('--ignore-scripts --omit=dev', workflow)
        self.assertIn('if: always()', workflow)


if __name__ == '__main__':
    unittest.main()
