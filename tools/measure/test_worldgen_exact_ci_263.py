import io
import queue
import json
import unittest
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import zipfile
import worldgen_exact_ci_263 as ci


class SourceTests(unittest.TestCase):
    def metadata(self):
        run={'id':ci.RUN,'head_sha':ci.SOURCE,'status':'completed','conclusion':'success'}
        jobs={'total_count':2,'jobs':[{'name':n,'run_id':ci.RUN,'head_sha':ci.SOURCE,'status':'completed','conclusion':v}
                                      for n,v in [('source','success'),('baseline','skipped')]]}
        artifact={'id':ci.ARTIFACT,'expired':False,'name':f'allay-source-{ci.RUN}-1',
                  'digest':'sha256:'+ci.ARCHIVE_SHA,'workflow_run':{'id':ci.RUN,'head_sha':ci.SOURCE}}
        return run,jobs,artifact

    def test_only_named_baseline_can_be_skipped(self):
        run,jobs,artifact=self.metadata();ci.validate_metadata(run,jobs,artifact)
        jobs['jobs'][0]['conclusion']='skipped'
        with self.assertRaises(ValueError):ci.validate_metadata(run,jobs,artifact)
        jobs['jobs'][0]['conclusion']='success';jobs['jobs'][1]['name']='other'
        with self.assertRaises(ValueError):ci.validate_metadata(run,jobs,artifact)

    def test_artifact_hash_and_inner_receipt_checked(self):
        elf=b'\x7fELFexample'; good_sha=ci.digest(elf)
        receipt={'source':ci.SOURCE,'files':{'runtime-elf':{'bytes':len(elf),'sha256':good_sha}}}
        buf=io.BytesIO()
        with zipfile.ZipFile(buf,'w') as z:
            z.writestr('receipt.json',json.dumps(receipt));z.writestr('runtime-elf',elf)
            z.writestr('world/mcsa-world.json','opaque and must not be read')
        archive=buf.getvalue()
        with patch.object(ci,'ARCHIVE_SHA',ci.digest(archive)),patch.object(ci,'ELF_SHA',good_sha):
            self.assertEqual(ci.extract_runtime(archive)[0],elf)
            with self.assertRaises(ValueError):ci.extract_runtime(archive+b'x')
        with patch.object(ci,'ARCHIVE_SHA',ci.digest(archive)),patch.object(ci,'ELF_SHA','0'*64):
            with self.assertRaises(ValueError):ci.extract_runtime(archive)

    def test_plan_is_two_fixed_seeds_with_twelve_unique_targets(self):
        self.assertEqual(len(set(ci.SEEDS)),2)
        self.assertEqual(len(ci.CHUNKS),12)
        self.assertEqual(len(set(ci.CHUNKS)),12)
        self.assertNotIn((0,0),ci.CHUNKS)
        self.assertTrue({(69,-30),(70,-30),(69,-31),(70,-31)}.issubset(ci.CHUNKS))

    def test_allocated_header_detects_incomplete_chunks(self):
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp)
            anvil=folder/'r.0.0.mca';anvil.write_bytes(b'\0\0\0\1'+bytes(4092))
            native=folder/'r.-1.-1.mcs';header=bytearray(8200)
            slot=31+31*32;header[8+slot*8:8+slot*8+8]=(1).to_bytes(4,'big')+bytes(4)
            native.write_bytes(header)
            with patch.dict('sys.modules',{'genaudit_263_io':SimpleNamespace(region_dir=lambda *_:folder)}):
                self.assertEqual(ci.allocated_chunks(folder,[(0,0),(-1,-1),(1,1)]),{(0,0),(-1,-1)})

    def test_freeze_failure_still_closes_server_and_records_cleanup(self):
        class FakeConsole:
            closed=False
            def __init__(self,*args): pass
            def wait_ready(self,*args): return 'Готово (0.1 с)! Чтобы узнать команды, набери help'
            def command(self,*args,**kwargs): return 'unknown response'
            def close(self):
                FakeConsole.closed=True
                return {'server_exit':0,'forced':False,'group_gone':True,'port_free':True}
        with tempfile.TemporaryDirectory() as tmp:
            folder=Path(tmp); (folder/'scratch').mkdir(); (folder/'evidence').mkdir()
            with patch.object(ci,'Console',FakeConsole), patch.object(ci,'port_free',return_value=True):
                with self.assertRaisesRegex(ValueError,'freeze'):
                    ci.generate('ours',1,folder/'fake',folder/'scratch',folder/'evidence',27861)
            self.assertTrue(FakeConsole.closed)
            self.assertTrue(json.loads((folder/'evidence/ours-cleanup.json').read_text())['port_free'])
            self.assertIn('bedrock-port=0', (folder/'scratch/ours/config/mcsa.properties').read_text())

    def test_source_diff_rejects_changed_catalog_and_wrong_status(self):
        required=sorted(ci.ALLOWED_ADDITIONS-{'tools/measure/WORLDGEN-EXACT-20261116.md'})
        good='\n'.join('A\t'+p for p in required)
        self.assertEqual(ci.validate_source_diff(good),required)
        with self.assertRaises(ValueError):ci.validate_source_diff(good+'\nM\tsrc/blocks_table.rs')
        with self.assertRaises(ValueError):ci.validate_source_diff(good+'\nA\ttools/measure/genaudit_263_io.py')
        with self.assertRaises(ValueError):ci.validate_source_diff(good.replace('A\t','M\t',1))

    def test_github_only_guard(self):
        with patch.dict(ci.os.environ,{'GITHUB_ACTIONS':'false'}):
            with self.assertRaises(SystemExit):ci.main()

    def console_without_process(self):
        console=ci.Console.__new__(ci.Console)
        console.lines=queue.Queue()
        console.process=SimpleNamespace(poll=lambda:None,stdin=io.StringIO())
        console.commands=io.StringIO();console.serial=0
        return console

    def test_readiness_requires_complete_marker_without_sending_commands(self):
        for side, marker in [('vanilla','Done (2.5s)! For help, type "help"'),
                             ('ours','Готово (0.1 с)! Чтобы узнать команды, набери help')]:
            console=self.console_without_process()
            console.lines.put('Starting Minecraft server')
            console.lines.put('Сервер запущен на *:27861')
            console.lines.put(marker)
            self.assertEqual(console.wait_ready(side),marker)
            self.assertEqual(console.process.stdin.getvalue(),'')
            self.assertEqual(console.commands.getvalue(),'')

    def test_readiness_fails_for_exited_server_and_deadline(self):
        console=self.console_without_process()
        console.process.poll=lambda:1
        with self.assertRaises(RuntimeError):console.wait_ready('vanilla')
        console.process.poll=lambda:None
        with self.assertRaises(TimeoutError):console.wait_ready('vanilla',timeout=0)

    def test_unexpected_command_error_fails_before_marker_timeout(self):
        console=self.console_without_process()
        console.lines.put('System chat: An unexpected error occurred while trying to execute that command')
        with self.assertRaisesRegex(RuntimeError,'command rejected'):console.command('tick freeze')


if __name__=='__main__':unittest.main()
