#!/usr/bin/env python3
"""Проверка обновления нашего формата мира; без --run только план."""
import argparse,ctypes,ctypes.util,hashlib,json,os,shutil,signal,socket,struct,subprocess,time,traceback,zlib
from pathlib import Path
PLAN=['old native775 standalone, isolated config/world/26048; setblock,save-all,stop', 'freeze raw old world tree; no gameplay manifest', 'new standalone same world: one full backup+storage receipt, no gameplay label; backup byte hashes equal frozen old tree', 'save-all/stop, inspect own region payload version5; no player login so no player11 runtime claim', 'restart: same receipt/backup IDs, no extra backup', 'copied malformed/future receipts: fail before listener, world hashes unchanged']
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def tree(p):
 return {str(f.relative_to(p)):{'size':f.stat().st_size,'sha256':sha(f)} for f in sorted(p.rglob('*')) if f.is_file()}
def free(port):
 with socket.socket() as s:return s.connect_ex(('127.0.0.1',port))!=0
def config(root):
 (root/'config').mkdir(parents=True)
 (root/'config/mcsa.properties').write_text('java-version=26.1.2\nsend-error-reports=false\nauth-enabled=false\nauth-premium=false\noffline-license-mode=disabled\nbedrock-port=0\n')
 (root/'config/server.properties').write_text('server-port=26048\nonline-mode=false\nlevel-type=minecraft:flat\nlevel-seed=1\nview-distance=2\nsimulation-distance=2\nspawn-protection=0\n')
def launch(binary,root,label,blocked=False):
 assert free(26048),'port26048 occupied'
 log=root/(label+'.log');p=None;observed_listener=False
 try:
  with log.open('w') as out:
   p=subprocess.Popen([str(binary)],cwd=root,stdin=subprocess.PIPE,stdout=out,stderr=subprocess.STDOUT,text=True,start_new_session=True)
   deadline=time.monotonic()+(20 if blocked else 75)
   while time.monotonic()<deadline:
    listening=not free(26048);observed_listener|=listening
    if blocked:
     assert not listening,'blocked startup opened listener'
     if p.poll() is not None:break
    else:
     if 'Готово (' in log.read_text():break
     assert p.poll() is None,'early server exit'
    time.sleep(.025)
   else:raise AssertionError('startup timeout')
   if blocked:
    assert p.poll() not in [None,0],('blocked exit',p.poll())
   else:
    assert observed_listener or not free(26048)
    if label in ['old','new']:
     p.stdin.write('forceload add 0 0\n');p.stdin.flush();time.sleep(1)
     p.stdin.write(('setblock 2 64 0 stone' if label=='old' else 'setblock 3 64 0 gold_block')+'\n');p.stdin.flush();time.sleep(1)
     expected='Changed the block at '+('2' if label=='old' else '3')+', 64, 0'
     assert expected in log.read_text(),('setblock-success-not-observed',expected)
    p.stdin.write('save-all\n');p.stdin.flush();time.sleep(1)
    p.stdin.write('stop\n');p.stdin.flush();p.stdin.close();p.wait(timeout=45);assert p.returncode==0
 finally:
  if p and p.poll() is None:
   os.killpg(p.pid,signal.SIGTERM)
   try:p.wait(timeout=5)
   except subprocess.TimeoutExpired:os.killpg(p.pid,signal.SIGKILL);p.wait(timeout=5)
  if p:
   try:os.killpg(p.pid,0);gone=False
   except ProcessLookupError:gone=True
   assert gone and free(26048),'cleanup incomplete'
 return dict(exit=p.returncode,group_gone=gone,port_free=True,listener_observed=observed_listener,log=str(log))
def versions(world):
 # Own MCS region format: 1024 big-endian offset/length entries; zstd payload.
 lib=ctypes.CDLL(ctypes.util.find_library('zstd'));lib.ZSTD_getFrameContentSize.argtypes=[ctypes.c_void_p,ctypes.c_size_t];lib.ZSTD_getFrameContentSize.restype=ctypes.c_ulonglong
 lib.ZSTD_decompress.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t];lib.ZSTD_decompress.restype=ctypes.c_size_t
 rows=[]
 for f in sorted(world.rglob('*.mcs')):
  b=f.read_bytes();assert b[:5]==b'MCS\x01\x01'
  for i in range(1024):
   off,n=struct.unpack('>II',b[8+i*8:16+i*8])
   if not n:continue
   record=b[off*256:off*256+n];crc,size=struct.unpack('>II',record[:8]);packed=record[8:];assert len(packed)==size and zlib.crc32(packed)==crc
   source=ctypes.create_string_buffer(packed);length=lib.ZSTD_getFrameContentSize(source,len(packed));assert 0<length<=64*1024*1024
   dest=ctypes.create_string_buffer(length);assert lib.ZSTD_decompress(dest,length,source,len(packed))==length
   rows.append(dict(region=str(f.relative_to(world)),slot=i,version=dest.raw[0]))
 assert rows,'no actual chunk record'
 return rows
def main():
 ap=argparse.ArgumentParser();ap.add_argument('--run',action='store_true');ap.add_argument('--old-binary',type=Path);ap.add_argument('--old-sha256');ap.add_argument('--new-binary',type=Path);ap.add_argument('--new-sha256');ap.add_argument('--output',type=Path);a=ap.parse_args()
 if not a.run:print(json.dumps({'plan':PLAN,'required':'--old-binary --old-sha256 --new-binary --new-sha256 --output','port':26048,'scope':'native775 chunk runtime only; no player login/no player11 claim'},indent=2));return
 assert a.old_binary and a.old_sha256 and a.new_binary and a.new_sha256 and a.output
 OLD=a.old_binary.resolve();OLD_SHA=a.old_sha256
 assert sha(OLD)==OLD_SHA and sha(a.new_binary)==a.new_sha256
 root=a.output.resolve();root.mkdir();live=root/'live';live.mkdir();config(live);report={'plan':PLAN,'old_binary_sha256':OLD_SHA,'new_binary_sha256':a.new_sha256,'runs':{}}
 try:
  report['runs']['old']=launch(OLD,live,'old');world=live/'world';assert not (world/'mcsa-world.json').exists() and not (world/'mcsa-storage.json').exists()
  frozen=root/'old-world-frozen';shutil.copytree(world,frozen);before=tree(frozen);(root/'old-tree.json').write_text(json.dumps(before,indent=2)+'\n');report['old_chunk_versions']=versions(frozen);assert all(x['version']==4 for x in report['old_chunk_versions'])
  report['runs']['new']=launch(a.new_binary.resolve(),live,'new');receipt=json.loads((world/'mcsa-storage.json').read_text());assert receipt==dict(schema=1,region_layout=1,chunk_layout=5,playerdata_layout=11,snapshot_id=receipt['snapshot_id']);assert not (world/'mcsa-world.json').exists()
  backup=live/'backups/world'/receipt['snapshot_id'];assert tree(backup/'world')==before;report['backup_exact_old_tree']=True;report['receipt']=receipt
  report['new_chunk_versions']=versions(world)
  new_chunks={(x['region'],x['slot']):x['version'] for x in report['new_chunk_versions']};old_keys={(x['region'],x['slot']) for x in report['old_chunk_versions']}
  assert old_keys<=set(new_chunks) and all(v in [4,5] for v in new_chunks.values())
  targets=[v for (region,slot),v in new_chunks.items() if region=='region/r.0.0.mcs' and slot==0]
  assert targets==[5],('changed-target-chunk-not-v5',targets)
  backups=tree(live/'backups');report['runs']['restart']=launch(a.new_binary.resolve(),live,'restart');assert tree(live/'backups')==backups and json.loads((world/'mcsa-storage.json').read_text())==receipt
  report['restart_no_extra_backup']=True
  for label,data in [('malformed',b'{'),('future',json.dumps({**receipt,'chunk_layout':999}).encode())]:
   case=root/label;case.mkdir();config(case);shutil.copytree(frozen,case/'world');(case/'world/mcsa-storage.json').write_bytes(data);original=tree(case/'world');report['runs'][label]=launch(a.new_binary.resolve(),case,label,True);assert tree(case/'world')==original and not (case/'backups').exists();report[label+'_preserved']=True
  report['result']='PASS'
 except BaseException as e:report.update(result='FAIL',error=repr(e),traceback=traceback.format_exc());raise
 finally:(root/'result.json').write_text(json.dumps(report,indent=2)+'\n')
 print(json.dumps(report,indent=2))
if __name__=='__main__':main()
