import hashlib,json,re,subprocess
from pathlib import Path
root=Path.cwd();base=root/'tools/protocol-bridge/testdata/client-chain-770-773';proof=base/'completion769-verification'
cache=Path('/home/lenox/.local/share/mcsa-java-26.3-probe')
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
def report(folder):
 p=base/folder/'result.json';r=json.loads(p.read_text());assert r['status']=='pass',p
 return r,dict(path=str(p.relative_to(root)),sha256=sha(p),binary_sha256=r.get('binary_sha256'),checks=len(r.get('checks',[])))
def gui(folder):
 r,row=report(folder);assert len(r['payloads'])==196 and len(r['checks'])==6 and len(r['statue_poses'])==4 and r['payload_post_pong'];return row
own=[];regressions=[]
for primary in [775,777]:
 own.append(gui(f'completion769-live-v7/769-gui{primary}'))
 for category,count in [('gameplay',11),('dimensions',5)]:
  r,row=report(f'completion769-live-final/769-{category}{primary}');assert row['checks']==count;own.append(row)
 for category,count in [('password',5),('auth-gui',4)]:
  r,row=report(f'completion769-chat-live/769-{category}{primary}');assert row['checks']==count;own.append(row)
for client in [770,771,772,773,774,776]:
 for primary in [775,777]:
  regressions.append(gui(f'completion769-regression/{client}-gui{primary}'))
  for category,count in [('gameplay',11),('dimensions',5)]:
   r,row=report(f'completion769-regression/{client}-{category}{primary}');assert row['checks']==count;regressions.append(row)
  if client!=770:
   r,row=report(f'completion769-regression-passwords/{client}-password{primary}');assert row['checks']==5;regressions.append(row)
  else:
   for category,count in [('password',5),('auth-gui',4)]:
    r,row=report(f'completion769-chat770-regression/770-{category}{primary}');assert row['checks']==count;regressions.append(row)
assert len(own)==10 and len(regressions)==50
rust=(proof/'rust-final.log').read_text();rows=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',rust)
assert len(rows)==7 and 'FAILED' not in rust
counts=dict(rust=sum(int(a) for a,b in rows),ignored=sum(int(b) for a,b in rows),python=int(re.search(r'Ran (\d+) tests',(proof/'python-final.log').read_text())[1]))
assert counts==dict(rust=3205,ignored=102,python=237),counts
for name in ['clippy-workspace-final.log','clippy-root-final.log','build-final.log']:
 text=(proof/name).read_text();assert 'Finished `release`' in text and 'error:' not in text and 'warning:' not in text,name
binary=cache/'s26-mv5-target/release/mcsheriffanya';binary_hash=sha(binary)
public,public_row=report('completion769-public-final');assert len(public['checks'])==28 and public['binary_sha256']==binary_hash
expected={(p,c) for p in ['26.1.2','26.3'] for c in [769,770,771,772,773,774,776]}
assert {(r['primary'],r['client']) for r in public['sessions'] if r['admitted']}==expected
production=[]
for client in [769,770]:
 for primary in [775,777]:
  r,row=report(f'completion769-production-auth/{client}-password{primary}');assert row['checks']==5 and r['binary_sha256']==binary_hash;production.append(row)
reference,reference_row=report('completion769-official777-reference');assert len(reference['entities'])==43 and reference['server_exit']==143 and reference['ports']==[26190,26191]
fixture=base/'payload-live-769.json';payloads=json.loads(fixture.read_text())['packets'];previous=json.loads((base/'payload-live-770.json').read_text())['packets']
assert len(payloads)==196 and [r['label'] for r in payloads]==[r['label'] for r in previous]
paths=subprocess.check_output(['git','diff','--name-only'],text=True).splitlines()
paths += ['tools/protocol-bridge/'+p for p in ['src/client_769.rs','src/client_769_tests.rs','src/generated_payload_769.rs','generate_payload_769.py','payload-769-report.json']]
paths += [str(fixture.relative_to(root)),str(Path(__file__).relative_to(root))]
source_hashes={p:sha(root/p) for p in sorted(set(paths))}
locks=json.loads((root/'tools/protocol-bridge/client-chain-sources.json').read_text())['versions']['769']['files']
for kind,record in locks.items():
 p=cache/f'client-chain-json/769/{kind}.json';assert sha(p)==record['sha256'] and p.stat().st_size==record['bytes']
for folder in base.glob('completion769-*'):
 if folder.is_dir():assert not any(p.suffix in ['.jar','.mca','.db','.dat'] for p in folder.rglob('*') if p.is_file()),folder
result=dict(status='pass',scope='Client translation only; primary gameplay and canonical saves unchanged; no new admission gates',counts=counts,
 admission=[dict(primary=p,client=c,open=True) for p,c in sorted(expected)],public_checks=28,release_sha256=binary_hash,
 own_sessions=own,regression_sessions=regressions,production_auth=production,official_native777_reference=reference_row,
 sources769=locks,opaque_client=json.loads((cache/'client-1.21.4/provenance.json').read_text()) if (cache/'client-1.21.4/provenance.json').exists() else json.loads((proof/'client-prepare.log').read_text().splitlines()[-1]),
 source_hashes=source_hashes,fixture_sha256=sha(fixture),unchanged_prior_fixtures={str(p.relative_to(root)):sha(p) for p in [base/f'payload-live-{n}.json' for n in [770,771,772,773]]+[base.parent/'step1-774-776'/f'payload-live-{n}.json' for n in ['774-v2','776']]},
 limitations=['Nether/End disk persistence, arbitrary custom forms, dynamic hanging-entity reorientation and exhaustive entity/attribute semantics remain unclaimed; unchanged checklist.',
 'One authorized copied official26.3 reference run,43 samples,Java25/DEVNULL/26190-26191,stopped by owned PID; temporary world removed.',
 'Earlier failures remain in completion769-live-v1/v2/v3/v5/v6 and verification logs; no failure relabeled. Unsupported older clients keep the existing Russian rejection.'])
(base/'completion769-verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(dict(status='pass',counts=counts,new_sessions=len(own),regression_sessions=len(regressions),public=28)))
