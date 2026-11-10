"""Только sampled authority gate нового instrumented опыта; старый87 не меняет."""
import math
import re


def named_position(response):
    prefix='blackbox has the following entity data: '
    assert response.startswith(prefix), 'wrong named authority'
    number=r'[+-]?(?:\d+(?:\.\d*)?|\.\d+)(?:[eE][+-]?\d+)?[dD]?'
    found=re.fullmatch(r'\[\s*('+number+r')\s*,\s*('+number+r')\s*,\s*('+number+r')\s*\]',response[len(prefix):])
    assert found, 'malformed authority Pos'
    values=[float(v.rstrip('dD')) for v in found.groups()]
    assert all(math.isfinite(v) for v in values), 'nonfinite authority Pos'
    return values


def qualify(sample, movement10_at, attack_gate_at):
    """Вызывается ровно на fixed100ms gate; ошибка означает ABORT, не ожидание."""
    assert not sample.get('error') and sample.get('state','complete')=='complete', 'authority worker incomplete/error'
    assert 0<=attack_gate_at-(movement10_at+.100)<=.005, 'attack gate late'
    assert movement10_at+.060<=sample['started_at']<=movement10_at+.065, 'worker missed fixed start budget'
    assert sample['ended_at']<=movement10_at+.090, 'authority deadline missed'
    reads=sample['reads']
    assert len(reads)==3 and [r['command'] for r in reads]==[
        'data get entity blackbox Pos','data get entity blackbox OnGround','data get entity blackbox Pos'], 'authority order'
    assert sample['started_at']<=reads[0]['at'] and reads[-1]['ended_at']<=sample['ended_at']
    assert all(a['ended_at']<=b['at'] for a,b in zip(reads,reads[1:])), 'overlapping/reordered reads'
    assert all(movement10_at+.060<=r['at']<=r['ended_at']<=movement10_at+.090 for r in reads), 'stale/late authority read'
    for row in (reads[0],reads[2]):
        assert all(abs(a-b)<=1e-9 for a,b in zip(named_position(row['response']),[100.5,64.04,100.5])), 'authority pose mismatch'
    assert reads[1]['response']=='blackbox has the following entity data: 0b', 'authority grounded or wrong player'
    return {'qualified_sampled_authority':True,'atomic_Attack_tick_proof':False}


class AuthorityWorker:
    """Отдельное заранее открытое RCON-соединение; cleanup не держит общий RCON lock."""
    def __init__(self, control, t0, clock, sleep):
        import threading
        self.control, self.t0, self.clock, self.sleep = control, t0, clock, sleep
        self.lock = threading.RLock()
        self.cancelled = threading.Event()
        self.sample = {'reads': [], 'state': 'scheduled'}
        self.thread = threading.Thread(target=self._run, daemon=True)

    def start(self):
        self.thread.start()

    def _run(self):
        try:
            self.sleep(max(0,self.t0+.060-self.clock()))
            if self.cancelled.is_set():return
            with self.lock:
                self.sample.update(started_at=self.clock(),state='reading')
                assert self.sample['started_at']<=self.t0+.065, 'worker start deadline'
            for field in ('Pos','OnGround','Pos'):
                assert not self.cancelled.is_set(), 'worker cancelled'
                assert self.clock()<self.t0+.090, 'read issue deadline'
                row={'command':'data get entity blackbox '+field,'at':self.clock()}
                with self.lock:self.sample['reads'].append(row)
                response=self.control.run(row['command'])
                with self.lock:row.update(response=response,ended_at=self.clock())
            with self.lock:self.sample.update(ended_at=self.clock(),state='complete')
        except BaseException as error:
            with self.lock:self.sample.update(error=repr(error),state='failed',ended_at=self.clock())

    def snapshot(self):
        import copy
        with self.lock:return copy.deepcopy(self.sample)

    def finish(self):
        import socket
        self.cancelled.set()
        try:self.control.sock.shutdown(socket.SHUT_RDWR)
        except OSError:pass
        self.control.sock.close()
        self.thread.join(.1)
        result=self.snapshot()
        result['worker_gone']=not self.thread.is_alive()
        return result


def actual_attack_receipt(outbound, t0, gate_at, claimed_at, victim_hex):
    attacks=[e for e in outbound if e['name']=='attack' and e['at']>=t0]
    assert len(attacks)==1, 'missing/duplicate actual Attack'
    actual=attacks[0]
    assert actual['hex']==victim_hex, 'wrong actual Attack target'
    assert actual['at']==claimed_at, 'claimed Attack timestamp differs from actual send'
    assert 0<=gate_at-t0-.100<=.005, 'late predispatch gate'
    assert gate_at<=actual['at']<=t0+.105, 'actual Attack dispatch missed deadline'
    return actual
