# Только чтение region площадок оригинала. JAR и код игры не читаются.
# python3 decoration_plots_263.py REGION SPEC_JSON BLOCKS_JSON_GZ MASKS_JSON
import sys,json,gzip,collections
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import census_263 as c
from camp_templates_263 import pname
region, spec, output, masks_output = sys.argv[1:]
plots=json.load(open(spec))
for f in Path(region).glob('r.*.mca'):
 for root in c.chunks(f):
  for s in root['sections']:
   if s['Y']<12:continue
   bs=s.get('block_states')
   if not bs:continue
   pal=[pname(p)[0] for p in bs['palette']]
   ids=[0]*4096 if len(pal)==1 else c.unpack(bs['data'],max(4,(len(pal)-1).bit_length()),4096)
   for k,pid in enumerate(ids):
    name=pal[pid]
    if name=='air':continue
    x,y,z=root['xPos']*16+(k&15),s['Y']*16+(k>>8),root['zPos']*16+((k>>4)&15)
    for p in plots:
     ox,oy,oz=p['origin']
     if abs(x-ox)<=12 and abs(z-oz)<=12 and y>=199:
      p.setdefault('blocks',[]).append([x-ox,y-oy,z-oz,name]);break
for p in plots:
 bs=p['blocks'];lit=[b for b in bs if b[3]=='leaf_litter']
 print(p['feature'],p['surface'],len(lit),'range',max([max(abs(b[0]),abs(b[2])) for b in lit],default=0),'dy',collections.Counter(b[1] for b in lit),flush=True)
gzip.open(output,'wt').write(json.dumps(plots,separators=(',',':'))+'\n')

masks={}
for p in plots:
 if p['feature'].endswith('_leaf_litter') and p['surface'] in ('grass_block','dirt','stone'):
  masks.setdefault(p['feature'],[]).append(sorted({(b[0],b[2]) for b in p['blocks'] if b[3]=='leaf_litter'}))
Path(masks_output).write_text(json.dumps(masks,separators=(',',':'))+'\n')
