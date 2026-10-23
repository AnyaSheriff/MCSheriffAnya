"""Проверки закрепления разрешённых таблиц, без сети и внешнего кеша."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest

from generate_client_chain import locked, HERE


class ClientChainSourcesTests(unittest.TestCase):
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
        self.assertEqual(list(report['edges']),['773-774','772-773','771-772','770-771'])
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
