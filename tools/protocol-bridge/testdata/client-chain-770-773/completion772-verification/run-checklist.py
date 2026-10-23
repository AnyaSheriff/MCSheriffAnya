import subprocess,json,os,sys,time,hashlib
from pathlib import Path
root=Path.cwd();base=Path('/home/lenox/.local/share/mcsa-java-26.3-probe');runner=root/'tools/protocol-bridge';out=runner/('testdata/client-chain-770-773/'+sys.argv[2])
out.mkdir(exist_ok=True)
binary=Path(sys.argv[1]);assert binary.exists()
protocols={772:base/'client-chain-json/772/protocol.json',773:base/'client-chain-json/773/protocol.json',774:base/'step1-json/protocol.json',776:base/'registry-26.2-json/924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/data/pc/26.2/protocol.json'}
caches={772:base/'client-chain-json/772',773:base/'client-chain-json/773',774:base/'step1-json',776:base/'registry-26.2-json'}
env=os.environ.copy();env.update(PATH='/tmp/s26-mv12-gui/usr/bin:'+env['PATH'],LD_LIBRARY_PATH='/tmp/s26-mv12-gui/usr/lib/x86_64-linux-gnu',LIBGL_ALWAYS_SOFTWARE='1',__GLX_VENDOR_LIBRARY_NAME='mesa',MESA_LOADER_DRIVER_OVERRIDE='llvmpipe',PYTHONDONTWRITEBYTECODE='1')
env['__EGL_VENDOR_LIBRARY_FILENAMES']='/usr/share/glvnd/egl_vendor.d/50_mesa.json'
env['JDK_JAVA_OPTIONS']='-Dorg.lwjgl.glfw.libname=/tmp/s26-mv12-gui/usr/lib/x86_64-linux-gnu/libglfw.so.3.4'
results=[]
for category in sys.argv[4].split(','):
 for client in map(int,sys.argv[3].split(',')):
  for primary in (775,777):
   name=f'{client}-{category}{primary}';folder=out/name
   if folder.exists():
    report=json.loads((folder/'result.json').read_text());assert report['status']=='pass';continue
   jobenv=env.copy()
   if client==776:jobenv.pop('JDK_JAVA_OPTIONS',None)
   common=['--test-binary',str(binary),'--protocol',str(protocols[client]),'--client',str(client),'--primary',str(primary),'--output',str(folder)]
   if category=='gui':
    cmd=['python3',str(runner/'probe_776_action_client.py'),*common,'--client-dir',str(base/('client-1.21.8' if client==772 else 'client-1.21.10' if client==773 else 'client-1.21.11' if client==774 else 'client-26.2')),'--block-entities','--payloads',str(runner/('testdata/client-chain-770-773/payload-live-772.json' if client==772 else 'testdata/client-chain-770-773/payload-live-773.json' if client==773 else 'testdata/step1-774-776/payload-live-774-v2.json' if client==774 else 'testdata/step1-774-776/payload-live-776.json'))]
   elif category=='gameplay':
    cmd=['python3',str(runner/'probe_step1_gameplay_776.py'),*common,'--native-protocol',str(base/'candidate-json/protocol-26.1.json'),'--cache',str(caches[client]),'--peer','777','--peer-protocol',str(base/'candidate-json/protocol.json'),'--restart']
   else:
    cmd=['python3',str(runner/'probe_step1_dimensions_recipes_776.py'),*common,'--cache',str(caches[client])]
    cmd.append('--password' if category=='password' else '--end')
   started=time.monotonic()
   with (out/(name+'.log')).open('w') as log:result=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,env=jobenv)
   row={'name':name,'argv':cmd,'exit':result.returncode,'seconds':round(time.monotonic()-started,2)}
   if (folder/'result.json').exists():
    report=json.loads((folder/'result.json').read_text());row.update(status=report['status'],checks=len(report.get('checks',[])),result_sha256=hashlib.sha256((folder/'result.json').read_bytes()).hexdigest())
   results.append(row);(out/'jobs.json').write_text(json.dumps(results,indent=2)+'\n');print(row['name'],row['exit'],row.get('checks'),flush=True)
   if result.returncode:sys.exit(result.returncode)
