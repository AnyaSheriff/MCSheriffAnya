"""Итоговые проверки существующего checklist; новые ворота допуска не добавляются."""
import hashlib
import json
from pathlib import Path
import re
import subprocess

here=Path(__file__).resolve().parent
base=here.parent
root=here.parents[4]
target=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target')
def read(path):return json.loads(path.read_text())
def sha(path):return hashlib.sha256(path.read_bytes()).hexdigest()
rust=(here/'rust-final.log').read_text()
rows=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',rust)
assert sum(int(a) for a,b in rows)==3169,rows
assert sum(int(b) for a,b in rows)==94,rows
assert 'warning:' not in rust
assert 'Ran 237 tests' in (here/'python-final.log').read_text() and '\nOK\n' in (here/'python-final.log').read_text()
for name in ['clippy-workspace-final.log','clippy-root-final.log','build-final.log']:
    text=(here/name).read_text()
    assert 'Finished `release`' in text and 'warning:' not in text and 'error:' not in text,name
fixtures={773:base/'payload-live-773.json',774:base.parent/'step1-774-776/payload-live-774-v2.json',776:base.parent/'step1-774-776/payload-live-776.json'}
assert sha(fixtures[774])=='f49c99cd5a0d5e41bf07a95be35ca2c18ddca5e1c5759d80afedd9dce872da41'
assert sha(fixtures[776])=='b315a5d410c666ec1b0bc4b8a38635ec38c765693decc46f15ed82752810bab3'
evidence={}
for client in [773,774,776]:
    for primary in [775,777]:
        cell={}
        for kind,count in [('gui',6),('gameplay',11),('dimensions',5),('password',5)]:
            run=('completion773-live-v4' if kind=='gui' else 'completion773-live-v5') if client==773 else 'completion773-regression'
            folder=base/run/f'{client}-{kind}{primary}'
            report=read(folder/'result.json')
            assert report['status']=='pass' and report['server_exit']==0,folder
            assert report['primary']==primary and report.get('client',report.get('protocol'))==client
            assert len(report['checks'])==count
            if kind=='gui':
                assert report['payload_source_sha256']==sha(fixtures[client])
                assert len(report['payloads'])==196 and all(p['post_pong'] for p in report['payloads'])
                assert report['payload_post_pong'] and len(report['statue_poses'])==4
                if client==773:
                    assert report['attack_equivalent'] and report['checks'][-1]['target_health']==[4.0]
                    assert report['checks'][-1]['authoritative_hands']['mainhand']['damage']==1
            if kind=='gameplay':assert report['first_server_exit']==0
            cell[kind]={'path':str(folder.relative_to(root)),'checks':count,'result_sha256':sha(folder/'result.json'),'test_binary_sha256':report['binary_sha256']}
        evidence[f'{client}/{primary}']=cell
public=read(base/'completion773-public-final/result.json')
assert public['status']=='pass' and len(public['checks'])==24
for row in public['sessions']:
    assert row['admitted']==(row.get('client',row.get('protocol')) in [773,774,776]),row.keys()
official=read(base/'completion773-official777-reference/result.json')
assert official['status']=='pass' and len(official['checks'])==7 and official['exit_codes']==[143]
assert not (base/'completion773-official777-reference/server').exists()
changed=subprocess.check_output(['git','diff','--name-only'],cwd=root,text=True).splitlines()
result={'client_translation_only':True,'no_old_gameplay_profiles':True,'no_new_admission_gates':True,
        'public_admission':{str(p):{str(c):('open' if c in [773,774,776] else 'closed') for c in [770,771,772,773,774,776]} for p in [775,777]},
        'checks':{'rust':3169,'core':2908,'startup':3,'protocol':247,'seedmap':11,'ignored':94,'python':237,'clippy_workspace':'PASS -D warnings','clippy_root':'PASS -D warnings','release':'PASS','public':24,'official_native777_reference':7},
        'evidence':evidence,'fixtures':{str(c):sha(p) for c,p in fixtures.items()},
        'final_binaries':{'test':sha(target/'release/deps/mcsheriffanya-b5a369944de6d5e0'),'release':sha(target/'release/mcsheriffanya')},
        'source_sha256':{p:sha(root/p) for p in changed},
        'notes':['773 has no spear jab: existing sixth GUI attack check uses ordinary sword attack with authoritative cow health10→4 and wear1; 774/776 retain spear jab7.',
                 'Nether/End persistence, arbitrary custom components and exhaustive entity/attribute semantics remain unclaimed; no new gate.',
                 'Private traces retain per-run binary hashes; final full Rust suite and public release check follow the admission switch and reverse adjacent main-hand serializer correction.',
                 'Only one copied official26.3 reference run; this is native777 reference evidence, not773 proof.'],
        'retained_failures':['compile-protocol-first.log: generator lacked baseline item lock path','protocol-first.log/protocol-second.log: old closed-path assertions',
                             'rust-first.log: spawn bypassed explicit entity views','completion773-live-v1: unsupported timeline tag registry',
                             'completion773-live-v2: old dimension fields and packed integer biome colors required',
                             'completion773-live-v3: old player main-hand byte serializer required',
                             'completion773-live-v4/773-gameplay775: robot omitted773 long light masks',
                             'protocol-final.log/protocol-final-v2.log: assertion type mismatch, corrected to slices']}
proof_dirs=[here,*(p for p in base.glob('completion773-*') if p.is_dir() and p!=here)]
result['artifact_sha256']={str(p.relative_to(root)):sha(p) for folder in proof_dirs for p in sorted(folder.rglob('*')) if p.is_file() and '__pycache__' not in p.parts}
(base/'completion773-verification.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps(result['checks']))
