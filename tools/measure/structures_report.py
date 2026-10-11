#!/usr/bin/env python3
"""Сводка завершённых офлайн-замеров без запуска сервера."""
import collections
import json
import math
from pathlib import Path
import statistics

ROOT=Path(__file__).resolve().parents[2]
TMP=ROOT/'tmp'
OUT=ROOT/'tools/research/structures-audit-data'


def read(path):
    return json.loads(path.read_text())


def chunks(path):
    rows=[json.loads(line) for line in path.open()]
    assert len(rows)==49152, (path,len(rows))
    assert len({tuple(r['chunk']) for r in rows})==49152
    blocks=collections.Counter(); windows=collections.Counter(); spawners=[]; containers=[]
    for r in rows:
        blocks.update(r['blocks']); windows[str(r['window'])]+=1
        spawners.extend(r['spawners']); containers.extend(r['containers'])
    return dict(chunks=len(rows),windows=dict(windows),blocks=dict(blocks),spawners=spawners,containers=containers)


def main():
    OUT.mkdir(parents=True,exist_ok=True)
    va=read(TMP/'vanilla-overworld.json'); vn=read(TMP/'vanilla-nether.json')
    normal=read(TMP/'normal-all-overworld.json'); features=read(TMP/'vanilla-features.json')
    summary={}; starts={}
    for dim in ('overworld','the_nether','the_end'):
        folder=TMP/('ours-final' if dim=='overworld' else 'ours-baseline')
        summary[dim]=chunks(folder/f'{dim}-chunks.jsonl')
        starts[dim]=read(folder/f'{dim}-starts.json')
        (OUT/f'ours-{dim}-starts.json').write_text(json.dumps(starts[dim],indent=2,sort_keys=True)+'\n')
    support=read(TMP/'ours-final/support.json')
    (OUT/'support.json').write_text(json.dumps(support,indent=2,sort_keys=True)+'\n')
    (OUT/'vanilla-features.json').write_text(json.dumps(features,indent=2,sort_keys=True)+'\n')
    evidence={'seed':100554032945340,'vanilla_overworld':{k:v for k,v in va.items() if k not in ('block_entities','starts')},'vanilla_nether':{k:v for k,v in vn.items() if k not in ('block_entities','starts')},'vanilla_structure_starts':{'big_overworld':len(va['starts']),'big_nether':len(vn['starts']),'normal_all_overworld':len(normal['starts'])},'normal_full_chunks':sum(normal['chunks'].values()),'ours':summary}
    (OUT/'summary.json').write_text(json.dumps(evidence,indent=2,sort_keys=True)+'\n')
    ow=summary['overworld']; sp=[s for s in ow['spawners'] if s['entity'] in ('minecraft:zombie','minecraft:skeleton','minecraft:spider') and s['block']=='spawner']
    vs=[f for f in features if f['entity']]
    species=collections.Counter(s['entity'] for s in sp)
    grouped=collections.defaultdict(list)
    for dim,rows in starts.items():
        for s in rows: grouped[(dim,s['kind'])].append(s)
    kinds=[('overworld',s) for s in ['village_plains','village_desert','village_savanna','village_taiga','village_snowy','pillager_outpost','mansion','desert_pyramid','jungle_pyramid','swamp_hut','igloo','ruined_portal','ruined_portal_desert','ruined_portal_jungle','ruined_portal_swamp','ruined_portal_mountain','ruined_portal_ocean','ruined_portal_standard','shipwreck','shipwreck_beached','ocean_ruin_cold','ocean_ruin_warm','monument','buried_treasure','mineshaft','mineshaft_mesa','stronghold','ancient_city','trial_chambers','trail_ruins']]+[('the_nether',s) for s in ['fortress','bastion_remnant','ruined_portal_nether']]+[('the_end','end_city')]
    # ruined_portal — обычный вариант; отдельного standard в реестре нет.
    kinds.remove(('overworld','ruined_portal_standard'))
    count_windows={}
    for (dim,kind),rows in grouped.items():
        count_windows[f'{dim}/{kind}']=dict(collections.Counter(str(r['window']) for r in rows))
    (OUT/'counts-by-window.json').write_text(json.dumps(count_windows,indent=2,sort_keys=True)+'\n')
    baseline=chunks(TMP/'ours-baseline/overworld-chunks.jsonl')
    old={tuple(p) for p in baseline['spawners']}
    new={tuple(s['pos']) for s in summary['overworld']['spawners']}
    evidence['spawner_fix']={'before_records':len(old),'before_blocks':baseline['blocks']['spawner'],'after_records':len(new),'after_blocks':summary['overworld']['blocks']['spawner'],'removed_records':sorted(old-new),'added_records':sorted(new-old)}
    (OUT/'summary.json').write_text(json.dumps(evidence,indent=2,sort_keys=True)+'\n')
    lines=['# Structures audit — 26.1.2', '',
    '**Partial audit; 1:1 parity is not established.** The supplied vanilla worlds have no saved structure starts. This is an observed absence of the required reference data, not evidence that vanilla naturally generates zero structures. All requested structures are covered below, but unavailable comparisons are explicitly marked.', '',
    '## Inputs, coverage, and method', '',
    '- Read `AGENTS.md` before work. No server was started or contacted; no jars, game code, mappings, datapacks, structure-template dumps, or forbidden directories were read. Only this copy was written.',
    '- Both supplied `server.properties` files say `generate-structures=false`. Region records corroborate this: **0 valid starts** in all 49,152 requested Overworld chunks of `vanilla-big`, in 4,624 saved Nether chunks inside those windows, and in all 33,886 full Overworld chunks of the biased `vanilla-normal`. Neither world has saved End region chunks. The settings alone would not have established historical generation; the region census does.',
    '- The Overworld area is **12,582,912 block columns**: `[-1024,1024)²`, plus eight 1024² windows centered at `(gx*8192,gz*8192)`, gx/gz ∈ {-1,0,1}, excluding the center. Each outer interval is center ±512, upper bound excluded. All 49,152 chunks are full. The Nether reference covers only 4,624 of the requested 49,152 chunks; do not use it as a full-area denominator.',
    '- Generated **49,152 real chunks in each dimension**, 147,456 total, using seed `100554032945340`, `Style::Vanilla`, and `Chunk::generated` directly. No sockets or world-loading server path. One chunk is retained at a time for the census. Overworld was regenerated after the fixes; Nether/End census predates the final diagnostic-only changes and uses the same placement/block generators.',
    '- Reused `nether_census.nbt/unpack` and `village.World`. Our starts use the production seedmap placement calls, not a second placement implementation. Chunk output separately measures actual blocks, containers and spawners. A start is not proof that every planned piece survived painting.',
    '- Counts use start chunks within a window, never references in neighboring chunks. Nearest-neighbor (NN) distances use anchors in the **same window**; they are edge-censored, pooled over available neighbors, and cannot estimate global spacing. Biomes in the table are our surface anchor biomes, not a 3D validity test for underground starts.',
    '- Own geometry inventories retain measured-world assets already in this repository. Their provenance comments were inspected; they were not treated as a fresh independent vanilla sample.',
    '- JSON evidence: [summary](structures-audit-data/summary.json), [Overworld starts](structures-audit-data/ours-overworld-starts.json), [Nether starts](structures-audit-data/ours-the_nether-starts.json), [End starts](structures-audit-data/ours-the_end-starts.json), [feature samples](structures-audit-data/vanilla-features.json), [support diagnostics](structures-audit-data/support.json). Raw chunk JSONL and logs remain in `tmp/`.', '',
    '## Counts, spacing, biomes, and coordinates', '',
    '`Unavailable` in the vanilla column means structures were disabled (Overworld/Nether) or chunks are absent (End). It is not a valid frequency baseline. Own zero counts do not establish a missing feature unless code inspection also confirms it. Coordinates below are block X/Z anchors; detailed floor/origin/box coordinates are in the JSON.', '',
    '| Structure | Vanilla | Ours: count; same-window NN min/median blocks | Ours: observed anchor biomes | Ours example X,Z | Verdict |',
    '|---|---|---|---|---|---|']
    for dim,kind in kinds:
        rows=grouped[(dim,kind)]; distances=[]
        for s in rows:
            neighbors=[t for t in rows if t is not s and t['window']==s['window']]
            if neighbors: distances.append(min(math.dist(s['anchor'],t['anchor']) for t in neighbors))
        nn=f'{min(distances):.0f}/{statistics.median(distances):.0f}' if distances else 'n/a'
        biomes=', '.join(sorted({s.get('biome','unknown') for s in rows})) or '—'
        example=', '.join(map(str,rows[0]['anchor'])) if rows else '—'
        verdict='Missing layouts' if kind in ('village_taiga','village_snowy') else 'Comparison unavailable'
        lines.append(f'| {kind} | Unavailable | {len(rows)}; {nn} | {biomes} | {example} | {verdict} |')
    lines += ['', 'Measured own plan composition (not a count of surviving pieces after painting); all vanilla comparison cells remain unavailable. `Y` is the planner datum, not a universal floor definition. A missing value means that metric was not exported.', '',
        '| Structure | Vanilla | Ours piece count min/median/max | Ours placement Y min/median/max | Verdict |', '|---|---|---|---|---|']
    def span(values):
        return f'{min(values):g}/{statistics.median(values):g}/{max(values):g}' if values else 'Not exported / no sample'
    for dim,kind in kinds:
        rows=grouped[(dim,kind)]
        pieces=[r['pieces'] for r in rows if 'pieces' in r]
        heights=[r['floor'] if 'floor' in r else r['origin'][1] for r in rows if 'floor' in r or 'origin' in r]
        if pieces or heights:
            lines.append(f'| {kind} | Unavailable | {span(pieces)} | {span(heights)} | Reference absent |')
    lines += ['', '[Per-window counts](structures-audit-data/counts-by-window.json) retain the nine areas separately; missing keys mean zero accepted starts.', 'Portal rows cover standard, desert, jungle, swamp, mountain, ocean and Nether variants separately. The library contains 13 portal forms; no independent form-frequency comparison is possible. End ships are piece names in the End-start evidence, not a separate placement set.', '',
    '### Features still measurable with structures disabled', '',
    '| Measurement in the same Overworld windows | Vanilla | Ours after fixes | Verdict |',
    '|---|---:|---:|---|',
    f'| Dungeon-type spawner blocks/records | 934 | {len(sp)} | Different observed density; biome/terrain differences confound a rule-level conclusion |',
    f'| Zombies / skeletons / spiders | 472 / 231 / 231 | {species["minecraft:zombie"]} / {species["minecraft:skeleton"]} / {species["minecraft:spider"]} | Seed dependence fixed; 50/25/25 rule retained |',
    '| Dungeon loot-table records | 1,449 `chests/simple_dungeon` | 0; dungeon builder creates chest blocks without loot/container data | Missing loot |',
    f'| Budding-amethyst blocks | 66,263 | {ow["blocks"].get("budding_amethyst",0):,} | Strong density mismatch; block count is NOT geode-instance count |',
    '| Fossils | 14,864 bone blocks; instance count not resolved | No fossil generator; bone blocks from other structures must not be counted as fossils | Missing feature |',
    '| Desert wells | 8 suspicious-sand BEs with `archaeology/desert_well`, four separated local pairs | No desert-well generator | Missing feature |',
    '| Nether fossils | No reference starts in the covered subset | No Nether-fossil generator | Missing feature; frequency unavailable |',
    '| End gateways | No End reference chunks | No `end_gateway` blocks in the generated squares; no gateway path in `world/the_end.rs` | Outer gateway feature missing; dragon-fight gateways are a separate lifecycle |', '',
    'Vanilla dungeon spawner Y min/median/max = **-54 / -21 / 95**; depth below the post-generation OCEAN_FLOOR height = **7 / 82 / 181** blocks. Of 934 spawner-center columns, one has air immediately below its floor; this is not evidence that the whole room floats. Vanilla dungeon chest/spawner ratio is 1,449/934 = 1.551; assignment to individual rooms is not inferred from that ratio. Dungeon biomes, floors, underlying blocks and positions are preserved in the feature evidence.',
    f'Own dungeon-type spawner Y min/median/max = **{min(s["pos"][1] for s in sp)} / {statistics.median(s["pos"][1] for s in sp):g} / {max(s["pos"][1] for s in sp)}**. These species exclude mineshaft cave-spider and stronghold silverfish spawners. Counts describe surviving final spawners, not attempted rooms.', '',
    'Concrete feature examples (X,Y,Z):', '',
    f'- Dungeon: vanilla `{next(f["pos"] for f in features if f["entity"])}`; ours `{sp[0]["pos"]}`. Vanilla chest at `(-33,-28,-481)` has `minecraft:chests/simple_dungeon`.',
    '- Geode material: vanilla budding amethyst at `(-162,3,-499)`; ours contains budding amethyst in chunk `(-544,-544)` (X/Z -8704…-8689). The census does not retain an exact block Y or identify whole geodes, so the instance/height comparison remains incomplete.',
    '- Fossil material: vanilla bone block at `(-434,-38,-326)`; no corresponding feature exists in our generator.',
    '- Wells: vanilla suspicious-sand pairs near `(-600,70,575)`, `(475,65,418)`, `(125,69,8265)`, `(751,62,-59)`; none from a well in ours. The reference top at each pair includes the well roof, so its 6–7 block depth must not be described as burial below original terrain.', '',
    '## Height, adaptation, composition, loot and entities', '',
    'The vanilla side of every structure row below is **unavailable in the supplied worlds**. Existing code comments describe earlier measurements but cannot supply the requested fresh comparison. Rules listed here describe our implementation, not verified vanilla parity.', '',
    '| Structure group | Our height / adaptation | Our composition / loot / entity path | Verdict and missing measurement |',
    '|---|---|---|---|',
    '| Plains village | Road follows supported density; rigid floors follow nearby road; 12-block cubic beard approximation | 51 measured piece forms, 10 sampled layouts. No village inhabitant hook; no village loot initialization | Partial. Need per-building spawn markers/age/profession, cats/golems, chest tables and whole-layout distribution |',
    '| Desert village | Same road/rigid scheme | 37 forms, 6 sampled layouts; no inhabitants or village loot; camel marker can have no painted blocks | Partial; no start in this area |',
    '| Savanna village | Same scheme | 51 forms, 6 sampled layouts; no inhabitants or village loot | Partial; no start in this area |',
    '| Taiga / snowy villages | No library, therefore no accepted start | No layouts/pieces/entities | Missing. Measure terrain-relative piece positions, rotations, joints, household populations, loot, beds and abandoned variants across many villages |',
    '| Pillager outpost | Ring-median placement, beard approximation; exclusion currently tests accepted village starts | 8 forms, 13 layouts; tower loot, cage golem and allay hooks; pillagers use structure-aware natural spawning | Partial; measure cage rates, village exclusion and tower/feature terrain cross-sections |',
    '| Woodland mansion | Terrain-supported entrance and descending foundation | 72 forms and sampled layouts; mansion loot and evoker/vindicator/allay hooks | Partial; no start in these squares; room/piece/inhabitant distributions unverified |',
    '| Desert pyramid | Minimum sampled ring terrain height; descending sandstone foundation | Measured shell, four-chest/trap path and archaeology; eager loot | Partial; no start; burial room and floor/water intersections unmeasured |',
    '| Jungle temple | Average surrounding height +1 | Measured shell, chests and arrow dispensers | Partial; no start; stair/puzzle and support comparisons unavailable |',
    '| Swamp hut | Average terrain/water surface +1; descending log supports | Witch and black-cat fresh-chunk hooks already exist, with collision filtering | Partial; one own start; need persistence/spawn success and water-height examples |',
    '| Igloo | Surface top and optional shaft/basement | Resident villager + zombie villager hooks and loot already exist | Partial; no start; basement probability/depth distribution unavailable |',
    '| Ruined portals, every variant | Variant selects surface/ocean-floor/underground/mountain/buried/Nether placement; local support/decorations | 13 measured forms; variant processing; eager portal loot | Partial; frequency of each form, giant form, air pocket, burial and support needs reference samples |',
    '| Shipwrecks, submerged/beached | Median local bottom or land height approximation | 20 forms; randomized timber; supply/map/treasure loot | Partial; beached sample empty; water/floor and broken-piece frequencies unavailable |',
    '| Ocean ruins, warm/cold | Local seafloor, multi-piece sites | 24 measured forms; integrity processing, suspicious blocks, loot and drowned hooks | Partial; six-neighbor support and clustering statistics unmeasured |',
    '| Ocean monument | Constant bottom Y39; 58×58 footprint, 23 high, supports/water handling | Measured composition; elder hook (three positions), guardians via runtime spawning | Partial; biome radius, floor contact and live spawn success unavailable |',
    '| Buried treasure | Beach chunk offset 9,9; measured depth approximation | Single eager-loot chest | Partial; depth versus original surface and natural support unavailable |',
    '| Mineshaft, normal / mesa | Underground plan; mesa height uses local surface | Procedural pieces, bridge/support rules; cave-spider spawners and chest-minecart hook | Partial; room/corridor/rail counts and liquid veto/support statistics needed |',
    '| Dungeons | 14 attempts; room kept wholly inside one chunk, dimensions 7/9, height 6 | Physical spawner plus up to two chest blocks; missing loot/container initialization | Divergent chunk-boundary rule and loot; seeded mob selection fixed |',
    '| Strongholds | Ring starts and underground plan | Procedural pieces, silverfish spawner and stronghold loot | Partial; no dedicated bury-density pass; ring-biome relocation and topology unverified |',
    '| Ancient cities | Constant floor Y-51; whole measured city painted into terrain; no dedicated beard_box pass | Four measured whole-city snapshots, eager loot; many disconnected-solid flags | Partial; snapshot reuse and terrain adaptation do not establish jigsaw/statistical parity |',
    '| Trial chambers | Underground sampled chamber layouts | Six measured whole-layout samples, trial spawner/vault data; no dedicated encapsulation-density pass | Partial; placement/topology and enclosing terrain distributions unmeasured |',
    '| Trail ruins | Tower base buried by fixed 15 blocks; sampled layouts | Archaeology processing; no surrounding burial/slope blend | Partial; tower exposure, hillside intersections, sediment and piece rates need reference |',
    '| Desert wells / fossils | No worldgen path found | Well archaeology support alone is not a generator | Missing; measure placements, forms, burial/replacement and biome-conditioned rates |',
    '| Amethyst geodes | Own noise/shell shape and budding probability | Actual budding-block density is much lower in these windows | Partial; identify instances and measure radius/shell/cracks, water and vertical distributions before retuning |',
    '| Nether fortress | Shared Nether placement set, procedural layout; supports | Blaze spawners; chest blocks without fortress-loot initialization | Partial; terrain/bridge supports and piece frequencies unavailable |',
    '| Bastion remnant | Shared placement set; sampled layouts | 165 forms, 40 layouts; chest blocks without bastion loot; no bastion-specific brute/piglin/hoglin generation hook found | Partial; layouts, mob positions and counts, chest types, magma-cube spawners need measurement |',
    '| Nether ruined portals / fossils | Portals use Nether height search; fossils absent | Portal loot exists; no fossils | Partial / missing |',
    '| End cities with ships | Island/height gate and procedural piece attachment | Shulker hook exists; chest blocks lack End-city loot; ship piece frequency recorded below | Partial; no reference End chunks; need ship loot, elytra frame and sentry/attachment measurements |',
    '| End gateways | No generated gateway feature | No sampled blocks | Missing; outer-island generation must be measured separately from dragon-fight gateway lifecycle |', '',
    'Generated blocks versus initialized container records (a container count includes non-chest types, so these totals are not a per-chest match):', '',
    '| Dimension | Chest blocks | Barrel blocks | Container records |', '|---|---:|---:|---:|']
    for dim,r in summary.items(): lines.append(f'| {dim} | {r["blocks"].get("chest",0)} | {r["blocks"].get("barrel",0)} | {len(r["containers"])} |')
    ship_cities=sum(any('ship' in p for p in s.get('piece_types',[])) for s in starts['the_end'])
    lines += ['', f'End sample: **{len(starts["the_end"])} cities, {ship_cities} with a ship piece**. Piece names/counts and full boxes are in the start evidence. Our containers store eagerly generated item slots; a persistent `LootTable` field is not represented. That schema difference must not be confused with the separate bug where a chest has neither loot nor a container record.', '',
    '## Support, air below floors, terrain adaptation and chunk borders', '',
    'The diagnostic paints one local production-builder mask for the first village/outpost/ancient-city/trail-ruin start, then compares it with final generated blocks. Six-connected collision-bearing structure cells are grouped. A component is provisionally supported when adjacent to an external full cube. Missing neighbor data marks it unknown. This is a conservative voxel diagnostic: it does not model sub-block collision connections or prove intentional floating decoration is a bug. Air below an exposed cell includes roofs and overhangs, not just floors.', '',
    '| Own specimen anchor X,Z | Painted chunks | Retained solid cells | Components without external support | Exposed cells with air immediately below | Mask/final changed examples |', '|---|---:|---:|---:|---:|---:|']
    for s in support: lines.append(f'| {s["anchor"]} | {s["chunks"]} | {s["structure_solid_cells"]} | {len(s["unsupported_components"])} | {s["air_below_exposed_cells"]} | {len(s["changed_examples"])} |')
    lines += ['', '- Village flags include `(-8380,83,-8708)`, `(-8381,83,-8710)`, `(-8422,73,-8717)` (component representative coordinates can vary with hash iteration). Ancient-city flags include larger disconnected components; inspect the JSON before changing geometry. No unsupported component was flagged in the sampled outpost or trail ruin.',
    '- No changed blocks were found in these sampled masks. This does **not** prove absence of chunk-border clipping: a builder using the same too-small search radius in both paths can omit a piece twice. An independent monolithic layout-to-chunk comparison remains needed for every structure.',
    '- Village/outpost adaptation explicitly uses a cubic 12-block falloff; it is an approximation. Ancient city, stronghold, trial chamber and trail-ruin paths lack corresponding dedicated density adaptation. The requested vanilla slope shape, added-ground heights and carved-space comparison cannot be measured from worlds without these structures.',
    '- Vanilla support/connected-component counts and original pre-structure terrain are unavailable. No unsupported geometry was “repaired” based only on these flags.', '',
    '## Fixes made and prioritized remaining work', '',
    '1. **Done — spawner lifecycle during generation.** A rejected non-overwriting placement can no longer create a spawner record behind a solid block. A later generated non-spawner block removes the overwritten spawner record. Regressions cover rejection, replacement and save/load. The full pre-fix Overworld census had 582 spawner records for 580 physical spawners; after the fix both counts are 580, with zero mismatches. Removed ghost records were at `(-8427,-10,-7948)` and `(1020,22,-580)`.',
    '2. **Done — dungeon spawner randomness.** Mob choice now uses the already world/chunk-dependent room seed, retaining zombie/skeleton/spider weights 2:1:1. Previously identical local geometry chose the same mob independently of world/chunk. The regression exercises repeated and varying room seeds.',
    '3. **P0 — obtain suitable reference worlds.** Need structure-enabled vanilla 26.1.2, the same seed and nine fully generated windows in all dimensions, including border halos larger than the largest structure. Preserve `structures.starts`, block entities and entity regions; save after generation with minimal simulation. Never mix the biased sample into rate denominators.',
    '4. **P1 — missing village inhabitants and loot.** Add a generation-once entity path using measured per-piece markers (villager type/age/profession; golem; cat variants; desert camel/animals). The wiki documents these occupants, but our sampled libraries discarded entity markers. Guessing one entity per bed or one cat/golem per village would not establish the requested population statistics. Measure several rotations/biomes and verify collision filtering, persistence and reload behavior.',
    '5. **P1 — generated chest functionality.** Implement and verify dungeon/village/fortress/bastion/End-city container initialization and correct table-specific loot. Add ship elytra/item-frame and bastion entity/spawner coverage. Audit actual container position against block after overlapping structure painting.',
    '6. **P1 — large missing generation features.** Taiga/snowy village layouts, wells, Overworld/Nether fossils, outer End gateways. Obtain biome-conditioned rates, terrain-relative bounds, variants and piece/entity/loot inventories. No substitute layouts were invented.',
    '7. **P2 — terrain adaptation and connected geometry.** Measure village/outpost skirts, ancient-city room envelopes, stronghold burial, trial chamber encapsulation and trail-ruin hillside burial. Retain pre/post terrain profiles if obtainable by independent controlled world measurements; classify intentional ruin gaps before adding supports.',
    '8. **P2 — procedural composition.** Replace uniform selection from a small set of complete sampled layouts with independently implemented placement/connection rules where needed. Measure piece counts/types, graph depth, size, rotations, failed attachments and abandoned villages; fixed samples cannot demonstrate the same generative rules.',
    '9. **P2 — placement statistics.** Once the reference exists, compare per-window and biome-conditioned rates with uncertainty, proper edge treatment, exclusion zones and vertical gates. Same seed does not remove terrain/biome exposure differences. Retune geode counts only after separating instance frequency from size/budding density.',
    '10. **P2 — dungeon chunk-boundary bias.** Current rooms are constrained entirely inside their source chunk. Need neighboring terrain queries and deterministic cross-chunk carving before changing that rule.', '',
    '## Sources and reproduction', '',
    'Primary evidence is the saved 26.1.2 region/level metadata and our own generator. Wiki prose consulted: [Village/Structure](https://minecraft.wiki/w/Village/Structure) (occupants and terrain support), [Monster Room](https://minecraft.wiki/w/Monster_Room) and [defeating a monster room](https://minecraft.wiki/w/Tutorial:Defeating_a_monster_room) (spawner and chest behavior), [Loot table](https://minecraft.wiki/w/Loot_table) (lazy table data). No post-26.1.2 feature was used. Some direct wiki fetches were robots-blocked; the report does not treat unavailable pages as verified numeric data.', '',
    'Commands from the copy root (all temporary outputs stay under `tmp/`; scripts read vanilla worlds only):', '',
    '```sh',
    'PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit.py ~/.cache/mcsheriffanya/vanilla-big/world tmp/vanilla-overworld.json',
    'PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit.py ~/.cache/mcsheriffanya/vanilla-big/world tmp/vanilla-nether.json --dimension the_nether',
    'PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit.py ~/.cache/mcsheriffanya/vanilla-normal/world tmp/normal-all-overworld.json --all',
    'PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_features.py ~/.cache/mcsheriffanya/vanilla-big/world tmp/vanilla-overworld.json tmp/vanilla-features.json',
    'CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/ours-final AUDIT_DIM=overworld nice -n 19 cargo test --release -j 4 offline_structures_audit -- --ignored --nocapture',
    '# Repeat AUDIT_DIM=the_nether and the_end; baseline files for those runs are in tmp/ours-baseline.',
    'CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/ours-final nice -n 19 cargo test --release -j 4 offline_structure_support_audit -- --ignored --nocapture',
    'PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_report.py',
    '```', '',
    'Validation: plugin examples built for `wasm32-wasip2` first. `cargo test --release -j 4`: 2,207 passed, 64 ignored (including explicit offline measurement tests, run separately). `cargo clippy --release --all-targets --workspace -j 4`: clean. Three Python census unit tests passed. The later user-requested `-j 4` was used; cargo invocations were serialized. No `cargo fmt`, commits, pushes or server operations. No new black-box scenarios: the two fixes are internal generator invariants covered offline; a socket-based scenario would not test their trigger.', '',
    '**Remaining limitation:** no honest full vanilla-versus-ours 1:1 verdict or paired structure coordinates can be produced from the supplied structure-disabled/absent reference data. Population/loot fixes requiring missing measurements and the detailed geometry census remain partial, as identified above.']
    (ROOT/'tools/research/structures-audit.md').write_text('\n'.join(lines)+'\n')

if __name__=='__main__': main()
