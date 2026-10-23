"""Прежний checklist:770 отдельно от допущенных771–776, без новых ворот."""
import hashlib,json,re,subprocess
from pathlib import Path
here=Path(__file__).resolve().parent;base=here.parent;root=here.parents[4]
target=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target/release')
def read(p):return json.loads(p.read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
rust=(here/'rust-final.log').read_text();rows=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',rust)
assert rows and 'FAILED' not in rust and 'warning:' not in rust
assert 'network::online::tests::login_with_in_memory_session_service ... ok' in rust
python=(here/'python-final.log').read_text();assert '\nOK\n' in python
counts={'rust':sum(int(a) for a,b in rows),'ignored':sum(int(b) for a,b in rows),'python':int(re.search(r'Ran (\d+) tests',python)[1]),'public':24,'shared_online_login':True,'official_native777_reference':7}
for name in ['clippy-workspace-final.log','clippy-root-final.log','build-final.log','clippy-comment-final.log','build-comment-final.log']:
 text=(here/name).read_text();assert 'Finished `release`' in text and 'warning:' not in text and 'error:' not in text,name
fixtures={c:base/f'payload-live-{c}.json' for c in [770,771,772,773]}
fixtures.update({774:base.parent/'step1-774-776/payload-live-774-v2.json',776:base.parent/'step1-774-776/payload-live-776.json'})
assert [r['label'] for r in read(fixtures[770])['packets']]==[r['label'] for r in read(fixtures[771])['packets']]
old=read(base/'completion771-verification.json')['fixtures']
for c in [771,772,773,774,776]:assert sha(fixtures[c])==old[str(c)]
evidence={}
for client in [770,771,772,773,774,776]:
 for primary in [775,777]:
  cell={}
  for kind,count in [('gui',6),('gameplay',11),('dimensions',5),('password',5)]:
   run='completion770-live-v3' if client==770 else 'completion770-regression'
   if client==770 and kind=='password':run='completion770-password'
   folder=base/run/f'{client}-{kind}{primary}';report=read(folder/'result.json')
   if client==770 and kind=='password':
    assert report['status']=='failed' and not report['checks']
    cell[kind]={'path':str(folder.relative_to(root)),'status':'failed','error':report['error'],'result_sha256':sha(folder/'result.json')};continue
   assert report['status']=='pass' and report['server_exit']==0,folder
   assert report['primary']==primary and report.get('client',report.get('protocol'))==client
   assert len(report['checks'])==count
   if kind=='gui':
    assert report['payload_source_sha256']==sha(fixtures[client])
    assert len(report['payloads'])==196 and all(p['post_pong'] for p in report['payloads'])
    assert report['payload_post_pong'] and len(report['statue_poses'])==4
    if client in [770,771,772,773]:
     assert report['attack_equivalent'] and report['checks'][-1]['target_health']==[4.0]
     assert report['checks'][-1]['authoritative_hands']['mainhand']['damage']==1
   if kind=='gameplay':assert report['first_server_exit']==0
   cell[kind]={'path':str(folder.relative_to(root)),'checks':count,'result_sha256':sha(folder/'result.json'),'test_binary_sha256':report['binary_sha256']}
  evidence[f'{client}/{primary}']=cell
public=read(base/'completion770-public-final/result.json');assert public['binary_sha256']==sha(target/'mcsheriffanya');assert public['status']=='pass' and len(public['checks'])==24
for row in public['sessions']:assert row['admitted']==(row.get('client',row.get('protocol')) in [771,772,773,774,776])
official=read(base/'completion770-official777-reference/result.json');assert official['status']=='pass' and len(official['checks'])==7 and official['exit_codes']==[143]
assert not (base/'completion770-official777-reference/server').exists()
changed=subprocess.check_output(['git','diff','--name-only'],cwd=root,text=True).splitlines()
changed+=['tools/protocol-bridge/'+name for name in ['generate_payload_770.py','payload-770-report.json','src/client_770.rs','src/client_770_tests.rs','src/generated_payload_770.rs']]
result={'client_translation_only':True,'no_old_gameplay_profiles':True,'no_new_admission_gates':True,
 'public_admission':{str(p):{str(c):('closed' if c==770 else 'open') for c in [770,771,772,773,774,776]} for p in [775,777]},
 'checks':counts,'evidence':evidence,'fixtures':{str(c):sha(p) for c,p in fixtures.items()},
 'final_binaries':{'test':sha(target/'deps/mcsheriffanya-12cfc85e9e96ee73'),'release':sha(target/'mcsheriffanya')},
 'source_sha256':{p:sha(root/p) for p in changed},
 'notes':['770 reuses771/772 typed helpers with its own locked content/layout/serializer tables.111 item/65 block/1965 state/7 entity/3 particle views; canonical saves and primary gameplay unchanged.',
 '770 passes196 payloads/6 GUI/4 poses,11 gameplay/restart,recipes/Nether return and supplemental End on both primaries. Both public cells remain CLOSED: existing password Configuration dialog barrier cannot be represented by vanilla770 (no Show/Clear Dialog or Custom Click Action). No chat fallback or bypass added.',
 '771/772/773/774/776 retain admission and repeat their unchanged complete checklist; original payload files remain byte-for-byte unchanged.',
 'Nether/End disk persistence, arbitrary custom forms, dynamic hanging-entity orientation changes and exhaustive entity/attribute semantics remain unclaimed. No new gates.',
 'One authorized copied official26.3 native777 reference run, Java25/DEVNULL/26190–26191,7/7,stopped by PID. No implementation sources,JAR unpacking,reports,secrets,commit or push.'],
 'retained_failures':['protocol-first: stale assertions expecting770 codecs to be unavailable; compile-first/second unreachable old refusal arms corrected.',
 'export-first: painting direction metadata absent in770; removed that field while preserving Spawn Object data. Dynamic reorientation unproven.',
 'protocol-fourth: painting test incorrectly used an unpopulated entity tracker; corrected to the contextual wire path.',
 'completion770-live-v3/gameplay775-light-parser-failed: the robot light decoder missed770; fixed without changing core light bytes.',
 'clippy-workspace-first: independent integer min/max if statements needed a newline; no suppression or logic change.',
 'password777-time-wait-failed: observer preflight saw TCP TIME_WAIT; waited for the same port before retrying.',
 'completion770-live: JSON loader still selected774 locks for770; completion770-live-v2: opaque launch allowlist lacked1.21.5. Both corrected before live proof.']}
print(json.dumps(counts),flush=True)
proof_dirs=[p for p in base.glob('completion770-*') if p.is_dir()]
result['artifact_sha256']={str(p.relative_to(root)):sha(p) for folder in proof_dirs for p in sorted(folder.rglob('*')) if p.is_file() and '__pycache__' not in p.parts}
(base/'completion770-verification.json').write_text(json.dumps(result,indent=2)+'\n')
