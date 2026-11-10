"""Synthetic proc metadata only: no process launch."""
import unittest
from unittest.mock import patch
from types import SimpleNamespace
import owned_client_cleanup as c

def row(pid=10,state='S',starttime=50):
    return dict(pid=pid,state=state,ppid=1,pgid=10,sid=10,starttime=starttime)

def pinned():return dict(root=row(),initial=[row()])

class Tests(unittest.TestCase):
    def test_parse(self):
        fields=['S','1','10','10']+['0']*15+['50']
        self.assertEqual(c.parse_stat(10,'10 (own name) '+' '.join(fields)),row())
        for raw in ('bad','11 (x) S','10 (x) S'):
            with self.assertRaises((ValueError,IndexError)):c.parse_stat(10,raw)
    def test_launch_pin_before_poll(self):
        with patch.object(c.Path,'read_text',return_value='stat'),patch.object(c,'parse_stat',return_value=row()),patch.object(c,'snapshot',return_value=[row()]):
            self.assertEqual(c.pin_owned(SimpleNamespace(pid=10)),pinned())
    def test_child_after_wrapper(self):
        with patch.object(c,'snapshot',side_effect=[[row(),row(11)],[row(11)],[],[],[]]),patch.object(c,'same_identity',return_value=True),patch.object(c.os,'kill') as kill:
            result=c.finish_owned(SimpleNamespace(pid=10),lambda _:None,pinned())
        self.assertTrue(result['group_gone']);self.assertIn(unittest.mock.call(11,c.signal.SIGTERM),kill.call_args_list)
    def test_zombie(self):
        z=row(11,'Z')
        with patch.object(c,'snapshot',side_effect=[[row()],[z],[z]]),patch.object(c,'same_identity',side_effect=[True,False,False,False]),patch.object(c.time,'monotonic',side_effect=[0,4,5,9]):
            result=c.finish_owned(SimpleNamespace(pid=10),lambda _:None,pinned())
        self.assertFalse(result['group_gone']);self.assertEqual(result['zombies'],[z])
    def test_reuse_before_cleanup_snapshot(self):
        with patch.object(c,'same_identity',return_value=False),patch.object(c,'snapshot') as snapshot,patch.object(c.os,'kill') as kill:
            with self.assertRaises(ValueError):c.finish_owned(SimpleNamespace(pid=10),lambda _:self.fail('unowned stop'),pinned())
        snapshot.assert_not_called();kill.assert_not_called()
    def test_pin_failure_cleanup_no_signal(self):
        with patch.object(c,'snapshot') as snapshot,patch.object(c.os,'kill') as kill:
            with self.assertRaisesRegex(ValueError,'missing original launch identity'):
                c.finish_owned(SimpleNamespace(pid=10),lambda _:self.fail('unverified stop'),None)
        snapshot.assert_not_called();kill.assert_not_called()
    def test_reused_identity_check(self):
        with patch.object(c.Path,'read_text',return_value='stat'),patch.object(c,'parse_stat',return_value=row(starttime=51)):
            self.assertFalse(c.same_identity(row()))
    def test_descendant_requires_verified_anchor(self):
        child=row(11);child['ppid']=10
        reused=row(starttime=51)
        entries=[SimpleNamespace(name='10'),SimpleNamespace(name='11')]
        class FakeEntry:
            def __init__(self,n):self.name=n
            def __truediv__(self,_):return SimpleNamespace(read_text=lambda:self.name)
        with patch.object(c.Path,'iterdir',return_value=[FakeEntry('10'),FakeEntry('11')]),patch.object(c,'parse_stat',side_effect=[reused,child]):
            self.assertEqual(c.snapshot([row()]),[])
if __name__=='__main__':unittest.main()
