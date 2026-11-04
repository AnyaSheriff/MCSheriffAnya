# Бросает таблицы добычи на официальном сервере 26.3 (`/loot replace block` в сундук и `data get block`)
# и считает, сколько предметов в сундуке и как часто попадается каждый. Только чтение ответов сервера.
# python3 -I tools/measure/loot_sample_263.py <порт rcon> <пароль> <таблица> [бросков]
# Сундук ставится в (30000, 140, 29990) — ячейка должна быть загружена (forceload).
import sys,re,json,collections
import socket, struct
class R:
    def __init__(s, port, pw):
        s.s = socket.create_connection(('127.0.0.1', port)); s.i = 0; s.c(3, pw)
    def c(s, k, t):
        s.i += 1; b = t.encode() + b'\0\0'; s.s.sendall(struct.pack('<iii', len(b) + 8, s.i, k) + b)
        n = struct.unpack('<i', s.rv(4))[0]; return s.rv(n)[8:-2].decode('utf8', 'replace')
    def rv(s, n):
        o = b''
        while len(o) < n: o += s.s.recv(n - len(o))
        return o
    def run(s, t): return s.c(2, t)
r = R(int(sys.argv[1]), sys.argv[2])
r.run('forceload add 30000 29984 30015 29999')
def entries(d):
    s=d[d.index('[')+1:d.rindex(']')]; out=[]; depth=0; start=None
    for i,ch in enumerate(s):
        if ch=='{':
            if depth==0: start=i
            depth+=1
        elif ch=='}':
            depth-=1
            if depth==0: out.append(s[start:i+1])
    return out
out={}
for table in [sys.argv[3]]:
    N=int(sys.argv[4]) if len(sys.argv)>4 else 1200; items=collections.Counter(); counts=collections.defaultdict(collections.Counter); sizes=collections.Counter(); comps=collections.Counter()
    for i in range(N):
        r.run('setblock 30000 140 29990 air'); r.run('setblock 30000 140 29990 chest')
        r.run('loot replace block 30000 140 29990 container.0 loot minecraft:%s'%table)
        es=entries(r.run('data get block 30000 140 29990 Items')); sizes[len(es)]+=1
        for e in es:
            idm=re.search(r'id: "minecraft:(\w+)"',e); cm=re.search(r'count: (\d+)',e)
            name=idm.group(1)
            pm=re.search(r'potion: "minecraft:(\w+)"',e)
            if pm: name+=':'+pm.group(1)
            items[name]+=1; counts[name][int(cm.group(1)) if cm else 1]+=1
            if 'components' in e and not pm: comps[name+' '+re.sub(r'\d+','N',e[:120])]+=1
    out[table]={'N':N,'items':items,'counts':{k:dict(v) for k,v in counts.items()},'sizes':dict(sizes),'comps':comps}
    print(table,'sizes',sorted(sizes.items()))
    for k,c in sorted(items.items(), key=lambda kv:-kv[1]): print('  %-36s %5d %.3f/chest counts %s'%(k,c,c/N,dict(sorted(counts[k].items()))))
    for k,c in comps.most_common(8): print('   comp',c,k)
