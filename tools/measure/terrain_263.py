#!/usr/bin/env python3
# Только чтение region: дёрн по покрывающему блоку и перепады грунта >=8.
# Запуск из корня репозитория: /usr/bin/python3 tools/measure/terrain_263.py REGION.
# Нужен numpy; высоты исключают деревья/строения, но включают пещерные провалы.
# Частоты разных миров зависят от выборки биомов; причинность проверяют тесты.
import struct, ctypes, ctypes.util
import sys,re,json,collections
from pathlib import Path
sys.path.insert(0,str(Path(__file__).resolve().parent))
import census_263 as c
lib=ctypes.CDLL(ctypes.util.find_library('zstd'));lib.ZSTD_decompress.argtypes=[ctypes.c_void_p,ctypes.c_size_t,ctypes.c_void_p,ctypes.c_size_t];lib.ZSTD_decompress.restype=ctypes.c_size_t

def container(raw,p,n):
 k=raw[p];p+=1
 if k==0:return [0]*n,p
 if k==1:return [int.from_bytes(raw[p:p+2],'big')]*n,p+2
 size=n if k==3 else int.from_bytes(raw[p:p+2],'big');p+=2*(k==2)
 pal=struct.unpack_from('>'+str(size)+'H',raw,p);p+=2*size
 if k==3:return pal,p
 bits=(size-1).bit_length();bits=4 if bits==3 else 8 if 5<=bits<=7 else bits
 out=[];mask=(1<<bits)-1
 for i in range(n):
  bit=i*bits;byte=bit//8
  v=int.from_bytes(raw[p+byte:p+byte+3],'little')>>(bit%8)&mask;out.append(pal[v])
 return out,p+(n*bits+7)//8

def mcs_chunks(path,coords=None):
 for file in sorted(__import__('pathlib').Path(path).glob('*.mcs')):
  _,rx,rz,_=file.name.split('.');data=file.read_bytes()
  for slot in range(1024):
   cx,cz=int(rx)*32+slot%32,int(rz)*32+slot//32
   if coords is not None and (cx,cz) not in coords:continue
   off,size=struct.unpack_from('>II',data,8+slot*8)
   if not size:continue
   rec=data[off*256:off*256+size];buf=ctypes.create_string_buffer(4<<20);n=lib.ZSTD_decompress(buf,len(buf),rec[8:],len(rec)-8);assert n<len(buf)
   raw=buf.raw[:n];p=1;secs=[];bios=[]
   for _ in range(24):s,p=container(raw,p,4096);secs.append(s)
   for _ in range(24):s,p=container(raw,p,64);bios.append(s)
   yield cx,cz,secs,bios

names=['?']*65536
s=Path('src/blocks_table.rs').read_text().split('pub const BLOCK_NAMES:')[1].split('];')[0]
for lo,hi,n in re.findall(r'\((\d+), (\d+), "([^"]+)"\)',s):names[int(lo):int(hi)+1]=[n]*(int(hi)-int(lo)+1)
s=Path('tools/protocol-bridge/src/content.rs').read_text()
for n,lo,hi in re.findall(r'name: "([^"]+)"[^\n]*?first_state: (\d+), last_state: (\d+)',s):names[int(lo):int(hi)+1]=[n]*(int(hi)-int(lo)+1)
reg={'blocks':names,'biomes':['?']*128,'properties':[{}]*65536}
# Биомы MCSA: порядок enum/ALL/PREVIEW берётся из собственной таблицы.
s=Path('src/world/terrain.rs').read_text()
bs=re.findall(r'Biome::(\w+),',s.split('pub const ALL:')[1].split('];')[0])
bs+=re.findall(r'Biome::(\w+),?',s.split('pub const NETHER:')[1].split('];')[0])+re.findall(r'Biome::(\w+),?',s.split('pub const END:')[1].split('];')[0])+re.findall(r'Biome::(\w+),?',s.split('pub const PREVIEW:')[1].split('];')[0])
reg['biomes'][:len(bs)]=[re.sub(r'(?<!^)(?=[A-Z])','_',b).lower() for b in bs]
def roots(path, registry):
 if list(path.glob('*.mcs')):
  for cx,cz,sections,bios in mcs_chunks(path,plan):
   yield {'xPos':cx,'zPos':cz,'sections':[{'Y':sy-4,'palette':names,'ids':ids,'biomes':[registry['biomes'][i] for i in bios[sy]]} for sy,ids in enumerate(sections)]}
 else:
  for file in sorted(path.glob('*.mca')):
   for root in c.chunks(file):
    sections=[]
    for sec in root.get('sections',[]):
     bs=sec.get('block_states')
     if not bs:continue
     pal=[(p.get('id') or p.get('Name') or p.get('name') or p.get('')).removeprefix('minecraft:') if isinstance(p,dict) else p.removeprefix('minecraft:') for p in bs['palette']]
     ids=[0]*4096 if len(pal)==1 else c.unpack(bs['data'],max(4,(len(pal)-1).bit_length()),4096)
     sections.append({'Y':sec['Y'],'palette':pal,'ids':ids,'biomes':[b.removeprefix('minecraft:') for b in (c.biome_cells(sec) or ['?']*64)]})
    yield {'xPos':root['xPos'],'zPos':root['zPos'],'sections':sections}
roots=roots(Path(sys.argv[1]),reg)
import numpy as np
heights={};covered=collections.Counter();grass=0;bios={};examples={}
terrain={'stone','dirt','grass_block','gravel','sand','sandstone','red_sand','red_sandstone','snow_block','powder_snow','ice','packed_ice','blue_ice','clay','mud','podzol','coarse_dirt','mycelium','terracotta','moss_block','pale_moss_block','calcite','granite','diorite','andesite','deepslate','tuff','rooted_dirt','dirt_path'}
plan=set(map(tuple,json.loads(Path(sys.argv[2]).read_text())['chunks'])) if len(sys.argv)>2 else None
for root in roots:
 if plan is not None and (root['xPos'],root['zPos']) not in plan:continue
 cx,cz=root['xPos'],root['zPos']; a=np.full((384,16,16),'air',dtype=object); b={}
 for sec in root['sections']:
  sy=sec['Y']; pal=np.array(sec['palette'],dtype=object); a[(sy+4)*16:(sy+5)*16]=pal[np.array(sec['ids'])].reshape(16,16,16); b[sy]=sec['biomes']
 ys,zs,xs=np.where(a=='grass_block');grass+=len(ys)
 for y,z,x in zip(ys,zs,xs):
  above=a[y+1,z,x] if y<383 else 'air';covered[above]+=1;examples.setdefault(above,[cx*16+int(x),int(y)-64,cz*16+int(z)])
 mask=np.isin(a,list(terrain))
 top=np.where(mask.any(axis=0),383-np.argmax(mask[::-1],axis=0)-64,-65)
 for z in range(16):
  for x in range(16):
   p=(cx*16+x,cz*16+z);h=int(top[z,x]);heights[p]=h; sy=h//16; bio=b.get(sy,['?']*64);bios[p]=bio[((h&15)//4)*16+(z//4)*4+x//4]
byheight=collections.defaultdict(collections.Counter)
counts=collections.Counter(); bybio=collections.defaultdict(collections.Counter); worst=[]
for (x,z),h in heights.items():
 for dx,dz in [(1,0),(0,1)]:
  q=(x+dx,z+dz)
  if q not in heights or h == -65 or heights[q] == -65:continue
  border=(q[0]%16==0 if dx else q[1]%16==0);k='border' if border else 'interior';delta=abs(h-heights[q]);counts[k+'_pairs']+=1;counts[k+'_ge8']+=delta>=8;counts['max']=max(counts['max'],delta)
  bybio[str(bios[x,z])][k+'_pairs']+=1;bybio[str(bios[x,z])][k+'_ge8']+=delta>=8
  band='below70' if h<70 else '70-99' if h<100 else '100-139' if h<140 else '140+'
  bh=str(bios[x,z]).lower()+':'+band
  byheight[bh][k+'_pairs']+=1;byheight[bh][k+'_ge8']+=delta>=8
  if delta>=25:worst.append([delta,x,z,h,heights[q],k,str(bios[x,z])])
out={'chunks':len(heights)//256,'grass':grass,'above':dict(covered.most_common()),'examples':examples,'jumps':dict(counts),'biomes':dict(bybio),'biomes_by_height':dict(byheight),'worst':sorted(worst,reverse=True)[:30]}
# Критерий переписи, а не правило гибели: листья/вода/снег тоже считаются.
plants={'short_grass','tall_grass','fern','large_fern','leaf_litter','wildflowers','pink_petals','dandelion','poppy','cornflower','oxeye_daisy','allium','azure_bluet','bush','firefly_bush','sugar_cane','vine','dead_bush','seagrass','tall_seagrass','brown_mushroom','red_mushroom','rose_bush','peony','lilac','sunflower','lily_of_the_valley','orange_tulip','white_tulip','red_tulip','pink_tulip'}
nonplant={name:n for name,n in covered.items() if name not in plants and name not in {'air','cave_air','void_air'} and not name.endswith('_sapling')}
out['non_air_non_plant_above']={'count':sum(nonplant.values()),'fraction':sum(nonplant.values())/grass if grass else 0,'by_type':dict(sorted(nonplant.items()))}
out['reported_cliff_rows']={str(z):[heights.get((x,z)) for x in range(1108,1129)] for z in range(-485,-471)}
print(json.dumps(out,indent=2))

