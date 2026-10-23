import os,subprocess,sys
from pathlib import Path
r=Path.cwd()/'tools/protocol-bridge';base=Path('/home/lenox/.local/share/mcsa-java-26.3-probe');out=r/('testdata/client-chain-770-773/'+sys.argv[2]);out.mkdir(exist_ok=True)
env={**os.environ,'PYTHONDONTWRITEBYTECODE':'1','PATH':'/tmp/s26-mv12-gui/usr/bin:'+os.environ['PATH'],
 'LD_LIBRARY_PATH':'/tmp/s26-mv12-gui/usr/lib/x86_64-linux-gnu','LIBGL_ALWAYS_SOFTWARE':'1','__GLX_VENDOR_LIBRARY_NAME':'mesa',
 'MESA_LOADER_DRIVER_OVERRIDE':'llvmpipe','__EGL_VENDOR_LIBRARY_FILENAMES':'/usr/share/glvnd/egl_vendor.d/50_mesa.json',
 'JDK_JAVA_OPTIONS':'-Dorg.lwjgl.glfw.libname=/tmp/s26-mv12-gui/usr/lib/x86_64-linux-gnu/libglfw.so.3.4'}
for kind in ['password','auth-gui']:
 for primary in [775,777]:
  folder=out/f'{sys.argv[3]}-{kind}{primary}'
  if folder.exists():continue
  args=['python3',str(r/('probe_chat_login_770.py' if kind=='password' else 'probe_chat_login_client_770.py')),
    '--test-binary',sys.argv[1],'--client',sys.argv[3],'--primary',str(primary),'--output',str(folder)]
  args+=['--protocol',str(base/f'client-chain-json/{sys.argv[3]}/protocol.json')] if kind=='password' else ['--client-dir',str(base/('client-1.21.4' if sys.argv[3]=='769' else 'client-1.21.5'))]
  with (out/(folder.name+'.log')).open('w') as log:result=subprocess.run(args,stdout=log,stderr=subprocess.STDOUT,env=env)
  print(folder.name,result.returncode,flush=True)
  if result.returncode:sys.exit(result.returncode)
