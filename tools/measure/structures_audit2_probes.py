#!/usr/bin/env python3
"""Запросы к нашему генератору: ядро окон и оболочка рамок, без сервера."""
import collections
import json
from pathlib import Path
import sys
from structures_audit2 import expected, box_chunks


def main():
    folder = Path(sys.argv[1])
    for dim in ('overworld','the_nether','the_end'):
        starts=json.loads((folder/f'{dim}-starts.json').read_text())
        chunks=expected(); halo=expected(8); probes=collections.defaultdict(set)
        for s in starts:
            if not s['window']: continue
            for p in s.get('piece_records',[]):
                b=p['box']; x,z=(b[0]+b[3])//2,(b[2]+b[5])//2;y=b[1]
                chunks.update(box_chunks(b)&halo)
                ps={(px,y,pz) for px in (b[0],x,b[3]) for pz in (b[2],z,b[5])}
                if p.get('projection')!='terrain_matching':
                    for d in (1,4,8,12,16):
                        ps.update(((b[0]-d,y,z),(b[3]+d,y,z),(x,y,b[2]-d),(x,y,b[5]+d)))
                for px,py,pz in ps:
                    c=(px//16,pz//16)
                    if c in halo: chunks.add(c);probes[f'{c[0]},{c[1]}'].add((px,py,pz))
        (folder/f'{dim}-selection.json').write_text(json.dumps(sorted(chunks)))
        (folder/f'{dim}-probes.json').write_text(json.dumps({c:sorted(ps) for c,ps in probes.items()}))
        print(dim,len(chunks),sum(map(len,probes.values())))

if __name__=='__main__':main()
