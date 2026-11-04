#!/usr/bin/env python3
"""Настоящий клиент776 на marker-only ядре; запуск только с --execute после GO.

Реле ничего не добавляет к протоколу. Изображения оценивает человек отдельно.
"""
import argparse
import copy
import hashlib
import io
import json
import math
import os
from pathlib import Path
import shutil
import signal
import socket
import struct
import subprocess
import tempfile
import threading
import time
import uuid

from live_client import launch
from observe import frame
from probe_core_776 import PROTOCOL_SHA, Probe776, stop_server
from probe_gameplay import mapping, connect
from probe_initial_play_776 import capture, read_play
from probe_registry_client import exact, integer, known_packs, read_string, require_end
from probe_session import Probe, slot
from smoke import send

PORT, RELAY = 26028, 26030
ITEMS_SHA = 'a0b334b6e8f9583f8d2ae07fb65c201022ee06b1e8a43beb556140e9d8ef2743'
MANDATORY = {'declare_commands', 'declare_recipes', 'player_info', 'entity_metadata',
             'window_items', 'recipe_book_settings', 'advancements', 'map_chunk'}


class FrameDeadline:
    # Общий deadline, включая длину и все части тела кадра.
    def __init__(self, sock, seconds):
        self.sock, self.deadline = sock, time.monotonic()+seconds

    def recv(self, size):
        remaining=self.deadline-time.monotonic()
        if remaining<=0: raise TimeoutError('whole frame deadline')
        self.sock.settimeout(remaining)
        return self.sock.recv(size)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def ground_ready(rows, after, now):
    streak = []
    for row in rows:
        if row['elapsed'] < after or 'motion' not in row:
            continue
        motion = row['motion']
        good = (motion['ground'] and all(v is not None and abs(v-e) < .02
                for v, e in zip(motion['pose'][:3], (.5, 64., .5))))
        if not good or (streak and row['elapsed'] - streak[-1] > 1.5):
            streak = []
        if good:
            streak.append(row['elapsed'])
    return len(streak) >= 3 and streak[-1]-streak[0] >= 2 and 0 <= now-streak[-1] <= 1.5


class Observer:
    """Вызывать под одним lock; переход ACK происходит ДО forwarding."""
    def __init__(self, protocol, rows, clock=time.monotonic):
        self.names = {p: {d: {v:k for k,v in mapping(protocol,p,d).items()}
                      for d in ('toClient','toServer')} for p in ('configuration','play')}
        self.rows, self.clock, self.began = rows, clock, clock()
        self.phase = {'toClient':'login', 'toServer':'login'}
        self.login_success = self.finish = False
        self.offered = self.reply = None
        self.teleports = set()
        self.acks = set()
        self.pose = (None,)*5
        self.inventory = Probe(None, protocol)
        self.inventory.names = []
        self.authority = {}
        self.last_inventory = -1
        self.self_entity = None
        self.self_metadata = False

    def observe(self, direction, body):
        if len(self.rows) >= 8192:
            raise ValueError('observation record limit')
        data = io.BytesIO(body); kind = integer(data); payload = data.read()
        phase = self.phase[direction]
        row = {'direction':direction, 'phase':phase, 'id':kind, 'bytes':len(payload),
               'sha256':digest(payload), 'elapsed':self.clock()-self.began}
        self.rows.append(row)
        if phase == 'login':
            if direction == 'toClient':
                if kind != 2 or self.login_success:
                    raise ValueError('unexpected Login/compression/encryption packet')
                data = io.BytesIO(payload)
                identity = exact(data,16)
                expected = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(),version=3).bytes
                if identity != expected or read_string(data) != 'McsaProbe' or integer(data) != 0:
                    raise ValueError('unexpected offline LoginSuccess')
                session = exact(data,16); require_end(data,'LoginSuccess776')
                if identity == session:
                    raise ValueError('identity/session UUID conflated')
                self.login_success = True; row['name'] = 'login_success'
            else:
                if kind != 3 or payload or not self.login_success:
                    raise ValueError('missing empty Login ACK')
                self.phase = dict.fromkeys(self.phase,'configuration'); row['name']='login_ack'
            return
        name = self.names[phase][direction].get(kind)
        if name is None or name in ('disconnect','kick_disconnect','transfer','start_configuration'):
            raise ValueError(('unknown/disallowed phase packet',phase,direction,kind,name))
        row['name'] = name
        if phase == 'configuration':
            if direction == 'toClient' and kind == 14:
                self.offered = known_packs(io.BytesIO(payload)); row['count']=len(self.offered)
                if self.offered != [('minecraft','core','26.2')]:
                    raise ValueError('unexpected advertised packs')
            elif direction == 'toServer' and kind == 7:
                self.reply = known_packs(io.BytesIO(payload)); row['count']=len(self.reply)
                if self.offered is None or self.reply not in ([],self.offered):
                    raise ValueError('unexpected known packs response')
            elif direction == 'toClient' and kind == 7:
                data=io.BytesIO(payload); row['registry']=read_string(data); row['count']=integer(data)
            elif direction == 'toClient' and kind == 3:
                if payload or self.reply is None: raise ValueError('out of order Finish')
                self.finish=True
            elif direction == 'toServer' and kind == 3:
                if payload or not self.finish: raise ValueError('invalid Finish ACK')
                self.phase=dict.fromkeys(self.phase,'play')
            return
        if direction == 'toClient':
            if name == 'login':
                self.self_entity=struct.unpack('>i',payload[:4])[0]
            if name == 'entity_metadata':
                self.self_metadata |= integer(io.BytesIO(payload))==self.self_entity
            if name == 'position':
                data=io.BytesIO(payload); teleport=integer(data)
                values=struct.unpack('>ddddddffI',exact(data,60));require_end(data,'Position776')
                if not all(math.isfinite(v) for v in values[:-1]): raise ValueError('nonfinite position')
                self.teleports.add(teleport); row['teleport']=teleport
                if len(self.teleports)>1: raise ValueError('unexpected position correction loop')
            if name in ('window_items','set_slot','set_cursor_item','set_player_inventory'):
                if name=='window_items':
                    header=io.BytesIO(payload);window=integer(header);integer(header);count=integer(header)
                    if window!=0 or count!=46: raise ValueError('unexpected authored inventory size/window')
                self.inventory.observe_packet(name,payload)
                self.inventory.names.clear()
                if name == 'set_player_inventory':
                    data=io.BytesIO(payload); place=integer(data);value=slot(data);require_end(data,name)
                    if self.inventory.inventory and 0 <= place <= 40:
                        # Inventory slots0..8 are hotbar; player menu36..44.
                        menu=place+36 if place<9 else place if place<36 else 44-place if place<40 else 45
                        self.inventory.inventory['slots'][menu]=value
                event=len(self.rows)-1
                if name=='window_items':
                    for key in (36,37,'cursor'): self.authority[key]=event
                elif name=='set_cursor_item': self.authority['cursor']=event
                elif name=='set_slot':
                    source=io.BytesIO(payload);window=integer(source);integer(source)
                    place=struct.unpack('>h',exact(source,2))[0]
                    if window==0 and place in (36,37): self.authority[place]=event
                elif name=='set_player_inventory' and place in (0,1):
                    self.authority[place+36]=event
                self.last_inventory=event
                row['inventory']=copy.deepcopy(self.inventory.inventory)
                if row['inventory'] and len(row['inventory']['slots']) > 46:
                    raise ValueError('unexpected inventory window size')
        else:
            if kind == 0:
                data=io.BytesIO(payload); teleport=integer(data);require_end(data,'short ACK')
                if teleport not in self.teleports or teleport in self.acks:
                    raise ValueError('unknown/duplicate teleport ACK')
                self.acks.add(teleport);row['teleport']=teleport
            elif kind in (30,31,32,33):
                # Собственный decoder, ACK не используется как доказательство позы.
                value=read_play(body,-1,self.pose); self.pose=value['pose']
                row['motion']={'pose':list(self.pose),'ground':value['ground']}
            elif name == 'window_click':
                data=io.BytesIO(payload)
                row['click']={'window':integer(data),'revision':integer(data),
                              'slot':struct.unpack('>h',exact(data,2))[0],
                              'button':exact(data,1)[0],'mode':integer(data),
                              'prediction_sha256':digest(data.read())}


def inventory_is(observer, stone, moved):
    value=observer.inventory.inventory
    if not value or value['window'] != 0 or len(value['slots']) != 46 or value['cursor'] is not None:
        return False
    slots=value['slots']; expected={'item':stone,'count':5}
    return slots[36 if moved else 37] is None and slots[37 if moved else 36] == expected and sum(
        s is not None for s in slots) == 1


def preflight_gui_tools():
    """Нет инструмента/динамической библиотеки — отказ до запуска сервера."""
    names=('xdotool','import','xvfb-run','Xvfb','xauth')
    tools={name:shutil.which(name) for name in names}
    missing=[name for name,path in tools.items() if path is None]
    if missing:
        raise RuntimeError('До запуска сервера не найдены GUI-инструменты: '+', '.join(missing))
    # --version не подключается к DISPLAY; проверяет и загрузку libxdo.
    result=subprocess.run([tools['xdotool'],'--version'],check=True,timeout=5,
                          stdout=subprocess.PIPE,stderr=subprocess.PIPE,text=True)
    if not result.stdout.startswith('xdotool version '):
        raise RuntimeError('Неожиданный ответ xdotool --version')
    return tools


def own_display(client):
    matches=[]
    for candidate in Path('/proc').iterdir():
        if not candidate.name.isdigit(): continue
        try:
            pid=int(candidate.name)
            if os.getsid(pid)!=client.pid or not (candidate/'exe').resolve().name.startswith('java'): continue
            pairs=(candidate/'environ').read_bytes().split(b'\0')
            env={k.decode():v.decode() for item in pairs if b'=' in item
                 for k,v in [item.split(b'=',1)] if k in (b'DISPLAY',b'XAUTHORITY')}
            if env.get('DISPLAY'): matches.append((pid,env))
        except (OSError,ProcessLookupError): continue
    if len(matches)!=1: raise RuntimeError('expected one Java on own private Xvfb')
    pid,private=matches[0]
    if not private['DISPLAY'].startswith(':'): raise RuntimeError('nonlocal display')
    env=os.environ.copy()
    for key in ('DISPLAY','XAUTHORITY','WAYLAND_DISPLAY','WAYLAND_SOCKET','DBUS_SESSION_BUS_ADDRESS'):
        env.pop(key,None)
    env.update(private)
    windows=subprocess.check_output(['xdotool','search','--onlyvisible','--pid',str(pid),
                                     '--name','^Minecraft'],env=env,timeout=10).decode().split()
    if len(windows)!=1 or not windows[0].isdigit(): raise RuntimeError('ambiguous own game window')
    return env,windows[0]


def gui(client, *arguments):
    env,window=own_display(client)
    subprocess.run(['xdotool','windowfocus','--sync',window],env=env,check=True,timeout=10,
                   stdout=subprocess.PIPE,stderr=subprocess.PIPE)
    subprocess.run(['xdotool',*arguments],env=env,check=True,timeout=10,
                   stdout=subprocess.PIPE,stderr=subprocess.PIPE)


def calibration(raw, screenshot_sha):
    value=json.loads(raw)
    if value.get('screenshot_sha256')!=screenshot_sha or value.get('inventory_verified') is not True:
        raise ValueError('calibration must reference inspected inventory PNG')
    points={}
    for key in ('slot36','slot37','recipe_book'):
        xy=value.get(key)
        if not isinstance(xy,list) or len(xy)!=2 or any(type(v) is not int for v in xy):
            raise ValueError('invalid calibrated point')
        if not (0<=xy[0]<854 and 0<=xy[1]<480): raise ValueError('point outside private display')
        points[key]=xy
    if len({tuple(p) for p in points.values()})!=3: raise ValueError('duplicate calibrated points')
    return points


def group_pids(group):
    pids=[]
    for entry in Path('/proc').iterdir():
        if entry.name.isdigit():
            try:
                if os.getpgid(int(entry.name))==group: pids.append(int(entry.name))
            except (OSError,ProcessLookupError): pass
    return pids


def free_port(port):
    try:
        with socket.socket() as sock:
            sock.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);sock.bind(('127.0.0.1',port))
        return True
    except OSError: return False


def stop_client_bounded(client):
    """Завершаем только свою группу; все ожидания имеют конечный срок."""
    try: os.killpg(client.pid,signal.SIGTERM)
    except ProcessLookupError: pass
    try: client.wait(timeout=10)
    except subprocess.TimeoutExpired:
        try: os.killpg(client.pid,signal.SIGKILL)
        except ProcessLookupError: pass
        client.wait(timeout=5)


def finish_run(report, output, core, game, client, server, audit, peers, listener,
               workers, ending, previous_signals):
    """Каждая ошибка уборки сохраняется; остальные шаги всё равно выполняются."""
    errors=report.setdefault('cleanup_errors',[])

    def attempt(label, action, fallback=None):
        try: return action()
        except BaseException as error:
            errors.append({'step':label,'error':repr(error)})
            report['result']='FAIL'
            return fallback

    # Повторный Ctrl-C/SIGTERM не должен оборвать finally посередине.
    for signum in (signal.SIGINT,signal.SIGTERM,signal.SIGALRM):
        attempt('ignore-cleanup-signal-'+str(signum),lambda signum=signum:signal.signal(signum,signal.SIG_IGN))
    attempt('cancel-alarm',lambda:signal.alarm(0))
    ending.set()
    if audit: attempt('close-raw-audit',audit.sock.close)
    if client and 'client_exit' not in report:
        attempt('stop-client',lambda:stop_client_bounded(client))
        report['client_exit']=client.returncode
    for index,sock in enumerate(peers):
        def shutdown(sock=sock):
            try: sock.shutdown(socket.SHUT_RDWR)
            except OSError: pass
        attempt('shutdown-peer-'+str(index),shutdown)
        attempt('close-peer-'+str(index),sock.close)
    if listener: attempt('close-listener',listener.close)
    for index,worker in enumerate(workers):
        attempt('join-relay-'+str(index),lambda worker=worker:worker.join(timeout=5))
    if server:
        report['shutdown']=attempt('stop-core',lambda:stop_server(server),'stop-failed')
        report['server_exit']=server.returncode

    # Если обычный stop бросил исключение или оставил потомка, не оставляем
    # свою группу жить. Принудительная уборка никогда не превращает FAIL в PASS.
    for process in (client,server):
        if process is None: continue
        group=process.pid
        alive=attempt('inspect-group-'+str(group),lambda:group_pids(group),[group])
        if alive:
            errors.append({'step':'forced-group-cleanup','group':group,'pids':alive})
            report['result']='FAIL'
            for sig in (signal.SIGTERM,signal.SIGKILL):
                def kill_group(sig=sig,group=group):
                    try: os.killpg(group,sig)
                    except ProcessLookupError: pass
                attempt('signal-owned-group-'+str(group)+'-'+str(sig),kill_group)
                deadline=time.monotonic()+2
                while time.monotonic()<deadline:
                    if not attempt('wait-group-'+str(group),lambda:group_pids(group),[group]): break
                    # Reap only the Popen child; never wait on somebody else's PID.
                    attempt('poll-owned-child-'+str(group),process.poll)
                    time.sleep(.05)
                if not attempt('inspect-group-'+str(group),lambda:group_pids(group),[group]): break
        attempt('reap-owned-child-'+str(group),lambda:process.wait(timeout=2))
    if client: report['client_exit']=client.returncode
    if server: report['server_exit']=server.returncode

    def diagnostics():
        for directory in ('debug','crash-reports','logs'):
            for source in (game/directory).rglob('*'):
                def copy_diagnostic(source=source):
                    if source.is_file() and source.stat().st_size<=1024*1024:
                        dest=output/'client-diagnostics'/source.relative_to(game)
                        dest.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(source,dest)
                attempt('preserve-diagnostic-'+source.name,copy_diagnostic)
    attempt('enumerate-diagnostics',diagnostics)
    report['cleanup']={
        'ports_free':{str(port):attempt('inspect-port-'+str(port),lambda port=port:free_port(port),False)
                      for port in (PORT,RELAY)},
        'workers_joined':attempt('inspect-workers',lambda:all(not w.is_alive() for w in workers),False),
        'owned_groups':{str(p.pid):attempt('inspect-final-group-'+str(p.pid),lambda p=p:group_pids(p.pid),[p.pid])
                        for p in (client,server) if p}}
    cleanup=report['cleanup']
    if (errors or report.get('error') or report.get('server_exit')!=0 or not all(cleanup['ports_free'].values()) or
        not cleanup['workers_joined'] or any(cleanup['owned_groups'].values()) or report.get('relay_errors')):
        report['result']='FAIL'
    if report['result']=='FAIL':
        attempt('preserve-failed-core',lambda:shutil.copytree(core,output/'failed-server-state'))
    review={'status':'pending','screenshots':report['screenshots'],
            'required':['floor and HUD','advancement card/tab/icon/lines',
            'stone5 first slot','stone5 second slot and empty cursor','same after reopen','recipe icons/titles']}
    attempt('write-visual-review',lambda:(output/'visual-review.json').write_text(json.dumps(review,indent=2)+'\n'))
    # Последний шаг не зависит от копирования диагностики или остановки Java.
    try:
        (output/'result.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    finally:
        for signum,handler in previous_signals.items():
            # Ошибки восстановления также видны в уже записанном результате.
            count=len(errors)
            attempt('restore-signal-'+str(signum),lambda signum=signum,handler=handler:signal.signal(signum,handler))
            if len(errors)!=count:
                (output/'result.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute',action='store_true')
    for name in ('test-binary','protocol','client-dir','items','output'):
        parser.add_argument('--'+name,type=Path)
    parser.add_argument('--binary-sha256')
    args=parser.parse_args()
    if not args.execute:
        print(json.dumps({'execute':False,'core':PORT,'relay':RELAY,'visual_review':'required',
                          'scope':'guarded ordinary core776; native775, not public support'}));return
    if not all((args.test_binary,args.protocol,args.client_dir,args.items,args.output,args.binary_sha256)):
        parser.error('explicit verified binary/SHA, pinned protocol/items, opaque client and fresh output required')
    gui_tools=preflight_gui_tools()
    if digest(args.test_binary.read_bytes())!=args.binary_sha256: raise ValueError('binary SHA mismatch')
    raw=args.protocol.read_bytes()
    if digest(raw)!=PROTOCOL_SHA: raise ValueError('protocol SHA mismatch')
    protocol=json.loads(raw)
    if digest(args.items.read_bytes())!=ITEMS_SHA: raise ValueError('pinned items SHA mismatch')
    items=json.loads(args.items.read_text()); stone=next(v['id'] for v in items if v['name']=='stone')
    if not all(free_port(p) for p in (PORT,RELAY)): raise RuntimeError('test port occupied')
    args.output=args.output.resolve();args.output.mkdir(parents=True,exist_ok=False)
    os.sched_setaffinity(0,sorted(os.sched_getaffinity(0))[:2])
    report={'result':'FAIL','scope':'actual normal core776, native775, bounded inventory/GUI',
            'gui_tools':gui_tools,
            'test_binary_sha256':args.binary_sha256,'items_sha256':digest(args.items.read_bytes()),
            'packets':[],'commands':[],'checks':[],'screenshots':[],
            'visual_review':'pending','complete_compatibility':False}
    observer=Observer(protocol,report['packets']); lock=threading.RLock();ending=threading.Event()
    peers=[];workers=[];server=client=None;listener=None;audit=None
    with tempfile.TemporaryDirectory(prefix='mcsa-actual776-') as folder:
        root=Path(folder);core=root/'core';game=root/'client';core.mkdir();game.mkdir()
        (core/'mcsa-gameplay-probe.marker').touch()
        log=args.output/'server.log'
        with log.open('w') as slog,(args.output/'client.log').open('w') as clog:
            def command(text):
                report['commands'].append({'text':text,'elapsed':time.monotonic()-observer.began})
                server.stdin.write(text+'\n');server.stdin.flush()

            def wait(predicate,seconds=30):
                deadline=time.monotonic()+seconds
                while time.monotonic()<deadline:
                    with lock:
                        if report.get('relay_errors'): raise RuntimeError(report['relay_errors'])
                        if server.poll() is not None or (client and not ending.is_set() and client.poll() is not None):
                            raise RuntimeError('owned process exited early')
                        text=log.read_text()
                        if 'MCSA play probe failure:' in text or 'probe connection:' in text:
                            raise RuntimeError('core session failed; see server.log')
                        if predicate(): return
                    time.sleep(.05)
                raise TimeoutError('evidence deadline')

            def relay(source,target,direction):
                try:
                    # Deadline applies to the entire frame; timeout is never retried.
                    while not ending.is_set():
                        body=frame(FrameDeadline(source,30))
                        with lock: observer.observe(direction,body)
                        send(target,body)
                except BaseException as error:
                    if not ending.is_set():
                        with lock: report.setdefault('relay_errors',[]).append(direction+': '+repr(error))
                    for sock in peers:
                        try: sock.shutdown(socket.SHUT_RDWR)
                        except OSError: pass

            def snapshot(name):
                filename=name+'.png';sha=capture(client,args.output/filename)
                report['screenshots'].append({'file':filename,'sha256':sha,'visual_review':'pending'})
                (args.output/'progress.json').write_text(json.dumps(report['screenshots'],indent=2)+'\n')
                return sha

            previous_signals={}
            def interrupted(signum,_frame):
                raise InterruptedError('bounded runner interrupted: '+str(signum))
            for signum in (signal.SIGTERM,signal.SIGINT,signal.SIGALRM):
                previous_signals[signum]=signal.signal(signum,interrupted)
            signal.alarm(600)
            try:
                server=subprocess.Popen([str(args.test_binary.resolve()),'--ignored','--exact',
                    'gameplay_probe::serve_java_26_2_probe','--nocapture'],cwd=core,stdin=subprocess.PIPE,
                    stdout=slog,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                report['core_pid']=server.pid
                wait(lambda:'MCSA gameplay probe ready on 26028' in log.read_text(),120)
                command('forceload add -24 -24 23 23')
                wait(lambda:'Marked 16 chunks' in log.read_text() and 'to be force loaded' in log.read_text())
                report['checks'].append('16 chunks retained before Java')
                listener=socket.socket();listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
                listener.bind(('127.0.0.1',RELAY));listener.listen(1);listener.settimeout(120)
                client=launch(args.client_dir.resolve(),game,RELAY,clog,version='26.2',
                              gui_scale=2,pause_on_lost_focus=False)
                report['client_group']=client.pid
                peer,_=listener.accept();peers.append(peer);peer.settimeout(30)
                backend=socket.create_connection(('127.0.0.1',PORT),timeout=10);peers.append(backend)
                body=frame(FrameDeadline(peer,30));data=io.BytesIO(body)
                if integer(data)!=0 or integer(data)!=776: raise ValueError('expected protocol776')
                read_string(data);exact(data,2)
                if integer(data)!=2: raise ValueError('expected Login handshake')
                require_end(data,'Handshake');send(backend,body)
                body=frame(FrameDeadline(peer,30));data=io.BytesIO(body)
                if integer(data)!=0 or read_string(data)!='McsaProbe': raise ValueError('unexpected Login Start')
                exact(data,16);require_end(data,'LoginStart');send(backend,body)
                for source,target,direction in ((backend,peer,'toClient'),(peer,backend,'toServer')):
                    worker=threading.Thread(target=relay,args=(source,target,direction),daemon=True)
                    workers.append(worker);worker.start()
                wait(lambda:observer.phase['toClient']=='play' and observer.acks and observer.self_metadata and
                     MANDATORY<={r.get('name') for r in report['packets'] if r['direction']=='toClient'} and
                     any(r.get('name')=='player_loaded' and r['direction']=='toServer' for r in report['packets']) and
                     'McsaProbe' in log.read_text(),120)
                report['checks'].append('normal entry and mandatory first ticks')
                wait(lambda:ground_ready(report['packets'],0,time.monotonic()-observer.began))
                # Проверяем обычный survival-инвентарь. Creative776 пока
                # закрыт: его произвольные ItemStackTemplate ещё не приняты.
                before=len(report['packets'])
                command('gamemode survival McsaProbe')
                wait(lambda:any(r.get('name')=='abilities' and r['direction']=='toClient'
                                for r in report['packets'][before:]))
                time.sleep(.5)
                snapshot('01-world')
                before=len(report['packets'])
                command('advancement grant McsaProbe only minecraft:story/root')
                wait(lambda:any(r.get('name')=='advancements' for r in report['packets'][before:]))
                gui(client,'key','l');time.sleep(1);snapshot('02-advancements');gui(client,'key','Escape')
                command('give McsaProbe stone 5');wait(lambda:inventory_is(observer,stone,False))
                gui(client,'key','e');time.sleep(.5);sha=snapshot('03-inventory-before')
                (args.output/'checkpoint.json').write_text(json.dumps({'waiting_for':'calibration.json',
                    'screenshot_sha256':sha,'deadline_seconds':120,'required_keys':['inventory_verified',
                    'slot36','slot37','recipe_book'],'coordinate_space':'private854x480 root screenshot'})+'\n')
                wait(lambda:(args.output/'calibration.json').is_file(),120)
                points=calibration((args.output/'calibration.json').read_text(),sha)
                for place in (36,37):
                    with lock:
                        before=len(report['packets']);revision=observer.inventory.inventory['revision']
                    x,y=points['slot'+str(place)];gui(client,'mousemove','--sync',str(x),str(y),'click','1')
                    wait(lambda:any(r.get('click',{}).get('slot')==place for r in report['packets'][before:]))
                    with lock:
                        clicks=[r['click'] for r in report['packets'][before:] if 'click' in r]
                        if len(clicks)!=1 or clicks[0] != dict(clicks[0],window=0,revision=revision,slot=place,button=0,mode=0):
                            raise ValueError('unexpected real GUI click/revision')
                    if place==36:
                        # Совпавшие predictions могут не вызывать ответа.
                        time.sleep(.5)
                report['checks'].append('actual GUI click slots36/37 and real received revisions')
                report['gui_authoritative_final_before_logout']=inventory_is(observer,stone,True)
                snapshot('04-inventory-after');gui(client,'key','e');time.sleep(.3);gui(client,'key','e')
                time.sleep(.5)
                snapshot('05-inventory-reopen')
                x,y=points['recipe_book'];gui(client,'mousemove','--sync',str(x),str(y),'click','1')
                time.sleep(1);snapshot('06-recipe-book')
                if not any(r.get('name')=='recipe_book_add' for r in report['packets']):
                    raise ValueError('ordinary recipe unlock packet absent')
                gui(client,'key','Escape');after=time.monotonic()-observer.began
                wait(lambda:ground_ready(report['packets'],after,time.monotonic()-observer.began))
                wait(lambda:any(server_row.get('name')=='keep_alive' and
                     server_row['direction']=='toClient' and server_row['elapsed']>=after and any(client_row.get('name')=='keep_alive' and
                     client_row['direction']=='toServer' and client_row['sha256']==server_row['sha256'] and
                     client_row['elapsed']>=server_row['elapsed'] for client_row in report['packets'])
                     for server_row in report['packets']))
                report['checks'].append('real keepalive response after GUI session')
                latest=game/'logs/latest.log'
                if not latest.exists(): raise RuntimeError('client log unavailable')
                text=latest.read_text()
                if any(s in text for s in ('Failed to handle packet','Failed to load chunk','Reported exception')):
                    raise RuntimeError('client rejected session data')
                if list((game/'debug').glob('disconnect-*.txt')) or list((game/'crash-reports').glob('*.txt')):
                    raise RuntimeError('client diagnostic present')
                # Отдельный обычный RAW Login тем же игроком, после GUI logout.
                # Ни одного inventory click/command этот audit не отправляет.
                ending.set();stop_client_bounded(client);report['client_exit']=client.returncode
                for sock in peers:
                    try: sock.shutdown(socket.SHUT_RDWR)
                    except OSError: pass
                    sock.close()
                for worker in workers: worker.join(timeout=5)
                listener.close();listener=None
                identity=uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(),version=3)
                saved=core/'world/playerdata'/(identity.hex+'.mcrw')
                wait(lambda:saved.is_file() and 'McsaProbe потерял соединение' in log.read_text())
                signal.alarm(45)
                audit=connect(PORT,protocol,Probe776,776,login_uuid=identity.bytes)
                audit.sock.settimeout(10)
                audit.pump(20,lambda row:row['name']=='window_items')
                report['gui_logout_save']={'own_player_file':saved.name,'bytes':saved.stat().st_size,
                                          'sha256':digest(saved.read_bytes()),
                                          'hash_taken_after_session_lease_and_relogin':True,
                                          'debug_save_load_log':'unavailable: ignored server leaves debug disabled'}
                # Primary authoritative initial snapshot, not a client prediction.
                report['raw_relogin_audit']={'kind':'separate non-GUI ordinary Login',
                    'same_offline_uuid':identity.hex,'initial_inventory':copy.deepcopy(audit.inventory),
                    'received_names':list(audit.names),'inventory_actions_sent':0}
                class Snapshot:
                    pass
                snapshot_observer=Snapshot();snapshot_observer.inventory=audit
                if not inventory_is(snapshot_observer,stone,True):
                    raise ValueError('RAW relogin authoritative result differs:36empty/37stone5/cursorempty')
                wait(lambda:log.read_text().count('McsaProbe зашёл на сервер')>=2,10)
                report['checks'].append('separate RAW relogin authoritative persisted GUI result')
                report['packet_evidence']='PASS';report['client_acceptance']='PASS'
                report['result']='NETWORK_PASS_VISUAL_PENDING'
            except BaseException as error:
                report['error']=repr(error)
            finally:
                finish_run(report,args.output,core,game,client,server,audit,peers,listener,
                           workers,ending,previous_signals)
    print(json.dumps({'result':report['result'],'output':str(args.output)}))
    if report['result']=='FAIL': raise SystemExit(1)


if __name__=='__main__': main()
