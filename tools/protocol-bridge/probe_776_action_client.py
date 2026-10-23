#!/usr/bin/env python3
"""Смысл enum из обычных GUI-действий непрозрачного клиента776, без чтения JAR."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import socket
import shutil
import struct
import subprocess
import tempfile
import threading
import time

from live_client import launch
from observe import frame
from probe_core_776 import PROTOCOL_SHA, stop_server
from probe_core_776_client import FrameDeadline, gui, stop_client_bounded
from probe_block_entities import BLOCKS
from generate_tables import states
from probe_white_concrete_slab import pack_position
from probe_gameplay import mapping
from probe_776_payloads import table
from probe_actual_777_use_actions import use_slot
from probe_initial_play_776 import capture
from probe_registry_client import integer
from smoke import send


def main():
    p = argparse.ArgumentParser(description=__doc__)
    for name in ('test-binary', 'protocol', 'client-dir', 'output'):
        p.add_argument('--' + name, type=Path, required=True)
    p.add_argument('--primary', type=int, choices=(775, 777), required=True)
    p.add_argument('--collect-only', action='store_true')
    p.add_argument('--block-entities', action='store_true')
    p.add_argument('--payloads',type=Path)
    p.add_argument('--client',type=int,choices=(771,772,773,774,776),default=776)
    a = p.parse_args()
    assert hashlib.sha256(a.protocol.read_bytes()).hexdigest() == (PROTOCOL_SHA if a.client==776 else '11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be' if a.client==771 else '11fb521446a1e1780bfa140b074436551738e1bfb867e4855784988612e2a0be' if a.client==772 else '0f34fce39ca160cf9856013befd52bdc67cf30687306f47dacd007693d5a2d70' if a.client in (771,772,773) else '4108833ded28ee21786e4905d0a07b3a06164a3842a8d6bab98096fd3c2e28ab')
    protocol = json.loads(a.protocol.read_text())
    items={r['name']:r['id'] for r in table(a.protocol.parent if a.client in (771,772,773,774) else a.protocol.resolve().parents[4],'items')}
    arrow=next(r['id'] for r in table(a.protocol.parent if a.client in (771,772,773,774) else a.protocol.resolve().parents[4],'entities') if r['name']=='arrow')
    hands=dict(mainhand=None,offhand=None)
    statue_states={number:key for key,number in states(table(a.protocol.parent if a.client in (771,772,773,774) else a.protocol.resolve().parents[4],'blocks')).items() if key[0]=='copper_golem_statue'}
    if a.client in (771,772):
        poses={'cut_copper':'standing','exposed_cut_copper':'sitting','weathered_cut_copper':'running','oxidized_cut_copper':'star'}
        statue_states={number:('copper_golem_statue',(('copper_golem_pose',poses[key[0]]),)) for key,number in states(table(a.protocol.parent,'blocks')).items() if key[0] in poses}
    names = {phase: {d: {v:k for k,v in mapping(protocol, phase, d).items()}
                    for d in ('toClient','toServer')} for phase in ('configuration','play')}
    port, relay_port = (26128,26134) if a.client==771 else (26028,26034)
    for number in (port, relay_port):
        with socket.socket() as check:
            check.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
            check.bind(('127.0.0.1', number))
    a.output.mkdir(parents=True, exist_ok=False)
    report = dict(status='failed', primary=a.primary, protocol=a.client, collect_only=a.collect_only,
                  runner_sha256=hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                  binary_sha256=hashlib.sha256(a.test_binary.read_bytes()).hexdigest(),
                  packets=[], commands=[], checks=[], screenshots=[])
    phases = dict.fromkeys(('toClient','toServer'), 'login')
    lock, ending = threading.RLock(), threading.Event()
    send_lock=threading.Lock()
    peers, workers = [], []
    stage = 'login'
    server = client = listener = None
    with tempfile.TemporaryDirectory(prefix='mcsa-776-actions-client-') as folder:
        root = Path(folder); core=root/'core'; game=root/'client'; core.mkdir(); game.mkdir()
        (core/'mcsa-gameplay-probe.marker').touch()
        if a.primary == 777:
            (core/'mcsa-catalog-preview263.marker').touch()
        with (a.output/'server.log').open('w') as slog, (a.output/'client.log').open('w') as clog:
            def wait(predicate, seconds=30):
                deadline=time.monotonic()+seconds
                while time.monotonic()<deadline:
                    if report.get('relay_errors'):
                        raise RuntimeError(report['relay_errors'])
                    assert server.poll() is None, 'core exited'
                    if predicate(): return
                    time.sleep(.05)
                raise TimeoutError(stage)

            def command(text):
                report['commands'].append(text)
                token='MCSA_ACTION_BARRIER_'+str(len(report['commands']))
                server.stdin.write(text+'\nsay '+token+'\n');server.stdin.flush()
                wait(lambda:token in (a.output/'server.log').read_text())
                time.sleep(.3)

            def relay(source, target, direction):
                try:
                    while not ending.is_set():
                        body=frame(FrameDeadline(source, 45)); data=io.BytesIO(body)
                        ident=integer(data); payload=data.read()
                        filtered=False
                        with lock:
                            phase=phases[direction]
                            name=names[phase][direction].get(ident) if phase!='login' else 'login'
                            if phase=='login' and direction=='toServer' and ident==3:
                                phases.update(dict.fromkeys(phases,'configuration'))
                            elif phase=='configuration' and direction=='toServer' and name=='finish_configuration':
                                phases.update(dict.fromkeys(phases,'play'))
                            if phase=='play' and (name in {'block_dig','use_item','attack','use_entity','block_change',
                                'window_items','set_slot','set_player_inventory','entity_metadata','spawn_entity',
                                'position','player_loaded','system_chat','pong','entity_velocity','block_place'}):
                                row=dict(direction=direction,name=name,stage=stage,hex=payload.hex())
                                if direction=='toClient' and name in {'set_slot','set_player_inventory','window_items'}:
                                    value=io.BytesIO(payload)
                                    if name=='window_items':
                                        window=integer(value);integer(value);count=integer(value)
                                        entries=[use_slot(value) for _ in range(count)]
                                        use_slot(value)
                                        if window==0 and count==46:hands.update(mainhand=entries[36],offhand=entries[45])
                                    else:
                                        if name=='set_slot':
                                            window=integer(value);integer(value);index=struct.unpack('>h',value.read(2))[0]
                                        else:
                                            window=0;index=integer(value);index=36 if index==0 else 45 if index==40 else -1
                                        item=use_slot(value)
                                        if window==0 and index in (36,45):hands['mainhand' if index==36 else 'offhand']=item
                                    row['authoritative_hands']=dict(hands)
                                if name=='block_dig':
                                    row['status']=integer(io.BytesIO(payload))
                                    filtered=a.collect_only and row['status'] not in (0,2)
                                    row['filtered']=filtered
                                report['packets'].append(row)
                        if not filtered:
                            with send_lock:send(target,body)
                except BaseException as error:
                    if not ending.is_set(): report.setdefault('relay_errors',[]).append(direction+': '+repr(error))

            def actions(label):
                return [r['status'] for r in report['packets'] if r['stage']==label and r['name']=='block_dig' and r['direction']=='toServer']

            def shot(label):
                path=a.output/(label+'.png'); sha=capture(client,path)
                report['screenshots'].append(dict(file=path.name,sha256=sha))

            try:
                test='serve_java_1_21_6_probe' if a.client==771 else 'serve_java_1_21_8_probe' if a.client==772 else 'serve_java_1_21_9_probe' if a.client in (771,772,773) else 'serve_java_1_21_11_probe' if a.client in (771,772,773,774) else ('serve_java_26_2_primary_26_3_probe' if a.primary==777 else 'serve_java_26_2_probe')
                server=subprocess.Popen([str(a.test_binary.resolve()),'--ignored','--exact','gameplay_probe::'+test,'--nocapture'],
                    cwd=core,stdin=subprocess.PIPE,stdout=slog,stderr=subprocess.STDOUT,text=True,start_new_session=True)
                wait(lambda:'MCSA gameplay probe ready' in (a.output/'server.log').read_text(),120)
                if a.block_entities:
                    command('forceload add -16 -16')
                    command('fill -16 63 -16 -1 63 -1 stone')
                    command('fill -16 65 -16 -1 65 -1 stone')
                    for i,block in enumerate(BLOCKS):
                        if block=='piston':continue
                        suffix='[attached=true]' if block=='oak_hanging_sign' else ''
                        if block=='oak_sign':suffix='{front_text:{messages:["MCSA probe","","",""]}}'
                        command(f'setblock {-16+i%16} 64 {-16+i//16} '+block+suffix)
                    command('setblock -16 64 -13 copper_golem_statue')
                    report['authored_blocks']=[b for b in BLOCKS if b!='piston']+['copper_golem_statue']
                listener=socket.socket();listener.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1)
                listener.bind(('127.0.0.1',relay_port));listener.listen(1);listener.settimeout(120)
                client=launch(a.client_dir.resolve(),game,relay_port,clog,version='1.21.6' if a.client==771 else '1.21.8' if a.client==772 else '1.21.10' if a.client==773 else '1.21.11' if a.client==774 else '26.2',gui_scale=2,pause_on_lost_focus=False)
                peer,_=listener.accept(); peers.append(peer)
                backend=socket.create_connection(('127.0.0.1',port),timeout=10);peers.append(backend)
                for _ in range(2): send(backend,frame(FrameDeadline(peer,30)))
                for source,target,direction in ((peer,backend,'toServer'),(backend,peer,'toClient')):
                    worker=threading.Thread(target=relay,args=(source,target,direction),daemon=True)
                    workers.append(worker);worker.start()
                wait(lambda:any(r['name']=='player_loaded' for r in report['packets']),120)
                time.sleep(3)
                if a.payloads:
                    assert a.block_entities
                    payloads=json.loads(a.payloads.read_text());assert payloads['protocol']==a.client
                    report['payload_source_sha256']=hashlib.sha256(a.payloads.read_bytes()).hexdigest()
                    stage='payload-injection'
                    report['payloads']=[]
                    ping=mapping(protocol,'play','toClient')['ping']
                    for index,row in enumerate(payloads['packets']):
                        body=bytes.fromhex(row['hex']);token=0x4d430000+index
                        with send_lock:
                            send(peer,body)
                            send(peer,bytes([ping])+struct.pack('>i',token))
                        recorded=dict(label=row['label'],hex=row['hex'],post_pong=False)
                        report['payloads'].append(recorded)
                        wait(lambda:any(r['name']=='pong' and r['hex']==struct.pack('>i',token).hex() for r in report['packets']))
                        recorded['post_pong']=True
                    report['payload_post_pong']=True
                    time.sleep(2)
                    report['payload_post_pong']=True
                    assert 'Failed to handle packet' not in (a.output/'client.log').read_text()
                    shot('payloads-accepted')
                if a.block_entities:
                    stage='statue-setup'
                    command('gamemode survival McsaProbe')
                    command('fill -20 63 -16 -17 63 -10 stone')
                    command('item replace entity McsaProbe weapon.mainhand with stone')
                    command('setblock -16 65 -13 air')
                    command('tp McsaProbe -17.5 64 -12.5 -90 30')
                    time.sleep(.5)
                    shot('statue-ready')
                    report['statue_poses']=[]
                    for pose in ('sitting','running','star','standing'):
                        stage='statue-'+pose
                        gui(client,'click','3')
                        def pose_seen():
                            for row in report['packets']:
                                if row['name']=='block_change' and row['direction']=='toClient' and row['stage']==stage:
                                    raw=bytes.fromhex(row['hex'])
                                    if raw[:8]!=pack_position((-16,64,-13)):continue
                                    key=statue_states.get(integer(io.BytesIO(raw[8:])))
                                    if key and dict(key[1]).get('copper_golem_pose')==pose:return True
                            return False
                        wait(pose_seen)
                        report['statue_poses'].append(pose)
                        time.sleep(.7)
                        shot('statue-'+pose)
                command('gamemode survival McsaProbe')
                command('fill -3 63 -3 4 63 3 stone')
                command('tp McsaProbe 0.5 64 0.5 -90 30')
                command('setblock 2 64 0 obsidian')
                command('item replace entity McsaProbe weapon.mainhand with iron_pickaxe')
                shot('ready-cancel')
                stage='cancel';gui(client,'mousedown','1');time.sleep(.25);gui(client,'mouseup','1')
                wait(lambda:bool(actions(stage)));time.sleep(.5)
                command('execute if block 2 64 0 obsidian run say MCSA_CANCEL_RETAINED')
                assert 'MCSA_CANCEL_RETAINED' in (a.output/'server.log').read_text()
                report['checks'].append(dict(case=stage,statuses=actions(stage),block_retained=True))
                command('setblock 2 64 0 air')
                for label,item,control in [('drop-one','stone 7',('key','q')),('drop-stack','cobblestone 11',('key','ctrl+q')),
                                           ('swap','iron_ingot 3',('key','f'))]:
                    stage=label+'-setup'
                    command('kill @e[type=item]')
                    command('item replace entity McsaProbe weapon.mainhand with '+item)
                    command('item replace entity McsaProbe weapon.offhand with gold_ingot 5')
                    stage=label
                    if label=='drop-stack':
                        gui(client,'keydown','Control_L');time.sleep(.15);gui(client,'key','q')
                        wait(lambda:bool(actions(stage)));gui(client,'keyup','Control_L')
                    else:
                        gui(client,*control);wait(lambda:bool(actions(stage)))
                    time.sleep(.4)
                    if not a.collect_only:
                        expected_hands={
                            'drop-one':dict(mainhand=dict(item=items['stone'],count=6),offhand=dict(item=items['gold_ingot'],count=5)),
                            'drop-stack':dict(mainhand=None,offhand=dict(item=items['gold_ingot'],count=5)),
                            'swap':dict(mainhand=dict(item=items['gold_ingot'],count=5),offhand=dict(item=items['iron_ingot'],count=3)),
                        }[label]
                        wait(lambda:hands==expected_hands)
                    report['checks'].append(dict(case=stage,statuses=actions(stage),authoritative_hands=dict(hands)))
                stage='release-setup'
                command('item replace entity McsaProbe weapon.mainhand with bow')
                command('give McsaProbe arrow 8')
                stage='release';gui(client,'mousedown','3');time.sleep(1.2);gui(client,'mouseup','3')
                wait(lambda:bool(actions(stage)))
                if not a.collect_only:
                    def arrow_spawned():
                        for row in report['packets']:
                            if row['stage']=='release' and row['direction']=='toClient' and row['name']=='spawn_entity':
                                value=io.BytesIO(bytes.fromhex(row['hex']));integer(value);value.read(16)
                                if integer(value)==arrow:return True
                        return False
                    wait(arrow_spawned)
                report['checks'].append(dict(case=stage,statuses=actions(stage),arrow_spawn=not a.collect_only))
                stage='jab-setup'
                if a.client in (771,772,773):
                    command('item replace entity McsaProbe weapon.mainhand with iron_sword')
                    command('kill @e[type=cow]')
                    command('summon cow 2.5 64 0.5 {NoAI:1b}')
                else:
                    command('item replace entity McsaProbe weapon.mainhand with iron_spear')
                if a.client not in (771,772,773):command('enchant McsaProbe lunge 1')
                time.sleep(1)
                stage='jab';gui(client,'click','1')
                if a.client in (771,772,773):
                    wait(lambda:any(r['stage']=='jab' and r['name']=='use_entity' and r['direction']=='toServer' for r in report['packets']))
                    report['attack_equivalent']=f'{a.client} iron sword: ordinary use_entity attack, authoritative weapon wear and target health'
                    command('data get entity @e[type=cow,limit=1] Health')
                else:wait(lambda:bool(actions(stage)))
                if not a.collect_only:wait(lambda:hands['mainhand'] is not None and hands['mainhand'].get('damage')==1)
                report['checks'].append(dict(case=stage,statuses=actions(stage),authoritative_hands=dict(hands)))
                shot('finished-actions')
                expected={'cancel':[0,1],'drop-one':[4],'drop-stack':[3],'swap':[6],'release':[5],'jab':[7]}
                if a.client in (771,772,773):
                    expected['jab']=[]
                    raw=[r for r in report['packets'] if r['stage']=='jab' and r['name']=='entity_metadata' and r['direction']=='toClient']
                    health=[]
                    for row in raw:
                        data=io.BytesIO(bytes.fromhex(row['hex']));integer(data)
                        tail=data.read()
                        if tail[:2]==bytes([9,3]):health.append(struct.unpack('>f',tail[2:6])[0])
                    assert health and any(0<h<10 for h in health),health
                    report['checks'][-1]['target_health']=health
                assert {v['case']:v['statuses'] for v in report['checks']}==expected,report['checks']
                report['status']='pass'
            except BaseException as error:
                report['error']=repr(error)
                raise
            finally:
                for subdir in ('crash-reports','debug'):
                    if (game/subdir).is_dir():shutil.copytree(game/subdir,a.output/('client-'+subdir))
                ending.set()
                if client: stop_client_bounded(client);report['client_exit']=client.returncode
                for sock in peers:
                    try:sock.shutdown(socket.SHUT_RDWR)
                    except OSError:pass
                    sock.close()
                if listener:listener.close()
                for worker in workers:worker.join(timeout=2)
                if server:report['shutdown']=stop_server(server);report['server_exit']=server.returncode
                (a.output/'result.json').write_text(json.dumps(report,indent=2)+'\n')
    print(json.dumps(dict(status=report['status'],checks=report['checks'])))


if __name__=='__main__':
    main()
