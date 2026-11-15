import hashlib
import io
import json
import zipfile
import unittest
from unittest.mock import patch
from prepare_ci import API, ARTIFACT, RUN, BASE, request, verify_jobs, read_verified_archive


class RequestTests(unittest.TestCase):
    def test_source_jobs_require_complete_success_and_exact_head(self):
        good=dict(run_id=RUN,head_sha=BASE,status='completed',conclusion='success')
        verify_jobs(dict(total_count=1,jobs=[good]))
        for changed in (dict(conclusion='failure'),dict(status='in_progress'),dict(head_sha='wrong'),dict(run_id=1)):
            with self.assertRaises(ValueError): verify_jobs(dict(total_count=1,jobs=[dict(good,**changed)]))
        for jobs in (dict(total_count=2,jobs=[good]),dict(total_count=0,jobs=[])):
            with self.assertRaises(ValueError): verify_jobs(jobs)

    def test_all_receipt_bytes_verified_without_job_status_field(self):
        elf=b'\x7fELFsynthetic-own-test'
        other=b'own test source evidence'
        receipt=dict(source=BASE,scope='test',files={name:dict(bytes=len(data),sha256=hashlib.sha256(data).hexdigest()) for name,data in [('runtime-elf',elf),('source.tar.gz',other)]})
        def archive(extra=other):
            buf=io.BytesIO()
            with zipfile.ZipFile(buf,'w') as z:
                z.writestr('runtime-elf',elf);z.writestr('source.tar.gz',extra);z.writestr('receipt.json',json.dumps(receipt))
            return buf.getvalue()
        good=archive()
        with patch('prepare_ci.DIGEST',hashlib.sha256(good).hexdigest()),patch('prepare_ci.ELF_SHA',hashlib.sha256(elf).hexdigest()):
            self.assertEqual(read_verified_archive(good),(elf,receipt))
            with self.assertRaises(ValueError): read_verified_archive(good+b'tamper')
        bad=archive(b'tampered evidence')
        with patch('prepare_ci.DIGEST',hashlib.sha256(bad).hexdigest()),patch('prepare_ci.ELF_SHA',hashlib.sha256(elf).hexdigest()):
            with self.assertRaisesRegex(ValueError,'receipt member SHA mismatch'): read_verified_archive(bad)

    def test_gh_manages_metadata_and_archive_auth_without_reading_token(self):
        for suffix in (f'runs/{RUN}',f'artifacts/{ARTIFACT}',f'artifacts/{ARTIFACT}/zip'):
            url=API+'/actions/'+suffix
            with self.subTest(url=url), patch('prepare_ci.subprocess.check_output',return_value=b'opaque') as call, \
                 patch('prepare_ci.urllib.request.urlopen') as direct, \
                 patch('prepare_ci.os.environ',{}):
                self.assertEqual(request(url,True),b'opaque')
                call.assert_called_once_with(['gh','api','--method','GET',url],timeout=180)
                direct.assert_not_called()

    def test_authenticated_foreign_url_rejected_before_network(self):
        with patch('prepare_ci.subprocess.check_output') as call:
            with self.assertRaises(ValueError): request('https://blob.example/archive',True)
            with self.assertRaises(ValueError): request(API+'.example/actions/archive',True)
            call.assert_not_called()

    def test_public_pinned_download_has_no_auth_header(self):
        url='https://piston-data.mojang.com/v1/objects/pinned/server.jar'
        with patch('prepare_ci.urllib.request.urlopen') as call, \
             patch('prepare_ci.subprocess.check_output') as gh:
            call.return_value.__enter__.return_value.read.return_value=b'opaque-jar'
            self.assertEqual(request(url),b'opaque-jar')
            req=call.call_args.args[0]
            self.assertEqual(req.full_url,url)
            self.assertEqual(req.header_items(),[])
            gh.assert_not_called()

if __name__=='__main__': unittest.main()
