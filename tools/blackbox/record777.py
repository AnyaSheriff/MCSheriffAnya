#!/usr/bin/env python3
"""Сценарии blackbox через собственный клиент777; JAR остаётся непрозрачным.

Обе стороны используют один клиент. Сырые пакеты сохраняются для анализа;
неподдержанные действия завершают сценарий явной ошибкой.
"""
import argparse
import collections
import hashlib
import io
import json
import math
from pathlib import Path
import re
import select
import shutil
import socket
import struct
import subprocess
import sys
import threading
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / 'tools/protocol-bridge'))
sys.path.insert(0, str(ROOT / 'tools/measure'))
from probe_gameplay import Client, connect
from probe_white_concrete_slab import pack_position, inputs, JAR_SHA1
from smoke import read_varint, varint
class Rcon:
    def __init__(self, port, password):
        self.sock=socket.create_connection(('127.0.0.1',port),timeout=10)
        self.number=0
        self.call(3,password)
    def receive(self,size):
        data=b''
        while len(data)<size:
            part=self.sock.recv(size-len(data))
            if not part:raise ConnectionError('RCON closed')
            data+=part
        return data
    def call(self,kind,text):
        self.number+=1;body=text.encode()+b'\0\0'
        self.sock.sendall(struct.pack('<iii',len(body)+8,self.number,kind)+body)
        data=self.receive(struct.unpack('<i',self.receive(4))[0])
        assert struct.unpack('<i',data[:4])[0]==self.number, 'RCON request failed'
        return data[8:-2].decode('utf8','replace')
    def run(self,text):return self.call(2,text)



def velocity(data):
    first = data.read(1)[0]
    if not first:
        return [0., 0., 0.]
    packed = first | (data.read(1)[0] << 8) | (int.from_bytes(data.read(4), 'big') << 16)
    scale = (first & 3) + (read_varint(data.read) << 2 if first & 4 else 0)
    return [(((packed >> (3 + 15*i)) & 32767) / 32766 * 2 - 1) * scale for i in range(3)]


class Recorder(Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1)
        self.entity = None
        self.events = []
        self.entities = []
        self.age_samples = []
        self.calibration = []
        self.calibrating = False
        self.motion = [0., 0., 0.]
        self.floor = -59.
        self.flying = True
        self.controls = {}
        self.sequence = 0
        self.recording = False
        self.failure = None
        self.running = True
        self.lock = threading.RLock()

    def send(self, name, payload=b''):
        with self.lock:
            if name == 'tick_end' and self.position:
                x,y,z,yaw,pitch = self.position
                grounded = y <= self.floor + 1e-7
                if not self.flying:
                    # Сухая ровная площадка; прочие коллизии явно вне покрытия.
                    if self.controls.get('forward'):
                        a = math.radians(yaw)
                        self.motion[0] += -math.sin(a) * .1
                        self.motion[2] += math.cos(a) * .1
                    if self.controls.get('jump') and grounded:
                        self.motion[1] = .42
                    x += self.motion[0]; y += self.motion[1]; z += self.motion[2]
                    if y <= self.floor:
                        y = self.floor; self.motion[1] = 0.; grounded = True
                    else:
                        grounded = False
                    self.motion[1] = (self.motion[1] - .08) * .98
                    drag = .546 if grounded else .91
                    self.motion[0] *= drag; self.motion[2] *= drag
                    self.position = [x,y,z,yaw,pitch]
                super().send('position_look', struct.pack('>dddffB', x,y,z,yaw,pitch,int(grounded)))
            super().send(name, payload)

    def confirm_teleport(self, teleport, values):
        self.motion = [0., 0., 0.]
        super().confirm_teleport(teleport, values)

    def observe_packet(self, name, payload):
        data = io.BytesIO(payload)
        if name == 'login':
            self.entity = struct.unpack('>i', data.read(4))[0]
        elif name == 'respawn':
            self.send('player_loaded')
        elif name == 'update_health' and struct.unpack('>f', payload[:4])[0] <= 0:
            self.send('client_command', b'\0')
        elif name == 'update_time':
            age = struct.unpack('>q', payload[:8])[0]
            self.age_samples.append((age, time.monotonic()))
        elif name == 'spawn_entity':
            entity = read_varint(data.read); data.read(16); kind = read_varint(data.read)
            self.entities.append((entity, kind))
        elif name == 'entity_velocity':
            entity = read_varint(data.read)
            if entity == self.entity:
                self.motion = velocity(data)
        if self.calibrating and name in ('block_change','multi_block_change'):
            self.calibration.append(time.monotonic())
        if self.recording:
            self.events.append({'at':time.monotonic(), 'name':name, 'hex':payload.hex()})

    def start(self):
        def run():
            try:
                while self.running:
                    self.pump(.05)
            except Exception as error:
                if self.running:
                    self.failure = repr(error)
        self.thread = threading.Thread(target=run, daemon=True)
        self.thread.start()

    def close(self):
        self.running = False
        self.sock.close()
        self.thread.join(2)


def expand(text, origin):
    text = text.replace('$(bot2)', 'blackboxb').replace('$(bot)', 'blackbox')
    return re.sub(r'\$\((-?[\d.]+),(-?[\d.]+),(-?[\d.]+)\)',
                  lambda m:' '.join(format(float(m[i+1])+origin[i], '.10g') for i in range(3)), text)


def special(text, clients):
    parts = text.split(); op = parts[0]
    if op == '!wait':
        time.sleep(float(parts[1])*.05); return
    who = 1 if op in ('!hotbar2','!look2','!sprint2','!nosprint2') else 0
    c = clients[who]
    if op.startswith('!hotbar'):
        c.send('held_item_slot', struct.pack('>h', int(parts[1])))
    elif op in ('!stopfly','!startfly'):
        c.flying = op == '!startfly'; c.send('abilities', bytes([2 if c.flying else 0]))
    elif op in ('!look','!look2'):
        c.position[3:] = list(map(float,parts[1:]))
    elif op == '!respawn':
        c.send('client_command', b'\0')
    else:
        raise ValueError('unsupported setup '+text)
    time.sleep(.1)


def validate(scenario):
    supported = {'after','cmd','bot','attack','which','target','use','release','hand','hotbar','control','look','fly','set','click','face','cursor','dig','status','respawn'}
    for a in scenario['actions']:
        if set(a)-supported:
            raise ValueError('unsupported actions: '+str(sorted(set(a)-supported)))
    for c in scenario.get('setup', []) + scenario.get('cleanup', []):
        if c.startswith('!') and c.split()[0] not in ('!wait','!hotbar','!hotbar2','!look','!look2','!stopfly','!startfly','!respawn'):
            raise ValueError('unsupported setup '+c)


def record(port, protocol, s, rcon, output):
    clients=[]; commands=[]
    origin=s.get('origin',[100,-59,100])
    def command(text, delay=True):
        text=expand(text,origin)
        encoded=text.encode()
        clients[0].send('chat_command',varint(len(encoded))+encoded)
        result='777 chat_command; responses retained in packet trace'
        commands.append({'command':text,'response':result})
        if delay: time.sleep(.1)
    try:
        for username in ['blackbox','blackboxb'][:2 if s.get('second') else 1]:
            c=connect(port,protocol,Recorder,username=username)
            clients.append(c); c.pump(15,lambda row:row['name']=='position'); c.pump(6); c.start()
        command('gamerule spawn_mobs false'); command('gamerule natural_health_regeneration false')
        command('difficulty peaceful'); command('kill @e[type=!player]')
        command(f'forceload add {origin[0]-8} {origin[2]-8} {origin[0]+8} {origin[2]+8}')
        for i,c in enumerate(clients):
            name='blackboxb' if i else 'blackbox'
            command(f'gamemode creative {name}')
            command(f'tp {name} $(3.5,0,3.5)')
        time.sleep(3)
        command('fill $(-8,-1,-8) $(8,8,8) air')
        command('fill $(-8,-1,-8) $(8,-1,8) stone')
        for low in range(origin[1]+9, 318, 100):
            command(f'fill {origin[0]-8} {low} {origin[2]-8} {origin[0]+8} {min(low+99,318)} {origin[2]+8} air')
        for i,c in enumerate(clients):
            c.floor=float(origin[1])
            name='blackboxb' if i else 'blackbox'
            for cmd in [f'gamemode creative {name}',f'clear {name}',f'effect clear {name}',f'attribute {name} minecraft:attack_damage base reset',f'tp {name} $(3.5,0,3.5)']:
                command(cmd)
        for block in s['blocks']:
            command('setblock $('+','.join(map(str,block['at']))+') '+block['state'])
        for text in s.get('setup',[]):
            special(text,clients) if text.startswith('!') else command(text)
        # Второй игрок в survival тоже использует ту же физику площадки.
        for c in clients:
            c.flying=False
        time.sleep(6)
        for c in clients:
            if c.failure: raise RuntimeError(c.failure)
            deadline=time.monotonic()+10
            while len(c.age_samples)<2 and time.monotonic()<deadline:time.sleep(.1)
            if len(c.age_samples)<2: raise RuntimeError('not enough server clock samples; packets='+str(collections.Counter(v['name'] for v in c.records)))
        for c in clients: c.calibrating=True
        for i in range(6):
            command('setblock $(8,0,8) '+('stone' if i%2 else 'glass')); time.sleep(.15)
        deadline=time.monotonic()+10
        while len(clients[0].calibration)<6 and time.monotonic()<deadline:time.sleep(.1)
        for c in clients: c.calibrating=False
        # Только поздние samples: первый update_time приходит вне тика.
        bases=[stamp-age*.05 for age,stamp in clients[0].age_samples[1:][-6:]]
        clock=min(bases)
        if len(clients[0].calibration)<6: raise RuntimeError("calibration responses missing")
        phases=[(t-clock)%.05 for t in clients[0].calibration]
        # Центр событий находится на +полтакта от счётной границы.
        phase=min(phases, key=lambda x:sum(min(abs(x-y),.05-abs(x-y)) for y in phases))
        clock += phase-.025
        start_tick=math.ceil((time.monotonic()-clock)/.05)+3
        if s.get('align'): start_tick=math.ceil(start_tick/s['align'])*s['align']
        start=clock+start_tick*.05
        initial=[{'entity':c.entity,'entities':c.entities.copy(),'position':c.position.copy()} for c in clients]
        for c in clients: c.recording=True; c.events=[]
        sent=[]
        for action in s['actions']:
            at=start+action.get('after',0)*.05
            time.sleep(max(0,at-time.monotonic()))
            c=clients[1 if action.get('bot')==2 else 0]
            sent.append({'tick':action.get('after',0),'at':time.monotonic(),'action':action,'pose':c.position.copy()})
            if 'cmd' in action: command(action['cmd'], delay=False)
            if action.get('attack'):
                target=clients[0].entity if action.get('bot')==2 else clients[1].entity if action.get('target')=='second' else c.entities[action.get('which',-1)][0]
                c.send('attack',varint(target)); c.send('arm_animation',b'\0')
            if 'hotbar' in action: c.send('held_item_slot',struct.pack('>h',action['hotbar']))
            if 'control' in action:
                key,value=action['control'];c.controls[key]=value
                if key=='sprint':c.send('entity_action',varint(c.entity)+varint(1 if value else 2)+b'\0')
            if 'look' in action:c.position[3:]=action['look']
            if action.get('use'):
                c.sequence+=1;c.send('use_item',varint(action.get('hand',0))+varint(c.sequence)+struct.pack('>ff',*c.position[3:]))
            if action.get('release'):c.send('block_dig',b'\x06'+bytes(8)+b'\0\0')
            if 'fly' in action:c.flying=action['fly'];c.send('abilities',bytes([2 if c.flying else 0]))
            if action.get('respawn'):c.send('client_command',b'\0')
            if 'set' in action:
                b=action['set'];command('setblock $('+','.join(map(str,b['at']))+') '+b['state'])
            if 'click' in action:
                pos=[int(origin[i]+action['click'][i]) for i in range(3)];c.sequence+=1
                c.send('block_place',b'\0'+pack_position(pos)+varint(action.get('face',1))+struct.pack('>fff',*action.get('cursor',[.5,.5,.5]))+b'\0\0'+varint(c.sequence))
            if 'dig' in action:
                pos=[int(origin[i]+action['dig'][i]) for i in range(3)];c.sequence+=1
                c.send('block_dig',varint(action.get('status',0))+pack_position(pos)+bytes([action.get('face',1)])+varint(c.sequence))
        time.sleep(max(0,start+(s['record_ticks']+13)*.05-time.monotonic()))
        for c in clients:c.recording=False
        result={'scenario':s['name'],'protocol':777,'port':port,'start':start,'clock':clock,'initial':initial,'sent':sent,'clients':[{'events':c.events,'failure':c.failure} for c in clients],'commands':commands,'recorder_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scope':'raw777 flat stone physics; full client collision/GUI not implemented'}
        output.write_text(json.dumps(result,indent=1)+'\n')
        for text in s.get('cleanup',[]):
            special(text,clients) if text.startswith('!') else command(text)
        if any(c.failure for c in clients):raise RuntimeError(str([c.failure for c in clients]))
        return result
    finally:
        for c in clients:c.close()


class Console:
    def __init__(self, stream): self.sock=self; self.stream=stream
    def run(self, text): self.stream.write(text+"\n");self.stream.flush();return "console: see server.log"
    def close(self): self.stream.close()


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--jar',type=Path,required=True)
    p.add_argument('--core-port',type=int,default=26142)
    p.add_argument('--skip-recorded-core',action='store_true')
    p.add_argument('--resume',action='store_true');p.add_argument('--core-only',action='store_true')
    p.add_argument('scenarios',nargs='*')
    a=p.parse_args();assert a.binary.is_file(), 'build binary first';assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==JAR_SHA1, 'pinned official JAR hash mismatch';tables=inputs(a.data);a.output.mkdir(parents=True,exist_ok=a.resume)
    selected=a.scenarios or sorted({v.stem for pattern in ('pvp-*','cow-crit*','sword-sweep-*','itemaudit4-knockback*','*shield*') for v in (ROOT/'tools/blackbox/scenarios').glob(pattern+'.json')})
    manifest={'protocol':777,'primary':'26.3','binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'jar_sha1':JAR_SHA1,'recorder_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scenario_names':selected,'core_port':a.core_port}
    (a.output/f'run-{time.time_ns()}.json').write_text(json.dumps(manifest,indent=2)+'\n')
    servers=[];summary=[]
    try:
        assert 26100<=a.core_port<=26198 and a.core_port not in (26139,26140,26141)
        labels=[('ours',a.core_port)] if a.core_only else [('official',26140),('ours',a.core_port)]
        for label,port in labels:
            for n in (port,port+1):
                with socket.socket() as check:
                    check.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);check.bind(('127.0.0.1',n))
            folder=a.output/label;folder.mkdir(exist_ok=a.resume);(folder/'config').mkdir(exist_ok=a.resume)
            if label=='official':shutil.copyfile(a.jar,folder/'server.jar')
            else:
                (folder/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
                (folder/'config/skins.toml').write_text('mojang=false\nely=false\n')
            (folder/'eula.txt').write_text('eula=true\n')
            (folder/('server.properties' if label=='official' else 'config/server.properties')).write_text(f'server-ip=127.0.0.1\nserver-port={port}\nenable-rcon=true\nrcon.port={port+1}\nrcon.password=local-blackbox\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\nmax-tick-time=0\n')
            ops=[]
            for name in ('blackbox','blackboxb'):
                value=bytearray(hashlib.md5(('OfflinePlayer:'+name).encode()).digest());value[6]=(value[6]&15)|48;value[8]=(value[8]&63)|128
                ops.append({'uuid':str(uuid.UUID(bytes=bytes(value))),'name':name,'level':4})
            (folder/'ops.json').write_text(json.dumps(ops))
            log=(folder/'server.log').open('a' if a.resume else 'w')
            argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar','server.jar','nogui'] if label=='official' else [str(a.binary.resolve())]
            proc=subprocess.Popen(argv,cwd=folder,stdin=subprocess.DEVNULL if label=='official' else subprocess.PIPE,text=True,stdout=log,stderr=subprocess.STDOUT)
            (folder/'pid').write_text(str(proc.pid))
            servers.append((proc,log,None,port))
            deadline=time.monotonic()+120
            while True:
                if proc.poll() is not None:raise RuntimeError(label+' exited: '+(folder/'server.log').read_text()[-2000:])
                try:
                    if label=='official':rcon=Rcon(port+1,'local-blackbox')
                    else:
                        with socket.create_connection(('127.0.0.1',port),timeout=1):pass
                        rcon=Console(proc.stdin)
                    break
                except OSError:
                    if time.monotonic()>deadline:raise TimeoutError(label+' startup')
                    time.sleep(.2)
            servers[-1]=(proc,log,rcon,port)
        for name in selected:
            row={'scenario':name,'status':'mismatch-open'};summary.append(row)
            try:
                s=json.loads((ROOT/'tools/blackbox/scenarios'/f'{name}.json').read_text());validate(s)
                for (label,_),(_,_,rcon,port) in zip(labels,servers):
                    if a.resume and (label=='official' or a.skip_recorded_core) and (a.output/f'{name}-{label}.json').exists():continue
                    print(label,name,flush=True)
                    record(port,tables['protocol.json'],s,rcon,a.output/f'{name}-{label}.json')
                row['reason']='recorded; strict packet/tick analysis pending'
            except Exception as error:row['reason']=repr(error)
            (a.output/'summary.json').write_text(json.dumps(summary,indent=2)+'\n')
            print(row,flush=True)
    finally:
        for proc,log,rcon,_ in reversed(servers):
            if rcon:rcon.sock.close()
            if proc.poll() is None:
                proc.terminate()
                try:proc.wait(30)
                except subprocess.TimeoutExpired:proc.kill();proc.wait(10)
            log.close()


if __name__=='__main__':main()
