#!/usr/bin/env python3
"""Куб серы 26.3 (777): один прогон официального сервера. Команды /summon, /data get, /attribute,
/kill, /loot; сохраняются вывод консоли и сырые пакеты сущностей (spawn/metadata/equipment/attributes/звук).
JAR остаётся непрозрачным."""
import argparse,os,json,hashlib,subprocess,time,signal,traceback
from pathlib import Path
from probe_gameplay import Client,connect
from probe_white_concrete_slab import JAR_SHA1

KEEP={'spawn_entity','entity_metadata','entity_equipment','entity_update_attributes','entity_destroy','entity_status','entity_velocity','sound_effect','entity_sound_effect','world_particles','entity_event','collect','damage_event','hurt_animation','entity_head_rotation','rel_entity_move','entity_move_look','entity_teleport','sync_entity_position','set_equipment'}
BLOCKS=['stone','tnt','magma_block','white_wool','packed_ice','dirt','soul_sand','honeycomb_block','iron_block','sand','slab_probe']

def main():
 p=argparse.ArgumentParser();p.add_argument('--jar',type=Path,required=True);p.add_argument('--data',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--port',type=int,default=26160);a=p.parse_args()
 out=a.output.resolve();out.mkdir(parents=True,exist_ok=False);world=out/'world';world.mkdir();jar=a.jar.resolve()
 assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 protocol=json.loads((a.data/'protocol.json').read_text())
 class P(Client):
  def __init__(s,*x):super().__init__(*x);s.stage='join';s.raw=[]
  def observe_packet(s,name,payload):
   if name in KEEP:s.raw.append({'stage':s.stage,'name':name,'hex':payload.hex()})
 rep={'jar_sha1':JAR_SHA1,'console':{}};log=out/'server.log';port=a.port;server=None;client=None
 (world/'eula.txt').write_text('eula=true\n');(world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=creative\nspawn-protection=0\n')
 def cmd(t,w=.4,stage=None):
  if stage:client.stage=stage
  n=len(log.read_text());server.stdin.write(t+'\n');server.stdin.flush();client.pump(w)
  rep['console'].setdefault(client.stage,[]).append({'cmd':t,'out':log.read_text()[n:].strip()})
 try:
  with log.open('w') as o:
   server=subprocess.Popen(['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar),'nogui'],cwd=world,stdin=subprocess.PIPE,stdout=o,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   rep['server_pid']=server.pid
   until=time.monotonic()+100
   while 'Done (' not in log.read_text():assert server.poll() is None and time.monotonic()<until;time.sleep(.1)
   client=connect(port,protocol,lambda s,pr:P(s,pr),777);client.pump(10,lambda r:r['name']=='position');client.pump(.5)
   for t in ['gamerule minecraft:spawn_mobs false','fill -8 63 -8 8 63 8 stone','tp McsaProbe 0.5 64 -4 0 0','clear McsaProbe']:cmd(t)
   # 1. обычный куб: данные, атрибуты, прыжки за 6 с
   cmd('summon minecraft:sulfur_cube 0.5 64 3',.6,'summon_plain')
   for t in ['data get entity @e[type=minecraft:sulfur_cube,limit=1]']+['attribute @e[type=minecraft:sulfur_cube,limit=1] minecraft:%s get'%x for x in ['max_health','scale','movement_speed','jump_strength','gravity','step_height','knockback_resistance','attack_damage','follow_range','safe_fall_distance','armor']]:cmd(t,.2)
   cmd('data get entity @e[type=minecraft:sulfur_cube,limit=1] Health',.2)
   client.stage='idle6s';client.pump(6)
   cmd('data get entity @e[type=minecraft:sulfur_cube,limit=1] Pos',.2)
   # попытки задать размер
   for t in ['data modify entity @e[type=minecraft:sulfur_cube,limit=1] Size set value 1','data get entity @e[type=minecraft:sulfur_cube,limit=1] Size','data modify entity @e[type=minecraft:sulfur_cube,limit=1] Age set value -100','data get entity @e[type=minecraft:sulfur_cube,limit=1] Age','attribute @e[type=minecraft:sulfur_cube,limit=1] minecraft:max_health get']:cmd(t,.4,'size_try')
   cmd('kill @e[type=minecraft:sulfur_cube]',1.0,'kill_plain')
   cmd('data get entity @e[type=minecraft:sulfur_cube,limit=1]',.2,'after_kill_split')
   cmd('kill @e[type=minecraft:sulfur_cube]',1.0)
   cmd('kill @e[type=minecraft:item]',.3)
   # 2. поглощённые блоки
   for i,b in enumerate(BLOCKS[:-1]):
    cmd('summon minecraft:sulfur_cube 0.5 64 3 {equipment:{body:{id:"minecraft:%s",count:1}}}'%b,.8,'body_'+b)
    for t in ['data get entity @e[type=minecraft:sulfur_cube,limit=1]','data get entity @e[type=minecraft:sulfur_cube,limit=1] equipment','attribute @e[type=minecraft:sulfur_cube,limit=1] minecraft:movement_speed get','attribute @e[type=minecraft:sulfur_cube,limit=1] minecraft:max_health get']:cmd(t,.2)
    client.pump(1.5)
    cmd('kill @e[type=minecraft:sulfur_cube]',1.0,'kill_'+b)
    cmd('kill @e[type=minecraft:item]',.3)
   # 3. поглощение брошенного предмета
   cmd('summon minecraft:sulfur_cube 0.5 64 3',.6,'absorb_item')
   cmd('summon minecraft:item 0.5 64.5 3 {Item:{id:"minecraft:stone",count:1}}',.3)
   client.pump(3)
   cmd('data get entity @e[type=minecraft:sulfur_cube,limit=1] equipment',.2)
   cmd('data get entity @e[type=minecraft:sulfur_cube,limit=1] pickup_timer',.2)
   cmd('kill @e[type=minecraft:sulfur_cube]',1.0,'kill_absorbed')
   cmd('kill @e[type=minecraft:sulfur_cube]',1.0)
   cmd('kill @e[type=minecraft:item]',.3)
   # 4. /loot
   cmd('loot spawn 0.5 64 3 kill @e[type=minecraft:sulfur_cube,limit=1]',.3,'loot_none')
   cmd('summon minecraft:sulfur_cube 0.5 64 3 {equipment:{body:{id:"minecraft:stone",count:1}}}',.6,'loot')
   cmd('loot spawn 0.5 64 3 kill @e[type=minecraft:sulfur_cube,limit=1]',.6)
   cmd('give McsaProbe minecraft:sulfur_cube_bucket',.3,'items');cmd('give McsaProbe minecraft:sulfur_cube_spawn_egg',.3)
   cmd('summon minecraft:magma_cube 0.5 64 3',.6,'magma_cube');cmd('data get entity @e[type=minecraft:magma_cube,limit=1]',.2);cmd('kill @e[type=minecraft:magma_cube]',.5)
   cmd('summon minecraft:slime 0.5 64 3',.6,'slime');cmd('data get entity @e[type=minecraft:slime,limit=1]',.2);cmd('kill @e[type=minecraft:slime]',.5)
   rep['packets']=client.raw
 except BaseException as e:rep['error']=repr(e);rep['traceback']=traceback.format_exc()
 finally:
  if client:client.sock.close()
  if server and server.poll() is None:
   try:server.stdin.write('stop\n');server.stdin.flush();server.stdin.close();server.wait(timeout=25)
   except BaseException:os.killpg(server.pid,signal.SIGTERM)
  (out/'result.json').write_text(json.dumps(rep,indent=1)+'\n');print('done',rep.get('error'))
if __name__=='__main__':main()
