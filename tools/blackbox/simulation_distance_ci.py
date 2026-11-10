#!/usr/bin/env python3
"""CI-only real production startup/config→777PlayLogin; no gameplay claim."""
import argparse,hashlib,io,json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'tools/protocol-bridge'))
from combat_ci import fetch,helper_sources
from combat_process import ports_free,process_identity,stop_process
from probe_gameplay import Client,handshake,send,frame,read_varint,mapping,varint
from analyze777 import Decoder
PORTS=[26180,26181,26182,26183]
PRIMARY=ROOT/'tools/blackbox/ci/simulation-distance-two-primary.json'
RUST_REFERENCE=ROOT/'tools/blackbox/ci/simulation-distance-two-rust-reference.json'
RUST_REFERENCE_SHA='e94eb15cdc9d569b9b80d985682b74a060a62908125ee1ec98a04ebfc1b193d3'
PINNED_LOCK=ROOT/'tools/blackbox/ci/simulation-distance-two-runtime.lock'
PRIMARY_SHA='8b8053cccd3f4eb206f91685f7b97d8e402bdf1a54abbcdd7950981cddffe5cb'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def decode_login(protocol,hex_payload):
 stream=io.BytesIO(bytes.fromhex(hex_payload));value=Decoder(protocol).read(stream,'packet_login');assert not stream.read(1)
 return value

def check_primary(protocol):
 assert sha(PRIMARY)==PRIMARY_SHA
 primary=json.loads(PRIMARY.read_text())
 for key,view,simulation in [('original_two_two',2,2),('historical_old_core_ten_ten',10,10),('original_play_login',8,2),('historical_old_core_rejected_simulation10',8,10)]:
  row=primary[key];payload=bytes.fromhex(row['play_login_payload_hex']);assert hashlib.sha256(payload).hexdigest()==row['play_login_payload_sha256']
  value=decode_login(protocol,row['play_login_payload_hex']);assert value['viewDistance']==view and value['simulationDistance']==simulation
 return primary

def verify_rust_reference():
 assert sha(RUST_REFERENCE)==RUST_REFERENCE_SHA
 reference=json.loads(RUST_REFERENCE.read_text())
 assert reference['source']=='fd706e601bdcb8624526481f159129376d134b37'
 assert reference['overall_run_conclusion']=='failure' and all(v=='success' for v in reference['validated_steps'].values())
 tracked=subprocess.check_output(['git','ls-files'],cwd=ROOT,text=True).splitlines()
 assert {p for p in tracked if p.endswith('.rs')}==set(reference['rust_source_sha256'])
 assert {p for p in tracked if p=='Cargo.toml' or p.endswith('/Cargo.toml')}==set(reference['manifest_sha256'])
 for name,h in reference['rust_source_sha256'].items():assert sha(ROOT/name)==h
 for name,h in reference['manifest_sha256'].items():assert sha(ROOT/name)==h
 assert sha(PINNED_LOCK)==reference['runtime_Cargo.lock_sha256']
 assert sha(PRIMARY)==reference['PRIMARY_sha256']==PRIMARY_SHA
 return {'status':'byte-exact-Rust-tree-and-manifests-to-previous-successful-steps','reference_source':reference['source'],'reference_run_id':reference['run_id'],'reference_overall_run':'failure','source_rust_files':len(reference['rust_source_sha256']),'source_manifests':len(reference['manifest_sha256']),'Cargo.lock_sha256':sha(PINNED_LOCK),'reference_sha256':sha(RUST_REFERENCE),'Rust_suites_rerun':False}

def record_first_login(packet_bytes,protocol,target):
 target.with_suffix('.packet').write_bytes(packet_bytes)
 packet=io.BytesIO(packet_bytes);packet_id=read_varint(packet.read);payload=packet.read()
 names={v:k for k,v in mapping(protocol,'login','toClient').items()};name=names.get(packet_id,'unknown')
 kind='LoginDisconnect' if name=='disconnect' else 'LoginSuccess' if name=='success' else 'UnexpectedLoginResponse'
 row={'phase':'login','packet_id':packet_id,'name':name,'kind':kind,'packet_hex':packet_bytes.hex(),'payload_hex':payload.hex(),'packet_sha256':hashlib.sha256(packet_bytes).hexdigest(),'at':time.monotonic()}
 write(target,row)  # Сохраняем реальные байты ДО проверки admission.
 if name=='disconnect':
  stream=io.BytesIO(payload);n=read_varint(stream.read);reason=stream.read(n).decode();assert not stream.read(1)
  raise RuntimeError('LoginDisconnect: '+reason)
 assert name=='success','Unexpected first Login response: '+name
 return payload

def connect_observed(port,protocol,view_distance,first_response,client_info):
 sock=socket.create_connection(('127.0.0.1',port),timeout=10)
 try:
  handshake(sock,777,port,2);username=b'McsaDistance';send(sock,b'\0'+varint(len(username))+username+bytes([37])*16)
  payload=record_first_login(frame(sock),protocol,first_response)
  data=io.BytesIO(payload);success=Decoder(protocol).read(data,protocol['login']['toClient']['types']['packet_success']);assert not data.read(1) and success['username']=='McsaDistance'
  send(sock,b'\x03')
  settings=b'\x00\x05en_us'+bytes([view_distance])+b'\x00\x01\x7f\x01\x00\x01\x00'
  send(sock,settings);write(client_info,{'phase':'configuration','packet_hex':settings.hex(),'view_distance':view_distance,'at':time.monotonic()})
  known=mapping(protocol,'configuration','toClient')['select_known_packs']
  for _ in range(160):
   packet=io.BytesIO(frame(sock));packet_id=read_varint(packet.read);body=packet.read()
   if packet_id==known:send(sock,b'\x07'+body)
   elif packet_id==4:send(sock,b'\x04'+body)
   elif packet_id==3:send(sock,b'\x03');return LoginClient(sock,protocol)
  raise RuntimeError('Configuration packet budget exhausted')
 except BaseException:sock.close();raise

class LoginClient(Client):
 def __init__(self,*args):super().__init__(*args);self.actual=[]
 def observe_packet(self,name,payload):
  if name=='login':self.actual.append({'name':name,'hex':payload.hex(),'at':time.monotonic()})

def status(port):
 with socket.create_connection(('127.0.0.1',port),timeout=3) as sock:
  handshake(sock,777,port,1);send(sock,b'\0');packet=io.BytesIO(frame(sock));assert read_varint(packet.read)==0
  size=read_varint(packet.read);value=json.loads(packet.read(size));assert not packet.read(1);assert value['version']['protocol']==777
  return value

def capture_login(port,protocol,expected_view,first_response,client_info):
 client=connect_observed(port,protocol,expected_view,first_response,client_info)
 try:
  client.pump(8,lambda row:row['name']=='login');assert len(client.actual)==1
  event=client.actual[0];value=decode_login(protocol,event['hex'])
  assert value['viewDistance']==expected_view and value['simulationDistance']==2, value
  return {'actual_event':event,'payload_sha256':hashlib.sha256(bytes.fromhex(event['hex'])).hexdigest(),'decoded':value,'client_requested_view':expected_view,'protocol':777,'first_login_response_sha256':sha(first_response),'configuration_client_info_sha256':sha(client_info)}
 finally:client.sock.close()

def run(output,binary):
 assert os.environ.get('GITHUB_ACTIONS')=='true','no local servers'
 assert not output.exists();output.mkdir(parents=True)
 source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
 reused=verify_rust_reference();assert sha(ROOT/'Cargo.lock')==reused['Cargo.lock_sha256']
 write(output/'reused-rust-validation.json',reused)
 work=Path(tempfile.mkdtemp(prefix='mcsa-simulation-config-',dir=os.environ['RUNNER_TEMP']))
 pin=json.loads((ROOT/'tools/blackbox/ci/protection-edges-777.json').read_text())
 entry=next(e for e in pin['json_files'] if e['file']=='protocol.json');fetch(entry,work/'protocol.json');fetch(pin['official_jar'],work/'server.jar')
 protocol=json.loads((work/'protocol.json').read_text());primary=check_primary(protocol)
 assert sha(work/'protocol.json')==primary['protocol_json_sha256']
 assert binary.read_bytes()[:4]==b'\x7fELF';runtime=sha(binary)
 with (output/'runtime-elf-header.txt').open('w') as log:subprocess.run(['readelf','-h',str(binary)],stdout=log,check=True)
 selected=helper_sources(ROOT,[Path(__file__),ROOT/'tools/protocol-bridge/probe_gameplay.py',ROOT/'tools/protocol-bridge/observe.py',ROOT/'tools/protocol-bridge/smoke.py'])
 inputs={'source':source,'runtime_sha256':runtime,'primary_sha256':sha(PRIMARY),'protocol_sha256':sha(work/'protocol.json'),'official_jar':pin['official_jar'],'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in selected},'rust_source_sha256':{p:sha(ROOT/p) for p in subprocess.check_output(['git','ls-files','*.rs'],cwd=ROOT,text=True).splitlines()},'Cargo.lock_sha256':sha(ROOT/'Cargo.lock')}
 inputs['source_sha256'].update({str(p.relative_to(ROOT)):sha(p) for p in [PRIMARY,RUST_REFERENCE,PINNED_LOCK,ROOT/'.github/workflows/simulation-distance-checks.yml']})
 write(output/'inputs-before-start.json',inputs)
 life={'servers':[],'cleanup':[],'ports_free':ports_free(PORTS)};assert all(life['ports_free'].values());results={};frozen={}
 def save():write(output/'lifecycle.json',life)
 def interrupted(sig,frame):raise TimeoutError('bounded CI launch '+str(sig))
 previous={sig:signal.signal(sig,interrupted) for sig in [signal.SIGALRM,signal.SIGTERM,signal.SIGINT]};signal.alarm(300)
 try:
  for case,expected_view,side,port in [(case,view,side,port) for case,view in [('two-two',2),('eight-two-control',8)] for side,port in [('official',26180),('ours',26182)]]:
   label=case+'-'+side
   if side=='ours':assert frozen[case]==sha(output/(case+'-original-frozen.json')) and sha(binary)==runtime
   folder=work/label;folder.mkdir();(folder/'config').mkdir();(folder/'eula.txt').write_text('eula=true\n')
   config=folder/('server.properties' if side=='official' else 'config/server.properties')
   text=f'server-ip=127.0.0.1\nserver-port={port}\nwhite-list=false\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nview-distance={expected_view}\nsimulation-distance=2\nlevel-type=minecraft:flat\ngenerator-settings={{"layers":[{{"block":"minecraft:bedrock","height":1}},{{"block":"minecraft:stone","height":3}}],"biome":"minecraft:plains"}}\nlevel-seed=1\ngenerate-structures=false\nspawn-protection=0\nmax-tick-time=0\n'
   config.write_text(text);(output/(label+'-server.properties.before')).write_text(text)
   if side=='ours':
    (folder/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
    (folder/'config/skins.toml').write_text('mojang=false\nely=false\n')
    argv=[str(binary.resolve())]
   else:argv=['java','-Xmx1G','-XX:ActiveProcessorCount=2','-jar',str(work/'server.jar'),'nogui']
   assert not (folder/'world').exists();before={str(p.relative_to(folder)):sha(p) for p in folder.rglob('*') if p.is_file()}
   log=(output/(label+'-server.log')).open('x');proc=subprocess.Popen(argv,cwd=folder,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True,start_new_session=True);identity=process_identity(proc.pid)
   life['servers'].append({'side':side,'case':case,'expected_view':expected_view,'pid':proc.pid,'identity':identity,'argv':argv,'cwd':str(folder),'port':port,'fresh_world':True,'config_before_sha256':before,'runtime_sha256':runtime if side=='ours' else pin['official_jar']['sha256']});save()
   try:
    if side=='ours':
     assert sha(Path(f'/proc/{proc.pid}/exe'))==runtime
     life['servers'][-1]['actual_proc_exe_sha256']=runtime;save()
    deadline=time.monotonic()+100
    while True:
     if proc.poll() is not None:raise RuntimeError(side+' boot exit')
     if time.monotonic()>deadline:raise TimeoutError(side+' boot readiness timeout')
     observed_log=(output/(label+'-server.log')).read_text(errors='replace')
     ready_marker='Done (' if side=='official' else 'Готово ('
     if ready_marker not in observed_log:
      time.sleep(.2);continue
     try:
      observed=status(port)
      life['servers'][-1]['actual_ready_log_marker']=ready_marker
      life['servers'][-1]['status777_observed_at']=time.monotonic();save();break
     except (OSError,EOFError) as error:
      life['servers'][-1].setdefault('bounded_startup_status_retries',[]).append({'at':time.monotonic(),'error':repr(error)})
      save();time.sleep(.2)
    write(output/(label+'-status.json'),observed)
    result=capture_login(port,protocol,expected_view,output/(label+'-first-login-response.json'),output/(label+'-client-info.json'));result.update({'source':source,'properties_before_sha256':hashlib.sha256(text.encode()).hexdigest(),'runtime_sha256':runtime if side=='ours' else pin['official_jar']['sha256'],'production_startup_not_constructor':True})
    write(output/(label+'-login.json'),result);results[label]=result
   finally:
    life['cleanup'].append(stop_process(proc,identity=identity));log.close();life['ports_free']=ports_free(PORTS);save()
    if config.exists():(output/(label+'-server.properties.after')).write_bytes(config.read_bytes())
   assert all(r['exit_code']==0 and r['group_gone'] for r in life['cleanup']) and all(life['ports_free'].values())
   if side=='official':write(output/(case+'-original-frozen.json'),{'actual_login_sha256':sha(output/(label+'-login.json')),'inputs_sha256':sha(output/'inputs-before-start.json'),'result':result});frozen[case]=sha(output/(case+'-original-frozen.json'))
 finally:
  signal.alarm(0)
  for sig,handler in previous.items():signal.signal(sig,handler)
 assert len(results)==4 and sha(binary)==runtime
 write(output/'verification.json',{'status':'production-config-distances2-login-paired-pass','source':source,'runtime_sha256':runtime,'results':results,'actual_status777_each':True,'original_first_frozen_sha256':frozen,'all_owned_lifecycles_clean':True,'full_active_distance_or_hopper_tested':False})
 print('PASS: production real-file Login2/2 plus8/2 control, original first each; no hopper claim')

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--binary',type=Path,default=ROOT/'target/release/mcsheriffanya');parser.add_argument('--verify-rust-reference',action='store_true');args=parser.parse_args()
 if args.verify_rust_reference:
  args.output.parent.mkdir(parents=True,exist_ok=True);write(args.output,verify_rust_reference())
 else:run(args.output,args.binary)
