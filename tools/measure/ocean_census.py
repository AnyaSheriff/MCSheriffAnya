# Подводное на чанк по биомам — по сохранённому миру оригинала (region-файлы):
# кораллы, веера, морские огурцы, айсберги, лёд, магма, пузырьковые колонны,
# пятна песка, глины и гравия под водой. Читаются только данные мира; jar
# и код игры не трогаются.
#
# python3 tools/measure/ocean_census.py <папка region> [--tsv] [биом ...]
# --tsv: построчно «биом<TAB>чанков<TAB>что<TAB>на чанк».
#
# Биом чанка — по середине чанка на поверхности, как у plants_census.py.
# Счёт того же вида у нас: `cargo test --release ocean_census -- --ignored --nocapture`.
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

# Верхний блок воды в море у оригинала — y=62 (уровень моря 63 — первый
# блок над водой).
SEA=62
KINDS=('tube','brain','bubble','fire','horn')

def group(name):
    """К какой строке переписи относится блок; None — не считаем."""
    if name.startswith('dead_'): return None
    if name.endswith('_coral_block'): return 'coral_block'
    if name.endswith('_coral_wall_fan'): return 'coral_wall_fan'
    if name.endswith('_coral_fan'): return 'coral_fan'
    if name.endswith('_coral'): return 'coral'
    if name in ('sea_pickle','packed_ice','blue_ice','snow_block','magma_block','bubble_column','ice'): return name
    return None

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
        hms=root.get('Heightmaps',{})
        hm=hms.get('WORLD_SURFACE'); floor_hm=hms.get('OCEAN_FLOOR')
        if not hm or not floor_hm: continue
        surface=unpack(hm,9,256)
        floor=[h-64 for h in unpack(floor_hm,9,256)]   # первый не твёрдый над дном
        top=surface[8*16+8]-64-1
        sections={s['Y']:s for s in root.get('sections',[])}
        s=sections.get(top//16)
        if not s or 'biomes' not in s: continue
        b=s['biomes']; pal=b['palette']
        ids=[0]*64 if len(pal)==1 else unpack(b['data'],max(1,math.ceil(math.log2(len(pal)))),64)
        cell=((top%16)//4)*16+2*4+2
        biome=pal[ids[cell]].replace('minecraft:','')
        if wanted and biome not in wanted: continue
        chunks[biome]+=1
        c=per_biome[biome]

        # Разбор секций лениво: только те, где есть что считать или сосед.
        cache={}
        def sec(Y):
            if Y in cache: return cache[Y]
            s=sections.get(Y); out=None
            if s and 'block_states' in s:
                bs=s['block_states']; pal=bs['palette']
                names=[e['Name'].replace('minecraft:','') for e in pal]
                props=[e.get('Properties',{}) for e in pal]
                ids=[0]*4096 if len(pal)==1 else unpack(bs['data'],max(4,math.ceil(math.log2(len(pal)))),4096)
                out=(names,props,ids)
            cache[Y]=out
            return out
        def at(x,y,z):
            s=sec(y>>4)
            if not s: return ('air',{})
            v=s[2][((y&15)*16+z)*16+x]
            return (s[0][v],s[1][v])

        found=False
        coral_kinds=set()
        ice_top=0; open_top=0; berg_heights=[]; berg_cols=0
        for Y,s in sorted(sections.items()):
            if 'block_states' not in s: continue
            names=[e['Name'].replace('minecraft:','') for e in s['block_states']['palette']]
            if not any(group(nm) for nm in names): continue
            names,props,ids=sec(Y)
            for idx,v in enumerate(ids):
                g=group(names[v])
                if not g: continue
                y=Y*16+(idx>>8); z=(idx>>4)&15; x=idx&15
                if g=='ice':
                    continue
                if g in ('coral_block','coral','coral_fan','coral_wall_fan'):
                    coral_kinds.add(names[v].split('_')[0])
                    found=True
                if g=='sea_pickle':
                    k=int(props[v].get('pickles','1'))
                    c['pickles']+=k
                    under=at(x,y-1,z)[0]
                    c['sea_pickle_on_coral' if under.endswith('_coral_block') else 'sea_pickle_on_other']+=1
                if g=='magma_block':
                    above=at(x,y+1,z)[0] if y<319 else 'air'
                    wet=above in ('water','bubble_column')
                    if not wet: c['magma_dry']+=1; continue
                    deep=floor[z*16+x]-1-y   # на сколько ниже дна столбца
                    c['magma_wet_floor' if deep<=0 else 'magma_wet_under']+=1
                    if deep<=0: c['magma_floor_y_sum']+=y
                    continue
                if g=='bubble_column':
                    below=at(x,y-1,z)[0]
                    if below=='magma_block': c['bubble_bottoms']+=1
                if g in ('packed_ice','snow_block','blue_ice'):
                    if y>SEA: c[g+'_above']+=1
                    else: c[g+'_below']+=1
                c[g]+=1
        # Поверхность моря: лёд или открытая вода на уровне моря там, где
        # над ним ничего; айсберги — по столбцам.
        for col in range(256):
            x=col&15; z=col>>4
            surf=surface[col]-64-1
            if at(x,surf,z)[0]=='snow': surf-=1   # слой снега на льду и айсберге
            if surf==SEA:
                nm=at(x,SEA,z)[0]
                if nm=='ice': ice_top+=1
                elif nm=='water': open_top+=1
            elif SEA<surf<=SEA+60:
                nm=at(x,surf,z)[0]
                if nm in ('packed_ice','snow_block','blue_ice'):
                    berg_cols+=1; berg_heights.append(surf-SEA)
        c['sea_ice']+=ice_top; c['sea_open']+=open_top
        c['berg_cols']+=berg_cols
        if berg_heights:
            c['berg_chunks']+=1; c['berg_max_sum']+=max(berg_heights)
            c['berg_max_top']=max(c['berg_max_top'],max(berg_heights))
        if found: c['coral_chunks']+=1
        c['coral_kinds_sum']+=len(coral_kinds)
    return chunks, per_biome

ORDER=['coral_block','coral','coral_fan','coral_wall_fan','sea_pickle','pickles','sea_pickle_on_coral',
       'sea_pickle_on_other','coral_chunks','coral_kinds_sum',
       'packed_ice','packed_ice_above','packed_ice_below','snow_block','snow_block_above','snow_block_below',
       'blue_ice','blue_ice_above','blue_ice_below','berg_cols','berg_chunks','berg_max_sum','berg_max_top',
       'sea_ice','sea_open','magma_block','magma_wet_floor','magma_wet_under','magma_dry','magma_floor_y_sum',
       'bubble_column','bubble_bottoms']

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
            for k,v in pb.items():
                for kk,vv in v.items():
                    if kk=='berg_max_top': per_biome[k][kk]=max(per_biome[k][kk],vv)
                    else: per_biome[k][kk]+=vv
    for biome,n in chunks.most_common():
        if n<20: continue
        c=per_biome[biome]
        if tsv:
            for k in ORDER:
                if c[k]: print('%s\t%d\t%s\t%.3f'%(biome,n,k,c[k] if k=='berg_max_top' else c[k]/n))
            continue
        line=', '.join('%s %.2f'%(k,c[k]/n) for k in ORDER if c[k] and k!='berg_max_top')
        print('%s (%d чанков): %s'%(biome,n,line))
