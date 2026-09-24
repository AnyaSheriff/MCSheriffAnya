# Сколько растений на чанк в каждом биоме — по сохранённому миру оригинала
# (region-файлы). Читаются только данные мира; jar и код игры не трогаются.
#
# python3 tools/measure/plants_census.py <папка region> [--tsv] [биом ...]
# --tsv: все растения построчно «биом<TAB>чанков<TAB>растение<TAB>на чанк».
import sys, struct, zlib, glob, collections, math

def nbt(buf, pos, tag):
    if tag==1: return struct.unpack_from('>b',buf,pos)[0], pos+1
    if tag==2: return struct.unpack_from('>h',buf,pos)[0], pos+2
    if tag==3: return struct.unpack_from('>i',buf,pos)[0], pos+4
    if tag==4: return struct.unpack_from('>q',buf,pos)[0], pos+8
    if tag==5: return struct.unpack_from('>f',buf,pos)[0], pos+4
    if tag==6: return struct.unpack_from('>d',buf,pos)[0], pos+8
    if tag==7:
        n=struct.unpack_from('>i',buf,pos)[0]; return buf[pos+4:pos+4+n], pos+4+n
    if tag==8:
        n=struct.unpack_from('>H',buf,pos)[0]; return buf[pos+2:pos+2+n].decode('utf8','replace'), pos+2+n
    if tag==9:
        t=buf[pos]; n=struct.unpack_from('>i',buf,pos+1)[0]; pos+=5; out=[]
        for _ in range(n):
            v,pos=nbt(buf,pos,t); out.append(v)
        return out,pos
    if tag==10:
        d={}
        while True:
            t=buf[pos]; pos+=1
            if t==0: return d,pos
            n=struct.unpack_from('>H',buf,pos)[0]; name=buf[pos+2:pos+2+n].decode('utf8','replace'); pos+=2+n
            v,pos=nbt(buf,pos,t); d[name]=v
    if tag==11:
        n=struct.unpack_from('>i',buf,pos)[0]; return list(struct.unpack_from('>%di'%n,buf,pos+4)), pos+4+4*n
    if tag==12:
        n=struct.unpack_from('>i',buf,pos)[0]; return list(struct.unpack_from('>%dq'%n,buf,pos+4)), pos+4+8*n
    raise Exception('tag %d'%tag)

def unpack(longs, bits, count):
    per=64//bits; mask=(1<<bits)-1; out=[]
    for i in range(count):
        l=longs[i//per] & 0xFFFFFFFFFFFFFFFF
        out.append((l>>((i%per)*bits))&mask)
    return out

# Что считать; у двухблочных — только нижнюю половину.
WATCH = {'sunflower','lilac','rose_bush','peony','tall_grass','large_fern','pumpkin','melon','sugar_cane',
         'cactus','sweet_berry_bush','lily_pad','dandelion','poppy','cornflower','azure_bluet','oxeye_daisy',
         'allium','red_tulip','orange_tulip','white_tulip','pink_tulip','lily_of_the_valley','short_grass','fern',
         'pink_petals','wildflowers','dead_bush','bush','firefly_bush','leaf_litter','short_dry_grass','tall_dry_grass',
         'blue_orchid','brown_mushroom','red_mushroom','seagrass','tall_seagrass','kelp','cactus_flower'}

def census(f):
    per_biome=collections.defaultdict(collections.Counter)
    chunks=collections.Counter()
    data=open(f,'rb').read()
    if len(data)<8192: return chunks, per_biome
    for i in range(1024):
        off=int.from_bytes(data[i*4:i*4+3],'big')
        if off==0: continue
        p=off*4096; ln=struct.unpack_from('>i',data,p)[0]
        if data[p+4]!=2: continue
        raw=zlib.decompress(data[p+5:p+4+ln]); n=struct.unpack_from('>H',raw,1)[0]
        root,_=nbt(raw,3+n,10)
        if root.get('Status') not in ('minecraft:full','full'): continue
        hm=root.get('Heightmaps',{}).get('WORLD_SURFACE')
        if not hm: continue
        top=unpack(hm,9,256)[8*16+8]-64-1
        sections={s['Y']:s for s in root.get('sections',[])}
        s=sections.get(top//16)
        if not s or 'biomes' not in s: continue
        b=s['biomes']; pal=b['palette']
        ids=[0]*64 if len(pal)==1 else unpack(b['data'],max(1,math.ceil(math.log2(len(pal)))),64)
        cell=((top%16)//4)*16+2*4+2
        biome=pal[ids[cell]].replace('minecraft:','')
        if wanted and biome not in wanted: continue
        chunks[biome]+=1
        for Y,s in sections.items():
            if Y<2 or Y>12 or 'block_states' not in s: continue
            bs=s['block_states']; pal=bs['palette']
            names=[e['Name'].replace('minecraft:','') for e in pal]
            if not any(nm in WATCH for nm in names): continue
            lower=[e.get('Properties',{}).get('half','lower')=='lower' for e in pal]
            ids=[0]*4096 if len(pal)==1 else unpack(bs['data'],max(4,math.ceil(math.log2(len(pal)))),4096)
            for v in ids:
                nm=names[v]
                if nm in WATCH and lower[v]: per_biome[biome][nm]+=1
    return chunks, per_biome

reg=sys.argv[1]
tsv='--tsv' in sys.argv[2:]
wanted=set(a for a in sys.argv[2:] if a!='--tsv')
if __name__=='__main__':
    import multiprocessing
    per_biome=collections.defaultdict(collections.Counter)
    chunks=collections.Counter()
    with multiprocessing.Pool() as pool:
        for c,pb in pool.imap_unordered(census, glob.glob(reg+'/*.mca')):
            chunks.update(c)
            for k,v in pb.items(): per_biome[k].update(v)
    for biome,n in chunks.most_common():
        if n<20: continue
        if tsv:
            for k,v in sorted(per_biome[biome].items()):
                print('%s\t%d\t%s\t%.3f'%(biome,n,k,v/n))
            continue
        top=', '.join('%s %.1f'%(k,v/n) for k,v in per_biome[biome].most_common(8))
        print('%s (%d чанков): %s'%(biome,n,top))
