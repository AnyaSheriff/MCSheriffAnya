#!/usr/bin/env python3
"""Воспроизводимые таблицы второго аудита; неизвестное не превращается в ноль."""
import collections
import gzip
import json
import math
from pathlib import Path
import random
import statistics
from structures_audit import window, inside
from structures_audit2 import box_chunks, attach_loot

ROOT=Path(__file__).resolve().parents[2]
TMP=ROOT/'tmp/audit2'
OUT=ROOT/'tools/research/structures-audit-2-data'
DIMS=('overworld','the_nether','the_end')
WIN=[f'{x},{z}' for x in (-1,0,1) for z in (-1,0,1)]
AIR={'air','cave_air','void_air'}


def read(p):return json.loads(p.read_text())
def save(p,v):p.write_text(json.dumps(v,indent=2,sort_keys=True)+'\n')
def med(xs):return round(statistics.median(xs),2) if xs else None
def span(xs):
    xs=[x for x in xs if x is not None]
    return '/'.join(f'{x:g}' for x in (min(xs),statistics.median(xs),max(xs))) if xs else 'unavailable'
def piece_count(s):
    if 'pieces' in s:return s['pieces']
    records=s.get('piece_records',[])
    return len(records) if records and all(p['name']!='whole_snapshot' for p in records) else None

def short(c):return ', '.join(f'{k.removeprefix("minecraft:")}:{v}' for k,v in sorted(c.items())) or '—'
def fixed_piece(p):
    # У дорог и стогов Y=0 — отложенная привязка к земле, а не мировая высота.
    return p.get('projection')!='terrain_matching' and p.get('name')!='feature:pile_hay'


def boxof(s):
    bs=[p['box'] for p in s.get('piece_records',[])]
    ys=[p['box'] for p in s.get('piece_records',[]) if fixed_piece(p)]
    if not bs or not ys:return None
    return [min(b[0] for b in bs),min(b[1] for b in ys),min(b[2] for b in bs),max(b[3] for b in bs),max(b[4] for b in ys),max(b[5] for b in bs)]


def interval(counts,areas):
    # Кластерный bootstrap по окнам; семя воспроизводимо. Один мир — не ансамбль семян.
    rng=random.Random(2612);values=[]
    for _ in range(4000):
        ks=rng.choices(range(9),k=9)
        values.append(sum(counts[k] for k in ks)*1e6/(256*sum(areas[k] for k in ks)))
    values.sort()
    return [round(values[100],3),round(values[3899],3)]


def nearest(rows):
    out=[];censored=0
    for s in rows:
        if not s['window']:continue
        x,z=[v*16+8 for v in s['chunk']]
        gx,gz=map(int,s['window'].split(','));h=72 if gx==gz==0 else 40
        edge=min(x-(gx*512-h)*16,(gx*512+h)*16-x,z-(gz*512-h)*16,(gz*512+h)*16-z)
        ds=[math.dist(s['chunk'],t['chunk'])*16 for t in rows if t is not s]
        if ds and min(ds)<=edge:out.append(min(ds))
        else:censored+=1
    return dict(min=round(min(out),1) if out else None,median=med(out),n=len(out),censored=censored)


def chunks(folder,dim):
    rows={};blocks=collections.Counter();spawners=[];containers=[];missing=[];samples={};frames=[]
    path=folder/f'{dim}-chunks.jsonl'
    if not path.exists():return None
    for line in path.open():
        r=json.loads(line);c=tuple(r['chunk']);rows[c]=r
        for p in r.get('samples',[]):samples[tuple(p['pos'])]=p
        if window(*c) is not None:
            blocks.update(r['blocks']);spawners+=r['spawners'];containers+=r['containers'];missing+=r.get('missing_containers',[]);frames+=r.get('frames',[])
    return dict(rows=rows,samples=samples,core_chunks=sum(window(*c) is not None for c in rows),blocks=blocks,spawners=spawners,containers=containers,missing=missing,frames=frames)


def geometry_ours(s,data):
    pieces=[]
    for p in s.get('piece_records',[]):
        if not fixed_piece(p):continue
        b=p['box'];x,z=(b[0]+b[3])//2,(b[2]+b[5])//2;y=b[1];q=dict(p)
        samples=data['samples'];c=samples.get((x,y,z),{})
        q['datum_y']=y;q['surface']=c.get('surface');q['depth']=q['surface']-y if q['surface'] is not None else None
        q['base_probes']=[samples[(px,y,pz)] for px in sorted({b[0],x,b[3]}) for pz in sorted({b[2],z,b[5]}) if (px,y,pz) in samples]
        q['ring_height_minus_datum']={}
        if p.get('projection')!='terrain_matching':
            for d in (1,4,8,12,16):
                q['ring_height_minus_datum'][str(d)]=[None if (v:=samples.get((px,y,pz),{}).get('surface')) is None else v-y for px,pz in ((b[0]-d,z),(b[3]+d,z),(x,b[2]-d),(x,b[5]+d))]
        pieces.append(q)
    return pieces


def own_members(s,data,key):
    box=boxof(s)
    if not box or not data:return None
    cs=box_chunks(box)
    if not cs<=data['rows'].keys():return None
    out=[]
    for c in cs:
        for q in data['rows'][c].get(key,[]):
            pos=q['pos'] if isinstance(q,dict) else q
            if inside(pos,box):out.append(q)
    return out


def main():
    OUT.mkdir(exist_ok=True)
    summary={};table=[];geotable=[];comptable=[];loottable=[];entitytable=[];biometable=[];examples=[];rings=[];rotations=[]
    final=TMP/'after' if (TMP/'after/overworld-starts.json').exists() else TMP/'before'
    for dim in DIMS:
        vanilla=attach_loot(read(TMP/f'vanilla-{dim}.json'))
        before=read(TMP/f'before/{dim}-starts.json');ours=read(final/f'{dim}-starts.json')
        data=chunks(final,dim)
        if data: assert data["core_chunks"] == 49152, (dim, data["core_chunks"])
        allkinds=sorted({s['kind'] for s in vanilla['starts']+before+ours})
        if dim=='overworld':allkinds=sorted(set(allkinds)|{'village_snowy','mansion','igloo','jungle_pyramid','swamp_hut'})
        ds={};summary[dim]={'coverage':{k:vanilla[k] for k in ('core_chunks','chunks_by_window','issues','entity_issues','biome_exposure')},'kinds':ds}
        if data:
            summary[dim]['ours_chunks']={k:data[k] for k in ('core_chunks','blocks','spawners','containers','missing','frames')}
        for kind in allkinds:
            va=[s for s in vanilla['starts'] if s['kind']==kind];oa=[s for s in ours if s['kind']==kind]
            v=[s for s in va if s['window']];o=[s for s in oa if s['window']];old=[s for s in before if s['kind']==kind and s['window']]
            vc=collections.Counter(s['window'] for s in v);oc=collections.Counter(s['window'] for s in o)
            areas=[vanilla['chunks_by_window'].get(w,0) for w in WIN]
            vv=[vc[w] for w in WIN];oo=[oc[w] for w in WIN]
            vs=nearest(va);os=nearest(oa)
            r=dict(vanilla=len(v),before=len(old),after=len(o),vanilla_windows=vv,ours_windows=oo,
                   vanilla_rate_ci=interval(vv,areas),ours_rate_ci=interval(oo,areas),vanilla_nn=vs,ours_nn=os,
                   vanilla_biomes=dict(collections.Counter(s['biome'] for s in v)),ours_biomes=dict(collections.Counter(s.get('biome','unknown') for s in o)),
                   vanilla_complete=sum(s['complete_bounds'] for s in v))
            ds[kind]=r
            table.append(f'| {dim}/{kind} | {" ".join(map(str,vv))} | {" ".join(map(str,oo))} | {len(v)} / {len(old)} / {len(o)} | {r["vanilla_rate_ci"]} / {r["ours_rate_ci"]} | {vs["median"]} ({vs["n"]}) / {os["median"]} ({os["n"]}) |')
            biometable.append(f'| {dim}/{kind} | {short(r["vanilla_biomes"])} | {short(r["ours_biomes"])} |')
            vp=[p for s in v if s['complete_bounds'] for p in s['pieces'] if p.get('projection')!='terrain_matching']
            op=[p for s in o for p in geometry_ours(s,data)] if data else []
            r['geometry']={}
            for label,ps in [('vanilla',vp),('ours',op)]:
                probes=[q for p in ps for q in p.get('base_probes',[])]
                r['geometry'][label]=dict(pieces=len(ps),datum_y=span([p.get('datum_y') for p in ps]),depth=span([p.get('depth') for p in ps]),
                                          probes=len(probes),air_below=sum(q.get('below') in AIR for q in probes),
                                          unknown_below=sum(q.get('below') is None for q in probes),
                                          rings={str(d):med([h for p in ps for h in p.get('ring_height_minus_datum',{}).get(str(d),[]) if h is not None]) for d in (1,4,8,12,16)})
            vg,og=r['geometry']['vanilla'],r['geometry']['ours']
            geotable.append(f'| {dim}/{kind} | {r["vanilla_complete"]}/{len(v)} | {vg["datum_y"]} / {og["datum_y"]} | {vg["depth"]} / {og["depth"]} | {vg["air_below"]}/{vg["probes"]} / {og["air_below"]}/{og["probes"]} |')
            rings.append(f'| {dim}/{kind} | {list(vg["rings"].values())} | {list(og["rings"].values())} |')
            r['composition']={'vanilla_types':dict(collections.Counter(p['name'] for s in v for p in s['pieces'])),
                              'ours_types':dict(collections.Counter(p['name'] for s in o for p in s.get('piece_records',[]))),
                              'vanilla_rotations':dict(collections.Counter(str(p['rotation']) for s in v for p in s['pieces'])),
                              'ours_rotations':dict(collections.Counter(str(p.get('rotation')) for s in o for p in s.get('piece_records',[])))}
            boxes=[s['box'] for s in v];obs=[b for s in o if (b:=boxof(s))]
            size=lambda bs: '/'.join(str(med([b[i+3]-b[i]+1 for b in bs])) for i in range(3))
            comptable.append(f'| {dim}/{kind} | {span([len(s["pieces"]) for s in v])} / {span([piece_count(s) for s in o])} | {size(boxes)} / {size(obs)} | {len(r["composition"]["vanilla_types"])} / {len(r["composition"]["ours_types"])} |')
            rotations.append(f'| {dim}/{kind} | {short(r["composition"]["vanilla_rotations"])} | {short(r["composition"]["ours_rotations"])} |')
            own_cont=[c for start in o for c in (own_members(start,data,'containers') or [])]
            own_sp=[c for start in o for c in (own_members(start,data,'spawners') or [])]
            own_missing=[c for start in o for c in (own_members(start,data,'missing_containers') or [])]
            own_entities=[c for start in o for c in (own_members(start,data,'spawn_markers') or [])]
            own_frames=[c for start in o for c in (own_members(start,data,'frames') or [])]
            r['ours_saved_frames']=own_frames
            r['ours_complete_bounds']=sum(own_members(start,data,'containers') is not None for start in o)
            r['ours_containers']=len(own_cont);r['ours_missing_containers']=len(own_missing)
            r['ours_spawners']=dict(collections.Counter(q['entity'] for q in own_sp))
            r['ours_spawn_markers']=dict(collections.Counter(q['id'] for q in own_entities))
            be=[b for s in v if s['complete_bounds'] for b in s['block_entities']]
            tables=collections.Counter(b['LootTable'] for start in v if start['complete_bounds'] for b in start['compatible_loot'])
            sp=collections.Counter(b.get('SpawnData',{}).get('entity',{}).get('id','unknown') for b in be if b['id']=='minecraft:mob_spawner')
            r['spatial_spawners']=dict(sp)
            expected_mobs={'mineshaft':'minecraft:cave_spider','mineshaft_mesa':'minecraft:cave_spider','stronghold':'minecraft:silverfish','fortress':'minecraft:blaze','bastion_remnant':'minecraft:magma_cube'}
            sp=collections.Counter({k:n for k,n in sp.items() if k==expected_mobs.get(kind)})
            r['loot_tables']=dict(tables);r['spawners']=dict(sp)
            loottable.append(f'| {dim}/{kind} | {short(tables)} | {len(own_cont)} / {len(own_missing)} (bounds {r["ours_complete_bounds"]}/{len(o)}) | {short(sp)} / {short(r["ours_spawners"])} |')
            es=[e for s in v if s['complete_bounds'] for e in s['entities']]
            r['entities']=dict(collections.Counter(e['id'] for e in es))
            r['villagers']=dict(collections.Counter(str(e['VillagerData']) for e in es if 'VillagerData' in e))
            entitytable.append(f'| {dim}/{kind} | {short(r["entities"])} | {short(r["ours_spawn_markers"])}; saved frames:{len(own_frames)} |')
            if v or o:
                vexample=v[0] if v else None;oe=o[0] if o else None
                examples.append(f'| {dim}/{kind} | {vexample["chunk"] if vexample else "—"}; BB {vexample["box"] if vexample else "—"} | {oe["chunk"] if oe else "—"}; BB {boxof(oe) if oe else "—"} |')
        # Полное доказательство в сжатом JSON; это измерения мира, не шаблоны jar.
        for s in vanilla['starts']:
            for e in s['entities']:
                e['piece_membership']=[i for i,p in enumerate(s['pieces']) if inside(e['Pos'],p['box'])]
        (OUT/f'vanilla-{dim}.json.gz').write_bytes(gzip.compress(json.dumps(vanilla,separators=(',',':')).encode(),mtime=0))
        save(OUT/f'ours-{dim}-starts.json',ours)
    save(OUT/'summary.json',summary)
    lines=['# Structure audit 2 — saved vanilla 26.1.2 versus our generator','',
           'Seed **100554032945340**. Reference: `~/.cache/mcsheriffanya/vanilla-struct/world`, read only. No servers, jars, game sources, mappings, datapacks or forbidden directories were used. This report measures the supplied world; it does **not** establish 1:1 generation parity.','',
           '## Coverage and interpretation','',
           'The nine core windows contain 49,152 chunks per dimension (12,582,912 block columns). Centre: chunk intervals [-64,64); other centres (gx×512,gz×512): ±32, upper exclusive. The 8-chunk halo expands these to ±72/±40: 71,936 chunks per dimension. Only start chunks in the core count toward rates. Halo starts and blocks supply spacing neighbours and bounding-box coverage. Every chunk intersecting a box is checked, including its interior; corner-only completeness checks are insufficient.','',
           'The reader validates region record extents, zlib and NBT, full status, coordinates, sections and heightmap presence. The supplied reference currently has **49,152 full core chunks and 71,936 full core+halo chunks in each dimension**, with no missing/non-full/truncated records detected. The reported OOM location (8,-524) is presently saved as full. The separate palette/section integrity check is retained in `integrity.json`; a prior failed save is not evidence that the final saved record is incomplete. No suspect record is silently substituted with air.','',
           'Counts are exact for these saved windows. Bracketed uncertainty is the 95% percentile interval of 4,000 window-cluster bootstrap resamples, in starts per million block columns, fixed seed 2612. It measures variability between these nine windows, not a multi-seed confidence claim. Zero observations give a degenerate bootstrap interval and do not prove impossibility (under an independent Poisson model, the zero-count 95% upper rate is 3/area). Same-seed terrain and biomes differ; a count difference alone does not justify changing spacing.','',
           'Nearest-neighbour (NN) distances use all measured halo starts of the same type; a core start contributes only if its nearest measured neighbour is closer than the halo boundary. Values show median blocks and number of uncensored starts. Raw coordinates, composition, rotations, professions/types, containers, spawners and ring probes are in [the evidence summary](structures-audit-2-data/summary.json) and compressed per-dimension vanilla JSON files.','',
           '## Counts, rates and spacing','',
           'Window order throughout: **(-1,-1), (-1,0), (-1,1), (0,-1), (0,0), (0,1), (1,-1), (1,0), (1,1)**. Counts are vanilla / ours before / ours after. `None` means unavailable.','',
           '| Dimension/type | Vanilla per window | Ours per window | Total V/B/A | 95% rate interval V / A | NN median (n), V / A |',
           '|---|---|---|---|---|---|',*table,'',
           '## Biomes at starts','',
           'Vanilla samples the start-chunk centre at first-piece minimum Y. Our existing marker metadata samples the surface biome (Nether/End use their dimension biome map). These are observations, not equivalent 3D biome predicates. The saved biome-exposure counts at Y64/Y32 are in the summary; underground acceptance needs a separate 3D exposure denominator.','',
           '| Dimension/type | Vanilla start biomes | Ours marker biomes |','|---|---|---|',*biometable,'',
           '## Placement, support and terrain adaptation','',
           'Datum is the piece BB minimum Y plus saved ground_level_delta on the vanilla jigsaw side; our datum is the exported piece minimum Y. It is **not universally a walkable floor**. Depth uses final OCEAN_FLOOR on vanilla and highest collision-bearing block on ours, at the piece centre. Both can include the structure itself, trees and roofs. Positive depth denotes datum below that final top. Nether heightmaps see the roof; they do not measure the local cavern floor. Sea reference is Y63 Overworld, Y32 Nether; End has no sea. Raw per-piece sea deltas are in vanilla evidence.','',
           'Terrain-matching pieces are excluded from datum summaries on both sides. Our roads and hay features store a deferred Y=0, not a world elevation, and are excluded from vertical bounds; village box Y spans rigid pieces only. Nine BB-datum probes (corners, edges and centre; duplicates removed) retain the actual block and block immediately below. Air counts are diagnostic, not a floating-piece verdict: bridges, ruined pieces, entrances and empty BB margins intentionally contain air. Complete vanilla boxes are used; pieces with missing bounds are excluded from geometry inventories.','',
           '| Dimension/type | Complete V starts | Datum Y min/median/max V / A | Depth min/median/max V / A | Air below datum probes V / A |','|---|---|---|---|---|',*geotable,'',
           'Four side-midpoint probes at distances **1,4,8,12,16** from each non-terrain-matching piece give median final top minus datum. They may intersect neighbouring pieces; these profiles are descriptive. They cannot isolate beard_thin / beard_box / bury / encapsulate density changes without matching unmodified terrain. Our village/outpost cubic skirt differs in implementation; ancient-city, stronghold, trial-chamber and trail-ruin dedicated density adaptation remains absent. No ground was added solely because a BB probe found air.','',
           '| Dimension/type | Vanilla ring profile | Ours ring profile |','|---|---|---|',*rings,'',
           'Chunk-border completeness is a coverage check, not proof of correct painting. Procedural branch pruning and intentional ruin gaps cannot be classified as cut pieces from final bounding boxes alone. A production-mask comparison shares search-radius mistakes. The independent complete-plan comparisons below cover five families; other families still need that check before a clipping verdict.','',
           '## Composition','',
           'Piece counts include vanilla metadata children, including entity/feature markers. Our snapshot-based ancient cities and trial chambers do not retain an equivalent jigsaw decomposition. Whole snapshot boxes must not be compared as single vanilla pieces. Type and rotation histograms are preserved in the JSON; numeric orientation conventions are not assumed identical.','',
           '| Dimension/type | Pieces min/median/max V / A | Median total box X/Y/Z V / A | Distinct piece names V / A |','|---|---|---|---|',*comptable,'', '| Dimension/type | Saved vanilla rotations | Ours rotations (quarters or facing code) |', '|---|---|---|', *rotations,'',
           '## Containers, loot tables and spawners','',
           'Vanilla rows retain compatible literal LootTable identifiers in complete core-start boxes, including chest-minecart entity loot. Other families inside the same box (e.g. dungeon loot below a village) are kept in raw evidence but excluded here. Pyramid treasure and archaeology below the saved BB are included using their table IDs and horizontal footprint: at chunk (-63,22), the saved BB begins at Y67 but four treasure chests are at Y56. Spawner rows likewise filter compatible species; all spatial overlaps remain in the evidence. Overlapping starts of the same family can still share spatial records; this is measured association, not provenance from generator code. Our containers use eager item slots and have no persisted LootTable identifier; absence of a field is separate from a chest lacking initialization. Physical chest/barrel totals, initialized containers and uninitialized positions are in the summary.','',
           '| Dimension/type | Vanilla loot tables (count) | Ours containers / uninitialized chest+barrel blocks | Spawner species V / A |','|---|---|---|---|',*loottable,'',
           '## Saved entities','',
           'Entities below are read from entity regions and spatially associated with complete core-start boxes. Save-time populations include natural spawns, movement, breeding and deaths. Negative villager ages in the taiga sample have already advanced from their newborn value; snapshot coordinates are not trustworthy original template spawn markers. VillagerData (profession/type/level), age and persistence are preserved. Empty entity-region records mean no saved entities, not a failed chunk.','',
           '| Dimension/type | Saved vanilla entities inside complete bounds | Ours planned fresh-chunk markers (before collision filtering) |','|---|---|---|',*entitytable,'',
           'Our generator defers structure mobs to fresh-chunk hooks in tick.rs: End shulkers, monument elders, mansion occupants, hut witch/black cat, igloo inhabitants, outpost cage golems/allays, ocean-ruin drowned and mineshaft carts. Chunk::generated alone does not execute those hooks; its empty mob array is not proof these hooks are absent. Village inhabitants and bastion piglin/brute/hoglin hooks are missing. Pillager and ordinary guardian natural spawning are separate from saved generation entities. Collision filtering, actual hook success and reload populations remain unverified without running the simulation.','',
           '## Concrete coordinates','',
           'Coordinates are **start chunks** followed by inclusive **block bounding boxes**. Examples in opposite columns are distribution samples, not matched structures: our placement RNG and terrain differ.','',
           '| Dimension/type | Vanilla example | Ours example |','|---|---|---|',*examples,'']
    notes=OUT/'notes.md'
    if notes.exists():lines.append(notes.read_text())
    (ROOT/'tools/research/structures-audit-2.md').write_text('\n'.join(lines)+'\n')
    print('report written',len(table),'types')

if __name__=='__main__':main()
