#!/usr/bin/env python3
"""Два настоящих mousehold777 в отдельном Xvfb; без инъекции пакетов."""
import argparse
import hashlib
import io
import json
import math
import os
from pathlib import Path
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
from probe_core_776_client import FrameDeadline, gui, free_port, group_pids, stop_client_bounded, preflight_gui_tools
from probe_core_776 import stop_server
from probe_gameplay import mapping
from probe_initial_play_776 import capture
from probe_registry_client import exact, integer, known_packs, read_string, require_end
from probe_white_concrete_slab import (inputs, JAR_SHA1, TARGET, changes, item_metadata,
                                      slot, server_properties)
from smoke import send

PORT, RELAY = 26030, 26032
OUT = {'player_loaded','teleport_confirm','position','position_look','look','flying',
       'tick_end','player_input','held_item_slot','abilities','entity_action','arm_animation','block_dig'}
IN = {'position','abilities','game_state_change','acknowledge_player_digging',
      'block_change','multi_block_change','block_break_animation','set_slot',
      'set_player_inventory','window_items','set_cursor_item','spawn_entity','entity_metadata','collect'}


def bounded_calibration(raw, sha, label):
    value = json.loads(raw)
    if (value.get('screenshot_sha256') != sha or value.get('case') != label
            or value.get('crosshair_target_verified') is not True
            or value.get('floor_tool_hud_verified') is not True
            or value.get('target') != list(TARGET)):
        raise ValueError('need inspected current screenshot and exact crosshair target')
    return value


class Observer777:
    """Только777; вызывать под одним lock, ACK transition до forwarding."""
    def __init__(self, protocol, rows, item_entity, clock=time.monotonic):
        self.names = {p:{d:{v:k for k,v in mapping(protocol,p,d).items()}
                        for d in ('toClient','toServer')} for p in ('configuration','play')}
        self.rows, self.clock, self.began = rows, clock, clock()
        self.phase = dict.fromkeys(('toClient','toServer'),'login')
        self.login = self.finish = self.loaded = False
        self.offered = self.reply = None
        self.pose = [None]*5
        self.teleports, self.acks, self.counts = set(), set(), {}
        self.item_entity, self.items = item_entity, set()
        self.mainhand = None
        self.stage = 'join'

    def add(self, row):
        if len(self.rows) >= 8192: raise ValueError('selected record limit')
        self.rows.append(row)

    def observe(self, direction, body):
        data = io.BytesIO(body); ident = integer(data); payload = data.read()
        phase = self.phase[direction]
        row = {'direction':direction,'phase':phase,'id':ident,'bytes':len(payload),
               'elapsed':self.clock()-self.began,'stage':self.stage}
        if phase == 'login':
            if direction == 'toClient':
                if ident != 2 or self.login: raise ValueError('unexpected login/compression/encryption')
                data = io.BytesIO(payload)
                identity = uuid.UUID(bytes=hashlib.md5(b'OfflinePlayer:McsaProbe').digest(),version=3).bytes
                if exact(data,16)!=identity or read_string(data)!='McsaProbe' or integer(data)!=0:
                    raise ValueError('unexpected offline LoginSuccess777')
                if exact(data,16)==identity: raise ValueError('session/identity conflated')
                require_end(data,'LoginSuccess777'); self.login=True; row['name']='login_success'
            else:
                if ident!=3 or payload or not self.login: raise ValueError('invalid Login ACK')
                row['name']='login_ack'; self.phase=dict.fromkeys(self.phase,'configuration')
            self.add(row); return
        name = self.names[phase][direction].get(ident)
        if name is None or name in {'disconnect','kick_disconnect','transfer','start_configuration'}:
            raise ValueError(('unsupported packet/phase',phase,direction,ident,name))
        key = direction+'/'+phase+'/'+name
        self.counts[key] = self.counts.get(key,0)+1
        row['name']=name
        if phase == 'configuration':
            if name=='select_known_packs':
                packs=known_packs(io.BytesIO(payload)); row['count']=len(packs)
                if direction=='toClient':
                    if packs!=[('minecraft','core','26.3')]: raise ValueError('unexpected777packs')
                    self.offered=packs
                else:
                    if self.offered is None or packs not in ([],self.offered): raise ValueError('bad packs reply')
                    self.reply=packs
            elif name=='finish_configuration':
                if payload: raise ValueError('nonempty Finish')
                if direction=='toClient':
                    if self.reply is None: raise ValueError('Finish before packs reply')
                    self.finish=True
                else:
                    if not self.finish: raise ValueError('early Finish ACK')
                    self.phase=dict.fromkeys(self.phase,'play')
            else:
                # Никаких registry/chunk/datapack payload/hash на диске.
                return
            self.add(row); return
        if name not in (OUT if direction=='toServer' else IN): return
        data=io.BytesIO(payload)
        if direction=='toServer':
            if name=='teleport_confirm':
                teleport=integer(data); values=struct.unpack('>dddff',exact(data,32))
                require_end(data,name)
                if teleport not in self.teleports or teleport in self.acks: raise ValueError('unexpected TP ACK777')
                if not all(math.isfinite(v) for v in values): raise ValueError('nonfinite TP ACK')
                self.acks.add(teleport);row.update(teleport=teleport,confirmed=list(values))
            elif name in {'position','position_look','look','flying'}:
                if name in {'position','position_look'}: self.pose[:3]=struct.unpack('>ddd',exact(data,24))
                if name in {'look','position_look'}: self.pose[3:]=struct.unpack('>ff',exact(data,8))
                flags=exact(data,1)[0];require_end(data,name)
                if flags&~3 or any(v is not None and not math.isfinite(v) for v in self.pose):
                    raise ValueError('invalid movement')
                row['motion']={'pose':list(self.pose),'ground':bool(flags&1)}
            elif name=='block_dig':
                status=integer(data); packed=int.from_bytes(exact(data,8),'big')
                def signed(v,bits): return v-(1<<bits) if v&(1<<(bits-1)) else v
                target=[signed(packed>>38,26),signed(packed&4095,12),signed((packed>>12)&0x3ffffff,26)]
                face=struct.unpack('>b',exact(data,1))[0];sequence=integer(data);require_end(data,name)
                row.update(status=status,target=target,face=face,sequence=sequence,payload_hex=payload.hex(),
                           pose=list(self.pose),tick_end_count=self.counts.get('toServer/play/tick_end',0))
            elif name in {'player_loaded','tick_end'}:
                require_end(data,name)
                if name=='player_loaded':self.loaded=True
            else:
                if len(payload)>32: raise ValueError('unexpected large authored client action')
                row['payload_hex']=payload.hex()
        else:
            if name=='position':
                teleport=integer(data); values=struct.unpack('>ddddddffI',exact(data,60));require_end(data,name)
                if not all(math.isfinite(v) for v in values[:-1]): raise ValueError('nonfinite TP')
                self.teleports.add(teleport);row.update(teleport=teleport,position=list(values))
            elif name in {'block_change','multi_block_change'}:
                updates=[{'position':p,'state':s} for p,s in changes(name,payload) if tuple(p)==TARGET]
                if not updates:return
                row['updates']=updates
            elif name=='acknowledge_player_digging':
                row['sequence']=integer(data);require_end(data,name)
            elif name=='block_break_animation':
                row['entity']=integer(data)
                packed=exact(data,8); stage=struct.unpack('>b',exact(data,1))[0];require_end(data,name)
                # Используем свой block-position decoder, фиктивный state0 только локально.
                if tuple(changes('block_change',packed+b'\0')[0][0])!=TARGET:return
                row['destroy_stage']=stage
            elif name=='spawn_entity':
                entity=integer(data);exact(data,16);kind=integer(data)
                if kind!=self.item_entity:return
                position=list(struct.unpack('>ddd',exact(data,24)))
                if not all(math.isfinite(v) for v in position):raise ValueError('nonfinite item position')
                if not all(abs(v-e)<=6 for v,e in zip(position,TARGET)):return
                if len(self.items)>=32:raise ValueError('item limit')
                self.items.add(entity);row.update(entity=entity,position=position)
            elif name=='entity_metadata':
                entity=integer(data)
                if entity not in self.items:return
                entity,value=item_metadata(payload);row.update(entity=entity,slot=value)
            elif name=='collect':
                entity=integer(data);collector=integer(data);count=integer(data);require_end(data,name)
                if entity not in self.items:return
                row.update(entity=entity,collector=collector,count=count)
            elif name in {'set_slot','set_player_inventory','window_items','set_cursor_item'}:
                if name=='window_items':
                    window=integer(data);integer(data);count=integer(data)
                    if window!=0 or count!=46:raise ValueError('unexpected inventory')
                    slots=[slot(data) for _ in range(count)];cursor=slot(data)
                    self.mainhand=slots[36];row.update(mainhand=self.mainhand,cursor=cursor)
                elif name=='set_cursor_item':row['cursor']=slot(data)
                else:
                    if name=='set_slot':
                        window=integer(data);integer(data);index=struct.unpack('>h',exact(data,2))[0]
                    else:window=0;index=integer(data);index=index+36 if index<9 else index
                    value=slot(data)
                    if window!=0 or index!=36:return
                    self.mainhand=value;row['mainhand']=value
                require_end(data,name)
            else:
                if len(payload)>16:raise ValueError('large simple server event')
                row['payload_hex']=payload.hex()
        self.add(row)


def ground_ready(rows, after, now):
    good=[]
    for row in rows:
        if row['elapsed']<after or 'motion' not in row:continue
        motion=row['motion'];pose=motion['pose']
        valid=motion['ground'] and all(v is not None and abs(v-e)<.02 for v,e in zip(pose[:3],(.5,65.,.5)))
        if not valid or (good and row['elapsed']-good[-1]>1.5):good=[]
        if valid:good.append(row['elapsed'])
    return len(good)>=3 and good[-1]-good[0]>=2 and 0<=now-good[-1]<=1.5


def case_evidence(rows, start, slab):
    selected=rows[start:]
    starts=[r for r in selected if r['direction']=='toServer' and r['name']=='block_dig' and r['status']==0]
    if not starts:return False
    if any(r['target']!=list(TARGET) for r in starts):raise ValueError('real client targeted another block')
    air=any(any(u['state']==0 for u in r.get('updates',[])) for r in selected)
    drops={r['entity']:r['slot'] for r in selected if r['name']=='entity_metadata' and r.get('slot')}
    if slab and air and drops and any(v['item']!=721 for v in drops.values()):raise ValueError('unexpected slab drop')
    return air and (not slab or sum(v['count'] for v in drops.values() if v['item']==721)==1)


def run_mouse_dig(press, release, wait_air, wait_drop, diagnose, errors):
    """Отпускаем на первом air; диагностика не заменяет исходную ошибку."""
    failure=None
    try:
        press()
        wait_air()
    except BaseException as error:
        failure=error
    try:
        release()
    except BaseException as error:
        if failure is None:failure=error
        else:errors.append({'step':'release-mouse','error':repr(error)})
    if failure is None:
        try:wait_drop()
        except BaseException as error:failure=error
    try:
        diagnose()
    except BaseException as error:
        if failure is None:failure=error
        else:errors.append({'step':'after-state','error':repr(error)})
    if failure is not None:raise failure


def after_state_commands(label):
    if label not in ('stone','slab'):raise ValueError('unexpected diagnostic case')
    return [(name,'execute if block 2 65 0 minecraft:'+block+' run say MCSA_GUI_'+label+'_AFTER_'+name)
            for name,block in [('air','air'),('stone','stone'),
                               ('slab','white_concrete_slab[type=bottom,waterlogged=false]')]]


def finish_run(report, output, game, client, server, peers, listener, workers, ending, previous, counts):
    """Каждый шаг уборки независим; никаких копий миров или packet dumps."""
    errors=report.setdefault('cleanup_errors',[])
    def attempt(label, action, fallback=None):
        try:return action()
        except BaseException as error:
            errors.append({'step':label,'error':repr(error)})
            report['result']='FAIL'
            return fallback
    for signum in previous:
        attempt('ignore-cleanup-signal',lambda signum=signum:signal.signal(signum,signal.SIG_IGN))
    attempt('cancel-alarm',lambda:signal.alarm(0))
    if client:attempt('release-own-mouse',lambda:gui(client,'mouseup','1'))
    ending.set()
    if client:attempt('stop-client',lambda:stop_client_bounded(client))
    for sock in peers:
        def shutdown(sock=sock):
            try:sock.shutdown(socket.SHUT_RDWR)
            except OSError:pass
        attempt('shutdown-peer',shutdown)
        attempt('close-peer',sock.close)
    if listener:attempt('close-listener',listener.close)
    for worker in workers:attempt('join-relay',lambda worker=worker:worker.join(timeout=5))
    if server:report['shutdown']=attempt('stop-server',lambda:stop_server(server))
    for process in (client,server):
        if process is None:continue
        group=process.pid
        if attempt('inspect-group',lambda:group_pids(group),[group]):
            errors.append({'step':'forced-owned-group','group':group});report['result']='FAIL'
            for sig in (signal.SIGTERM,signal.SIGKILL):
                def kill_group(sig=sig,group=group):
                    try:os.killpg(group,sig)
                    except ProcessLookupError:pass
                attempt('kill-owned-group',kill_group)
                deadline=time.monotonic()+2
                while time.monotonic()<deadline:
                    attempt('poll-owned-child',process.poll)
                    if not attempt('inspect-group',lambda:group_pids(group),[group]):break
                    time.sleep(.05)
                if not attempt('inspect-group',lambda:group_pids(group),[group]):break
        attempt('reap-owned-child',lambda process=process:process.wait(timeout=2))
    report.update(server_exit=server.returncode if server else None,
                  client_exit=client.returncode if client else None,counts=counts)
    report['cleanup']={'ports_free':{str(p):attempt('inspect-port',lambda p=p:free_port(p),False) for p in (PORT,RELAY)},
        'workers_joined':attempt('inspect-workers',lambda:all(not w.is_alive() for w in workers),False),
        'owned_groups':{str(p.pid):attempt('inspect-final-group',lambda p=p:group_pids(p.pid),[p.pid]) for p in (client,server) if p}}
    cleanup=report['cleanup']
    if (errors or report.get('error') or report.get('relay_errors') or report.get('server_exit')!=0 or
        not all(cleanup['ports_free'].values()) or not cleanup['workers_joined'] or any(cleanup['owned_groups'].values())):
        report['result']='FAIL'
    latest=game/'logs/latest.log'
    def copy_latest():
        if latest.is_file():
            with latest.open('rb') as source:
                (output/'latest.log').write_bytes(source.read(1024*1024))
    attempt('copy-client-log',copy_latest)
    attempt('write-visual-review',lambda:(output/'visual-review.json').write_text(
        json.dumps({'status':'pending','screenshots':report['screenshots']},indent=2)+'\n'))
    try:
        (output/'result.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')
    finally:
        for signum,handler in previous.items():
            before=len(errors)
            attempt('restore-signal',lambda signum=signum,handler=handler:signal.signal(signum,handler))
            if len(errors)!=before:
                (output/'result.json').write_text(json.dumps(report,indent=2,allow_nan=False)+'\n')


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--execute',action='store_true')
    for name in ('jar','data','client-dir','output'):parser.add_argument('--'+name,type=Path)
    args=parser.parse_args()
    if not args.execute:
        print(json.dumps({'execute':False,'backend':PORT,'relay':RELAY,'checkpoint':'each target PNG needs calibration JSON','scope':'actual official777 stone/singlebottom slab dig'}));return
    if not all((args.jar,args.data,args.client_dir,args.output)):parser.error('explicit verified inputs and fresh output required')
    gui_tools=preflight_gui_tools()
    tables=inputs(args.data)
    if hashlib.sha1(args.jar.read_bytes()).hexdigest()!=JAR_SHA1:raise ValueError('server SHA mismatch')
    if not all(free_port(p) for p in (PORT,RELAY)):raise RuntimeError('test port occupied')
    cpus=sorted(os.sched_getaffinity(0))[:2]
    if len(cpus)!=2:raise RuntimeError('need two available CPUs')
    os.sched_setaffinity(0,cpus)
    output=args.output.resolve();output.mkdir(parents=True,exist_ok=False)
    report={'result':'FAIL','scope':'ordinary official777 stone/singlebottom slab, not core support',
            'gui_tools':gui_tools,
            'packets':[],'commands':[],'screenshots':[],'checks':[],'cpu_affinity':cpus,
            'runner_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            'jar_sha1':JAR_SHA1,'sources':{k:hashlib.sha256((args.data/k).read_bytes()).hexdigest() for k in tables if k.endswith('.json')}}
    observer=Observer777(tables['protocol.json'],report['packets'],next(v['id'] for v in tables['entities.json'] if v['name']=='item'))
    tool=next(v['id'] for v in tables['items.json'] if v['name']=='diamond_pickaxe')
    lock=threading.RLock();ending=threading.Event();peers=[];workers=[];server=client=listener=None
    previous={}
    with tempfile.TemporaryDirectory(prefix='mcsa-actual777-dig-') as folder:
        root=Path(folder);core=root/'server';game=root/'client';core.mkdir();game.mkdir()
        (core/'eula.txt').write_text('eula=true\n');(core/'server.properties').write_text(server_properties(PORT))
        log=output/'server.log'
        with log.open('w') as slog,(output/'client.log').open('w') as clog:
            def wait(predicate,seconds=30):
                deadline=time.monotonic()+seconds
                while time.monotonic()<deadline:
                    with lock:
                        if report.get('relay_errors'):raise RuntimeError(report['relay_errors'])
                        if any(path.is_file() and path.stat().st_size>1024*1024 for path in (log,output/'client.log')):
                            raise RuntimeError('bounded log limit')
                        if server.poll() is not None or (client and client.poll() is not None):raise RuntimeError('owned process exited')
                        latest=game/'logs/latest.log'
                        if latest.is_file():
                            with latest.open('rb') as source:diagnostic=source.read(1024*1024).lower()
                            if any(marker in diagnostic for marker in (b'failed to handle packet',b'failed to load chunk',b'reported exception thrown',b'crash report')):
                                raise RuntimeError('client diagnostic failure; see retained log')
                        if predicate():return
                    time.sleep(.05)
                raise TimeoutError('evidence deadline')
            def command(text):
                report['commands'].append({'text':text,'elapsed':time.monotonic()-observer.began})
                server.stdin.write(text+'\n');server.stdin.flush()
            marker=0
            def barrier(text):
                nonlocal marker
                marker+=1;token=f'MCSA_GUI_DIG_{marker:03d}_DONE'
                command(text);command('say '+token)
                wait(lambda:('[Server] '+token) in log.read_text(),8)
            def relay(source,target,direction):
                try:
                    while not ending.is_set():
                        body=frame(FrameDeadline(source,30))
                        with lock:observer.observe(direction,body)
                        send(target,body)
                except BaseException as error:
                    if not ending.is_set():
                        with lock:report.setdefault('relay_errors',[]).append(direction+': '+repr(error))
            def snapshot(label):
                path=output/(label+'.png');sha=capture(client,path)
                report['screenshots'].append({'file':path.name,'sha256':sha,'visual_review':'pending'})
                return sha
            def interrupted(signum,_frame):raise InterruptedError('bounded runner interrupted '+str(signum))
            for signum in (signal.SIGINT,signal.SIGTERM,signal.SIGALRM):previous[signum]=signal.signal(signum,interrupted)
            signal.alarm(600)
            try:
                server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(args.jar.resolve()),'nogui'],cwd=core,stdin=subprocess.PIPE,stdout=slog,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                wait(lambda:'Done (' in log.read_text(),120)
                for text in ['gamerule minecraft:spawn_mobs false','gamerule minecraft:random_tick_speed 0',
                             'forceload add -24 -24 23 23','fill -4 64 -4 5 64 4 stone','fill -4 65 -4 5 68 4 air']:barrier(text)
                listener=socket.socket();listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
                listener.bind(('127.0.0.1',RELAY));listener.listen(1);listener.settimeout(120)
                client=launch(args.client_dir.resolve(),game,RELAY,clog,version='26.3',gui_scale=2,pause_on_lost_focus=False)
                peer,_=listener.accept();peers.append(peer)
                backend=socket.create_connection(('127.0.0.1',PORT),timeout=10);peers.append(backend)
                body=frame(FrameDeadline(peer,30));data=io.BytesIO(body)
                if integer(data)!=0 or integer(data)!=777:raise ValueError('expected777 handshake')
                read_string(data);exact(data,2)
                if integer(data)!=2:raise ValueError('expected Login intention')
                require_end(data,'Handshake');send(backend,body)
                body=frame(FrameDeadline(peer,30));data=io.BytesIO(body)
                if integer(data)!=0 or read_string(data)!='McsaProbe':raise ValueError('unexpected LoginStart')
                exact(data,16);require_end(data,'LoginStart');send(backend,body)
                for source,target,direction in ((backend,peer,'toClient'),(peer,backend,'toServer')):
                    worker=threading.Thread(target=relay,args=(source,target,direction),daemon=True);workers.append(worker);worker.start()
                wait(lambda:observer.loaded and observer.acks and observer.phase['toClient']=='play',120)
                for label,block,pitch in [('stone','stone',30.),('slab','white_concrete_slab[type=bottom,waterlogged=false]',35.)]:
                    observer.stage=label+'-setup'
                    for text in ['kill @e[type=minecraft:item]','clear McsaProbe','gamemode survival McsaProbe',
                                 'item replace entity McsaProbe weapon.mainhand with minecraft:diamond_pickaxe',
                                 'setblock 2 65 0 minecraft:air','setblock 2 65 0 minecraft:'+block,
                                 f'tp McsaProbe 0.5 65 0.5 -90 {pitch}']:barrier(text)
                    ready_after=time.monotonic()-observer.began
                    wait(lambda:ground_ready(report['packets'],ready_after,time.monotonic()-observer.began) and
                         observer.mainhand is not None and observer.mainhand['item']==tool and
                         all(v is not None and abs(v-e)<.1 for v,e in zip(observer.pose[3:],(-90.,pitch))) and
                         observer.acks==observer.teleports,20)
                    barrier('execute if block 2 65 0 minecraft:'+block+' run say MCSA_GUI_'+label+'_BEFORE')
                    wait(lambda:('[Server] MCSA_GUI_'+label+'_BEFORE') in log.read_text(),8)
                    barrier('execute if entity @a[name=McsaProbe,gamemode=survival] run say MCSA_GUI_'+label+'_SURVIVAL')
                    wait(lambda:('[Server] MCSA_GUI_'+label+'_SURVIVAL') in log.read_text(),8)
                    for field in ('Pos','Rotation','OnGround','SelectedItem'):barrier('data get entity McsaProbe '+field)
                    barrier('time query gametime')
                    sha=snapshot(label+'-before')
                    calibration_path=output/('calibration-'+label+'.json')
                    (output/'checkpoint.json').write_text(json.dumps({'case':label,'waiting_for':calibration_path.name,
                        'screenshot':label+'-before.png','screenshot_sha256':sha,'target':list(TARGET),
                        'nominal_camera':{'yaw':-90,'pitch':pitch},'mouse_not_pressed':True},indent=2)+'\n')
                    wait(calibration_path.is_file,120)
                    bounded_calibration(calibration_path.read_text(),sha,label)
                    wait(lambda:ground_ready(report['packets'],ready_after,time.monotonic()-observer.began) and
                         all(v is not None and abs(v-e)<.1 for v,e in zip(observer.pose[3:],(-90.,pitch))) and
                         observer.acks==observer.teleports,5)
                    report.setdefault('calibration',[]).append({'case':label,'screenshot_sha256':sha,
                        'verified':True,'observed_pose':list(observer.pose),'all_teleports_acked':True})
                    report['checks'].append(label+' screenshot approved before real mousehold')
                    observer.stage=label+'-dig';start=len(report['packets'])
                    def diagnose_after():
                        # Свой console-only barrier не зависит от сломанного relay.
                        entry={'case':label,'predicates':{},'complete':False}
                        report.setdefault('after_state',[]).append(entry)
                        token='MCSA_GUI_'+label+'_AFTER_DONE'
                        try:
                            for _,text in after_state_commands(label):command(text)
                            command('say '+token)
                            deadline=time.monotonic()+6
                            while ('[Server] '+token) not in log.read_text():
                                if time.monotonic()>=deadline or server.poll() is not None:
                                    raise TimeoutError('bounded after-state console barrier')
                                time.sleep(.05)
                            entry['complete']=True
                        except BaseException as error:
                            entry['error']=repr(error)
                            raise
                        finally:
                            text=log.read_text()
                            entry['predicates']={name:('[Server] MCSA_GUI_'+label+'_AFTER_'+name) in text
                                                 for name,_ in after_state_commands(label)}
                    run_mouse_dig(lambda:gui(client,'mousedown','1'),
                        lambda:gui(client,'mouseup','1'),
                        lambda:wait(lambda:case_evidence(report['packets'],start,False),10),
                        lambda:wait(lambda:case_evidence(report['packets'],start,label=='slab'),3),
                        diagnose_after,report.setdefault('dig_diagnostic_errors',[]))
                    barrier('execute if block 2 65 0 minecraft:air run say MCSA_GUI_'+label+'_AIR')
                    wait(lambda:('[Server] MCSA_GUI_'+label+'_AIR') in log.read_text(),8)
                    barrier('time query gametime')
                    post_after=time.monotonic()-observer.began
                    wait(lambda:ground_ready(report['packets'],post_after,time.monotonic()-observer.began),10)
                    snapshot(label+'-after')
                    report['checks'].append(label+' real START + authoritative air + exact console air'+(' + slab721count1' if label=='slab' else ''))
                report['result']='PASS-packet-evidence-visual-review-pending'
            except BaseException as error:report['error']=repr(error)
            finally:
                finish_run(report, output, game, client, server, peers, listener,
                           workers, ending, previous, observer.counts)
    if report['result']!='PASS-packet-evidence-visual-review-pending':raise SystemExit(1)
    print(report['result'])


if __name__=='__main__':main()
