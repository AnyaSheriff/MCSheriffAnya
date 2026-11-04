"""776: проверяем происхождение, полный native-writer список и живые payload."""
import hashlib
import json
from pathlib import Path
import re
import unittest

ROOT=Path(__file__).parent
EVIDENCE=ROOT/'testdata/step1-774-776'


class Completion776Tests(unittest.TestCase):
    def test_schema_report_pins_generated_payload_and_all_primary_writer_components(self):
        report=json.loads((ROOT/'payload-776-report.json').read_text())
        self.assertEqual(hashlib.sha256((ROOT/'src/generated_payload_776.rs').read_bytes()).hexdigest(),report['rust_sha256'])
        supported={row['kind'] for row in report['components']}|{25,49,50,75}
        source=(ROOT.parents[1]/'src/stackdata.rs').read_text()
        constants={name:int(value) for name,value in re.findall(r'pub const (COMPONENT_\w+): i32 = (\d+);',source)}
        writer=source.split('pub fn encode_components(data: &Data)',1)[1].split('\npub fn ',1)[0]
        expected={constants[name] for name in re.findall(r'encode_varint\((COMPONENT_\w+)\)',writer)}
        expected|={int(value) for value in re.findall(r'encode_varint\((\d+)\)',writer)}
        expected|={54,55,59,86,88,89,90,104}
        self.assertFalse(expected-supported,expected-supported)
        self.assertEqual(len(report['components']),53)
        self.assertEqual(len(report['layouts']),158)

    def test_unmodified_gui_collector_establishes_all_six_meanings(self):
        report=json.loads((EVIDENCE/'action-client-collector775-v3/result.json').read_text())
        self.assertEqual(report['status'],'pass')
        self.assertTrue(report['collect_only'])
        expected={'cancel':[0,1],'drop-one':[4],'drop-stack':[3],'swap':[6],'release':[5],'jab':[7]}
        self.assertEqual({r['case']:r['statuses'] for r in report['checks']},expected)
        self.assertEqual(report['server_exit'],0)
        self.assertTrue(all(r['filtered'] for r in report['packets'] if r['name']=='block_dig' and r['status'] not in (0,2)))

    def test_original_reference_is_one_777_run_and_does_not_claim_776_observation(self):
        report=json.loads((EVIDENCE/'official777-blockentities-reference/result.json').read_text())
        self.assertEqual((report['status'],report['protocol']),('pass',777))
        self.assertEqual(len(report['entities']),43)
        self.assertEqual(report['missing'],['piston','white_bed'])
        self.assertTrue(all(26100<=port<=26199 for port in report['ports']))
        self.assertIn(report['server_exit'],(-15,143))

    def test_failed_live_null_update_is_retained_instead_of_counted_as_pass(self):
        report=json.loads((EVIDENCE/'payload-actions-live775-v2/result.json').read_text())
        self.assertEqual(report['status'],'failed')
        self.assertEqual(sum(row['post_pong'] for row in report['payloads']),87)
        self.assertFalse(report['payloads'][-1]['post_pong'])
        self.assertTrue(report['payloads'][-1]['hex'].endswith('2800'))
        diagnostics=list((EVIDENCE/'payload-actions-live775-v2/client-debug').glob('disconnect*.txt'))
        self.assertEqual(len(diagnostics),1)
        self.assertIn('Expected non-null compound tag',diagnostics[0].read_text())

    def test_final_ordinary_client_accepts_every_exported_payload_on_both_primaries(self):
        for primary in (775,777):
            report=json.loads((EVIDENCE/f'payload-actions-final{primary}/result.json').read_text())
            self.assertEqual((report['status'],report['primary'],report['server_exit']),('pass',primary,0))
            self.assertFalse(report['collect_only'])
            self.assertTrue(report['payload_post_pong'])
            self.assertEqual(len(report['payloads']),196)
            self.assertTrue(all(row['post_pong'] for row in report['payloads']))
            self.assertEqual(len(report['checks']),6)
            cases={row['case']:row for row in report['checks']}
            self.assertTrue(cases['cancel']['block_retained'])
            self.assertEqual(cases['drop-one']['authoritative_hands']['mainhand']['count'],6)
            self.assertIsNone(cases['drop-stack']['authoritative_hands']['mainhand'])
            self.assertEqual(cases['swap']['authoritative_hands']['mainhand']['count'],5)
            self.assertEqual(cases['swap']['authoritative_hands']['offhand']['count'],3)
            self.assertTrue(cases['release']['arrow_spawn'])
            self.assertEqual(cases['jab']['authoritative_hands']['mainhand']['damage'],1)


    def test_retained_instrument_bee_and_floor_failures_have_separate_causes(self):
        for folder,accepted,label in [('payload-actions-instrument775',139,'primary-component:61'),
                                      ('payload-actions-bees775',146,'primary-component:77')]:
            report=json.loads((EVIDENCE/folder/'result.json').read_text())
            self.assertEqual(report['status'],'failed')
            self.assertEqual(sum(r['post_pong'] for r in report['payloads']),accepted)
            self.assertEqual(report['payloads'][-1]['label'],label)
        floor=json.loads((EVIDENCE/'payload-actions-statue-floor775/result.json').read_text())
        self.assertEqual(floor['status'],'failed')
        self.assertEqual(sum(r['post_pong'] for r in floor['payloads']),196)
        self.assertIn('statue-sitting',floor['error'])

    def test_production_admission_opens_only_776_and_retains_both_774_refusals(self):
        report=json.loads((EVIDENCE/'public776-final/result.json').read_text())
        self.assertEqual(report['status'],'pass')
        self.assertEqual(len(report['checks']),8)
        self.assertEqual(len(report['sessions']),4)
        for row in report['sessions']:
            self.assertEqual(row['admitted'],row['client']==776)
            if row['client']==774:self.assertIn('Ваша версия не подходит под ядро сервера',row['disconnect'])
        self.assertTrue(all(row['exit']==0 for row in report['shutdown']))


if __name__=='__main__':unittest.main()
