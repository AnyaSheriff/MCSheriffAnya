"""Проверки цепочки артефактов без Minecraft и Cargo."""
import copy
import gzip
import json
from pathlib import Path
import tempfile

import numpy as np
import unittest
from unittest.mock import patch

import genaudit_263_end_ci as e


class EndCiTests(unittest.TestCase):
    def test_factorial_patches_keep_other_end_code(self):
        original = e.baseline()
        a,b,c,d = [e.variant_source(original,v) for v in e.VARIANTS]
        self.assertEqual(a, original)
        self.assertIn(b'Vec::new()\n    }\n\n    fn main_column',b)
        self.assertIn(b'const ISLETS: f64 = 0.3;',c)
        self.assertIn(b', 5) + ISLETS *',c)
        self.assertIn(b'const ISLETS: f64 = 0.3;',d)
        for value in [a,b,c,d]:
            self.assertEqual(value.split(b'    fn main_column',1)[1],original.split(b'    fn main_column',1)[1])
        with self.assertRaises(ValueError): e.variant_source(original,'other')
        with self.assertRaises(ValueError): e.variant_source(original.replace(b'0.225;',b'0.224;'),'C')

    def test_real_frozen_plan_end_only(self):
        frozen=e.ci.reference()
        for case in frozen['cases'].values():
            plan=e.end_plan(case['plan'])
            self.assertFalse(plan['windows']['overworld'])
            self.assertFalse(plan['windows']['the_nether'])
            self.assertEqual(plan['windows']['the_end'],case['plan']['windows']['the_end'])
            self.assertNotEqual(e.ci.canonical_sha(plan),case['plan_sha256'])
        with self.assertRaises(ValueError):
            e.end_plan({'windows':{'overworld':[],'the_nether':[],'the_end':[[0,0,0,0]]}})

    def report(self):
        item={'chunks':2803,'requested':2803,'chunk_sha256':'fixed','missing_first20':[]}
        return {'coverage':[{'dimension':'the_end','ours':copy.deepcopy(item),'vanilla':copy.deepcopy(item),
                             'complete':True,'same_chunks':True}],
                'rows':[{'dimension':'the_end','metric':'height','unit':'blocks','ours':1,
                         'vanilla':2,'tolerance':1,'verdict':'within'}]}

    def test_incomplete_wrong_dimension_empty_coverage_rejected(self):
        e.coverage(self.report())
        for mutation in [lambda r:r.update(coverage=[]),lambda r:r.update(rows=[]),
                         lambda r:r['coverage'][0].update(complete=False),
                         lambda r:r['coverage'][0]['ours'].update(chunks=0),
                         lambda r:r['coverage'][0]['ours'].update(chunk_sha256='other'),
                         lambda r:r['rows'][0].update(dimension='overworld')]:
            value=self.report();mutation(value)
            with self.assertRaises(ValueError):e.coverage(value)

    def test_compare_keeps_new_removed_and_failures(self):
        before=self.report()['rows'];after=copy.deepcopy(before)
        after[0].update(ours=5,verdict='outside')
        result=e.compare_rows(before,after)
        self.assertEqual(len(result['new_failure']),1)
        self.assertEqual(len(result['worsened']),1)
        after[0]['vanilla']=3
        with self.assertRaises(ValueError):e.compare_rows(before,after)
        after=copy.deepcopy(before);after[0]['metric']='other'
        result=e.compare_rows(before,after)
        self.assertEqual(len(result['new_rows']),1)
        self.assertEqual(len(result['removed_rows']),1)

    def test_corrupt_artifact_variant_source_and_head_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root=Path(tmp);(root/e.SOURCE).parent.mkdir(parents=True)
            original=e.baseline();(root/e.SOURCE).write_bytes(original)
            build=root/'build';build.mkdir()
            for name in ['mcsheriffanya','genaudit-starts','Cargo.lock']:(build/name).write_text(name)
            control=root/'control.gz';control.write_bytes(b'fixture')
            with patch.object(e,'REPO',root),patch.object(e,'CONTROL',control),patch.object(e,'baseline',return_value=original),patch.object(e.subprocess,'check_output',return_value=''),patch.object(e.ci,'head',return_value='fixture-head'):
                e.bundle(build,'A');e.verify_bundle(build,'A')
                with self.assertRaises(ValueError):e.verify_bundle(build,'B')
                receipt=build/'end-build.json';saved=receipt.read_text()
                for field in ['source_sha256','base_head','patch_sha256']:
                    value=json.loads(saved);value[field]='wrong';receipt.write_text(json.dumps(value))
                    with self.assertRaises(ValueError):e.verify_bundle(build,'A')
                receipt.write_text(saved);(build/'mcsheriffanya').write_text('corrupt')
                with self.assertRaises(ValueError):e.verify_bundle(build,'A')
                (root/e.SOURCE).write_bytes(original+b'\n')
                with self.assertRaises(ValueError):e.bundle(build,'A')

    def test_control_mismatch_evidence_saved_before_failure(self):
        expected=self.report();actual=copy.deepcopy(expected)
        actual['rows'][0]['ours']=99
        with tempfile.TemporaryDirectory() as tmp:
            out=Path(tmp)
            with self.assertRaisesRegex(ValueError,'не воспроизвёл'):
                e.save_measurement(out,actual,{'diagnostic':'retained'},{},{'seed':123},expected,'A')
            self.assertEqual(json.loads((out/'report.json').read_text()),actual)
            self.assertEqual(json.loads((out/'edges.json').read_text()),{'diagnostic':'retained'})
            for name in ['summary.json','provenance.json']:
                data=json.loads((out/name).read_text())
                self.assertFalse(data['control_validation']['passed'])
            e.save_measurement(out,expected,{}, {}, {},expected,'A')
            self.assertTrue(json.loads((out/'summary.json').read_text())['control_validation']['passed'])

    def test_central_projection_exact_coords_and_height_sensitive(self):
        chunks={(x,z):np.zeros((16,16),dtype=np.int16) for x in range(-12,13) for z in range(-12,13)}
        value=e.central_digest(chunks)
        self.assertEqual(value,e.central_digest(dict(reversed(list(chunks.items())))))
        chunks[0,0][3,4]=1
        self.assertNotEqual(value,e.central_digest(chunks))
        chunks[13,0]=chunks.pop((0,0))
        with self.assertRaises(ValueError):e.central_digest(chunks)
        chunks.pop((13,0))
        with self.assertRaises(ValueError):e.central_digest(chunks)

    def test_real_control_is_end_only_and_matches_frozen_official(self):
        frozen=e.ci.reference()
        for label in e.ci.SEEDS:
            case=e.control(label)
            self.assertEqual(case['statistics']['vanilla']['dimensions']['the_end'],
                frozen['cases'][label]['baseline_report']['statistics']['vanilla']['dimensions']['the_end'])

    def test_actual_cached_statistics_replay_both_seeds(self):
        import genaudit_263 as analyzer
        original_dims = analyzer.DIMS
        try:
            analyzer.DIMS = ('the_end',)
            for label in e.ci.SEEDS:
                case=e.control(label)
                report=analyzer.comparison(case['statistics']['ours'],case['statistics']['vanilla'])
                report['rows']=[r for r in report['rows'] if r['dimension']=='the_end']
                e.coverage(report)
                key=lambda r:(r['dimension'],r['metric'],r['unit'])
                self.assertEqual(sorted(report['rows'],key=key),sorted(case['rows'],key=key))
                self.assertEqual(report['coverage'],case['coverage'])
        finally:
            analyzer.DIMS=original_dims
        self.assertEqual(analyzer.DIMS,original_dims)

    def test_cli_signal_cancellation_cleans_owned_groups(self):
        # Без процессов: вызываем установленный обработчик во время execute.
        handlers={}
        def register(number,handler):
            handlers[number]=handler
        def interrupted_execute(*args):
            handlers[e.signal.SIGTERM](e.signal.SIGTERM,None)
        args=['end-ci','--execute','--variant','A','--label','owner',
              '--build','/tmp/end-build','--output','/tmp/end-out','--scratch','/tmp/end-scratch']
        with patch.object(e.sys,'argv',args),patch.object(e.ctypes,'CDLL') as lib, \
             patch.object(e.signal,'signal',side_effect=register), \
             patch.object(e,'execute',side_effect=interrupted_execute),patch.object(e.ci,'cleanup') as cleanup:
            lib.return_value.prctl.return_value=0
            with self.assertRaises(InterruptedError):e.main()
            cleanup.assert_called_once()
            self.assertTrue(all(handlers[sig] == e.signal.SIG_IGN
                for sig in [e.signal.SIGINT,e.signal.SIGTERM,e.signal.SIGHUP]))

    def test_control_corruption_and_seed_mix_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            p=Path(tmp)/'control.json.gz'
            with patch.object(e,'CONTROL',p):
                p.write_bytes(b'bad');p.with_suffix('').write_text(json.dumps({'fixture_sha256':'wrong'}))
                with self.assertRaises(ValueError):e.control('owner')
                case=self.report();case.update(seed=e.ci.SEEDS['second'],analyzer_source_sha256=e.ci.analyzer_sha())
                data={'schema':1,'analyzer_source_sha256':e.ci.analyzer_sha(),'cases':{'owner':case,'second':case}}
                p.write_bytes(gzip.compress(json.dumps(data).encode()))
                p.with_suffix('').write_text(json.dumps({'fixture_sha256':e.ci.sha(p),'bytes':p.stat().st_size,
                    'cases':{label:{'case_sha256_canonical':e.ci.canonical_sha(case)} for label in e.ci.SEEDS}}))
                with self.assertRaises(ValueError):e.control('owner')


if __name__ == '__main__':
    unittest.main()
