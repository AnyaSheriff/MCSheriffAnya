#!/usr/bin/env python3
"""CI-only real production startup/config→777PlayLogin; no gameplay claim."""
import argparse,hashlib,io,json,os,signal,socket,subprocess,sys,tempfile,time
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'tools/protocol-bridge'))
from combat_ci import fetch,helper_sources
from combat_process import ports_free,process_identity,stop_process
from probe_gameplay import Client,connect,handshake,send,frame,read_varint
from analyze777 import Decoder
PORTS=[26180,26181,26182,26183]
PRIMARY=ROOT/'tools/blackbox/ci/simulation-distance-two-primary.json'
PRIMARY_SHA='f075f1f712a7f1310c6e97dbfe6f6aa0c51239904994027c258a265c1c105af0'
sha=lambda p:hashlib.sha256(Path(p).read_bytes()).hexdigest()
def write(path,value):path.write_text(json.dumps(value,indent=2)+'\n')
def decode_login(protocol,hex_payload):
 stream=io.BytesIO(bytes.fromhex(hex_payload));value=Decoder(protocol).read(stream,'packet_login');assert not stream.read(1)
 return value

def check_primary(protocol):
 assert sha(PRIMARY)==PRIMARY_SHA
 primary=json.loads(PRIMARY.read_text())
 for key,expected in [('original_play_login',2),('historical_old_core_rejected_simulation10',10)]:
  row=primary[key];payload=bytes.fromhex(row['play_login_payload_hex']);assert hashlib.sha256(payload).hexdigest()==row['play_login_payload_sha256']
  value=decode_login(protocol,row['play_login_payload_hex']);assert value['viewDistance']==8 and value['simulationDistance']==expected
 return primary

class LoginClient(Client):
 def __init__(self,*args):super().__init__(*args);self.actual=[]
 def observe_packet(self,name,payload):
  if name=='login':self.actual.append({'name':name,'hex':payload.hex(),'at':time.monotonic()})

def status(port):
 with socket.create_connection(('127.0.0.1',port),timeout=3) as sock:
  handshake(sock,777,port,1);send(sock,b'\0');packet=io.BytesIO(frame(sock));assert read_varint(packet.read)==0
  size=read_varint(packet.read);value=json.loads(packet.read(size));assert not packet.read(1);assert value['version']['protocol']==777
  return value

def capture_login(port,protocol):
 client=connect(port,protocol,client_type=LoginClient,view_distance=8,username='McsaDistance')
 try:
  client.pump(8,lambda row:row['name']=='login');assert len(client.actual)==1
  event=client.actual[0];value=decode_login(protocol,event['hex'])
  assert value['viewDistance']==8 and value['simulationDistance']==2, value
  return {'actual_event':event,'payload_sha256':hashlib.sha256(bytes.fromhex(event['hex'])).hexdigest(),'decoded':value,'client_requested_view':8,'protocol':777}
 finally:client.sock.close()

def run(output,binary):
 assert os.environ.get('GITHUB_ACTIONS')=='true','no local servers'
 assert not output.exists();output.mkdir(parents=True)
 source=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
 work=Path(tempfile.mkdtemp(prefix='mcsa-simulation-config-',dir=os.environ['RUNNER_TEMP']))
 pin=json.loads((ROOT/'tools/blackbox/ci/protection-edges-777.json').read_text())
 entry=next(e for e in pin['json_files'] if e['file']=='protocol.json');fetch(entry,work/'protocol.json');fetch(pin['official_jar'],work/'server.jar')
 protocol=json.loads((work/'protocol.json').read_text());primary=check_primary(protocol)
 assert sha(work/'protocol.json')==primary['protocol_json_sha256']
 assert binary.read_bytes()[:4]==b'\x7fELF';runtime=sha(binary)
 with (output/'runtime-elf-header.txt').open('w') as log:subprocess.run(['readelf','-h',str(binary)],stdout=log,check=True)
 selected=helper_sources(ROOT,[Path(__file__),ROOT/'tools/protocol-bridge/probe_gameplay.py',ROOT/'tools/protocol-bridge/observe.py',ROOT/'tools/protocol-bridge/smoke.py'])
 inputs={'source':source,'runtime_sha256':runtime,'primary_sha256':sha(PRIMARY),'protocol_sha256':sha(work/'protocol.json'),'official_jar':pin['official_jar'],'source_sha256':{str(p.relative_to(ROOT)):sha(p) for p in selected},'rust_source_sha256':{p:sha(ROOT/p) for p in subprocess.check_output(['git','ls-files','*.rs'],cwd=ROOT,text=True).splitlines()},'Cargo.lock_sha256':sha(ROOT/'Cargo.lock')}
 inputs['source_sha256'].update({str(p.relative_to(ROOT)):sha(p) for p in [PRIMARY,ROOT/'.github/workflows/simulation-distance-checks.yml']})
 write(output/'inputs-before-start.json',inputs)
 life={'servers':[],'cleanup':[],'ports_free':ports_free(PORTS)};assert all(life['ports_free'].values());results={};frozen=None
 def save():write(output/'lifecycle.json',life)
 def interrupted(sig,frame):raise TimeoutError('bounded CI launch '+str(sig))
 previous={sig:signal.signal(sig,interrupted) for sig in [signal.SIGALRM,signal.SIGTERM,signal.SIGINT]};signal.alarm(300)
 try:
  for side,port in [('official',26180),('ours',26182)]:
   if side=='ours':assert frozen==sha(output/'original-frozen.json') and sha(binary)==runtime
   folder=work/side;folder.mkdir();(folder/'config').mkdir();(folder/'eula.txt').write_text('eula=true\n')
   config=folder/('server.properties' if side=='official' else 'config/server.properties')
   text=f'server-ip=127.0.0.1\nserver-port={port}\nonline-mode=false\nenforce-secure-profile=false\nnetwork-compression-threshold=-1\nview-distance=8\nsimulation-distance=2\nlevel-type=minecraft:flat\nlevel-seed=1\ngenerate-structures=false\nspawn-protection=0\nmax-tick-time=0\n'
   config.write_text(text);(output/(side+'-server.properties.before')).write_text(text)
   if side=='ours':
    (folder/'config/mcsa.properties').write_text('java-version=26.3\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\nprefetch-chunks=0\n')
    (folder/'config/skins.toml').write_text('mojang=false\nely=false\n')
    argv=[str(binary.resolve())]
   else:argv=['java','-Xmx1G','-XX:ActiveProcessorCount=2','-jar',str(work/'server.jar'),'nogui']
   assert not (folder/'world').exists();before={str(p.relative_to(folder)):sha(p) for p in folder.rglob('*') if p.is_file()}
   log=(output/(side+'-server.log')).open('x');proc=subprocess.Popen(argv,cwd=folder,stdin=subprocess.PIPE,stdout=log,stderr=subprocess.STDOUT,text=True,start_new_session=True);identity=process_identity(proc.pid)
   life['servers'].append({'side':side,'pid':proc.pid,'identity':identity,'argv':argv,'cwd':str(folder),'port':port,'fresh_world':True,'config_before_sha256':before,'runtime_sha256':runtime if side=='ours' else pin['official_jar']['sha256']});save()
   try:
    if side=='ours':
     assert sha(Path(f'/proc/{proc.pid}/exe'))==runtime
     life['servers'][-1]['actual_proc_exe_sha256']=runtime;save()
    deadline=time.monotonic()+100
    while True:
     if proc.poll() is not None:raise RuntimeError(side+' boot exit')
     try:
      observed=status(port);break
     except OSError:
      if time.monotonic()>deadline:raise TimeoutError(side+' boot timeout')
      time.sleep(.2)
    write(output/(side+'-status.json'),observed)
    result=capture_login(port,protocol);result.update({'source':source,'properties_before_sha256':hashlib.sha256(text.encode()).hexdigest(),'runtime_sha256':runtime if side=='ours' else pin['official_jar']['sha256'],'production_startup_not_constructor':True})
    write(output/(side+'-login.json'),result);results[side]=result
   finally:
    life['cleanup'].append(stop_process(proc,identity=identity));log.close();life['ports_free']=ports_free(PORTS);save()
    if config.exists():(output/(side+'-server.properties.after')).write_bytes(config.read_bytes())
   assert all(r['exit_code']==0 and r['group_gone'] for r in life['cleanup']) and all(life['ports_free'].values())
   if side=='official':write(output/'original-frozen.json',{'actual_login_sha256':sha(output/'official-login.json'),'inputs_sha256':sha(output/'inputs-before-start.json'),'result':result});frozen=sha(output/'original-frozen.json')
 finally:
  signal.alarm(0)
  for sig,handler in previous.items():signal.signal(sig,handler)
 assert len(results)==2 and sha(binary)==runtime
 write(output/'verification.json',{'status':'production-config-simulation2-login-paired-pass','source':source,'runtime_sha256':runtime,'results':results,'actual_status777_each':True,'original_first_frozen_sha256':frozen,'all_owned_lifecycles_clean':True,'full_active_distance_or_hopper_tested':False})
 print('PASS: ordinary production startup reads real simulation-distance2 and advertises2; original first; no hopper claim')

if __name__=='__main__':
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,required=True);parser.add_argument('--binary',type=Path,default=ROOT/'target/release/mcsheriffanya');args=parser.parse_args();run(args.output,args.binary)
