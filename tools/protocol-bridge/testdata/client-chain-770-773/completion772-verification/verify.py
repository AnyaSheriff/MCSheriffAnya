"""Проверка прежнего checklist772 и регрессий773/774/776; новых ворот нет."""
import hashlib,json,re,subprocess
from pathlib import Path
here=Path(__file__).resolve().parent;base=here.parent;root=here.parents[4]
target=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target/release')
def read(p):return json.loads(p.read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
rust=(here/'rust-final.log').read_text();rows=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',rust)
counts={'rust':sum(int(a) for a,b in rows),'ignored':sum(int(b) for a,b in rows),'python':237,'public':24,'official_native777_reference':7}
assert counts['rust']==3175 and counts['ignored']==96,counts
assert 'warning:' not in rust and 'network::online::tests::login_with_in_memory_session_service ... ok' in rust
python=(here/'python-final.log').read_text();assert 'Ran 237 tests' in python and '\nOK\n' in python
for name in ['clippy-workspace-final.log','clippy-root-final.log','build-final.log']:
    text=(here/name).read_text();assert 'Finished `release`' in text and 'warning:' not in text and 'error:' not in text,name
fixtures={c:base/f'payload-live-{c}.json' for c in [772,773]}
fixtures.update({774:base.parent/'step1-774-776/payload-live-774-v2.json',776:base.parent/'step1-774-776/payload-live-776.json'})
assert [r['label'] for r in read(fixtures[772])['packets']]==[r['label'] for r in read(fixtures[773])['packets']]
old=read(base/'completion773-verification.json')['fixtures']
for c in [773,774,776]:assert sha(fixtures[c])==old[str(c)]
evidence={}
for client in [772,773,774,776]:
    for primary in [775,777]:
        cell={}
        for kind,count in [('gui',6),('gameplay',11),('dimensions',5),('password',5)]:
            run=('completion772-live-final-v2' if kind in ['gui','gameplay'] else 'completion772-live-final-v3') if client==772 else 'completion772-regression'
            folder=base/run/f'{client}-{kind}{primary}';report=read(folder/'result.json')
            assert report['status']=='pass' and report['server_exit']==0,folder
            assert report['primary']==primary and report.get('client',report.get('protocol'))==client
            assert len(report['checks'])==count
            if kind=='gui':
                assert report['payload_source_sha256']==sha(fixtures[client])
                assert len(report['payloads'])==196 and all(p['post_pong'] for p in report['payloads'])
                assert report['payload_post_pong'] and len(report['statue_poses'])==4
                if client in [772,773]:
                    assert report['attack_equivalent'] and report['checks'][-1]['target_health']==[4.0]
                    assert report['checks'][-1]['authoritative_hands']['mainhand']['damage']==1
            if kind=='gameplay':assert report['first_server_exit']==0
            cell[kind]={'path':str(folder.relative_to(root)),'checks':count,'result_sha256':sha(folder/'result.json'),'test_binary_sha256':report['binary_sha256']}
        evidence[f'{client}/{primary}']=cell
public=read(base/'completion772-public-final/result.json');assert public['status']=='pass' and len(public['checks'])==24
for row in public['sessions']:assert row['admitted']==(row.get('client',row.get('protocol')) in [772,773,774,776])
official=read(base/'completion772-official777-reference/result.json');assert official['status']=='pass' and len(official['checks'])==7 and official['exit_codes']==[143]
assert not (base/'completion772-official777-reference/server').exists()
changed=subprocess.check_output(['git','diff','--name-only'],cwd=root,text=True).splitlines()
changed+=['tools/protocol-bridge/'+name for name in ['generate_payload_772.py','payload-772-report.json','src/client_772.rs','src/client_772_tests.rs','src/generated_payload_772.rs']]
result={'client_translation_only':True,'no_old_gameplay_profiles':True,'no_new_admission_gates':True,
        'public_admission':{str(p):{str(c):('open' if c in [772,773,774,776] else 'closed') for c in [770,771,772,773,774,776]} for p in [775,777]},
        'checks':counts,'evidence':evidence,'fixtures':{str(c):sha(p) for c,p in fixtures.items()},
        'final_binaries':{'test':sha(target/'deps/mcsheriffanya-12cfc85e9e96ee73'),'release':sha(target/'mcsheriffanya')},
        'source_sha256':{p:sha(root/p) for p in changed},
        'notes':['772 uses explicit91 item/64 block/1933 state/6 entity/3 particle views; canonical saves and primary gameplay unchanged.',
                 '772/773 have no spear jab: existing sixth GUI action uses sword attack, cow health10→4 and weapon damage1; 774/776 retain jab7.',
                 'Statue block-entity updates repeat the tracked client block state; unknown statue views fail closed.',
                 'End remains supplemental. Nether/End disk persistence, custom component forms and exhaustive entity/attribute semantics remain unclaimed; no new gate.',
                 'One copied official26.3 run is native777 reference proof, not772 proof. Opaque clients/JARs were never unpacked.'],
        'retained_failures':['generator-first/second: missing golden-dandelion/sword view rules, corrected from pinned policies',
                             'compile-protocol-first: invalid widened expression, corrected',
                             'protocol-first/second: stale closed assertions and chain rename input alias, corrected',
                             'completion772-live-v1: old dragon-breath particle power field',
                             'completion772-live-v2/772-gameplay777: teleport chunk barrier timeout; state views now use binary search',
                             'completion772-live-v3: tinted-leaves color accidentally removed; named particle regression added',
                             'completion772-live-v4: removed shelf fallback used wrong packet ID09 instead of08',
                             'completion772-live-v5: old test binary retained09 fallback in live statue update; final binary tracks pose views',
                             'completion772-live-final/772-gui775: local stone prediction caused two-hand retries; authoritative772 use inventory now resynchronized',
                             'completion772-live-final-v2/772-dimensions777: poplar shelf required a second view through missing mangrove shelf; canonical projections now compose',
                             'clippy-first: three missing newlines between independent if statements; corrected without suppressions']}
proof_dirs=[here,*(p for p in base.glob('completion772-*') if p.is_dir() and p!=here)]
print(json.dumps(counts),flush=True)
result['artifact_sha256']={str(p.relative_to(root)):sha(p) for folder in proof_dirs for p in sorted(folder.rglob('*')) if p.is_file() and '__pycache__' not in p.parts}
(base/'completion772-verification.json').write_text(json.dumps(result,indent=2)+'\n')
