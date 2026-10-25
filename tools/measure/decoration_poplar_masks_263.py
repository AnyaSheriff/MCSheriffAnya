# Естественные тополя: наблюдённые маски опада у изолированных стволов.
# Только чтение region, никакого чтения кода игры или конфигураций worldgen.
# python3 decoration_poplar_masks_263.py REGION OUTPUT_JSON
import sys,json,collections
from pathlib import Path
import census_263 as c
from camp_templates_263 import pname
blocks={}
chunks=set()
for path in Path(sys.argv[1]).glob('r.*.mca'):
 rx,rz=map(int,path.name.split('.')[1:3])
 if rx not in (-1,0) or rz!=0:continue
 for root in c.chunks(path):
  if not -10<=root['xPos']<=10 or not 10<=root['zPos']<=32:continue
  chunks.add((root['xPos'],root['zPos']))
  for sec in root['sections']:
   bs=sec.get('block_states')
   if not bs or not 2<=sec['Y']<12:continue
   pal=[pname(p)[0] for p in bs['palette']]
   ids=[0]*4096 if len(pal)==1 else c.unpack(bs['data'],max(4,(len(pal)-1).bit_length()),4096)
   for k,pid in enumerate(ids):
    x,y,z=root['xPos']*16+(k&15),sec['Y']*16+(k>>8),root['zPos']*16+((k>>4)&15)
    if pal[pid] in ('poplar_log','leaf_litter','grass_block','dirt','coarse_dirt','podzol','rooted_dirt'): blocks[x,y,z]=pal[pid]
columns=collections.defaultdict(set)
for (x,y,z),name in blocks.items():
 if name=='poplar_log':columns[x,z].add(y)
bases=[]
for (x,z),ys in columns.items():
 for y in sorted(ys):
  if all(y+d in ys for d in range(6)) and blocks.get((x,y-1,z)) in ('dirt','grass_block','coarse_dirt','podzol','rooted_dirt'):
   bases.append((x,y,z));break
samples=[]
for x,y,z in bases:
 if any((a-x)**2+(b-z)**2<100 for a,_,b in bases if (a,b)!=(x,z)):continue
 mask=[]
 valid=True
 for dx in range(-5,6):
  for dz in range(-5,6):
   if ((x+dx)//16,(z+dz)//16) not in chunks:valid=False
   if any(blocks.get((x+dx,y+dy,z+dz))=='leaf_litter' for dy in range(-4,5)):
    mask.append((dx,dz))
 if valid:samples.append({'origin':[x,y,z],'mask':mask})
Path(sys.argv[2]).write_text(json.dumps({'bases':len(bases),'isolation_horizontal':10,'samples':samples},indent=2)+'\n')
print('bases',len(bases),'isolated',len(samples),'counts',collections.Counter(len(s['mask']) for s in samples))
