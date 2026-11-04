"""Прежний checklist и пароль770; отрицательные/прерванные записи сохраняются."""
import hashlib
import json
import re
import subprocess
from pathlib import Path

root=Path.cwd();here=Path(__file__).resolve().parent;base=here.parent
build=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target/release')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def read(path):return json.loads(path.read_text())
def artifact(path):return dict(path=str(path.relative_to(root)),sha256=sha(path))
text=(here/'rust-final.log').read_text()
rows=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',text)
assert rows and 'FAILED' not in text and 'warning:' not in text
assert 'network::online::tests::login_with_in_memory_session_service ... ok' in text
counts=dict(rust=sum(int(a) for a,b in rows),ignored=sum(int(b) for a,b in rows))
python=(here/'python-final.log').read_text();assert '\nOK\n' in python
counts['python']=int(re.search(r'Ran (\d+) tests',python)[1])
for name in ['clippy-workspace-final.log','clippy-root-final.log','build-final.log']:
 text=(here/name).read_text();assert 'Finished `release`' in text and 'warning:' not in text and 'error:' not in text,name
fixtures={c:base/f'payload-live-{c}.json' for c in [770,771,772,773]}
fixtures.update({774:base.parent/'step1-774-776/payload-live-774-v2.json',776:base.parent/'step1-774-776/payload-live-776.json'})
prior=read(base/'completion770-verification.json')['fixtures']
for c,p in fixtures.items():assert sha(p)==prior[str(c)]
evidence={};regressions=0
for client in [770,771,772,773,774,776]:
 for primary in [775,777]:
  cell={}
  for kind,count in [('gui',6),('gameplay',11),('dimensions',5)]+([] if client==770 else [('password',5)]):
   run='completion770-live-v3' if client==770 else 'completion770-chat-regression'
   path=base/run/f'{client}-{kind}{primary}'/'result.json';report=read(path)
   assert report['status']=='pass' and len(report['checks'])==count,path
   assert report['server_exit']==0,path
   if client!=770:regressions+=1
   if kind=='gui':
    assert report['payload_source_sha256']==sha(fixtures[client])
    assert len(report['payloads'])==196 and all(p['post_pong'] for p in report['payloads'])
    assert len(report['statue_poses'])==4
   cell[kind]=dict(**artifact(path),checks=count,binary_sha256=report['binary_sha256'])
  evidence[f'{client}/{primary}']=cell
for primary in [775,777]:
 for kind in ['password','auth-gui']:
  path=base/'completion770-chat-live-v2'/f'770-{kind}{primary}'/'result.json';report=read(path)
  assert report['status']=='pass',path
  evidence[f'770/{primary}'][kind]=artifact(path)
 path=base/'completion770-chat-production-auth'/f'770-password{primary}'/'result.json';report=read(path)
 assert report['status']=='pass' and report['public_admission'] and report['binary_sha256']==sha(build/'mcsheriffanya')
 evidence[f'770/{primary}']['production_auth']=artifact(path)
public=base/'completion770-chat-public-final/result.json';report=read(public)
assert report['status']=='pass' and len(report['checks'])==28 and report['binary_sha256']==sha(build/'mcsheriffanya')
assert len(report['sessions'])==14
for row in report['sessions']:assert row['admitted']==(row['client']!=769)
counts.update(public=28,full_regression_sessions=regressions,shared_encrypted_login=True)
assert regressions==40
reference=base/'completion770-chat-official777-reference/result.json';r=read(reference)
assert r['status']=='failed' and r['exit_codes']==[143]
assert not (reference.parent/'server').exists()
source=subprocess.check_output(['git','diff','--name-only'],text=True).splitlines()
source+=['src/network/play/auth_limbo.rs','tools/protocol-bridge/probe_chat_login_770.py','tools/protocol-bridge/probe_chat_login_client_770.py']
result=dict(client_translation_only=True,no_old_gameplay_profiles=True,no_new_checklist_items=True,
 public_admission={str(p):{str(c):('closed' if c==769 else 'open') for c in [769,770,771,772,773,774,775,776,777] if c not in [775,777] or c==p} for p in [775,777]},
 checks=counts,evidence=evidence,public=artifact(public),fixtures={str(c):sha(p) for c,p in fixtures.items()},
 final_binaries={'release':sha(build/'mcsheriffanya'),'test':sha(build/'deps/mcsheriffanya-12cfc85e9e96ee73')},
 source_sha256={p:sha(root/p) for p in source},failed_reference=artifact(reference),
 notes=['770 GUI/Play/content/gameplay/recipes/dimensions proof is retained from completion770-live-v3; typed codecs and fixture bytes are unchanged apart from strict empty Configuration-reset controls.',
 '770 password registration, three wrong attempts, wrong/correct relogin and post-login credential privacy pass on both primaries; opaque1.21.5 GUI registration/wrong/relogin also passes on both.',
 'Frozen isolated spectator world never creates a shared player, loads playerdata, grants permissions or calls plugins; credentials go only to the existing password service. After success, Configuration resets before real Play.',
 '771/772/773/774/776 freshly repeat all40 unchanged checklist sessions; original payload files remain byte-for-byte unchanged.',
 '769 tables/adapter/Play/live checklist remain unfinished; admission retains the existing Russian refusal. Nether/End disk persistence, arbitrary custom forms and exhaustive entity/attribute semantics remain unclaimed.',
 'One authorized copied official26.3 run, Java25/DEVNULL/26190–26191,stopped by PID(exit143),copied world removed.3 preliminary reference checks pass but the later existing slab observer has no target update; this failed run is not used as770 evidence.',
 'No implementation sources,JAR unpacking,reports,secrets,commit,push or25565 listener.'],
 retained_failures=['completion770-chat-live-v1: isolated negative Login entity ID rejected by existing tracker; use a positive per-connection ID before Configuration reset.',
 'completion770-chat-official777-reference: existing slab observer IndexError, no target update; no repeated official run.',
 'Network403 interrupted rust-final and772-gui777; completed artifacts retained, partial logs/folder marked network-interrupted and only incomplete verification resumed.'])
(base/'completion770-chat-verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(counts))
