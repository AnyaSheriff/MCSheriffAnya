from pathlib import Path
import argparse,copy,json
from priority_trace import compare
r=Path(__file__).resolve().parent
spec=json.loads((r/'fixtures/priority-expected.json').read_text())
parser=argparse.ArgumentParser(description='Проверить настоящую legacy capture и 10 повреждений')
parser.add_argument('capture',type=Path)
args=parser.parse_args()
capture=json.loads(args.capture.read_text())
rows=capture['cohorts']
def raw(trace):
    return [(bytes([8])+len(text.encode()).to_bytes(2,'big')+text.encode()).hex() for text in trace]
assert raw(['trace A0 c0>0'])==['08000d74726163652041302063303e30']
native=copy.deepcopy(rows)
for row in native:row['raw_message_nbt_hex']=raw(row['trace'])
assert compare(rows,spec,require_raw=True)==[]
assert compare(native,spec,require_raw=True)==[]
assert len(compare(native,spec,True,True))==6
ideal=[]
for target in spec['cohorts']:
    value={'name':target['name'],'load_order':target['load_order'],'mode':target['mode'],**copy.deepcopy(target['public_contract'])}
    value['raw_message_nbt_hex']=raw(value['trace']);ideal.append(value)
assert compare(ideal,spec,True,True)==[]
cases=[]
v=copy.deepcopy(native);v.pop();cases.append(('missing cohort',v))
v=copy.deepcopy(native);v[1]=copy.deepcopy(v[0]);cases.append(('duplicate cohort',v))
v=copy.deepcopy(native);v[0]['trace'].remove('trace A5 c0');v[0]['raw_message_nbt_hex']=raw(v[0]['trace']);cases.append(('missing monitor',v))
v=copy.deepcopy(native);v[0]['trace'][1:3]=reversed(v[0]['trace'][1:3]);v[0]['raw_message_nbt_hex']=raw(v[0]['trace']);cases.append(('reordered handlers',v))
v=copy.deepcopy(native);v[1]['trace'][-1]='trace A5 c0';v[1]['raw_message_nbt_hex']=raw(v[1]['trace']);cases.append(('wrong cancellation',v))
v=copy.deepcopy(native);v[0]['trace']=[x for x in v[0]['trace'] if not x.startswith('trace B')];v[0]['raw_message_nbt_hex']=raw(v[0]['trace']);cases.append(('missing B',v))
v=copy.deepcopy(native);v[2]['verdict']='allow';cases.append(('wrong verdict',v))
v=copy.deepcopy(native);v[0]['raw_message_nbt_hex'][0]='00'+v[0]['raw_message_nbt_hex'][0][2:];cases.append(('corrupt raw',v))
v=copy.deepcopy(native);del v[0]['raw_message_nbt_hex'];cases.append(('missing raw',v))
v=copy.deepcopy(native);v[0]['trace'].append('trace A5 c0');v[0]['raw_message_nbt_hex']=raw(v[0]['trace']);cases.append(('extra handler',v))
for name,data in cases:
    try:rejected=bool(compare(data,spec,require_raw=True))
    except ValueError:rejected=True
    assert rejected,name
result={'positive_legacy_checked':True,'positive_ideal_public_is_synthetic':True,'actual_native_capture_used':bool(capture.get('wasm_rust_runtime_executed',False)),'negative_controls_rejected':len(cases),'cases':[name for name,_ in cases]}

print(json.dumps(result,ensure_ascii=False))
