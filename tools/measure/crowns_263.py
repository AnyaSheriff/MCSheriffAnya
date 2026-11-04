"""Срезы крон из region официального мира; файлы открываются только для чтения."""
import sys, json, collections, argparse, gzip
from pathlib import Path
from census_263 import chunks, unpack

def read_world(world):
    blocks = {}
    for path in Path(world).rglob('region/r.*.mca'):
        if 'minecraft/the_' in str(path):
            continue
        for root in chunks(path):
            cx, cz = root['xPos'], root['zPos']
            for sec in root['sections']:
                bs = sec.get('block_states', {})
                pal = bs.get('palette', [])
                pal = [dict(Name=e.split('[')[0], Properties=dict(kv.split('=') for kv in e.split('[')[1].rstrip(']').split(',')) if '[' in e else {}) if isinstance(e,str) else dict(Name=e.get('id',e.get('Name',e.get('', ''))),Properties=e.get('properties',e.get('Properties',{}))) for e in pal]
                if not any(e['Name'].endswith(('_log', '_leaves')) for e in pal):
                    continue
                ids = unpack(bs['data'], max(4, (len(pal)-1).bit_length()), 4096) if len(pal)>1 else [0]*4096
                for i, ident in enumerate(ids):
                    e = pal[ident]; name = e['Name'].removeprefix('minecraft:')
                    if name.endswith(('_log', '_leaves')):
                        blocks[(cx*16+(i&15),sec['Y']*16+(i>>8),cz*16+((i>>4)&15))] = (name,e.get('Properties', {}))
    return blocks

def sample(blocks, x, z):
    ys = sorted(y for (bx,y,bz),(n,p) in blocks.items() if bx==x and bz==z and n.endswith('_log') and p.get('axis','y')=='y')
    if not ys: return None
    bottom=ys[0];top=bottom
    while (x,top+1,z) in blocks and blocks[x,top+1,z][0].endswith('_log'): top+=1
    name=blocks[x,bottom,z][0]
    logs={(bx,y,bz) for (bx,y,bz),(n,p) in blocks.items() if n==name and abs(bx-x)<=8 and abs(bz-z)<=8 and bottom<=y<=top+10}
    leaves={(bx,y,bz):p for (bx,y,bz),(n,p) in blocks.items() if n.endswith('_leaves') and abs(bx-x)<=8 and abs(bz-z)<=8 and bottom<=y<=top+12}
    return {'type':name,'trunk':top-bottom+1,'logs':[[bx-x,y-top,bz-z,blocks[bx,y,bz][1].get('axis')] for bx,y,bz in sorted(logs)],'leaves':[[bx-x,y-top,bz-z,int(p.get('distance',7))] for (bx,y,bz),p in sorted(leaves.items())]}

def summary(samples):
    out={}
    for name in sorted({s['type'] for s in samples}):
        ss=[s for s in samples if s['type']==name];layers=collections.defaultdict(list)
        for s in ss:
            for dy in sorted({v[1] for v in s['leaves']}):
                layer=[v for v in s['leaves'] if v[1]==dy];r=max(max(abs(v[0]),abs(v[2])) for v in layer)
                corners=sum(any(v[0]==x and v[2]==z for v in layer) for x in [-r,r] for z in [-r,r])/4 if r else 0
                layers[dy].append((len(layer),r,corners))
        out[name]={'n':len(ss),'trunk':sum(s['trunk'] for s in ss)/len(ss),'leaves':sum(len(s['leaves']) for s in ss)/len(ss),'branches':sum(sum(v[0]!=0 or v[2]!=0 for v in s['logs']) for s in ss)/len(ss),'distance':dict(sorted(collections.Counter(v[3] for s in ss for v in s['leaves']).items())),'layers':{dy:[round(sum(v[i] for v in vv)/len(vv),3) for i in range(3)] for dy,vv in sorted(layers.items())}}
    return out

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('world',type=Path)
    parser.add_argument('--plots',type=Path,required=True)
    parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args()
    blocks=read_world(args.world)
    plots=json.loads(args.plots.read_text())
    result={}
    for name,plot in plots.items():
        points=plot['points']
        xmin,xmax=min(x for x,z in points)-8,max(x for x,z in points)+8
        zmin,zmax=min(z for x,z in points)-8,max(z for x,z in points)+8
        # Высокие изолированные площадки: естественные деревья под ними не входят в замер.
        subset={p:v for p,v in blocks.items() if xmin<=p[0]<=xmax and zmin<=p[2]<=zmax and p[1]>=plot['base_y']}
        trees=[tree for x,z in points if (tree:=sample(subset,x,z))]
        for tree in trees: tree['type']=name
        result[name]=trees
    opener=gzip.open if args.output.suffix=='.gz' else open
    with opener(args.output,'wt') as stream: json.dump(result,stream,separators=(',',':'))
    print(json.dumps(summary([tree for trees in result.values() for tree in trees]),indent=2))

if __name__=='__main__':main()
