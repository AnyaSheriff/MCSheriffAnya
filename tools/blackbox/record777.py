#!/usr/bin/env python3
"""Сценарии blackbox через собственный клиент775/777; JAR остаётся непрозрачным.

Обе стороны используют один клиент. Сырые пакеты сохраняются для анализа;
неподдержанные действия завершают сценарий явной ошибкой.
"""
import argparse
import collections
from concurrent.futures import ThreadPoolExecutor
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
        self.lock=threading.RLock()
        self.call(3,password)
    def receive(self,size):
        data=b''
        while len(data)<size:
            part=self.sock.recv(size-len(data))
            if not part:raise ConnectionError('RCON closed')
            data+=part
        return data
    def call(self,kind,text):
        # Два одновременных входа делят одно RCON-соединение: запрос и ответ
        # должны оставаться одной операцией, включая проверку request ID.
        with self.lock:
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


def pack_interact_location():
    # Та же точка щелчка, что у bot.js; масштаб1 в общем формате775/777.
    packed = 1
    for i, value in enumerate((0., .35, 0.)):
        packed |= math.floor((value * .5 + .5) * 32766 + .5) << (3 + 15*i)
    return (packed & 65535).to_bytes(2, 'little') + (packed >> 16).to_bytes(4, 'big')


class Recorder(Client):
    def __init__(self, sock, protocol):
        super().__init__(sock, protocol)
        self.sock.setsockopt(socket.IPPROTO_TCP,socket.TCP_NODELAY,1)
        self.version = 777
        self.state_id = 0
        self.window = None
        self.entity = None
        self.events = []
        self.preroll = []
        self.outbound = []
        self.wire_sent = []
        self.moves = []
        self.entities = []
        self.targets = []
        self.target_filter = None
        self.age_samples = []
        self.attribute_samples = []
        self.speed_attribute = None
        self.calibration = []
        self.calibrating = False
        self.observing_sounds = False
        self.item_samples = []
        self.motion = [0., 0., 0.]
        self.floor = -59.
        self.flying = True
        self.last_movement = None
        self.movement_ticks = 0
        self.controls = {}
        self.sequence = 0
        self.recording = False
        self.external_ticks = False
        self.failure = None
        self.respawns = 0
        self.health = None
        self.respawn_pending = False
        self.running = True
        self.lock = threading.RLock()

    def send(self, name, payload=b'', *, scheduled=False):
        if name=='tick_end' and self.external_ticks and not scheduled:return
        with self.lock:
            if name != 'tick_end':
                self.wire_sent.append({'at':time.monotonic(),'name':name,'hex':payload.hex()})
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
                        # Respawn может быть ниже площадки: до команды tp
                        # не переносим клиента вверх на десятки блоков.
                        y = min(self.floor,self.position[1]); self.motion[1] = 0.; grounded = True
                    else:
                        grounded = False
                    self.motion[1] = (self.motion[1] - .08) * .98
                    drag = .546 if grounded else .91
                    self.motion[0] *= drag; self.motion[2] *= drag
                    self.position = [x,y,z,yaw,pitch]
                movement=struct.pack('>dddffB', x,y,z,yaw,pitch,int(grounded))
                self.movement_ticks += 1
                # Неподвижный bot.js присылает место раз в20тактов (см.
                # piston-push-player); движение и поворот отправляются сразу.
                due = movement != self.last_movement or self.movement_ticks >= 20
                if self.recording and due:
                    self.outbound.append({'at':time.monotonic(),'name':'position_look','hex':movement.hex()})
                    self.moves.append({'at':time.monotonic(),'position':dict(zip(('x','y','z'),(x,y,z))),'velocity':dict(zip(('x','y','z'),self.motion)),'onGround':grounded})
                if due:
                    super().send('position_look',movement)
                    self.last_movement = movement
                    self.movement_ticks = 0
            super().send(name, payload)

    def confirm_teleport(self, teleport, values):
        self.motion = [0., 0., 0.]
        self.last_movement = None
        if self.version == 775:
            self.send('teleport_confirm',varint(teleport))
        else:
            super().confirm_teleport(teleport, values)

    def observe_packet(self, name, payload):
        data = io.BytesIO(payload)
        if name == 'entity_update_attributes' and attribute_sample(payload,self.entity,self.speed_attribute):
            self.attribute_samples.append(time.monotonic())
        if name in ('set_slot','set_player_inventory') and self.observing_sounds:
            self.item_samples.append((name,payload))
        if name == 'open_window':
            self.window = read_varint(data.read)
        elif name == 'close_window':
            self.window = None
        elif name in ('set_slot','window_items'):
            read_varint(data.read); self.state_id = read_varint(data.read)
        if name == 'login':
            self.entity = struct.unpack('>i', data.read(4))[0]
        elif name == 'respawn':
            self.respawns += 1
            self.send('player_loaded')
        elif name == 'update_health':
            self.health = struct.unpack('>f', payload[:4])[0]
            if self.health > 0:
                self.respawn_pending = False
            elif not self.respawn_pending:
                self.respawn_pending = True
                self.send('client_command', b'\0')
        elif name == 'update_time':
            age = struct.unpack('>q', payload[:8])[0]
            self.age_samples.append((age, time.monotonic()))
        elif name == 'spawn_entity':
            entity = read_varint(data.read); data.read(16); kind = read_varint(data.read)
            self.entities.append((entity, kind))
            if self.target_filter and self.target_filter(kind, struct.unpack('>ddd', data.read(24)), self.recording):
                self.targets.append(entity)
        elif name == 'entity_destroy':
            removed={read_varint(data.read) for _ in range(read_varint(data.read))}
            self.entities=[row for row in self.entities if row[0] not in removed]
            self.targets=[entity for entity in self.targets if entity not in removed]
        # bot.js26.1 получает Entity Velocity, но его физика не переносит этот
        # импульс в движение: BB_TRACE_POS/pvp-armor остаётся на земле. Пакет
        # сохраняем и сравниваем; не добавляем клиенту отсутствующий у него шаг.
        if self.calibrating and name in ('block_change','multi_block_change'):
            self.calibration.append(time.monotonic())
        if not self.recording and name in ('spawn_entity','entity_metadata','entity_velocity','entity_teleport','sync_entity_position','rel_entity_move','entity_move_look','entity_look','entity_destroy'):
            self.preroll.append({'at':time.monotonic(), 'name':name, 'hex':payload.hex()})
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
        if hasattr(self,'thread'):self.thread.join(2)


def expand(text, origin):
    text = text.replace('$(bot2)', 'blackboxb').replace('$(bot)', 'blackbox')
    return re.sub(r'\$\((-?[\d.]+),(-?[\d.]+),(-?[\d.]+)\)',
                  lambda m:' '.join(format(float(m[i+1])+origin[i], '.10g') for i in range(3)), text)


def protocol_rotation(yaw, pitch):
    # bot.js хранит радианы mineflayer; в Position/Use Item идут градусы.
    return [(math.pi-yaw)*180/math.pi, -pitch*180/math.pi]


def aim_at(client, pos):
    # Обычная стоячая поза; [проверить] высоту глаз при sneaking/swimming.
    dx=pos[0]-client.position[0];dy=pos[1]-client.position[1]-1.62;dz=pos[2]-client.position[2]
    client.position[3:]=[math.degrees(math.atan2(-dx,dz)), -math.degrees(math.atan2(dy,math.hypot(dx,dz)))]


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
        c.position[3:] = protocol_rotation(*map(float,parts[1:]))
    elif op == '!respawn':
        c.send('client_command', b'\0')
    else:
        raise ValueError('unsupported setup '+text)
    time.sleep(.1)


def window_click_payload(window,state,action):
    # Та же форма, что у bot.js: без предполагаемых изменённых ячеек и курсора.
    return varint(window)+varint(state)+struct.pack('>hb',action['slot'],action.get('button',0))+varint(action.get('mode',0))+b'\0\0'


def tracking_origin(packets,entities,kind):
    # Первый нулевой сдвиг отмечает начало60-тактового цикла слежения.
    # Подготовка команд может занять разное время; это состояние начала,
    # а не допуск compare.js. ID вида берётся из закреплённого entities.json.
    candidates={entity for entity,entity_kind in entities if entity_kind==kind}
    for packet in packets:
        if packet['name']!='rel_entity_move':continue
        data=io.BytesIO(bytes.fromhex(packet['hex']))
        entity=read_varint(data.read)
        if entity not in candidates:continue
        flags=read_varint(data.read)
        if flags>>1==0 and data.read()==bytes(6):return packet['at']
    return None


def attribute_sample(payload,entity,speed_attribute=None):
    data=io.BytesIO(payload)
    if read_varint(data.read)!=entity:return False
    if speed_attribute is None:return True
    allowed=tuple(speed_attribute) if isinstance(speed_attribute,tuple) else (speed_attribute,)
    count=read_varint(data.read)
    if not 1<=count<=len(allowed):return False
    keys=[]
    for _ in range(count):
        keys.append(read_varint(data.read));data.read(8)
        for _ in range(read_varint(data.read)):
            data.read(read_varint(data.read));data.read(9)
    # Sweeping Edge повторяется в том же пакете, полный пакет смены режима
    # сюда не относится. Оба ID — из закреплённых Slot-наблюдений.
    return allowed[0] in keys and set(keys)<=set(allowed)


def validate(scenario):
    supported = {'after','cmd','bot','attack','which','target','use','release','hand','hotbar','control','look','fly','set','click','face','cursor','dig','status','respawn','window_click','close','inventory','interact','sneaking','leave_bed','button','slot_state','select_trade'}
    for a in scenario['actions']:
        if set(a)-supported:
            raise ValueError('unsupported actions: '+str(sorted(set(a)-supported)))
    for c in scenario.get('setup', []) + scenario.get('cleanup', []):
        if c.startswith('!') and c.split()[0] not in ('!wait','!hotbar','!hotbar2','!look','!look2','!stopfly','!startfly','!respawn'):
            raise ValueError('unsupported setup '+c)


def dig_status(status, version):
    # Сценарии bot.js используют enum775; actual777-dig + wire::digging:
    # Cancel1→2, Finish2→3, остальные подтверждённые действия тоже сдвинуты.
    if not 0 <= status <= 7:
        raise ValueError('unsupported digging action '+str(status))
    return status + int(version == 777 and status > 0)


def record(port, protocol, s, rcon, output, version=777, sound_output=None, reference=None, tracking_kind=None, speed_attribute=None, entity_names=None, isolate_vitals=False):
    clients=[]; commands=[]
    origin=s.get('origin',[100,-59,100])
    def command(text, delay=True):
        text=expand(text,origin)
        encoded=text.encode()
        clients[0].send('chat_command',varint(len(encoded))+encoded)
        result=f'{version} chat_command; responses retained in packet trace'
        commands.append({'command':text,'response':result})
        if delay: time.sleep(.1)
    try:
        usernames=['blackbox','blackboxb'][:2 if s.get('second') else 1]
        clients=[None]*len(usernames)
        def join_client(entry):
            i,username=entry
            c=connect(port,protocol,lambda sock,table: make_recorder(sock,table,version,speed_attribute),version=version,username=username)
            clients[i]=c
            # Те же наборы и границы, что у исходного bot.js: случайный
            # предмет/игрок не должен становиться целью Attack.
            source=(ROOT/'tools/blackbox/bot.js').read_text()
            def kinds(name):
                values=re.search(r'const '+name+r" = new Set\(\[(.*?)\]\)",source).group(1)
                return set(re.findall(r"'([^']+)'",values))
            mobs=kinds('MOB_TYPES')
            for values in re.findall(r"for \(const t of \[(.*?)\]\) MOB_TYPES.add",source):mobs.update(re.findall(r"'([^']+)'",values))
            mobs.add('warden');carts=kinds('CART_TYPES');projectiles=kinds('PROJECTILE_TYPES')
            def target_filter(kind,pos,recording):
                name=(entity_names or {}).get(kind)
                inside=abs(pos[0]-origin[0])<=s.get('pad_wide',8) and -8<=pos[1]-origin[1]<=s.get('pad_up',8) and abs(pos[2]-origin[2])<=s.get('pad_wide',8)
                return inside and ((recording and name in carts) or (s.get('track_mobs') and name in mobs|{'experience_orb'}) or (s.get('track_items') and name=='item') or (s.get('track_projectiles') and name in projectiles) or (s.get('track_fishing') and name=='fishing_bobber'))
            c.target_filter=target_filter
            c.pump(15,lambda row:row['name']=='position')
            # Прошлый сценарий мог оставить survival над пустотой. Не ждём
            # шесть секунд до creative: оригинал иначе отключает за полёт.
            rcon.run('gamemode creative '+username)
            c.pump(6);c.start()
        # bot.js тоже впускает обоих игроков одновременно.
        with ThreadPoolExecutor(max_workers=len(usernames)) as pool:
            list(pool.map(join_client,enumerate(usernames)))
        if sound_output is not None and not sound_output.exists():
            # Только разрешённое сетевое наблюдение имён из прежней JSON-таблицы.
            rcon.run('gamerule spawn_mobs false');rcon.run('kill @e[type=!player]')
            for username in usernames:rcon.run('gamemode creative '+username)
            time.sleep(.2)
            known=json.loads((ROOT/'tools/blackbox/data/sounds.json').read_text())
            names=sorted(set(known.values()))
            from analyze777 import Decoder
            decoder=Decoder(protocol)
            c=clients[0];c.observing_sounds=True;sounds={};unobserved={};evidence={}
            for name in names:
                before=len(c.item_samples)
                response=rcon.run('item replace entity blackbox weapon.mainhand with minecraft:stone[consumable={sound:"'+name+'"}]')
                deadline=time.monotonic()+1;found=None
                while found is None and time.monotonic()<deadline:
                    for packet,payload in c.item_samples[before:]:
                        value=decoder.read(io.BytesIO(payload),'packet_'+packet)
                        item=value.get('item',value.get('contents',{}))
                        for component in item.get('components',[]):
                            if component['type']=='consumable' and 'soundId' in component['data']['sound']:
                                found=component['data']['sound']['soundId']
                                evidence[name]={'packet':packet,'hex':payload.hex()}
                    if found is None:time.sleep(.005)
                if found is not None:sounds[str(found)]=name
                else:unobserved[name]=response
            c.observing_sounds=False
            sound_output.write_text(json.dumps({'sounds':sounds,'unobserved':unobserved,'slots':evidence,'method':'ordinary stone consumable.sound; captured Slot bytes'},indent=2)+'\n')
        command('gamerule spawn_mobs false'); command('gamerule natural_health_regeneration false')
        command('difficulty peaceful'); command('kill @e[type=!player]')
        # Те же сбросы, что перед setup исходного bot.js. Измерение, опыт,
        # ячейка и остановленное время переживают предыдущий сценарий.
        for name, client in zip(usernames, clients):
            command(f'gamemode creative {name}')
            command(f'execute in minecraft:overworld run tp {name} $(3.5,1,3.5)')
            command(f'experience set {name} 0 levels')
            command(f'experience set {name} 0 points')
            client.send('held_item_slot',struct.pack('>h',0))
        command('gamerule advance_time true')
        if isolate_vitals:
            # Начальные здоровье/еда обоих игроков одинаковы: длительность
            # RCON и консоли между сценариями иначе меняет мирное лечение.
            for i, (name, client) in enumerate(zip(usernames, clients)):
                token='$(bot2)' if i else '$(bot)'
                if 'kill '+token in s.get('setup',[]):continue
                before = client.respawns
                rcon.run('gamemode survival '+name)
                rcon.run('kill '+name)
                deadline = time.monotonic()+10
                while (client.respawns == before or not client.health or client.health <= 0) and time.monotonic() < deadline:
                    time.sleep(.05)
                if client.respawns == before or not client.health or client.health <= 0:
                    raise RuntimeError('initial vitals reset did not respawn '+name)
            # После неудачного предыдущего сценария выбранная ячейка могла
            # разойтись. Явные !hotbar в setup по-прежнему имеют приоритет.
            for client in clients:client.send('held_item_slot',struct.pack('>h',0))
        command(f'forceload add {origin[0]-8} {origin[2]-8} {origin[0]+8} {origin[2]+8}')
        for i,c in enumerate(clients):
            name='blackboxb' if i else 'blackbox'
            command(f'gamemode creative {name}')
            command(f'tp {name} $(3.5,0,3.5)')
        time.sleep(3)
        command('fill $(-8,-1,-8) $(8,8,8) air')
        command('fill $(-8,-1,-8) $(8,-1,8) stone')
        # В свежем flat-мире трава под полом иначе случайно превращается в землю.
        command('fill $(-8,-2,-8) $(8,-2,8) dirt replace grass_block')
        command('weather clear')
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
        # Исходный bot.js заранее смотрит в центр первой цели. Без этого
        # размещение зависело от поворота, оставшегося после возрождения.
        first_click=next((a for a in s['actions'] if 'click' in a and a.get('bot')!=2),None)
        if first_click:
            aim_at(clients[0],[origin[i]+first_click['click'][i]+.5 for i in range(3)])
            time.sleep(.5)
        for c in clients:
            if c.failure: raise RuntimeError(c.failure)
            deadline=time.monotonic()+10
            while len(c.age_samples)<2 and time.monotonic()<deadline:time.sleep(.1)
            if len(c.age_samples)<2: raise RuntimeError('not enough server clock samples; packets='+str(collections.Counter(v['name'] for v in c.records)))
        for c in clients: c.calibrating=True
        for i in range(6):
            before=len(clients[0].calibration)
            command('setblock $(8,0,8) '+('stone' if i%2 else 'glass'))
            # Как calibrate() в bot.js: до восьми тактов на каждый отклик,
            # чтобы пересылка чанков после respawn не склеила шесть замен в одну.
            deadline=time.monotonic()+.4
            while len(clients[0].calibration)==before and time.monotonic()<deadline:time.sleep(.01)
        deadline=time.monotonic()+10
        while len(clients[0].calibration)<3 and time.monotonic()<deadline:time.sleep(.1)
        for c in clients: c.calibrating=False
        # Только поздние samples: первый update_time приходит вне тика.
        bases=[stamp-age*.05 for age,stamp in clients[0].age_samples[1:][-6:]]
        clock=min(bases)
        if len(clients[0].calibration)<3: raise RuntimeError(f'calibration responses missing: {len(clients[0].calibration)}/6; client failure={clients[0].failure}')
        phases=[(t-clock)%.05 for t in clients[0].calibration]
        # Центр событий находится на +полтакта от счётной границы.
        # Тот же минимум по развёрнутому кругу и порог 3/6, что у bot.js.
        first=phases[0]
        around=[first+((p-first+.025)%.05-.025) for p in phases]
        clock += min(around)+.025
        start_tick=math.ceil((time.monotonic()-clock)/.05)+3
        if s.get('align'): start_tick=math.ceil(start_tick/s['align'])*s['align']
        elif any(c.attribute_samples for c in clients):
            # Слежение игрока идёт через такт. Совмещаем фазу нажатия с уже
            # наблюдённой рассылкой атрибутов; допуск compare.js не расширяем.
            first_attack=next((action for action in s['actions'] if action.get('attack')),{})
            attacker=first_attack.get('bot',1)-1
            phase_client=clients[attacker] if clients[attacker].attribute_samples else next(c for c in clients if c.attribute_samples)
            phase_tick=math.floor((phase_client.attribute_samples[-1]-clock)/.05)
            desired_phase=0
            if reference:
                samples=[t for t in reference.get('attribute_phase_samples',[[]]*len(clients))[attacker] if t<reference['start']]
                # Полный пакет после смены режима не отмечает фазу слежения.
                # Для777 берём настоящий повтор attack_speed после износа.
                tracked=[e['at'] for e in reference['clients'][attacker]['events']
                    if speed_attribute is not None and e['name']=='entity_update_attributes'
                    and e['at']>reference['start']+first_attack.get('after',0)*.05
                    and attribute_sample(bytes.fromhex(e['hex']),reference['initial'][attacker]['entity'],speed_attribute)]
                if tracked:samples=tracked[:1]
                if samples:
                    before=math.floor((samples[-1]-reference['clock'])/.05)
                    reference_start=round((reference['start']-reference['clock'])/.05)
                    desired_phase=(reference_start+first_attack.get('after',0)-before)%2
            start_tick+=(phase_tick-start_tick-first_attack.get('after',0)+desired_phase)%2
        if version==777 and reference and s.get('track_mobs') and tracking_kind is not None:
            before=tracking_origin(reference['clients'][0].get('preroll',[]),reference['initial'][0]['entities'],tracking_kind)
            current=tracking_origin(clients[0].preroll,clients[0].entities,tracking_kind)
            if before is not None and current is not None:
                phase=(round((reference['start']-reference['clock'])/.05)-math.floor((before-reference['clock'])/.05))%60
                current_tick=math.floor((current-clock)/.05)
                start_tick+=(phase-(start_tick-current_tick))%60
        start=clock+start_tick*.05
        initial=[{'entity':c.entity,'entities':c.entities.copy(),'position':c.position.copy()} for c in clients]
        for c in clients:c.recording=True;c.events=[];c.external_ticks=True
        sent=[]
        # Физика обоих клиентов идёт по одной сетке, независимо от частоты
        # входящих пакетов: иначе за четыре такта один успевал пройти три шага.
        timeline=s['actions']+[{'after':tick,'_physics':True} for tick in range(s['record_ticks']+14)]
        for action in sorted(timeline,key=lambda row:(row.get('after',0),row.get('_physics',False))):
            at=start+action.get('after',0)*.05
            time.sleep(max(0,at-time.monotonic()))
            if action.get('_physics'):
                for client in clients:client.send('tick_end',scheduled=True)
                continue
            c=clients[1 if action.get('bot')==2 else 0]
            sent.append({'tick':action.get('after',0),'at':time.monotonic(),'action':action,'pose':c.position.copy()})
            if 'cmd' in action: command(action['cmd'], delay=False)
            if action.get('attack'):
                target=clients[0].entity if action.get('bot')==2 else clients[1].entity if action.get('target')=='second' else next(iter(c.targets[action.get('which',len(c.targets)-1):][:1]),None)
                if target is not None:c.send('attack',varint(target+action.get('part',0))); c.send('arm_animation',b'\0')
            if action.get('interact'):
                target=next(iter(c.targets[action.get('which',len(c.targets)-1):][:1]),None)
                # В777/775 Interact уже не содержит прежнего action enum.
                if target is None:continue
                c.send('use_entity',varint(target)+b'\0'+pack_interact_location()+bytes([bool(action.get('sneaking'))]))
                c.send('arm_animation',b'\0')
            if action.get('leave_bed'):c.send('entity_action',varint(c.entity)+b'\0\0')
            if 'button' in action:
                if c.window is None:raise ValueError('button without open window')
                c.send('enchant_item',varint(c.window)+varint(action['button']))
            if 'select_trade' in action:c.send('select_trade',varint(action['select_trade']))
            if 'slot_state' in action:
                if c.window is None:raise ValueError('slot_state without open window')
                w=action['slot_state'];c.send('set_slot_state',varint(w['slot'])+varint(c.window)+bytes([bool(w['state'])]))
            if 'hotbar' in action: c.send('held_item_slot',struct.pack('>h',action['hotbar']))
            if 'control' in action:
                key,value=action['control'];c.controls[key]=value
                if key=='sprint':c.send('entity_action',varint(c.entity)+varint(1 if value else 2)+b'\0')
            if 'look' in action:c.position[3:]=protocol_rotation(*action['look'])
            if action.get('use'):
                c.sequence+=1;c.send('use_item',varint(action.get('hand',0))+varint(c.sequence)+struct.pack('>ff',*c.position[3:]))
            if action.get('release'):c.send('block_dig',bytes([6 if version==777 else 5])+bytes(8)+b'\0\0')
            if 'fly' in action:c.flying=action['fly'];c.send('abilities',bytes([2 if c.flying else 0]))
            if action.get('respawn'):c.send('client_command',b'\0')
            if 'set' in action:
                b=action['set'];command('setblock $('+','.join(map(str,b['at']))+') '+b['state'], delay=False)
            if 'click' in action:
                pos=[int(origin[i]+action['click'][i]) for i in range(3)];c.sequence+=1
                if action.get('bot')!=2:
                    aim_at(c,[v+.5 for v in pos])
                    c.send('look',struct.pack('>ffB',*c.position[3:],int(c.position[1]<=c.floor+1e-7)))
                c.send('block_place',b'\0'+pack_position(pos)+varint(action.get('face',1))+struct.pack('>fff',*action.get('cursor',[.5,.5,.5]))+b'\0\0'+varint(c.sequence))
                c.send('arm_animation',b'\0')
            if action.get('close'):
                window=0 if action.get('inventory') else c.window
                if window is None:raise ValueError('close without open window')
                c.send('close_window',varint(window));c.window=None
            if 'window_click' in action:
                w=action['window_click'];window=0 if w.get('inventory') else c.window
                if window is None:raise ValueError('window_click without open window')
                # Пустой changedSlots и отсутствующий HashedSlot — как у bot.js.
                c.send('window_click',window_click_payload(window,c.state_id,w))
            if 'dig' in action:
                pos=[int(origin[i]+action['dig'][i]) for i in range(3)];c.sequence+=1
                c.send('block_dig',varint(dig_status(action.get('status',0),version))+pack_position(pos)+bytes([action.get('face',1)])+varint(c.sequence))
        time.sleep(max(0,start+(s['record_ticks']+13)*.05-time.monotonic()))
        for c in clients:c.recording=False;c.external_ticks=False
        result={'scenario':s['name'],'protocol':version,'origin':origin,'port':port,'start':start,'clock':clock,'initial':initial,'attribute_phase_samples':[c.attribute_samples for c in clients],'calibration_samples':[c.calibration for c in clients],'sent':sent,'clients':[{'events':c.events,'preroll':c.preroll,'outbound':c.outbound,'moves':c.moves,'failure':c.failure} for c in clients],'commands':commands,'recorder_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'isolate_vitals':isolate_vitals,'scope':'raw flat stone physics; original bot.js comparison projection'}
        output.write_text(json.dumps(result,indent=1)+'\n')
        for text in s.get('cleanup',[]):
            special(text,clients) if text.startswith('!') else command(text)
        if any(c.failure for c in clients):raise RuntimeError(str([c.failure for c in clients]))
        return result
    finally:
        if not output.exists():
            failed={'scenario':s['name'],'commands':commands,'clients':[{'events':c.events,'records':c.records,'wire_sent':c.wire_sent,'failure':c.failure} for c in clients if c is not None]}
            output.with_name(output.stem+'-failed.json').write_text(json.dumps(failed,indent=1)+'\n')
        for c in clients:
            if c is not None:c.close()


def make_recorder(sock,protocol,version,speed_attribute=None):
    client=Recorder(sock,protocol);client.version=version;client.speed_attribute=speed_attribute;return client


class Console:
    def __init__(self, stream): self.sock=self; self.stream=stream
    def run(self, text): self.stream.write(text+"\n");self.stream.flush();return "console: see server.log"
    def close(self): self.stream.close()


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True)
    p.add_argument('--binary',type=Path,required=True);p.add_argument('--jar',type=Path,required=True)
    p.add_argument('--version',type=int,choices=(775,777),default=777)
    p.add_argument('--parallel',action='store_true')
    p.add_argument('--official-port',type=int,default=26140)
    p.add_argument('--core-port',type=int,default=26142)
    p.add_argument('--skip-recorded-core',action='store_true')
    p.add_argument('--resume',action='store_true');p.add_argument('--core-only',action='store_true')
    p.add_argument('--official-only',action='store_true')
    p.add_argument('--isolate-vitals',action='store_true')
    p.add_argument('--core-debug',action='store_true')
    p.add_argument('scenarios',nargs='*')
    a=p.parse_args();assert a.binary.is_file(), 'build binary first'
    speed_attribute=None
    if a.version==777:
        assert hashlib.sha1(a.jar.read_bytes()).hexdigest()==JAR_SHA1, 'pinned official JAR hash mismatch'
        tables=inputs(a.data)
        from analyze777 import Decoder
        decoder=Decoder(tables['protocol.json'])
        evidence=json.loads((ROOT/'tools/protocol-bridge/testdata/attribute-id-observations.json').read_text())
        tracked_attributes={}
        for row in evidence['new']['cases']:
            name=row['name'].removeprefix('minecraft:')
            if name in ('attack_speed','sweeping_damage_ratio'):
                slot=decoder.read(io.BytesIO(bytes.fromhex(row['slot_hex'])),'Slot')
                tracked_attributes[name]=next(c['data'][0]['typeId'] for c in slot['components'] if c['type']=='attribute_modifiers')
        speed_attribute=tuple(tracked_attributes[name] for name in ('attack_speed','sweeping_damage_ratio'))
    else:
        raw=(a.data/'protocol.json').read_bytes()
        assert hashlib.sha256(raw).hexdigest()=='0bb4f020fa45cc6629748f09546ec2bfd645f1843cccac9da2cab9d0539ab7e3','pinned775 protocol mismatch'
        tables={'protocol.json':json.loads(raw)}
    a.output.mkdir(parents=True,exist_ok=a.resume)
    selected=a.scenarios or sorted({v.stem for pattern in ('pvp-*','cow-crit*','sword-sweep-*','itemaudit4-knockback*','*shield*') for v in (ROOT/'tools/blackbox/scenarios').glob(pattern+'.json')})
    manifest={'isolate_vitals':a.isolate_vitals,'protocol':a.version,'primary':'26.3' if a.version==777 else '26.1.2','binary_sha256':hashlib.sha256(a.binary.read_bytes()).hexdigest(),'jar_sha1':hashlib.sha1(a.jar.read_bytes()).hexdigest(),'recorder_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'scenario_names':selected,'core_port':a.core_port,'data_sha256':{path.name:hashlib.sha256(path.read_bytes()).hexdigest() for path in a.data.glob('*.json') if path.name in ('protocol.json','entities.json','items.json','blocks.json')}}
    manifest['comparison_sha256']={name:hashlib.sha256((Path(__file__).parent/name).read_bytes()).hexdigest() for name in ('bot.js','compare.js','analyze777.py','replay777.js')}
    (a.output/f'run-{time.time_ns()}.json').write_text(json.dumps(manifest,indent=2)+'\n')
    servers=[]
    summary=json.loads((a.output/'summary.json').read_text()) if a.resume and (a.output/'summary.json').exists() else []
    try:
        assert 26100<=a.core_port<=26198 and 26100<=a.official_port<=26198
        assert not ({a.core_port,a.core_port+1}&{a.official_port,a.official_port+1})
        assert not (a.core_only and a.official_only)
        labels=[('ours',a.core_port)] if a.core_only else [('official',a.official_port)] if a.official_only else [('official',a.official_port),('ours',a.core_port)]
        for label,port in labels:
            for n in (port,port+1):
                with socket.socket() as check:
                    check.setsockopt(socket.SOL_SOCKET,socket.SO_REUSEADDR,1);check.bind(('127.0.0.1',n))
            folder=a.output/label;folder.mkdir(exist_ok=a.resume);(folder/'config').mkdir(exist_ok=a.resume)
            if label=='official':shutil.copyfile(a.jar,folder/'server.jar')
            else:
                (folder/'config/mcsa.properties').write_text(('java-version=26.3' if a.version==777 else 'java-version=26.1.2')+'\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\nbedrock-port=0\nprefetch-chunks=0\n')
                if a.core_debug:
                    with (folder/'config/mcsa.properties').open('a') as config:config.write('debug=true\n')
                (folder/'config/skins.toml').write_text('mojang=false\nely=false\n')
            (folder/'eula.txt').write_text('eula=true\n')
            (folder/('server.properties' if label=='official' else 'config/server.properties')).write_text(f'server-ip=127.0.0.1\nserver-port={port}\nenable-rcon=true\nrcon.port={port+1}\nrcon.password=local-blackbox\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\nallow-flight=true\ngamemode=creative\nmax-tick-time=0\n')
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
            row={'scenario':name,'status':'mismatch-open'}
            existing=next((i for i,value in enumerate(summary) if value['scenario']==name),None)
            if existing is None:summary.append(row)
            else:summary[existing]=row
            try:
                s=json.loads((ROOT/'tools/blackbox/scenarios'/f'{name}.json').read_text());validate(s)
                def run_side(label,rcon,port):
                    if a.resume and (label=='official' or a.skip_recorded_core) and (a.output/f'{name}-{label}.json').exists():return
                    print(label,name,flush=True)
                    reference_path=a.output/f'{name}-official.json'
                    reference=json.loads(reference_path.read_text()) if label=='ours' and a.core_only and reference_path.exists() else None
                    tracking_kind=next(row['id'] for row in json.loads((a.data/'entities.json').read_text()) if row['name']=='cow')
                    record(port,tables['protocol.json'],s,rcon,a.output/f'{name}-{label}.json',a.version,a.output/'sound-observations.json' if label=='official' and a.version==777 else None,reference,tracking_kind,speed_attribute,{row['id']:row['name'] for row in json.loads((a.data/'entities.json').read_text())},a.isolate_vitals)
                if a.parallel:
                    with ThreadPoolExecutor(max_workers=2) as pool:
                        futures=[pool.submit(run_side,label,rcon,port) for (label,_),(_,_,rcon,port) in zip(labels,servers)]
                        for future in futures:future.result()
                else:
                    for (label,_),(_,_,rcon,port) in zip(labels,servers):run_side(label,rcon,port)
                row['reason']='recorded; original comparison projection pending'
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
