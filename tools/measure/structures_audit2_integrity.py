#!/usr/bin/env python3
"""Проверка всех сохранённых секций и длин палитр; мир только читается."""
import sys,collections,json,struct,zlib
from pathlib import Path

from structures_audit2 import World,expected,signed,valid
out={}
for dim in ('overworld','the_nether','the_end'):
 w=World(sys.argv[1],dim)
 errors=[];versions=collections.Counter();sections=collections.Counter()
 for cx,cz in sorted(expected(8),key=lambda p:(p[0]//32,p[1]//32,p[1],p[0])):
  try:
   r=w.read(cx,cz); issue=valid(r,cx,cz)
  except (ValueError,IndexError,struct.error,zlib.error) as e:
   issue=str(e)
  if issue:
   errors.append([cx,cz,issue]);continue
  versions[r.get('DataVersion')]+=1
  for s in r['sections']:
   bs=s.get('block_states')
   if not bs:continue
   p=bs['palette'];bits=max(4,(len(p)-1).bit_length());n=(4096+(64//bits)-1)//(64//bits)
   if len(p)>1 and len(bs.get('data',[]))!=n:errors.append([cx,cz,signed(s['Y']),'invalid block palette length'])
  present={signed(s['Y']) for s in r['sections'] if s.get('block_states')}
  expect=set(range(-4,20)) if dim=='overworld' else set(range(16))
  if not expect<=present:errors.append([cx,cz,'missing sections',sorted(expect-present)])
  sections[len(present)]+=1
 out[dim]={'errors':errors,'DataVersion':versions,'sections':sections}
 print(dim,out[dim],flush=True)
Path(sys.argv[2]).write_text(json.dumps(out,indent=2))
