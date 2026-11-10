import copy
import unittest
from wind_gap_authority_guard import qualify


def good():
    result={'started_at':1.0601,'ended_at':1.070,'reads':[]}
    for i,field in enumerate(('Pos','OnGround','Pos')):
        result['reads'].append({'command':'data get entity blackbox '+field,'at':1.061+i*.002,'ended_at':1.062+i*.002,'response':'blackbox has the following entity data: '+('0b' if field=='OnGround' else '[100.5d, 64.04d, 100.5d]')})
    return result


class Guard(unittest.TestCase):
    def test_sampled_not_atomic(self):
        self.assertFalse(qualify(good(),1.,1.1)['atomic_Attack_tick_proof'])

    def test_late_stale_wrong_pose_ground_order(self):
        for mutate in [lambda x:x.update(started_at=1.066),lambda x:x.update(ended_at=1.091),
                       lambda x:x['reads'][0].update(at=.9),
                       lambda x:x['reads'][0].update(response='local predicted pose64.04'),
                       lambda x:x['reads'][2].update(response='blackbox has the following entity data: [100.5d,64d,100.5d]'),
                       lambda x:x['reads'][1].update(response='blackbox has the following entity data: 1b'),
                       lambda x:x['reads'].reverse()]:
            sample=copy.deepcopy(good());mutate(sample)
            with self.assertRaises(AssertionError):qualify(sample,1.,1.1)
        with self.assertRaises(AssertionError):qualify(good(),1.,1.12)

class WorkerGuards(unittest.TestCase):
    def test_actual_worker_order_error_receipts_and_cleanup(self):
        from wind_gap_authority_guard import AuthorityWorker
        class Clock:
            value=1.0
            def now(self):return self.value
            def sleep(self,n):self.value+=n
        class Socket:
            closed=False
            def shutdown(self,*a):pass
            def close(self):self.closed=True
        class Control:
            def __init__(self,clock,fail=False):self.clock=clock;self.sock=Socket();self.fail=fail;self.calls=[]
            def run(self,command):
                self.calls.append(command);self.clock.value+=.001
                if self.fail:raise TimeoutError('synthetic timeout')
                return 'blackbox has the following entity data: '+('0b' if command.endswith('OnGround') else '[100.5d,64.04d,100.5d]')
        for fail in (False,True):
            clock=Clock();control=Control(clock,fail);worker=AuthorityWorker(control,1.,clock.now,clock.sleep)
            worker.start();worker.thread.join(.1);result=worker.finish()
            self.assertTrue(result['worker_gone']);self.assertTrue(control.sock.closed)
            self.assertTrue(result['reads'])
            if fail:
                self.assertIn('error',result)
                with self.assertRaises(AssertionError):qualify(result,1.,1.1)
            else:
                self.assertEqual(len(control.calls),3);qualify(result,1.,1.1)

    def test_incomplete_early_gate_duplicate_and_nonfinite_rejected(self):
        for change in [lambda x:x.update(state='reading'),lambda x:x.update(error='timeout'),
                       lambda x:x['reads'].append(dict(x['reads'][0])),
                       lambda x:x['reads'][0].update(response='blackbox has the following entity data: [1e999d,64.04d,100.5d]')]:
            sample=good();change(sample)
            with self.assertRaises(AssertionError):qualify(sample,1.,1.1)
        with self.assertRaises(AssertionError):qualify(good(),1.,1.099)

class ActualWireGuards(unittest.TestCase):
    def test_actual_send_deadline_missing_duplicate_and_mismatch(self):
        from wind_gap_authority_guard import actual_attack_receipt
        e={'name':'attack','at':1.101,'hex':'02'}
        actual_attack_receipt([e],1.,1.100,1.101,'02')
        for wires,claimed in [([],1.101),([e,e],1.101),([dict(e,at=1.106)],1.106),([e],1.100)]:
            with self.assertRaises(AssertionError):actual_attack_receipt(wires,1.,1.100,claimed,'02')

    def test_invalid_sample_prevents_dispatch(self):
        sent=[];sample=good();sample['reads'][1]['response']='blackbox has the following entity data: 1b'
        with self.assertRaises(AssertionError):
            qualify(sample,1.,1.1)
            sent.append('attack')
        self.assertEqual(sent,[])

    def test_blocked_socket_read_is_unblocked_by_cleanup(self):
        import socket,time,threading
        from wind_gap_authority_guard import AuthorityWorker
        left,right=socket.socketpair();entered=threading.Event()
        class Control:
            sock=left
            def run(self,command):
                entered.set()
                data=self.sock.recv(1)
                if not data:raise EOFError('shutdown interrupted blocked read')
                return data.decode()
        worker=AuthorityWorker(Control(),time.monotonic()-.060,time.monotonic,time.sleep)
        try:
            worker.start();self.assertTrue(entered.wait(.1))
            result=worker.finish();self.assertTrue(result['worker_gone']);self.assertIn('error',result)
            self.assertEqual(len(result['reads']),1)
        finally:right.close();left.close()

if __name__=='__main__':unittest.main()
