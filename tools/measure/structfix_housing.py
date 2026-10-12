#!/usr/bin/env python3
"""Кровати, рабочие блоки и маркеры деревень в сохранённых рамках."""
import collections,gzip,json,sys
from pathlib import Path
from structures_audit2 import World
JOBS={'composter','smoker','blast_furnace','cartography_table','brewing_stand','barrel','fletching_table','cauldron','lectern','stonecutter','loom','smithing_table','grindstone'}
w=World(sys.argv[1],'overworld')
data=json.loads(gzip.decompress(Path(sys.argv[2]).read_bytes()))
rows=[]
for s in data['starts']:
    if not s['kind'].startswith('village_') or not s['window'] or not s['complete_bounds']:continue
    cells=set();counts=collections.Counter();markers=collections.Counter()
    for p in s['pieces']:
        if '/villagers/' in p['name'] or '/animals/cat_' in p['name'] or p['name'].endswith('/iron_golem'):
            markers[p['name'].split('/')[-1]]+=1
        if not any(k in p['name'] for k in ('/houses/','/town_centers/')):continue
        b=p['box']
        for x in range(b[0],b[3]+1):
            for z in range(b[2],b[5]+1):
                for y in range(b[1],b[4]+1):
                    if (x,y,z) in cells:continue
                    cells.add((x,y,z));name=w.block(x,y,z)
                    if name is None:raise ValueError((x,y,z))
                    if name.endswith('_bed') or name in JOBS:counts[name]+=1
    rows.append(dict(kind=s['kind'],chunk=s['chunk'],bed_blocks=sum(v for k,v in counts.items() if k.endswith('_bed')),workstations={k:v for k,v in counts.items() if k in JOBS},markers=markers))
Path(sys.argv[3]).write_text(json.dumps(rows,indent=2)+'\n')
print(len(rows),'villages')
