#!/usr/bin/env python3
"""Ручной крафт первой плиты: план по умолчанию; --run запускает isolated777."""
import argparse,sys,os,json,hashlib,subprocess,socket,time,struct,signal,copy
from pathlib import Path
from probe_container_session import Probe
from probe_gameplay import connect
from probe_white_concrete_slab import JAR_SHA1,pack_position,HASHES
from smoke import varint

def main():
 parser=argparse.ArgumentParser(description=__doc__)
 parser.add_argument('--run',action='store_true')
 target=parser.add_mutually_exclusive_group(required=True)
 target.add_argument('--jar',type=Path)
 target.add_argument('--test-binary',type=Path)
 parser.add_argument('--binary-sha256')
 parser.add_argument('--data',type=Path,required=True)
 parser.add_argument('--output',type=Path,required=True)
 args=parser.parse_args()
 if not args.run:
  print(json.dumps({'plan':'upper/middle/lower rows -> six slabs; vertical refused','port':26043 if args.jar else 26026,'scope':'raw777, not GUI; memory-world core harness'}));return
 if args.test_binary and not args.binary_sha256: parser.error('--binary-sha256 required')
 out=args.output.resolve();out.mkdir(parents=True,exist_ok=False)
 world=out/'world';world.mkdir(exist_ok=False)
 jar=args.jar
 data=args.data
 if jar: assert hashlib.sha1(jar.read_bytes()).hexdigest()==JAR_SHA1
 else: assert hashlib.sha256(args.test_binary.read_bytes()).hexdigest()==args.binary_sha256
 # Проверка закреплённых разрешённых JSON, без чтения JAR содержимого.
 protocol=json.loads((data/'protocol.json').read_text())
 from probe_white_concrete_slab import HASHES
 for p,h in HASHES.items():assert hashlib.sha256((data/p).read_bytes()).hexdigest()==h
 names={v['id']:v['name'] for v in json.loads((data/'items.json').read_text())}
 port=26043 if jar else 26026
 with socket.socket() as s:s.bind(('127.0.0.1',port))
 (world/'eula.txt').write_text('eula=true\n')
 (world/'server.properties').write_text(f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nwhite-list=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\ngamemode=survival\nspawn-protection=0\n')
 report={'scope':'ordinary raw777 manual crafting, not GUI','jar_sha1':JAR_SHA1 if jar else None,'binary_sha256':args.binary_sha256,'port':port,'snapshots':{},'commands':[],'clicks':[],'checks':[]}
 server=None;client=None
 log=out/'server.log'
 def command(s):
  report['commands'].append(s);server.stdin.write(s+'\n');server.stdin.flush()
 def snapshot(label):report['snapshots'][label]=copy.deepcopy(client.inventory)
 def click(slot,button=0):
  inv=client.inventory;report['clicks'].append({'window':inv['window'],'revision':inv['revision'],'slot':slot,'button':button})
  inv['cursor']=None
  client.send('window_click',varint(inv['window'])+varint(inv['revision'])+struct.pack('>hb',slot,button)+b'\0\0\0');client.pump(.4)
 try:
  with log.open('w') as output:
   if jar: argv=['/usr/lib/jvm/java-25-openjdk/bin/java','-Xmx1G','-jar',str(jar.resolve()),'nogui']
   else:
    for name in ['mcsa-gameplay-probe.marker','mcsa-catalog-preview263.marker']: (world/name).touch()
    argv=[str(args.test_binary.resolve()),'--ignored','--exact','catalog_preview_probe::serve_java_26_3_catalog_probe','--nocapture']
   server=subprocess.Popen(argv,cwd=world,stdin=subprocess.PIPE,stdout=output,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   deadline=time.monotonic()+100
   while ('Done (' if jar else 'MCSA gameplay probe ready') not in log.read_text():
    assert server.poll() is None and time.monotonic()<deadline,'startup deadline';time.sleep(.1)
   client=connect(port,protocol,lambda s,p:Probe(s,p,777,names),777);client.pump(10,lambda r:r['name']=='position');client.pump(1)
   for s in ['gamerule minecraft:spawn_mobs false','fill -4 63 -4 4 63 4 stone','tp McsaProbe 0.5 64 0.5 0 0','setblock 2 64 0 crafting_table','clear McsaProbe','give McsaProbe white_concrete 3']:
    command(s);client.pump(.4)
   client.send('block_place',b'\0'+pack_position((2,64,0))+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(1));client.pump(1)
   assert client.windows and len(client.inventory['slots'])==46,client.inventory
   snapshot('opened');assert client.inventory['slots'][37]=={'item':'white_concrete','count':3}
   for label,pattern,valid in [('upper',[1,2,3],True),('middle',[4,5,6],True),('lower',[7,8,9],True),('vertical',[1,4,7],False)]:
    if label!='upper':
     client.send('close_window',varint(client.inventory['window']));client.pump(.3)
     command('clear McsaProbe');command('give McsaProbe white_concrete 3');client.pump(.5)
     client.send('block_place',b'\0'+pack_position((2,64,0))+b'\1'+struct.pack('>fff',.5,.5,.5)+b'\0\0'+varint(2));client.pump(.5)
    click(37)
    for place in pattern:click(place,1)
    snapshot(label+'-inputs')
    expected={'item':'white_concrete_slab','count':6} if valid else None
    assert client.inventory['slots'][0]==expected,client.inventory
    if valid:
     click(0);snapshot(label+'-taken')
     assert all(client.inventory['slots'][place] is None for place in [0,*pattern]);assert client.inventory['cursor']==expected
     click(37);snapshot(label+'-stored')
     assert client.inventory['slots'][37]==expected and client.inventory['cursor'] is None
    else:
     before=copy.deepcopy(client.inventory);click(0);snapshot(label+'-refused')
     assert client.inventory['slots']==before['slots'] and client.inventory['cursor'] is None
    report['checks'].append(label+'-pattern-output-and-consumption')
   client.send('close_window',varint(client.inventory['window']));client.pump(.4)
   command('data get entity McsaProbe Inventory');
   if jar: command('recipe give McsaProbe minecraft:white_concrete_slab')
   client.pump(.7)
 except BaseException as e:
  import traceback;report['error']=repr(e);report['traceback']=traceback.format_exc()
 finally:
  if client:report['packets']=client.observed;client.sock.close()
  if server:
   if server.poll() is None:
    try:command('stop');server.stdin.close();server.wait(timeout=25)
    except BaseException:
     os.killpg(server.pid,signal.SIGTERM)
     try:server.wait(timeout=5)
     except subprocess.TimeoutExpired:os.killpg(server.pid,signal.SIGKILL);server.wait(timeout=5)
   report['server_exit']=server.returncode
   try:os.killpg(server.pid,0);report['group_gone']=False
   except ProcessLookupError:report['group_gone']=True
  with socket.socket() as s:
   report['port_free']=s.connect_ex(('127.0.0.1',port))!=0
  report['result']='PASS' if len(report['checks'])==4 and not report.get('error') and report.get('server_exit')==0 and report.get('group_gone') and report['port_free'] else 'FAIL'
  (out/'result.json').write_text(json.dumps(report,indent=2)+'\n')
  print(json.dumps({k:v for k,v in report.items() if k in ['result','error','checks','server_exit','group_gone','port_free']}))
  sys.exit(0 if report['result']=='PASS' else 1)

if __name__=='__main__':main()
