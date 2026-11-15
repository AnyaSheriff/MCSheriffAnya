import json
from pathlib import Path
import tempfile
import unittest
from suite import export, PREFLIGHT
from common_cases import common_cases
from special_cases import special_cases

class SuiteTests(unittest.TestCase):
    def test_controlled_actions_allow_complete_tracking_cycles(self):
        # Регрессия собственного CI43712687277: двух тиков не хватало
        # для гарантированной сверки; это проверка состояния, не задержки.
        for case in common_cases():
            if case['family'] in ('turn','teleport'):
                action=case['phases'][-1]
                self.assertTrue(action['commands'])
                self.assertGreaterEqual(action['ticks'],6)

    def test_preflight_and_eight_disjoint_shards(self):
        with tempfile.TemporaryDirectory() as d:
            root=Path(d)
            pre=export(root/'pre','preflight',0)
            self.assertEqual([c['id'] for c in pre],list(PREFLIGHT))
            shards=[export(root/str(i),'suite',i) for i in range(8)]
            ids=[c['id'] for shard in shards for c in shard]
            self.assertEqual(len(ids),574)
            self.assertEqual(len(set(ids)),574)
            self.assertEqual(set(ids),{c['id'] for c in common_cases()+special_cases()})
            self.assertTrue(all(1<=len(s)<=90 for s in shards))
            self.assertIsNone(json.loads((root/'pre/catalog.json').read_text())['runtime_passes'])

if __name__=='__main__':unittest.main()
