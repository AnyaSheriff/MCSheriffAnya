#!/usr/bin/env python3
"""Read-only raw E2E artifact verifier; no production Host/Java/CI imports."""
from pathlib import Path
import hashlib,json,struct,sys
A=Path(sys.argv[1]);load=lambda p:json.loads(p.read_text())
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def vi(b,i=0):
 n=0
 for shift in range(0,35,7):
  v=b[i];i+=1;n|=(v&127)<<shift
  if v<128:return n,i
 raise AssertionError('bad varint')
def packets(b):
 result=[];i=0
 while i<len(b):
  n,i=vi(b,i);assert 0<n<=len(b)-i;result.append(b[i:i+n]);i+=n
 return result
def text(h):
 b=bytes.fromhex(h);assert b[0]==8 and int.from_bytes(b[1:3],'big')==len(b)-3;return b[3:].decode()
def event(h,uuid):
 f=text(h).split('|');assert len(f)==9 and f[0]=='PLACE' and f[2:]==[uuid,'world','3','64','3','STONE','same=1'];return f[1]
def phases(rows,order,reference):
 assert len(rows)==len(order)*8
 state={'cancelled':False,'can_build':True}
 assert len({r['id']for r in rows})==1
 for row in rows:
  assert row['event']=='place' and row['ok'] and row['error'] is None
  assert row['cancelled']==state['cancelled'] and row['can_build']==state['can_build']
  if row['stage']=='phase':state=row['returned'];assert set(state)=={'cancelled','can_build'}
  if row['stage']=='finish':assert state==reference['final']
 for letter in order:
  stages=[r for r in rows if r['plugin']=='java-place-staged-'+letter]
  assert [r['stage']for r in stages]==['begin']+['phase']*6+['finish']
  assert [r['phase']for r in stages if r['stage']=='phase']==list(range(6))
  for x,y in zip(stages,stages[1:]):
   for key in ['fuel','guest_ns','native_epochs','native_ns']:assert y[key]<=x[key],(key,x,y)
 return state
receipt=load(A/'receipt.json');assert receipt['status']=='success'
checks=0
for f,v in receipt['files'].items():
 p=A/f;assert p.stat().st_size==v['bytes'] and sha(p)==v['sha256'],f;checks+=1
F=A/'source/tools/plugin-translator/fixtures';spec=load(F/'place-staged-expected.json');refs={r['name']:r for r in spec['cohorts']};old=load(F/'place-old-expected.json');oldrefs={r['name']:r for r in old['cohorts']};matrix=load(F/'place-staged-cases.json')
for f,h in load(F/'place-staged-frozen.json')['files'].items():assert sha(A/'source'/f)==h
assert len(refs)==20 and len(matrix['native'])==14 and len(oldrefs)==12
for f in A.glob('*.status.json'):assert load(f)['exit_code']==0 and not load(f)['timeout'],f
statuses=list(A.glob('*.status.json'));assert len(statuses)==83
runtime_statuses=[]
for p in statuses:
 status=load(p)
 if '--ignored' in status['command']:
  runtime_statuses.append(p)
  log=p.with_name(p.name.removesuffix('.status.json')+'.log').read_text()
  assert 'running 1 test' in log and 'test result: ok. 1 passed; 0 failed; 0 ignored;' in log,p
assert len(runtime_statuses)==72
for name in ['rust-target','rust-examples','tests-default','tests-no-default','tests-seedmap-default','tests-protocol-doc','integration-default-compile','integration-default']:
 assert (A/(name+'.status.json')).is_file()
for name in ['rust-examples','tests-no-default','tests-seedmap-default','tests-protocol-doc']:
 assert 'Finished `release`' in (A/(name+'.log')).read_text()
assert 'test result: ok. 3454 passed; 0 failed; 124 ignored;' in (A/'tests-default.log').read_text()
assert '3 passed; 0 failed;' in (A/'integration-default.log').read_text()
retained=load(A/'retained-runtime.json');assert retained['source']=='3d86b705e425d794008945cb8d647757a0ac21d3' and retained['historical_status']=='failure' and retained['default_strict_exit']==0 and retained['protocol_default_passed']==351 and not retained['core_rebuilt']
assert retained['test_elf']=='b5c46d45b52b6f0a2f8ea6cc24c8fbd002cb8d553e6cbacd9762170d3bfa33c9' and retained['server_elf']=='f522a9fc3ea62305127b007d0902356eec5184ee29975e1fcc1ba57ac7ade146'
assert load(A/'default-integration.json')['source_unmodified'] and load(A/'default-integration.json')['tests']==3
assert sha(A/'runtime-elf/mcsheriffanya')==retained['server_elf']
for path in runtime_statuses:
 command=load(path)['command'];assert command[0].endswith('/target/release/deps/mcsheriffanya-628cb65e72047376') and 'cargo'not in command
examples=load(A/'rust-example-inputs.json');assert len(examples['wasm'])==6
for name,v in examples['wasm'].items():assert sha(A/'rust-examples'/(name+'.wasm'))==v['sha256']
assert load(A/'no-default-runtime.json')['compiled_once']
for name,count in [('new-fixtures',3),('new-fault',1),('new-invalid',2)]:
 s=(A/(name+'.log')).read_text();assert s.count('INFO: Running TeaVM')==count and s.count('INFO: Output file successfully built')==count
# Artifact provenance receives separate full OLD Git proof in external audit; no equality with current compiler inputs.
for label,expected_count in [('break-staged',9),('break-baseline',2),('place-baseline',2),('old-five',5)]:
 folder=A/'old-inputs'/label;p=load(folder/'provenance.json');assert p['new_sdk_inputs_equal_old']==False and p['old_retranslated']==False
 assert len(p['old_source_files'])>100
 installed=load(folder/'installed.json');assert sum(n.endswith('/plugin.wasm')for n in installed)==expected_count
 if label=='old-five':assert p['actual_compile_source']=='e9cf7cdd67b4f2f9b883d5eab54343ac5fdc97dd' and p['producer_status']=='failure'
 if label=='break-staged':assert p['producer_status']=='failure'
case_rows=[]
for legacy,reference,m in [(False,refs,matrix),(True,oldrefs,{'host':list(oldrefs),'native':list(oldrefs)[:6]+['offhand-cancel','offhand-uncancel','creative-order','creative-cancel']})]:
 suite='place-old-baseline'if legacy else'place-staged';prefix='legacy-'if legacy else'new-'
 for kind in ['host','native']:
  for name in m[kind]:
   row=load(A/'runtime'/suite/'capture'/f'{kind}-{name}.json');assert row['id']==kind+':'+name and row['legacy']==legacy
   status=load(A/(prefix+kind+'-'+name+'.status.json'));assert status['exit_code']==0 and not status['timeout']
   log=(A/(prefix+kind+'-'+name+'.log')).read_text();assert '\nrunning 1 test\n'in log and '1 passed; 0 failed;'in log
   printed=[json.loads(s.partition('PLACE_STAGED ')[2])for s in log.splitlines()if'PLACE_STAGED {'in s];assert printed==[row]
   assert row['actual_worker'] and row['ready_java_view'] and row['event_hand_item_against_replaced']=='UNSUPPORTED' and not row['sdk_canBuild_is_native_permission']
   r=reference[row['cohort']];oracle=r['source_predicted_legacy'if legacy else'public_contract'];trace=[event(h,row['uuid'])for h in row['raw_message_nbt_hex']]
   wanted=[x.removeprefix('trace ')for x in oracle['trace']];assert trace==wanted and row['verdict']==oracle['verdict']
   if legacy:assert row['phase_boundaries']==[]
   else:phases(row['phase_boundaries'],r['load_order'],oracle)
   assert row['geometry']['collision_free_asserted'] and row['geometry']['target_explicit_air'] and row['before_cells'][1]==0
   before=row['before_stacks'];after=row['after_stacks'];assert len(before['slots'])==len(after['slots'])>=46
   if kind=='native':
    b=bytes.fromhex(row['native_input_hex']);hand,i=vi(b);assert hand==int(name.startswith('offhand-'))==row['native_hand']
    assert b[i:i+8]==((3<<38)|(3<<12)|63).to_bytes(8,'big');i+=8
    face,i=vi(b,i);assert face==1 and struct.unpack('>fff',b[i:i+12])==(.5,.5,.5);i+=12;assert b[i]==0;i+=1
    seq,i=vi(b,i);assert seq==41 and i==len(b)
    acks=[];updates=[]
    for packet in packets(bytes.fromhex(row['wire_hex'])):
     ident,j=vi(packet)
     if ident==4:val,end=vi(packet,j);assert end==len(packet);acks.append(val)
     if ident==8:pos=int.from_bytes(packet[j:j+8],'big');value,end=vi(packet,j+8);assert end==len(packet);updates.append([pos,value])
    assert acks==[41] and row['reaction']['acknowledge']==41
    if row['verdict']=='deny':
     assert before==after and row['before_cells']==row['after_cells']
     for y,state in zip([63,64],row['after_cells']):assert [(3<<38)|(3<<12)|y,state]in updates
    else:
     wanted=json.loads(json.dumps(before));wanted['slots'][45 if hand else 36]['count']=2 if name.startswith('creative-')else 1
     assert after==wanted and row['after_cells']==[row['before_cells'][0]]*2
   else:assert before==after and row['before_cells']==row['after_cells']
   case_rows.append(row)
assert len(case_rows)==56 and len({r['pid']for r in case_rows})==56
host=load(A/'runtime/place-staged/host-contract.json');assert len(host)==32
for row in host:
 r=refs[row['case']];legacy=row['legacy'];oracle=r['public_contract'if legacy is None else'source_predicted_legacy']
 actual=[event(h,'01010101-0101-0101-0101-010101010101')for h in row['raw_nbt']]
 wanted=[s.rsplit(' b',1)[0]if legacy==s[0].lower()else s for s in oracle['trace']];assert actual==wanted
 if legacy is None:
  phases(row['boundaries'],r['load_order'],oracle)
  assert [event(h,'01010101-0101-0101-0101-010101010101')for h in row['cross_place_raw_nbt']]==wanted
  ids=[]
  for x in row['cross_boundaries']:
   if x['id']not in ids:ids.append(x['id'])
  assert ids==[1,2,3,4]
  assert {x['event']for x in row['cross_boundaries']if x['id']in[1,3]}=={'place'}
  assert all('event'not in x for x in row['cross_boundaries']if x['id']in[2,4])
 else:assert row['boundaries']==[] and row['cross_boundaries']==[] and row['cross_place_raw_nbt']==[]
faults=load(A/'runtime/place-staged/fault-capture.json');assert len(faults)==24
for row in faults:
 assert row['cancelled']==(row['mode']in['cancel','uncancelonly']) and row['can_build']==(row['mode']in['order','cancel'])
 bad=[x for x in row['boundaries']if x['plugin']=='java-place-staged-fault'and not x['ok']];assert len(bad)==1
 for suffix in ['a','empty']:
  end=[x for x in row['boundaries']if x['plugin']=='java-place-staged-'+suffix and x['stage']=='finish'];assert len(end)==1 and end[0]['ok']and end[0]['cancelled']==row['cancelled']and end[0]['can_build']==row['can_build']
 if row['fault']in['fuel','guest','native']:assert 1<=len([x for x in row['boundaries']if x['plugin']=='java-place-staged-fault'and x['stage']=='phase'and x['ok']])<6
for p in (A/'runtime/place-staged/plugins').glob('*/plugin.wasm'):assert p.read_bytes()[:8]==b'\0asm\r\0\1\0'
wasms=list((A/'runtime/place-staged/plugins').glob('*/plugin.wasm'));assert len(wasms)==6
assert len(list((A/'sdk-inputs').glob('*.jar')))==6
assert len(load(A/'toolpins.json'))==3 and all(v['exit_code']==0 for v in load(A/'toolpins.json').values())
# Actual current test ELF and built server are retained independently of source hashes.
assert list((A/'test-elf').glob('mcsheriffanya-*')) and (A/'runtime-elf/mcsheriffanya').read_bytes()[:4]==b'\x7fELF'
print(json.dumps({'pass':True,'receipt_checks':checks,'fresh_worker_cases':56,'fresh_native':24,'new_host':20,'new_native':14,'old_host':12,'old_native':10,'direct_host_contract':32,'faults':24,'new_actual_component_translations':6,'rust_example_components_built_once':6,'same_retained_default_test_elf_all_72':True,'no_full_permission_context_or_client_rollback_claim':True}))
