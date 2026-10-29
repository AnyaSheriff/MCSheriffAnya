"""Оркестратор: отрицательные проверки без Minecraft, Cargo, npm и сети."""
import json
import fnmatch
import re
import ctypes
import os
import sys
import threading
from pathlib import Path
import signal
import subprocess
import tempfile
import unittest
from unittest.mock import Mock, patch
import combat_ci as ci
import combat_process as life


class PinTests(unittest.TestCase):
    def setUp(self):
        self.pin = {'build_run_id': '123', 'head': 'a' * 40, 'runtime_sha256': 'b' * 64}

    def test_unset_partial_and_mismatch_do_not_choose_latest(self):
        for saved, explicit in [({}, ('', '', '')), ({}, ('123', '', '')),
                                 (self.pin, ('124', 'a' * 40, 'b' * 64)),
                                 ({}, ('123\n456', 'a' * 40, 'b' * 64))]:
            with self.assertRaises(ValueError):
                ci.resolve_pin(saved, *explicit)
        self.assertEqual(ci.resolve_pin({}, '123', 'a' * 40, 'b' * 64), self.pin)
        self.assertEqual(ci.resolve_pin(self.pin), self.pin)

    def test_only_exact_successful_owned_build_workflow_is_accepted(self):
        run = {'id': 123, 'head_sha': 'a' * 40, 'status': 'completed', 'conclusion': 'success',
               'path': '.github/workflows/migration-checks.yml', 'run_attempt': 2,
               'repository': {'full_name': 'AnyaSheriff/MCSheriffAnya'},
               'head_repository': {'full_name': 'AnyaSheriff/MCSheriffAnya'}}
        self.assertEqual(ci.validate_run(run, self.pin, 'AnyaSheriff/MCSheriffAnya'), 'migration-checks-2')
        for key, value in [('conclusion', 'failure'), ('head_sha', 'c' * 40), ('status', 'in_progress'),
                           ('id', 124), ('path', '.github/workflows/other.yml'), ('run_attempt', 0),
                           ('head_repository', {'full_name': 'other/fork'})]:
            with self.assertRaises(ValueError):
                ci.validate_run({**run, key: value}, self.pin, 'AnyaSheriff/MCSheriffAnya')

    def test_exact_receipt_and_runtime_are_both_required(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            binary = root / 'mcsheriffanya'
            binary.write_bytes(b'verified fixture, never executed')
            pin = {**self.pin, 'runtime_sha256': ci.sha(binary)}
            selected = {'pin': pin, 'run_attempt': 1}
            receipt = {'head': pin['head'], 'run_id': '123', 'run_attempt': '1',
                       'runtime_built': True, 'status_before_receipt': 'success',
                       'sha256': {'target/release/mcsheriffanya': pin['runtime_sha256']}}
            path = root / 'receipt.json'
            ci.write_json(path, receipt)
            self.assertEqual(ci.verify_artifact(root, selected)[0], binary)
            for key, value in [('head', 'c' * 40), ('run_id', '124'), ('run_attempt', '2'),
                               ('status_before_receipt', 'failure'), ('runtime_built', False), ('sha256', {})]:
                ci.write_json(path, {**receipt, key: value})
                with self.assertRaises(ValueError):
                    ci.verify_artifact(root, selected)
            ci.write_json(path, receipt)
            binary.write_bytes(b'different')
            with self.assertRaises(ValueError):
                ci.verify_artifact(root, selected)
            (root / 'link').symlink_to(binary)
            with self.assertRaises(ValueError):
                ci.verify_artifact(root, selected)


class LifecycleTests(unittest.TestCase):
    def fake(self):
        proc = Mock(pid=424242, returncode=0)
        proc.poll.return_value = None
        return proc

    @patch('combat_process.group_exists', return_value=False)
    def test_stop_and_eof_precede_wait(self, _):
        proc = self.fake()
        result = life.stop_process(proc)
        proc.stdin.write.assert_called_once_with('stop\n')
        proc.stdin.close.assert_called_once()
        proc.wait.assert_called_once_with(timeout=8)
        self.assertTrue(result['group_gone'])

    @patch('combat_process.group_exists', return_value=False)
    def test_missing_journal_identity_still_stops_held_popen(self, _):
        proc = self.fake()
        proc.wait.side_effect = [subprocess.TimeoutExpired('ours', 8), subprocess.TimeoutExpired('ours', 3), 0]
        life.stop_process(proc, identity=None)
        self.assertEqual(proc.send_signal.call_args_list[0].args, (signal.SIGTERM,))
        self.assertEqual(proc.send_signal.call_args_list[1].args, (signal.SIGKILL,))

    @patch('combat_process.process_identity', return_value={'pid': 424242, 'pgid': 424242, 'start_time': 'new'})
    @patch('combat_process.os.killpg')
    def test_pid_reuse_refuses_group_signal(self, kill, _):
        with self.assertRaises(ValueError):
            life.signal_group(424242, signal.SIGKILL, {'pid': 424242, 'pgid': 424242, 'start_time': 'old'})
        kill.assert_not_called()

    @patch('combat_ci.group_exists', return_value=True)
    @patch('combat_ci.process_identity', return_value={'pid': 424242, 'pgid': 424242, 'start_time': 'new'})
    @patch('combat_process.os.killpg')
    @patch('combat_process.process_identity', return_value={'pid': 424242, 'pgid': 424242, 'start_time': 'new'})
    def test_watchdog_does_not_signal_reused_pid_from_file(self, _, send, *__):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            ci.write_json(root / 'lifecycle.json', {'servers': [{'pid': 424242, 'identity': {'pid': 424242, 'pgid': 424242, 'start_time': 'old'}}]})
            self.assertTrue(ci.cleanup_manifest(root))
            send.assert_not_called()


    @patch('combat_process.process_identity', return_value=None)
    @patch('combat_process.os.killpg')
    @patch('combat_process.group_exists', side_effect=[True, False, False])
    def test_already_reaped_held_parent_still_stops_own_orphan_group(self, _, kill, *__):
        # owned_members здесь настоящий: leader отсутствует, он вернёт [].
        proc=self.fake();proc.poll.return_value=0
        result=life.stop_process(proc,identity={'pid':424242,'pgid':424242,'start_time':'parent'})
        kill.assert_called_once_with(424242,signal.SIGKILL)
        self.assertTrue(result['group_gone'])


class RealOwnedGroupTest(unittest.TestCase):
    @unittest.skipUnless(sys.platform.startswith('linux'), 'Linux process groups')
    def test_real_child_survives_reaped_parent_then_held_group_cleanup_reaps_it(self):
        # Только этот тест принимает своего сироту, чтобы не оставлять zombie
        # у containerinit. В production subreaper не применяется.
        libc=ctypes.CDLL(None,use_errno=True)
        previous=ctypes.c_int()
        self.assertEqual(libc.prctl(37,ctypes.byref(previous),0,0,0),0)
        self.assertEqual(libc.prctl(36,1,0,0,0),0)
        parent=None;child_pid=None;reaper=None
        try:
            code=("import subprocess,sys; "
                  "child=subprocess.Popen([sys.executable,'-c','import time; time.sleep(20)'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL); "
                  "print(child.pid,flush=True); sys.stdin.readline()")
            parent=subprocess.Popen([sys.executable,'-c',code],stdin=subprocess.PIPE,stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True,start_new_session=True)
            child_pid=int(parent.stdout.readline())
            identity=life.process_identity(parent.pid)
            self.assertIsNotNone(identity)
            parent.stdin.write('exit\n');parent.stdin.flush()
            parent.wait(timeout=3)
            self.assertIsNone(life.process_identity(parent.pid))
            self.assertTrue(life.group_exists(parent.pid))
            # Родитель уже reaped; owned_members настоящий и возвращает[].
            self.assertEqual(life.owned_members(parent.pid,identity),[])
            reaper=threading.Thread(target=lambda:os.waitpid(child_pid,0),daemon=True)
            reaper.start()
            result=life.stop_process(parent,identity=identity)
            reaper.join(timeout=3)
            self.assertFalse(reaper.is_alive())
            self.assertTrue(result['group_gone'])
            self.assertFalse(life.group_exists(parent.pid))
        finally:
            if parent is not None:
                # Собственный удерживаемый Popen; никакихPIDизчужогофайла.
                if parent.poll() is None:
                    parent.kill();parent.wait(timeout=3)
                if child_pid is not None and life.group_exists(parent.pid):
                    os.killpg(parent.pid,signal.SIGKILL)
                if reaper is not None:
                    reaper.join(timeout=3)
                elif child_pid is not None:
                    try:os.waitpid(child_pid,0)
                    except ChildProcessError:pass
                for stream in (parent.stdin,parent.stdout,parent.stderr):
                    if stream is not None:stream.close()
            self.assertEqual(libc.prctl(36,previous.value,0,0,0),0)


class ResultTests(unittest.TestCase):
    def populated(self, root):
        name = 'pvp-fist'
        recorder_sha = ci.sha(ci.ROOT / 'tools/blackbox/record777.py')
        ci.write_json(root / 'run-one.json', {'protocol': 777, 'primary': '26.3', 'binary_sha256': 'b' * 64,
                      'scenario_names': [name], 'isolate_vitals': True, 'recorder_sha256': recorder_sha})
        for side in ('official', 'ours'):
            ci.write_json(root / f'{name}-{side}.json', {'scenario': name, 'protocol': 777,
                          'recorder_sha256': recorder_sha, 'clients': [{'events': [{'at': 0.0, 'name': 'damage_event', 'hex': '00'}], 'failure': None}]})
            ci.write_json(root / f'{name}-{side}-events.json', {'events': [{'kind': 'sv', 'tick': 0, 'text': 'Damage Event type=34'}]})
        (root / f'{name}-comparison.txt').write_text('saved actual comparator stdout fixture')
        row = {'scenario': name, 'comparison_counts': [6, 0, 0, 0], 'decode_errors': [[], []],
               'warnings': [], 'status': 'match', 'record_status': 'complete'}
        ci.write_json(root / 'comparison-summary.json', [row])
        lifecycle = {'servers': [{'pid': 1}, {'pid': 2}],
                     'cleanup': [{'pid': pid, 'exit_code': 0, 'group_gone': True} for pid in (1, 2)],
                     'ports_free': {str(p): True for p in ci.PORTS}, 'cleanup_errors': []}
        ci.write_json(root / 'lifecycle.json', lifecycle)
        return name, row, lifecycle

    def test_missing_raw_failed_client_and_incomplete_comparison_are_not_matches(self):
        for failure in ('missing-raw', 'client-failure', 'missing-counts', 'decode-gap', 'missing-scenario', 'cleanup-error', 'late-record-error'):
            with self.subTest(failure=failure), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                name, row, lifecycle = self.populated(root)
                self.assertEqual(ci.audit_results(root, [name], 'b' * 64)['status'], 'match')
                if failure == 'missing-raw':
                    (root / f'{name}-official.json').unlink()
                elif failure == 'client-failure':
                    p = root / f'{name}-ours.json'
                    raw = json.loads(p.read_text());raw['clients'][0]['failure'] = 'disconnected';ci.write_json(p, raw)
                elif failure == 'missing-scenario':
                    ci.write_json(root / 'comparison-summary.json', [])
                elif failure == 'late-record-error':
                    row['record_status']='failed';row['record_error']='late cleanup failed';ci.write_json(root / 'comparison-summary.json',[row])
                elif failure == 'cleanup-error':
                    lifecycle['cleanup_errors'] = ['log/journal write failed'];ci.write_json(root / 'lifecycle.json', lifecycle)
                else:
                    if failure == 'missing-counts':row.pop('comparison_counts')
                    else:row['decode_errors'] = [['unknown packet'], []]
                    ci.write_json(root / 'comparison-summary.json', [row])
                self.assertEqual(ci.audit_results(root, [name], 'b' * 64)['status'], 'infrastructure-failed')

    def test_nonempty_fixture_rejects_empty_raw_projection_and_zero_counts(self):
        for side in ('official', 'ours'):
            for failure in ('empty-client-list', 'empty-client-events', 'missing-client-events', 'empty-projection', 'zero-counts'):
                with self.subTest(side=side, failure=failure), tempfile.TemporaryDirectory() as folder:
                    root = Path(folder)
                    name, row, _ = self.populated(root)
                    self.assertEqual(ci.audit_results(root, [name], 'b' * 64)['status'], 'match')
                    path = root / f'{name}-{side}.json'
                    raw = json.loads(path.read_text())
                    if failure == 'empty-client-list':
                        raw['clients'] = []
                    elif failure == 'empty-client-events':
                        # Даже один пустой наблюдатель рядом с непустым запрещён.
                        raw['clients'].append({'events': [], 'failure': None})
                    elif failure == 'missing-client-events':
                        raw['clients'][0].pop('events')
                    elif failure == 'empty-projection':
                        ci.write_json(root / f'{name}-{side}-events.json', {'events': []})
                    else:
                        row['comparison_counts'] = [0, 0, 0, 0]
                        ci.write_json(root / 'comparison-summary.json', [row])
                    ci.write_json(path, raw)
                    result = ci.audit_results(root, [name], 'b' * 64)
                    self.assertEqual(result['status'], 'infrastructure-failed')
                    self.assertTrue(any('empty' in error or 'all-zero' in error for error in result['errors']))

    def test_nonzero_missing_event_remains_legitimate_mismatch(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            name, row, _ = self.populated(root)
            row.update(status='mismatch-open', comparison_counts=[0, 0, 1, 0])
            ci.write_json(root / 'comparison-summary.json', [row])
            result = ci.audit_results(root, [name], 'b' * 64)
            self.assertEqual(result['status'], 'mismatch-open')
            self.assertEqual(result['errors'], [])
            self.assertEqual(result['comparisons'][0]['counts'], [0, 0, 1, 0])

    def test_known_mismatch_is_retained_and_not_passed(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            name, row, _ = self.populated(root)
            row.update(status='mismatch-open', comparison_counts=[6, 1, 0, 0])
            ci.write_json(root / 'comparison-summary.json', [row])
            result = ci.audit_results(root, [name], 'b' * 64)
            self.assertEqual(result['status'], 'mismatch-open')
            self.assertEqual(result['comparisons'][0]['counts'], [6, 1, 0, 0])

    def test_helpers_include_transitive_local_imports_without_foreign_modules(self):
        paths = ci.helper_sources()
        for name in ['generate_tables.py', 'compare_schemas.py', 'combat_process.py']:
            self.assertIn(name, [p.name for p in paths])
        self.assertTrue(all(p.is_relative_to(ci.ROOT) for p in paths))
        self.assertFalse(any('node_modules' in p.parts for p in paths))

    def test_collection_keeps_evidence_but_excludes_world_and_jar(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            results, output, data = root / 'recordings', root / 'output', root / 'data'
            for p in (results, output, data):p.mkdir()
            (results / 'raw.json').write_text('{}')
            for label in ('official', 'ours'):
                server = results / label;server.mkdir();(server / 'world').mkdir()
                (server / 'server.jar').write_bytes(b'not a real JAR')
                (server / 'server.log').write_text('saved log')
                (server / 'world/private.dat').write_text('not uploaded')
            ci.collect(output, results, data)
            self.assertTrue((output / 'raw.json').is_file())
            self.assertFalse(list(output.rglob('*.jar')))
            self.assertFalse(any(p.name == 'world' for p in output.rglob('*')))
            (results / 'bad.json').symlink_to(results / 'raw.json')
            with self.assertRaises(ValueError):ci.collect(output, results, data)



class LateFailureTests(unittest.TestCase):
    def test_analyzer_retains_failed_record_status_even_when_projected_events_match(self):
        import analyze777
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)
            scenario='pvp-fist'
            summary=[{'scenario':scenario,'status':'mismatch-open','record_status':'failed','record_error':'late client/cleanup failure'}]
            ci.write_json(root/'summary.json',summary)
            for side in ('official','ours'):ci.write_json(root/f'{scenario}-{side}.json',{})
            protocol=root/'protocol.json'
            ci.write_json(protocol,{'types':{},'play':{'toClient':{'types':{}},'toServer':{'types':{}}}})
            ci.write_json(root/'entities.json',[])
            result=Mock(stdout='Совпало: 6. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.',stderr='',returncode=0)
            with patch('sys.argv',['analyze777',str(root),'--protocol',str(protocol)]), patch.object(analyze777,'project_record',return_value=[]), patch.object(analyze777.subprocess,'run',return_value=result):
                analyze777.main()
            row=json.loads((root/'comparison-summary.json').read_text())[0]
            self.assertEqual(row['status'],'match')
            self.assertEqual(row['record_status'],'failed')
            self.assertEqual(row['record_error'],'late client/cleanup failure')

    def test_analyzer_keeps_mismatch_evidence_and_continues_to_next_case(self):
        import analyze777
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)
            names=['pvp-fist','pvp-crit']
            ci.write_json(root/'summary.json',[{'scenario':name,'status':'recorded'} for name in names])
            for name in names:
                for side in ('official','ours'):ci.write_json(root/f'{name}-{side}.json',{})
            protocol=root/'protocol.json'
            ci.write_json(protocol,{'types':{},'play':{'toClient':{'types':{}},'toServer':{'types':{}}}})
            ci.write_json(root/'entities.json',[])
            results=[
                Mock(stdout='Совпало: 5. Сдвинуто по тактам: 0. Нет у нас: 1. Лишнее у нас: 0.',stderr='diagnostic',returncode=1),
                Mock(stdout='Совпало: 6. Сдвинуто по тактам: 0. Нет у нас: 0. Лишнее у нас: 0.',stderr='',returncode=0),
            ]
            with patch('sys.argv',['analyze777',str(root),'--protocol',str(protocol)]), patch.object(analyze777,'project_record',return_value=[]), patch.object(analyze777.subprocess,'run',side_effect=results):
                analyze777.main()
            rows=json.loads((root/'comparison-summary.json').read_text())
            self.assertEqual([row['status'] for row in rows],['mismatch-open','match'])
            self.assertEqual(rows[0]['comparison_counts'],[5,0,1,0])
            self.assertEqual((root/'pvp-fist-comparison.txt').read_text(),results[0].stdout)
            self.assertEqual((root/'pvp-fist-comparison.stderr.txt').read_text(),'diagnostic')

    def test_analyzer_replaces_stale_pass_when_comparator_rejects_recording(self):
        import analyze777
        with tempfile.TemporaryDirectory() as folder:
            root=Path(folder)
            old={'scenario':'pvp-fist','status':'match','comparison_counts':[6,0,0,0],'warnings':[],'ordering_notes':0}
            ci.write_json(root/'summary.json',[old])
            ci.write_json(root/'comparison-summary.json',[old])
            for side in ('official','ours'):ci.write_json(root/f'pvp-fist-{side}.json',{})
            protocol=root/'protocol.json'
            ci.write_json(protocol,{'types':{},'play':{'toClient':{'types':{}},'toServer':{'types':{}}}})
            ci.write_json(root/'entities.json',[])
            result=Mock(stdout='',stderr='Нет сравниваемых ответов сервера',returncode=1)
            with patch('sys.argv',['analyze777',str(root),'--protocol',str(protocol)]), patch.object(analyze777,'project_record',return_value=[]), patch.object(analyze777.subprocess,'run',return_value=result):
                analyze777.main()
            for file in ('summary.json','comparison-summary.json'):
                row=json.loads((root/file).read_text())[0]
                self.assertEqual(row['status'],'analysis-failed')
                self.assertEqual(row['comparison_exit_code'],1)
                self.assertNotIn('comparison_counts',row)
                self.assertIn(result.stderr,row['reason'])

    def test_journal_failure_and_missing_identity_cannot_skip_held_servers(self):
        import record777
        servers=[(Mock(pid=424241),Mock(),None,26140),(Mock(pid=424242),Mock(),None,26142)]
        lifecycle={'servers':[],'cleanup':[],'cleanup_errors':[]}
        with patch.object(record777,'stop_process',return_value={'exit_code':0,'group_gone':True}) as stop, patch.object(record777,'ports_free',return_value={}), patch.object(record777,'owned_members',return_value=[]):
            record777.cleanup_servers(servers,lifecycle,Mock(side_effect=OSError('journal write failure')),[])
        self.assertEqual(stop.call_count,2)
        self.assertTrue(all(call.args[2] is None for call in stop.call_args_list))
        self.assertEqual(len(lifecycle['cleanup']),2)
        self.assertTrue(lifecycle['cleanup_errors'])


class CancellationTests(unittest.TestCase):
    def test_signal_abort_bypasses_per_scenario_exception_handler(self):
        import record777
        for sig in (signal.SIGALRM,signal.SIGTERM,signal.SIGINT):
            caught=[]
            with self.assertRaises(record777.RunInterrupted):
                try:
                    record777.interrupt_run(sig,None)
                except Exception:
                    caught.append('incorrectly continued next scenario')
            self.assertEqual(caught,[])


class CommandWindowTests(unittest.TestCase):
    @patch('combat_ci.process_identity',side_effect=OSError('metadata unavailable'))
    @patch('combat_ci.subprocess.Popen')
    def test_identity_metadata_failure_cannot_leave_started_recorder(self, popen, _):
        proc=Mock(pid=424242)
        proc.poll.return_value=None
        popen.return_value=proc
        with tempfile.TemporaryDirectory() as folder:
            with self.assertRaises(OSError):
                ci.run_tool(['own-recorder'],Path(folder)/'log.txt',1)
        proc.send_signal.assert_called_once_with(signal.SIGTERM)
        proc.wait.assert_called_once_with(timeout=35)


class WorkflowBranchTests(unittest.TestCase):
    def test_combat_push_branch_does_not_trigger_migration_build(self):
        workflow = (ci.ROOT / '.github/workflows/combat-checks.yml').read_text()
        pattern = re.search(r"branches: \[\'([^\']+)\'\]", workflow).group(1)
        self.assertEqual(pattern, 'wip/combat-validate-*')
        self.assertTrue(fnmatch.fnmatchcase('wip/combat-validate-20261028', pattern))
        self.assertFalse(fnmatch.fnmatchcase('wip/combat-validate-20261028', 'wip/validate-*'))
        self.assertFalse(fnmatch.fnmatchcase('wip/prepare-combat-ci-20261028', pattern))


if __name__ == '__main__':
    unittest.main()
