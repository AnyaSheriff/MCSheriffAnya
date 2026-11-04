"""Проверка прежнего checklist771 и регрессий772/773/774/776; новых ворот нет."""
import hashlib,json,re,subprocess
from pathlib import Path
here=Path(__file__).resolve().parent;base=here.parent;root=here.parents[4]
target=Path('/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target/release')
def read(p):return json.loads(p.read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
rust=(here/'rust-final.log').read_text();rows=re.findall(r'test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;',rust)
counts={'rust':sum(int(a) for a,b in rows),'ignored':sum(int(b) for a,b in rows),'python':237,'public':24,'shared_online_login':True,'official_native777_reference':7}
assert counts['rust']==3183 and counts['ignored']==98,counts
assert 'warning:' not in rust and 'network::online::tests::login_with_in_memory_session_service ... ok' in rust
python=(here/'python-final.log').read_text();assert 'Ran 237 tests' in python and '\nOK\n' in python
for name in ['clippy-workspace-final.log','clippy-root-final.log','build-final.log']:
    text=(here/name).read_text();assert 'Finished `release`' in text and 'warning:' not in text and 'error:' not in text,name
fixtures={c:base/f'payload-live-{c}.json' for c in [771,772,773]}
fixtures.update({774:base.parent/'step1-774-776/payload-live-774-v2.json',776:base.parent/'step1-774-776/payload-live-776.json'})
assert [r['label'] for r in read(fixtures[771])['packets']]==[r['label'] for r in read(fixtures[773])['packets']]
old=read(base/'completion772-verification.json')['fixtures']
for c in [772,773,774,776]:assert sha(fixtures[c])==old[str(c)]
evidence={}
for client in [771,772,773,774,776]:
    for primary in [775,777]:
        cell={}
        for kind,count in [('gui',6),('gameplay',11),('dimensions',5),('password',5)]:
            run='completion771-live-final' if client==771 else 'completion771-regression'
            folder=base/run/f'{client}-{kind}{primary}';report=read(folder/'result.json')
            assert report['status']=='pass' and report['server_exit']==0,folder
            assert report['primary']==primary and report.get('client',report.get('protocol'))==client
            assert len(report['checks'])==count
            if kind=='gui':
                assert report['payload_source_sha256']==sha(fixtures[client])
                assert len(report['payloads'])==196 and all(p['post_pong'] for p in report['payloads'])
                assert report['payload_post_pong'] and len(report['statue_poses'])==4
                if client in [771,772,773]:
                    assert report['attack_equivalent'] and report['checks'][-1]['target_health']==[4.0]
                    assert report['checks'][-1]['authoritative_hands']['mainhand']['damage']==1
            if kind=='gameplay':assert report['first_server_exit']==0
            cell[kind]={'path':str(folder.relative_to(root)),'checks':count,'result_sha256':sha(folder/'result.json'),'test_binary_sha256':report['binary_sha256']}
        evidence[f'{client}/{primary}']=cell
public=read(base/'completion771-public-final/result.json');assert public['status']=='pass' and len(public['checks'])==24
for row in public['sessions']:assert row['admitted']==(row.get('client',row.get('protocol')) in [771,772,773,774,776])
official=read(base/'completion771-official777-reference/result.json');assert official['status']=='pass' and len(official['checks'])==7 and official['exit_codes']==[143]
assert not (base/'completion771-official777-reference/server').exists()
changed=subprocess.check_output(['git','diff','--name-only'],cwd=root,text=True).splitlines()
changed+=['tools/protocol-bridge/'+name for name in ['generate_payload_771.py','payload-771-report.json','src/client_771.rs','src/client_771_tests.rs','src/generated_payload_771.rs']]
result={'client_translation_only':True,'no_old_gameplay_profiles':True,'no_new_admission_gates':True,
        'public_admission':{str(p):{str(c):('open' if c in [771,772,773,774,776] else 'closed') for c in [770,771,772,773,774,776]} for p in [775,777]},
        'checks':counts,'evidence':evidence,'fixtures':{str(c):sha(p) for c,p in fixtures.items()},
        'final_binaries':{'test':sha(target/'deps/mcsheriffanya-12cfc85e9e96ee73'),'release':sha(target/'mcsheriffanya')},
        'source_sha256':{p:sha(root/p) for p in changed},
         'notes':['771 reuses the772 typed path with separate locked content/layout/serializer tables.92 item/64 block/1933 state/6 entity/3 particle views; canonical saves and primary gameplay unchanged.',
                 '771 uses the same sword attack equivalent as772/773; 774/776 keep jab7. No checklist gate added.',
                 'Nether/End disk persistence, custom component forms and exhaustive entity/attribute semantics remain unclaimed.',
                 'Existing pinned observations reused; one authorized copied official26.3 native777 reference run, Java25/DEVNULL/26190–26191, stopped by PID. No implementation sources or JAR unpacking.'],
        'retained_failures':['protocol-first: stale771 attribute refusal assertion; corrected for the new771 codec.',
                             'compile-first: generated771 block-entity view constant initially unused; production now selects that table.',
                             'completion771-live/771-gui775: chunk wrapper still rejected771; added771 to the existing772 chunk path.']}
proof_dirs=[here,*(p for p in base.glob('completion771-*') if p.is_dir() and p!=here)]
print(json.dumps(counts),flush=True)
result['artifact_sha256']={str(p.relative_to(root)):sha(p) for folder in proof_dirs for p in sorted(folder.rglob('*')) if p.is_file() and '__pycache__' not in p.parts}
(base/'completion771-verification.json').write_text(json.dumps(result,indent=2)+'\n')
