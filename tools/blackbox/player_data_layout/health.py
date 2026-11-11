#!/usr/bin/env python3
"""Health: один ограниченный замер оригинала26.3; только публичные команды/пакеты."""
from calibrate import *
import math
HEALTH_STATES=[dict(id='full',damage=None,health=20.0),dict(id='fractional',damage='0.5',health=19.5),dict(id='lower',damage='6.125',health=13.375)]
HEALTH_QUERIES=[
 'data get entity blackbox Health',
 'data get entity blackbox Health 2',
 'execute store result score raw MCSAHealth run data get entity blackbox Health',
 'scoreboard players get raw MCSAHealth',
 'execute store success score success MCSAHealth run data get entity blackbox Health',
 'scoreboard players get success MCSAHealth',
 'execute store result score scaled MCSAHealth run data get entity blackbox Health 2',
 'scoreboard players get scaled MCSAHealth',
 'execute store result score missing_result MCSAHealth run data get entity blackbox MCSA_MISSING_HEALTH_FIELD',
 'scoreboard players get missing_result MCSAHealth',
 'execute store success score missing_success MCSAHealth run data get entity blackbox MCSA_MISSING_HEALTH_FIELD',
 'scoreboard players get missing_success MCSAHealth',
]
def preflight_health(output):
 assert os.environ.get('GITHUB_ACTIONS')=='true' and os.environ.get('GITHUB_RUN_ATTEMPT')=='1'
 assert not output.exists()
 pin=read(BASE/'health-pins.json')
 for name,h in pin['source_files'].items():assert sha(ROOT/name)==h,name
 assert sha(BASE/'health-criteria.json')==pin['criteria_sha256']
 return pin

def health_proof(events,expected):
 rows=[e for e in events if e['name']=='update_health']
 assert rows,'missing actual update_health'
 event=rows[-1];b=io.BytesIO(bytes.fromhex(event['hex']))
 data=b.read(4);assert len(data)==4
 health=struct.unpack('>f',data)[0];food=own.read_varint(b.read)
 data=b.read(4);assert len(data)==4
 saturation=struct.unpack('>f',data)[0];assert not b.read()
 assert math.isfinite(health) and health==expected
 return dict(raw=event,health=health,food=food,saturation=saturation)

def survival_proof(events):
 mode=[e for e in events if e['name']=='game_state_change' and bytes.fromhex(e['hex'])[:1]==b'\x03']
 abilities=[e for e in events if e['name']=='abilities']
 if not mode or not abilities:raise LookupError('missing mode/abilities')
 data=bytes.fromhex(mode[-1]['hex']);assert len(data)==5 and struct.unpack('>f',data[1:])[0]==0.0
 flags=bytes.fromhex(abilities[-1]['hex']);assert len(flags)==9
 if flags[0]&13:raise LookupError('creative abilities still active')
 return dict(mode=mode[-1],abilities=abilities[-1])

def capture(output):
 pin=preflight_health(output);signal.signal(signal.SIGALRM,lambda *_: (_ for _ in ()).throw(TimeoutError('finite capture deadline')));signal.alarm(300);output.mkdir();work=output/'original';work.mkdir();clients=[];configurations=[];join_proofs=[];startup_attempts=[];cleanup_control=None;proc=None;control=None;log=None
 result=dict(status='INVALID',source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),original_only=True,core_executed=False,compiled=False);commands=[];states=[];life={}
 try:
  for port in [26230,26231]:
   with socket.socket() as check:check.bind(('127.0.0.1',port))
  source=read(ROOT/'tools/blackbox/ci/protection-edges-777.json');data=output/'data';data.mkdir()
  combat_ci.fetch(source['official_jar'],work/'server.jar')
  for e in source['json_files']:
   if e['file'] in ['protocol.json','items.json','entities.json','blocks.json']:combat_ci.fetch(e,data/e['file'])
  (work/'eula.txt').write_text('eula=true\n')
  (work/'server.properties').write_text(FIXTURE_FLAGS+'server-ip=127.0.0.1\nserver-port=26230\nenable-rcon=true\nrcon.port=26231\nrcon.password=local-layout-fixture\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nlevel-type=minecraft:flat\ngenerate-structures=false\nspawn-protection=0\nallow-flight=true\nview-distance=2\nsimulation-distance=2\nmax-tick-time=0\n')
  log=(work/'server.log').open('w');argv=['java','-Xmx1G','-XX:ActiveProcessorCount=2','-jar','server.jar','nogui']
  proc=subprocess.Popen(argv,cwd=work,stdin=subprocess.DEVNULL,stdout=log,stderr=subprocess.STDOUT,start_new_session=True);life['identity']=process_identity(proc.pid);life['argv']=argv
  deadline=time.monotonic()+120;startup_status=None;startup_done=False
  while time.monotonic()<deadline:
   assert proc.poll() is None
   startup_done='Done (' in (work/'server.log').read_text()
   if startup_done:break
   time.sleep(.1)
  assert startup_done,'Done readiness deadline'
  candidate=IsolatedRcon(26231,'local-layout-fixture');cleanup_control=candidate
  try:
   startup_status=semantic_startup(candidate,startup_done,lambda:status_query(26230))
  except BaseException as error:
   startup_attempts.append(dict(error=repr(error),calls=candidate.calls));raise
  control=candidate
  write(output/'startup-status.json',startup_status)
  protocol=read(data/'protocol.json');items=read(data/'items.json')
  for name in ['blackbox','blackboxb']:
   configuration=[]
   def receive_configuration(sock,packet_id,payload):
    configuration.append(dict(id=packet_id,hex=payload.hex()));return False
   c=own.connect(26230,protocol,lambda s,p:RawClient(s,p),version=777,username=name,configuration_handler=receive_configuration);c.start();clients.append(c);configurations.append(configuration)
  for c in clients:
   deadline=time.monotonic()+5
   while True:
    assert c.failure is None
    try:proof=creative_readiness(c.events,protocol);join_proofs.append(proof);break
    except MissingJoinProof:
     assert time.monotonic()<deadline,'actual Login/Abilities proof missing';time.sleep(.01)
  nonce='HEALTH'+os.environ['GITHUB_RUN_ID']
  def command(text):
   row=dict(command=text,issued=time.monotonic());row['response']=control.run(text);row['completed']=time.monotonic();commands.append(row);return row
  for text in SETUP:validate_setup_reply(text,command(text)['response'])
  command('scoreboard objectives add MCSAHealth dummy')
  mode=command('gamemode survival blackbox')
  assert mode['response']=="Set blackbox's game mode to Survival Mode"
  deadline=time.monotonic()+3
  while True:
   try:survival=survival_proof([e for e in clients[0].events if e['at']>=mode['issued']]);break
   except LookupError:
    assert time.monotonic()<deadline,'missing actual survival packets';time.sleep(.01)
  write(output/'survival-readiness.json',survival)
  command('clear blackbox')
  command('effect clear blackbox')
  for state in HEALTH_STATES:
   if state['damage'] is not None:
    # Фиксированное ожидание окна неуязвимости ДО единственного урона.
    time.sleep(1.1)
    command(f"damage blackbox {state['damage']} minecraft:generic")
   deadline=time.monotonic()+3
   while True:
    assert all(c.failure is None for c in clients)
    receipt=getattr(clients[0],'health_receipt',None)
    if receipt is not None and clients[0].health==state['health']:break
    assert time.monotonic()<deadline,'missing actual expected health packet'
    time.sleep(.01)
   begin=barrier(clients,control,nonce+'_'+state['id'].upper()+'_READY')
   proof=health_proof(clients[0].events,state['health'])
   replies=[command(q) for q in HEALTH_QUERIES]
   end=barrier(clients,control,nonce+'_'+state['id'].upper()+'_END')
   after=health_proof(clients[0].events,state['health'])
   assert proof['health']==after['health'],'health changed during queries'
   for event in clients[0].events:
    if begin['issued']<=event['at']<=end['issued'] and event['name']=='update_health':health_proof([event],state['health'])
   states.append(dict(id=state['id'],expected_health=state['health'],begin=begin,end=end,health=proof,after_health=after,queries=replies))
  result.update(status='COMPLETE-health-observation-not-parity',states=states)
 except BaseException:
  (output/'error.txt').write_text(traceback.format_exc());raise
 finally:
  signal.alarm(0)
  write(output/'commands.json',commands)
  write(output/'startup-attempts.json',startup_attempts)
  write(output/'configuration.json',configurations)
  write(output/'join-readiness.json',join_proofs)
  write(output/'pins.json',pin)
  shutil.copy2(BASE/'health-criteria.json',output/'criteria.json')
  write(output/'raw-clients.json',[dict(entity=c.entity,events=c.events,wire_sent=c.wire_sent,failure=c.failure) for c in clients])
  for c in clients:c.close()
  if proc is not None:
   life['cleanup_errors']=[];life['cleanup']=stop_process(proc,CleanupAdapter(cleanup_control,life['cleanup_errors']) if cleanup_control else None,identity=life['identity'])
  write(output/'raw-rcon-frames.json',cleanup_control.calls if cleanup_control else [])
  used=imported_own_files();missing=[n for n in used if n not in pin['source_files']]
  if missing:result.update(status='INVALID',unfrozen_imports=missing)
  write(output/'own-import-closure.json',used)
  if log:log.close()
  life['ports_free']=ports_free([26230,26231])
  if not all(life['ports_free'].values()) or (proc is not None and (life['cleanup']['exit_code']!=0 or not life['cleanup']['group_gone'])):result.update(status='INVALID',cleanup_failed=True)
  with tarfile.open(output/'own-source.tar.gz','w:gz') as archive:
   for n in sorted(set([*pin['source_files'],'tools/blackbox/player_data_layout/health-criteria.json','tools/blackbox/player_data_layout/health-pins.json','.github/workflows/player-health-original.yml'])):archive.add(ROOT/n,arcname=n)
  write(output/'lifecycle.json',life);write(output/'result.json',result)
  finalize_manifest(output)
 if result['status'].startswith('COMPLETE'):
  assert all(life['ports_free'].values()) and life['cleanup']['group_gone']
  first=output/'FIRST-ORIGINAL';first.mkdir(exist_ok=False)
  for n in ['result.json','commands.json','raw-clients.json','lifecycle.json','raw-rcon-frames.json','configuration.json','criteria.json','pins.json','own-source.tar.gz']:shutil.copy2(output/n,first/n)
  finalize_manifest(output)
 else:raise RuntimeError('technical INVALID retained; stop without retry')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);a=p.parse_args();capture(a.output.resolve())
