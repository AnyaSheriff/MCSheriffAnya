#!/usr/bin/env python3
"""Маркеры жителей и добычи относительно домов: сохранённый мир, не шаблоны jar."""
import collections
import gzip
import json
import sys
from pathlib import Path

TURNS={'NONE':0,'CLOCKWISE_90':1,'CLOCKWISE_180':2,'COUNTERCLOCKWISE_90':3}
def local(p, xyz):
    x,y,z=[a-b for a,b in zip(xyz,p['origin'])]
    t=TURNS[p['rotation']]
    return [(x,z),(z,-x),(-x,-z),(-z,x)][t][0],y,[(x,z),(z,-x),(-x,-z),(-z,x)][t][1]

def contains(p,xyz):
    return all(p['box'][i]<=xyz[i]<=p['box'][i+3] for i in range(3))

def measure(data):
    houses=collections.defaultdict(list); loot=collections.defaultdict(set); omitted=[]; counts=collections.Counter()
    for s in data['starts']:
        if not s['kind'].startswith('village_') or not s['complete_bounds']:continue
        ps=[p for p in s['pieces'] if p['projection']=='rigid' and any(k in p['name'] for k in ('/houses/','/town_centers/'))]
        groups={id(p):[] for p in ps}
        for q in s['pieces']:
            name=q['name']
            if not ('/villagers/' in name or name.endswith('/iron_golem') or '/animals/cat_' in name):continue
            xyz=q['origin']; candidates=[p for p in ps if contains(p,xyz)]
            if not candidates:
                omitted.append(dict(chunk=s['chunk'],name=name,origin=xyz));continue
            p=min(candidates,key=lambda p:(p['box'][3]-p['box'][0]+1)*(p['box'][5]-p['box'][2]+1)*(p['box'][4]-p['box'][1]+1))
            kind=name.split('/')[-1]
            groups[id(p)].append((*local(p,xyz),kind));counts[kind]+=1
        for p in ps:
            key=p['name'].removeprefix('minecraft:village/')
            houses[key].append(tuple(sorted(groups[id(p)])))
            for b in s['block_entities']:
                if '/village/' in b.get('LootTable','') and contains(p,[b['x'],b['y'],b['z']]):
                    loot[key].add((*local(p,[b['x'],b['y'],b['z']]),b['LootTable'].split('/')[-1]))
    return dict(houses={k:sorted(v) for k,v in sorted(houses.items())},loot={k:sorted(v) for k,v in sorted(loot.items())}),dict(markers=counts,unattached=omitted)

if __name__=='__main__':
    data=json.loads(gzip.decompress(Path(sys.argv[1]).read_bytes()))
    asset,report=measure(data)
    Path(sys.argv[2]).write_text(json.dumps(asset,separators=(',',':'))+'\n')
    Path(sys.argv[3]).write_text(json.dumps(report,indent=2)+'\n')
    print('houses',len(asset['houses']),'loot houses',len(asset['loot']),'unattached',len(report['unattached']),report['markers'])
