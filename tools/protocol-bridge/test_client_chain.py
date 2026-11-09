"""Проверки закрепления разрешённых таблиц, без сети и внешнего кеша."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from generate_client_chain import locked, HERE, compress_mapping, translate_prepared_id


class ClientChainSourcesTests(unittest.TestCase):
    def test_prepared_adjacent_767_768_769_keeps_exact_static_id_transforms_closed(self):
        report = json.loads((HERE/'client-chain-report.json').read_text())
        data = report['preparation']
        self.assertIn('admission CLOSED', data['scope'])
        self.assertIn('no payload codec', data['scope'])
        self.assertEqual(list(data['edges']), ['767-768', '768-769'])
        first, second = (data['edges'][key] for key in ('767-768', '768-769'))
        # Значения независимо закреплены из разрешённых JSON: diamond/cow/food/oak_log.
        for kind, old, middle, new in [('Item',805,827,836), ('Entity',22,28,28),
                                      ('Component',20,21,21), ('Block',46,49,49),
                                      ('BlockState',1,1,1), ('Particle',13,13,13)]:
            with self.subTest(kind=kind):
                self.assertEqual(translate_prepared_id(first,kind,767,768,old),middle)
                self.assertEqual(translate_prepared_id(second,kind,768,769,middle),new)
                self.assertEqual(translate_prepared_id(second,kind,769,768,new),middle)
                self.assertEqual(translate_prepared_id(first,kind,768,767,middle),old)
        # Совпадение/похожесть имени не разрешает выдумывать семантические замены.
        for kind, source, target, number, edge in [
                ('Entity',767,768,10,first), ('Entity',767,768,17,first),
                ('Component',767,768,21,first), ('Entity',768,769,30,second),
                ('Item',768,767,140,first)]:
            with self.subTest(kind=kind,source=source,number=number):
                with self.assertRaisesRegex(ValueError,'точная цель'):
                    translate_prepared_id(edge,kind,source,target,number)

    def test_prepared_ranges_are_total_bounded_and_partial_bijections(self):
        report = json.loads((HERE/'client-chain-report.json').read_text())['preparation']
        for edge in report['edges'].values():
            for key, row in edge['registries'].items():
                kind, pair = key.split('/')
                source,target = map(int,pair.split('-'))
                self.assertEqual(row['ranges'][0][0],0)
                self.assertEqual(row['ranges'][-1][1],row['count']-1)
                for left,right in zip(row['ranges'],row['ranges'][1:]):
                    self.assertEqual(left[1]+1,right[0])
                missing = 0
                for number in range(row['count']):
                    try:
                        mapped=translate_prepared_id(edge,kind,source,target,number)
                    except ValueError:
                        missing += 1
                    else:
                        self.assertEqual(translate_prepared_id(edge,kind,target,source,mapped),number)
                self.assertEqual(missing,row['missing'])
                for invalid in [-1,row['count'],1<<32,None,True,'1',1.0]:
                    with self.assertRaises(ValueError):
                        translate_prepared_id(edge,kind,source,target,invalid)
                with self.assertRaises(ValueError):
                    translate_prepared_id(edge,kind,source,777,0)
                with self.assertRaises(ValueError):
                    translate_prepared_id(edge,'Unknown',source,target,0)

    def test_prepared_schema_changes_require_typed_payloads_even_when_ids_match(self):
        edges=json.loads((HERE/'client-chain-report.json').read_text())['preparation']['edges']
        for key, client_id, server_id in [('767-768',0x13,0x0e),('768-769',0x13,0x10)]:
            rows=edges[key]['packets']
            for direction,name,old_id in [('toClient','window_items',client_id),
                                           ('toServer','window_click',server_id)]:
                row=next(r for r in rows if r['phase']=='play' and r['direction']==direction and r['name']==name)
                self.assertEqual(row['old_id'],old_id)
                self.assertFalse(row['schema_equal'])
                self.assertNotEqual(row['old_schema_sha256'],row['new_schema_sha256'])
        #767 recipe packets не становятся768 новым форматом только по совпавшему имени.
        added=next(r for r in edges['767-768']['packets'] if r['phase']=='play' and r['direction']=='toClient' and r['name']=='recipe_book_add')
        self.assertIsNone(added['old_id'])
        self.assertIsNotNone(added['new_id'])

    def test_prepared_sources_retain_exact_release_and_explicit_data_only_status(self):
        manifest=json.loads((HERE/'client-chain-sources.json').read_text())
        for protocol,releases in [(767,['1.21','1.21.1']),(768,['1.21.2','1.21.3'])]:
            row=manifest['versions'][str(protocol)]
            self.assertEqual(row['releases'],releases)
            self.assertIs(row['data_only'],True)
            self.assertEqual(set(row['files']),{'version','protocol','blocks','items','entities'})
            for record in row['files'].values():
                self.assertIn('/'+manifest['commit']+'/',record['url'])
                self.assertEqual(len(record['sha256']),64)
                self.assertGreater(record['bytes'],0)
        for protocol in (769,770,771,772,773):
            self.assertFalse(manifest['versions'][str(protocol)].get('data_only',False))

    def test_compressed_partial_mapping_does_not_merge_missing_with_exact_delta(self):
        self.assertEqual(compress_mapping([0,1,65535,65535,5,6]),
                         [[0,1,0],[2,3,None],[4,5,1]])

    def test_supplemental_end_uses_portals_and_receives_world_after_each_respawn(self):
        evidence = HERE/'testdata/client-chain-770-773'
        for client, primary, folder in [(774,775,'774-dimensions775-end-v3'),
                                         (774,777,'774-dimensions777'),
                                         (776,775,'776-dimensions775'),
                                         (776,777,'776-dimensions777')]:
            with self.subTest(client=client, primary=primary):
                row = json.loads((evidence/folder/'result.json').read_text())
                self.assertEqual((row['status'],row['server_exit'],row['client'],row['primary']),('pass',0,client,primary))
                self.assertEqual([d['name'] for d in row['dimensions']],['minecraft:the_nether','minecraft:overworld','minecraft:the_end','minecraft:overworld'])
                self.assertIn('end-portal-out-and-back-respawn-chunks-short-ack',row['checks'])
                self.assertFalse(any(c.startswith('execute in') for c in row['commands']))
                self.assertIn('fill 10 65 10 12 65 12 end_portal',row['commands'])
                self.assertTrue(any(c.startswith('execute as McsaProbe run setblock ') and c.endswith(' end_portal') for c in row['commands']))
                self.assertGreaterEqual(len(row['acks']),5)
                packets = row['packets']
                respawns = [i for i,r in enumerate(packets) if r['name']=='respawn']
                self.assertEqual(len(respawns),4)
                for start,end in zip(respawns,respawns[1:]+[len(packets)]):
                    self.assertTrue({'map_chunk','position'} <= {r['name'] for r in packets[start:end]})
                log = (evidence/folder/'server.log').read_text()
                self.assertIn('перешёл в minecraft:the_end',log)
                self.assertGreaterEqual(log.count('перешёл в minecraft:overworld'),2)

    def test_lock_checks_both_size_and_hash_before_json_is_used(self):
        raw = b'{"version":773}'
        record = dict(url='https://raw.githubusercontent.com/DallasCarraher/minecraft-data/' + 'a'*40 + '/data/pc/1.21.9/version.json',
                      bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest())
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            (root/'version.json').write_bytes(raw)
            self.assertEqual(locked(root,'version.json',record),dict(version=773))
            for changed in [raw.replace(b'773',b'774'),raw+b' ']:
                (root/'version.json').write_bytes(changed)
                with self.assertRaises(AssertionError):
                    locked(root,'version.json',record)

    def test_only_pinned_minecraft_data_json_urls_are_accepted(self):
        with tempfile.TemporaryDirectory() as folder:
            for url in ['https://example.com/protocol.json',
                        'https://raw.githubusercontent.com/PrismarineJS/minecraft-data/master/data/pc/1.21.9/protocol.json',
                        'https://raw.githubusercontent.com/PrismarineJS/node-minecraft-protocol/'+'a'*40+'/src/client.js',
                        'https://raw.githubusercontent.com/PrismarineJS/minecraft-data/'+'a'*40+'/data/pc/1.21.9/proto.yml']:
                with self.assertRaises(AssertionError):
                    locked(Path(folder),'protocol.json',dict(url=url,bytes=2,sha256='a'*64))

    def test_adjacent_report_keeps_schema_changes_separate_from_admission(self):
        report = json.loads((HERE/'client-chain-report.json').read_text())
        self.assertIn('admission CLOSED',report['scope'])
        self.assertEqual(list(report['edges']),['773-774','772-773','771-772','770-771','769-770'])
        rows = report['edges']['773-774']['packets']
        for name in ['window_items','entity_metadata','recipe_book_add']:
            row = next(r for r in rows if r['direction']=='toClient' and r['name']==name)
            self.assertFalse(row['schema_equal'])
            self.assertNotEqual(row['old_schema_sha256'],row['new_schema_sha256'])
        # Равные packet graphs771/772 не делают равными реестры содержимого.
        edge = report['edges']['771-772']
        self.assertTrue(all(r['schema_equal'] for r in edge['packets']))
        self.assertGreater(edge['registries']['Item/772-771']['missing'],0)


if __name__ == '__main__':
    unittest.main()
