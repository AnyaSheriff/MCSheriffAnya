import os, sys, struct, zlib, glob, collections, math
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
reg=sys.argv[1]
under=collections.Counter(); chunks=0
for f in glob.glob(reg+'/*.mca'):
    data=open(f,'rb').read()
    if len(data)<8192: continue
    for i in range(1024):
        off=int.from_bytes(data[i*4:i*4+3],'big'); 
        if off==0: continue
        p=off*4096; ln=struct.unpack_from('>i',data,p)[0]; comp=data[p+4]
        if comp!=2: continue
        root,_=nbt(zlib.decompress(data[p+5:p+4+ln]),3+struct.unpack_from('>H',zlib.decompress(data[p+5:p+4+ln]),1)[0],10) if False else (None,None)
        raw=zlib.decompress(data[p+5:p+4+ln])
        n=struct.unpack_from('>H',raw,1)[0]
        root,_=nbt(raw,3+n,10)
        if root.get('Status') not in ('minecraft:full','full'): continue
        hm=root.get('Heightmaps',{}).get('OCEAN_FLOOR')
        if not hm: continue
        heights=[h-64-1 for h in unpack(hm,9,256)]  # y of top block
        chunks+=1
        for s in root.get('sections',[]):
            b=s.get('biomes'); 
            if not b: continue
            pal=b['palette']; Y=s['Y']
            if len(pal)==1: ids=[0]*64
            else: ids=unpack(b['data'], max(1,math.ceil(math.log2(len(pal)))), 64)
            for idx,v in enumerate(ids):
                cx=idx&3; cz=(idx>>2)&3; cy=idx>>4
                y=Y*16+cy*4+2
                h=heights[(cz*4+2)*16+cx*4+2]
                if y<=h:
                    name=pal[v].replace('minecraft:','')
                    under[name if name in('lush_caves','dripstone_caves','deep_dark') else 'наземный']+=1
tot=sum(under.values())
print('чанков',chunks)
for k,v in under.most_common(): print('%6.2f%% %s'%(100*v/tot,k))
