import subprocess,json,os,sys,time,hashlib
from pathlib import Path
root=Path.cwd();base=Path('/home/lenox/.local/share/mcsa-java-26.3-probe');runner=root/'tools/protocol-bridge';out=runner/'testdata/client-chain-770-773/typed773-regression-final'
out.mkdir(exist_ok=False)
binary=Path(sys.argv[1]);assert binary.exists()
protocols={774:base/'step1-json/protocol.json',776:base/'registry-26.2-json/924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2/data/pc/26.2/protocol.json'}
caches={774:base/'step1-json',776:base/'registry-26.2-json'}
env=os.environ.copy();env.update(PATH='/tmp/s26-mv10-gui/usr/bin:'+env['PATH'],LD_LIBRARY_PATH='/tmp/s26-mv10-gui/usr/lib/x86_64-linux-gnu',LIBGL_ALWAYS_SOFTWARE='1',__GLX_VENDOR_LIBRARY_NAME='mesa',MESA_LOADER_DRIVER_OVERRIDE='llvmpipe',PYTHONDONTWRITEBYTECODE='1')
env['__EGL_VENDOR_LIBRARY_FILENAMES']='/usr/share/glvnd/egl_vendor.d/50_mesa.json'
env['JDK_JAVA_OPTIONS']='-Dorg.lwjgl.glfw.libname=/tmp/s26-mv10-gui/usr/lib/x86_64-linux-gnu/libglfw.so.3.4'
results=[]
for category in ('gui','gameplay','dimensions','password'):
 for client in (774,776):
  for primary in (775,777):
   name=f'{client}-{category}{primary}';folder=out/name
   common=['--test-binary',str(binary),'--protocol',str(protocols[client]),'--client',str(client),'--primary',str(primary),'--output',str(folder)]
   if category=='gui':
    cmd=['python3',str(runner/'probe_776_action_client.py'),*common,'--client-dir',str(base/('client-1.21.11' if client==774 else 'client-26.2')),'--block-entities','--payloads',str(runner/f'testdata/step1-774-776/payload-live-{client}.json')]
   elif category=='gameplay':
    cmd=['python3',str(runner/'probe_step1_gameplay_776.py'),*common,'--native-protocol',str(base/'candidate-json/protocol-26.1.json'),'--cache',str(caches[client]),'--peer','777','--peer-protocol',str(base/'candidate-json/protocol.json'),'--restart']
   else:
    cmd=['python3',str(runner/'probe_step1_dimensions_recipes_776.py'),*common,'--cache',str(caches[client])]
    cmd.append('--password' if category=='password' else '--end')
   started=time.monotonic()
   with (out/(name+'.log')).open('w') as log:result=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,env=env)
   row={'name':name,'argv':cmd,'exit':result.returncode,'seconds':round(time.monotonic()-started,2)}
   if (folder/'result.json').exists():
    report=json.loads((folder/'result.json').read_text());row.update(status=report['status'],checks=len(report.get('checks',[])),result_sha256=hashlib.sha256((folder/'result.json').read_bytes()).hexdigest())
   results.append(row);(out/'jobs.json').write_text(json.dumps(results,indent=2)+'\n');print(row['name'],row['exit'],row.get('checks'),flush=True)
   if result.returncode:sys.exit(result.returncode)
