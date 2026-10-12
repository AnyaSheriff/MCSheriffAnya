## Fixes, before/after and priorities

The baseline was regenerated from **this copy**, not copied from the first report. This copy already contains later dungeon/carver/geode work: for example, its baseline has 1,346 Overworld spawners and 69,262 budding-amethyst blocks. Differences from the first report are not credited to this task.

| Change | Before | After | Verification and limits |
|---|---:|---:|---|
| Bastion spawners assigned magma cubes, core physical records | 0 magma / 63 blaze | 6 magma / 57 blaze | All 63 positions and blocks unchanged; exact six changes in `spawner-fix.json`; generated-chunk and reload regression |
| Taiga village accepted starts, core windows | 0 | 2 | Vanilla has 1. Measured 32 forms and one layout at vanilla start chunk (510,484); no invented snowy substitution. This restores the type, not its distribution |
| End-ship elytra item frames, core chunks | 0 | 4 | Four sampled core-start ships now each retain one frame. Measured static frame coordinates from halo ships; rotation, supporting wall, negative coordinates and chunk save/reload tested |
| Overworld chest blocks | 2,404 | 2,425 | The two taiga villages add 21 chests. Their loot and inhabitants remain missing; this is a partial village implementation |
| Nether / End chest initialization | 34 / 0 container records | 34 / 0 | Unchanged. Core worlds contain 481 / 46 chest blocks; this functional gap remains open |

End example: our frame at **(-518,126,984)** contains elytra and faces west. Measured vanilla frame centres include **(-7634.5,109.5,-265.96875)** (south) and **(-7734.96875,125.5,7753.5)** (east); both reduce to ship-local frame cell **(6,5,7)**. There are no ships among the four vanilla **core-start** cities, but five ships among halo starts, with four frame associations usable directly inside their saved bounds. Halo specimens inform geometry, never core start-rate denominators. Full evidence is `ship-frame-measurements.json`.

The additional border audit bypasses production `near()`/search radii: it supplies the complete accepted plan directly to each intersecting chunk painter, then compares surviving solid cells against `Chunk::generated`. This catches a plan piece omitted by a too-small production search radius. It is stronger than calling the same radius-limited builder twice, but does not independently validate the planner or intentional topology.

| Our family | Core starts checked | Planned solid cells | Changed cells | Chunk-seam cells checked | Changed seam cells |
|---|---:|---:|---:|---:|---:|
| Plains villages | 3 | 14,526 | 0 | 3,467 | 0 |
| Taiga villages | 2 | 9,744 | 0 | 1,673 | 0 |
| Fortresses | 28 | 772,022 | 0 | 175,287 | 0 |
| Bastions | 32 | 1,505,683 | 0 | 372,762 | 0 |
| End cities | 13 | 69,337 | 0 | 10,011 | 0 |

This checks **2,371,312 solid cells**, including **563,200 seam cells**. No clipping correction was indicated in these families. Mineshaft, stronghold, portal, ruin, outpost and snapshot-family monolithic comparisons remain unverified; their saved bounds and probes alone do not establish absence of clipping.

Prioritized remaining work:

1. **P1 — initialize generated loot. Partial / unfixed.** Dungeon, village, fortress, bastion and End-city chests lack their expected loot initialization. The new reference resolves identifiers (including all four bastion table families), positions and pyramid basements. It does not resolve unopened contents or current pool weights. The indexed wiki loot module explicitly describes older Java data and is incomplete even for known trim pools; no older/invented table was presented as verified 26.1.2 loot. Implement version-verified pools and template-to-table markers, then test generation, overlap and reload. The taiga addition increases uninitialized chest blocks by 21; it does not solve village loot. Empty job-site barrels must not automatically receive loot.
2. **P1 — village/bastion inhabitants. Partial / unfixed.** Vanilla complete core village boxes contain 109 villagers; the taiga specimen has 12 (types/professions/ages retained), and bastions contain 169 brutes, 789 piglins and 49 hoglins. Some saved occupants moved or were born after generation. Derive validated per-piece spawn markers and a once-only persistence path; do not transpose all save-time animals or professions into every instance. Existing shulker/hut/igloo/outpost/mansion/monument hooks were counted as plans, not declared absent. Ship frames were safe to restore because their positions are static and repeated across rotations.
3. **P1 — missing types/features. Partial.** Taiga now has a measured layout; snowy villages remain skipped because no snowy specimen exists in this reference. Nether fossils have **793 core starts** and measured piece names/boxes but no own generator; end gateways have **12 core block entities** and no own generator. Fossil placement/support search and gateway exit/lifecycle semantics still need an independently derived implementation; their absence is not a frequency knob. Four desert-well pairs (8 archaeology records) are present in the core Overworld. Wells and Overworld fossils remain unimplemented. No jar-derived assets or substitute layouts were used.
4. **P2 — biome/terrain acceptance before frequency tuning.** Vanilla versus ours: desert villages 6/0, savanna villages 4/0, pyramids 4/0, buried treasure 22/4, End cities 4/13, ancient cities 1/8, monuments 4/10; trial chambers are 45/45. These are real same-area differences, but the terrain and biome fields differ. Validate biome-conditioned exposure and start-height gates before adjusting spacing or hard-coding this seed. Nether fortress 30/28, bastion 35/32 and portal 35/34 intervals overlap. No spacing/height rule was changed without a demonstrated mechanism.
5. **P2 — actual floors and density adaptation. Partial.** Ring-height and datum probes are measured for every vanilla piece and all exported own pieces. They do not isolate beard_thin/beard_box/bury/encapsulate deformation from natural terrain, foliage, neighbouring structures, or Nether ceilings. Dedicated paired terrain and voxel floor segmentation are still required for a 1:1 adaptation/floating verdict. Whole-snapshot cities/chambers lack comparable jigsaw decomposition. Do not interpret metadata BB air or End bridges as unsupported floors and add ground blindly.
6. **P2 — generative composition. Partial.** Full saved vanilla piece/type/rotation/box inventories and our available plans are retained. One taiga layout and the existing fixed libraries reproduce sampled arrangements, not the original distribution. Add more independently measured layouts or reconstruct connection rules. Taiga samples may retain natural blocks captured inside a piece; one specimen cannot establish a clean procedural template or abandoned-village frequency.

## Validation and reproduction

Final checks: **2,238 Rust tests passed, 0 failed, 65 ignored** (`RUST_TEST_THREADS=2`); **workspace/all-target release clippy passed with no warnings**; **13 Python audit tests passed**. The ignored native generation audits and independent border audit were run explicitly and passed. Plugin WASM examples built successfully for `wasm32-wasip2` before the root suite. Logs: `tmp/audit2/tests-final.log`, `clippy.log`, `python-tests.log`, `plugins.log`, `border.log`.

All commands operate in the copy. Cargo invocations are serialized; the explicit user-requested `-j 4` is used for root tests and clippy. Plugin WASM examples are built first with the copy's own target/tmp directories. No formatting, commits, pushes, sockets or Minecraft server operations.

```sh
# Reference census and geometry; repeat DIM=the_nether and the_end.
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2.py ~/.cache/mcsheriffanya/vanilla-struct/world tmp/audit2/vanilla-overworld.json --dimension overworld --geometry
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_integrity.py ~/.cache/mcsheriffanya/vanilla-struct/world tools/research/structures-audit-2-data/integrity.json
# Reproduce the measured taiga asset (no network/server entry point).
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_extract.py ~/.cache/mcsheriffanya/vanilla-struct/world tmp/audit2/vanilla-overworld.json tmp/audit2/taiga-reproduced.txt tmp/audit2/extract
# Native start export, then probe/chunk selection; repeat each dimension.
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/audit2/after AUDIT_DIM=overworld AUDIT_HALO=8 AUDIT_LIMIT=0 nice -n 19 cargo test --release -j 4 offline_structures_audit -- --ignored --nocapture
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_probes.py tmp/audit2/after
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/audit2/after AUDIT_DIM=overworld AUDIT_HALO=8 AUDIT_CHUNKS=$PWD/tmp/audit2/after/overworld-selection.json AUDIT_PROBES=$PWD/tmp/audit2/after/overworld-probes.json nice -n 19 cargo test --release -j 4 offline_structures_audit -- --ignored --nocapture
CARGO_TARGET_DIR=$PWD/target CARGO_NET_OFFLINE=true TMPDIR=$PWD/tmp AUDIT_OUT=$PWD/tmp/audit2/after nice -n 19 cargo test --release -j 4 offline_structure_border_audit -- --ignored --nocapture
PYTHONDONTWRITEBYTECODE=1 python3 -m unittest discover -s tools/measure -p 'test_structures_audit*.py'
PYTHONDONTWRITEBYTECODE=1 python3 tools/measure/structures_audit2_report.py
```

Raw before/after chunk JSONL and logs remain in `tmp/audit2`. Start-rate denominators are exactly 49,152 core chunks per dimension in both runs. Extra generated chunks supply geometry only. Regression tests cover malformed/truncated records, full chunks missing sections, palette lengths, halo separation, negative bounds, NN censoring, deterministic window intervals, pyramid basement attribution, magma-cube persistence, measured frame geometry/persistence and taiga whole-plan versus tiled painting.

No new black-box scenarios: the generation triggers and chunk serialization are exercised by the offline generator/regressions. Manually placing a spawner or frame would not exercise the fixed generation path. Runtime structure-population tests remain unverified. An unrelated pre-existing plugin test assumed a global tick counter of zero and failed under parallel tests (due 1243 instead of 1200); its assertions now use the plugin's actual enable tick, preserving the 1200-tick delay/period check.

Primary evidence: saved 26.1.2 world metadata, region/entity records, and this project's generator. Prose cross-checks: [End City](https://minecraft.wiki/w/End_City#End_ship), [Bastion magma-cube spawners](https://minecraft.wiki/w/Tutorial:Froglight_farming#Bastion_Remnants). [The indexed loot module](https://minecraft.wiki/w/Module:LootChest) exposes older data and was **not** used to invent current loot pools.

**Audit status: partial 1:1 coverage.** Counts, saved composition/entities/loot IDs, observed geometry profiles and selected independent border checks now have a real structure-enabled reference. Exact floors/floating classification, isolated density adaptation, complete procedural composition and several functional gaps remain unresolved as listed above.
