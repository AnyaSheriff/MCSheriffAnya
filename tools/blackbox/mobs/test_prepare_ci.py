import unittest
from unittest.mock import patch
from prepare_ci import API, ARTIFACT, RUN, request


class RequestTests(unittest.TestCase):
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
