#!/usr/bin/env python3
"""FireProtection и шипы на ломающейся броне: настоящие попадания777."""
import argparse
import json
import os
from pathlib import Path

import combat_ci as ci
from combat_wire import observe
from defense_ci import apply_semantic_result

CONFIG=ci.ROOT/'tools/blackbox/ci/protection-edges-777.json'
NAMES=['pvp-fire-protection-control-777','pvp-fire-protection-iv-777',
       'pvp-thorns-control-777','pvp-thorns-break-777']


def registry(raw, side, key, number):
    names=raw['clients'][side]['registry_names']['minecraft:'+key]
    assert isinstance(names,list) and 0<=number<len(names)
    return names[number]


def inspect(raw,items):
    x=observe(raw,NAMES,inventory=True);name=x['name'];fire=name in NAMES[:2]
    assert len(x['attacks'])==1
    incoming=[];reflected=[];flames=[]
    for damage in x['damage']:
        side=damage['side'];kind=registry(raw,side,'damage_type',damage['source'])
        if side==0 and kind=='minecraft:player_attack':
            assert damage['cause']==damage['direct']==x['attacker']+1
            incoming.append(damage)
        elif side==0 and kind=='minecraft:on_fire' and fire:
            assert damage['cause']==damage['direct']==0;flames.append(damage)
        elif side==1 and kind=='minecraft:thorns' and not fire:
            assert damage['cause']==damage['direct']==x['victim']+1;reflected.append(damage)
        else:raise AssertionError('unexpected damage source/recipient '+kind)
    assert len(incoming)==1 and len(x['health'][0])>=2 and x['health'][0][1]<20
    gear=[c for c in x['chest'] if c['stack'] and c['stack']['item']==items['diamond_chestplate']]
    assert gear and gear[0]['tick']<incoming[0]['tick'], 'missing initial chestplate before real hit'
    first=gear[0]['stack'];expected=527 if name==NAMES[3] else 0
    assert first.get('damage',0)==expected
    enchants={registry(raw,0,'enchantment',i):level for i,level in first.get('enchantments',[])}
    wanted=({'minecraft:fire_protection':4} if name==NAMES[1] else {}) if fire else {'minecraft:thorns':7}
    assert enchants==wanted, 'wrong initial armor enchantments'
    cycles=[]
    if fire:
        assert not reflected and x['health'][1]==[20.0]
        assert not any(x['observer_breaks']) and gear[-1]['stack'].get('damage')==1
        expected_hits=1 if name==NAMES[1] else 3
        assert len(flames)==expected_hits and len(x['health'][0])==expected_hits+2, 'actual fire damage missing'
        for side in [0,1]:
            flags=[row for row in x['burning'] if row['side']==side]
            on=next((r['tick'] for r in flags if r['burning']),None)
            assert on is not None
            off=next((r['tick'] for r in flags if not r['burning'] and r['tick']>on),None)
            assert off is not None and not flags[-1]['burning']
            assert not any(r['burning'] and r['tick']>off for r in flags)
            duration=round(off-on);expected_ticks=31 if name==NAMES[1] else 79
            assert abs(duration-expected_ticks)<=1, 'burn duration outside predeclared receipt ticks'
            cycles.append({'side':side,'on':on,'off':off,'duration':duration})
    else:
        assert not flames and len(x['health'][0])==2
        assert len(reflected)<=1 and len(x['health'][1])==1+len(reflected)
        if reflected:assert 15<=x['health'][1][-1]<19, 'reflected damage outside1..5'
        else:assert x['health'][1]==[20.0]
        if name==NAMES[2]:
            assert len(reflected)==1 and gear[-1]['stack'].get('damage')==3
            assert not any(x['observer_breaks'])
        else:
            assert x['health'][0]==[20.0,19.0]
            assert all(len(values)==1 for values in x['observer_breaks'])
            assert any(c['stack'] is None and c['tick']>=incoming[0]['tick'] for c in x['chest'])
    return {'health':x['health'],'initial_gear':first,'chest':x['chest'],
            'incoming':incoming,'reflected':reflected,'fire':flames,'burning':x['burning'],
            'burn_cycles':cycles,'break_ticks':x['observer_breaks']}


def cross_check(rows):
    """Сырые HP/наличие отражения не должны теряться за старыми масками RNG."""
    errors=[]
    for name in NAMES:
        pair=[r for r in rows if r['scenario']==name]
        if len(pair)!=2 or not all(r['status']=='pass' for r in pair):continue
        a,b=[r['observed'] for r in pair]
        if a['health'][0]!=b['health'][0] or len(a['reflected'])!=len(b['reflected']):
            errors.append(name+': actual HP or reflection presence differs')
        if name in NAMES[:2] and a['health'][1]!=b['health'][1]:errors.append(name+': attacker HP differs')
    for side in ['official','ours']:
        pair=[next((r for r in rows if r['scenario']==name and r['side']==side),{}) for name in NAMES[:2]]
        if not all(r.get('status')=='pass' for r in pair):continue
        control,protected=[r['observed'] for r in pair]
        if control['health'][0][1]!=protected['health'][0][1]:errors.append(side+': initial melee HP differs between fire controls')
        ordinary=control['health'][0][1]-control['health'][0][2]
        reduced=protected['health'][0][1]-protected['health'][0][2]
        if not 0<reduced<ordinary:errors.append(side+': FireProtection did not reduce actual fire hit')
        if not all(a['duration']>b['duration'] for a,b in zip(control['burn_cycles'],protected['burn_cycles'])):
            errors.append(side+': FireProtection did not reduce observed burn duration')
    return errors


def semantics(results,data):
    items={r['name']:r['id'] for r in json.loads((data/'items.json').read_text())};rows=[]
    for name in NAMES:
        for side in ['official','ours']:
            try:
                raw=json.loads(ci.checked_file(results/f'{name}-{side}.json').read_text())
                rows.append({'scenario':name,'side':side,'status':'pass','observed':inspect(raw,items)})
            except (AssertionError,EOFError,ValueError,KeyError,OSError,StopIteration) as error:
                rows.append({'scenario':name,'side':side,'status':'failed','error':repr(error)})
    errors=cross_check(rows)+[r['scenario']+'/'+r['side']+':'+r['error'] for r in rows if r['status']!='pass']
    return {'schema':'protection-edges-777-v1','cases':rows,'cross_errors':errors,
            'status':'failed' if errors else 'verified-semantic-controls','error':'; '.join(errors)}


def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('mode',choices=['pin','run'])
    for key in ['output','artifact','work']:parser.add_argument('--'+key,type=Path,required=key=='output')
    parser.add_argument('--java',default='java');a=parser.parse_args()
    if os.environ.get('GITHUB_ACTIONS')!='true':parser.error('CI-only')
    a.output.mkdir(parents=True,exist_ok=True);assert json.loads(CONFIG.read_text())['scenarios']==NAMES
    try:
        if a.mode=='pin':
            pin=ci.resolve_pin(json.loads(ci.PIN.read_text()),os.environ.get('INPUT_BUILD_RUN_ID',''),os.environ.get('INPUT_BUILD_HEAD',''),os.environ.get('INPUT_RUNTIME_SHA256',''))
            selected=ci.preflight(pin,a.output)
            with open(os.environ['GITHUB_OUTPUT'],'a') as f:f.write(f'build_run_id={pin["build_run_id"]}\nartifact_name={selected["artifact_name"]}\n')
            return
        assert a.work and a.artifact
        extras=[CONFIG,Path(__file__),ci.ROOT/'tools/blackbox/test_protection_edges_ci.py',
                ci.ROOT/'.github/workflows/protection-edges-checks.yml',ci.ROOT/'docs/protection-edges-26.3.md']
        result=ci.execute(a.artifact,json.loads((a.output/'build-selection.json').read_text()),a.work.resolve(),a.output.resolve(),a.java,sources_path=CONFIG,extra_sources=extras)
        check=semantics(a.work/'recordings',a.work/'data');apply_semantic_result(result,check)
        ci.write_json(a.output/'protection-edges-semantics.json',check);ci.write_json(a.output/'ci-receipt.json',result)
        ci.collect(a.output,a.work/'recordings',a.work/'data')
        if result['status']!='match':raise SystemExit(1)
    except Exception as error:
        ci.write_json(a.output/'entrypoint-error.json',{'status':'infrastructure-failed','error':repr(error)});raise SystemExit(1)


if __name__=='__main__':main()
