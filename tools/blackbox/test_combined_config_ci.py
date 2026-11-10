import hashlib,json,tempfile,unittest
from pathlib import Path
from unittest.mock import patch
from combined_config_ci import expected_tree,fresh_receipt,LOGS

class ProvenanceGuards(unittest.TestCase):
    def setUp(self):
        self.base={'a':('a',),'b':('b',),'c':('c',)}
        self.inputs={'witch':{'a':('w',),'b':('b',),'c':('c',)},'bukkit':{'a':('a',),'b':('p',),'c':('c',)},'config':{'a':('a',),'b':('b',),'c':('s',)}}
        self.actual={'a':('w',),'b':('p',),'c':('s',),'ci':('ci',)}
    def verify(self):return expected_tree(self.base,self.inputs,self.actual,frozenset({'ci'}))
    def test_disjoint_origins(self):self.assertEqual(self.verify(),{'witch':1,'bukkit':1,'config':1})
    def test_modified_production_rejected(self):
        self.actual['a']=('bad',)
        with self.assertRaises(ValueError):self.verify()
    def test_input_overlap_rejected(self):
        self.inputs['config']['a']=('other',)
        with self.assertRaises(ValueError):self.verify()
    def test_extra_helper_and_missing_helper_rejected(self):
        self.actual['rogue']=('x',)
        with self.assertRaises(ValueError):self.verify()
        del self.actual['rogue'];del self.actual['ci']
        with self.assertRaises(ValueError):self.verify()
    def test_deleted_or_added_input_path_rejected(self):
        del self.actual['b']
        with self.assertRaises(ValueError):self.verify()
        self.actual['b']=('p',);self.actual['invented']=('x',)
        with self.assertRaises(ValueError):self.verify()
    def test_exception_cannot_shadow_input(self):
        with self.assertRaises(ValueError):expected_tree(self.base,self.inputs,self.actual,frozenset({'a'}))

class FreshReceiptGuards(unittest.TestCase):
    def setUp(self):
        tmp=tempfile.TemporaryDirectory();self.addCleanup(tmp.cleanup);self.path=Path(tmp.name)
        self.runtime=self.path/'binary';self.runtime.write_bytes(b'\x7fELF\x02\x01'+bytes(12)+(62).to_bytes(2,'little'))
        self.patcher=patch('combined_config_ci.verify_source',return_value={'source':'abc','Cargo.lock_sha256':'lock'});self.patcher.start();self.addCleanup(self.patcher.stop)
        for step in LOGS:
            log=self.path/(step+'.log');log.write_text('test result: ok. 1 passed; 0 failed;\n' if step in ('workspace','no-default') else 'finished\n')
            (self.path/(step+'.success.json')).write_text(json.dumps({'step':step,'source':'abc','exit_code':0,'log_sha256':hashlib.sha256(log.read_bytes()).hexdigest()}))
    def test_complete_fresh_receipt(self):self.assertTrue(fresh_receipt(self.path,self.runtime)['fresh_tests'])
    def test_changed_log_rejected(self):
        (self.path/'workspace.log').write_text('modified')
        with self.assertRaises(AssertionError):fresh_receipt(self.path,self.runtime)
    def test_failed_marker_rejected(self):
        p=self.path/'build.success.json';data=json.loads(p.read_text());data['exit_code']=1;p.write_text(json.dumps(data))
        with self.assertRaises(AssertionError):fresh_receipt(self.path,self.runtime)
    def test_missing_test_success_rejected(self):
        p=self.path/'workspace.log';p.write_text('test result: FAILED. 0 passed; 1 failed;')
        m=self.path/'workspace.success.json';record=json.loads(m.read_text());record['log_sha256']=hashlib.sha256(p.read_bytes()).hexdigest();m.write_text(json.dumps(record))
        with self.assertRaises(AssertionError):fresh_receipt(self.path,self.runtime)
    def test_stale_marker_and_missing_marker_rejected(self):
        p=self.path/'no-default.success.json';data=json.loads(p.read_text());data['source']='older';p.write_text(json.dumps(data))
        with self.assertRaises(AssertionError):fresh_receipt(self.path,self.runtime)
        p.unlink()
        with self.assertRaises(FileNotFoundError):fresh_receipt(self.path,self.runtime)
    def test_wrong_binary_rejected(self):
        self.runtime.write_bytes(b'bad')
        with self.assertRaises(AssertionError):fresh_receipt(self.path,self.runtime)

if __name__=='__main__':unittest.main()
