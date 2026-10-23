# Step 1 continuation —776 admitted,774 closed

Latest continuation: [772 admission and773/774/776 regression](../client-chain-770-773/CONTINUED.md#772-admission-and-773774776-regression).772/773/774/776 are open;770–771 remain closed on both primaries. The historical sections below retain their original admission state and evidence.

The merged baseline initially failed (2903 passed, one startup-lock failure, 81 ignored).
StartupGuard now explicitly unlocks flock before closing; a duplicate-descriptor regression covers inheritance between fork and exec.
Final checks: 2909 core tests (81 ignored), 203 protocol tests (5 ignored), 213 Python tests; Clippy with -D warnings and release build pass.

The private776 adapter now permits five pinned entity layouts, tracks player movement and validates placement/use/attack/respawn actions. Unknown layouts and digging enums remain closed.
Three distinct raw scenarios were run once each; all original failures are retained:
- `gameplay-776-primary775/`: login/chunks/inventory and mixed775 spawns passed. Raw peer health is20→19→20; the robot sampled after regeneration and failed before reverse PvP.
- `lifecycle-776-primary775/`: placement/consumption, death/respawn/ACK and inventory pickup/deposit passed; the torch-update assertion failed.
- `relog-light-776-primary775/`: relog preserves three diamonds; embedded initial light masks/arrays decode, but no changed torch light was observed. This scenario also failed its final assertion.
The lifecycle reports' peer field names the configured775 peer; no peer connected in those scenarios. No complete scenario or client×primary pair is declared ready.
All three fixture servers stopped with stdin EOF and exit0; their temporary worlds were removed. The fixture uses primary775 and port26028; it does not test persistent-world restart.

`official-777-continued.json` / `.log`: exactly one copied official26.3 run, Java25, stdin=/dev/null, ports26186/26187,7/7 checks, stopped by PID (exit143). Its temporary copied folder was deleted.
`viaproxy-minimum.json`: production binary rejects319M with a Russian320M minimum error before world creation. README defaults to512M. This branch has no ViaProxy launcher.
`continued-verification.json` records counts, artifact hashes and the four closed admission cells. Existing evidence files were preserved.

Remaining:774 gameplay, both clients on primary26.3, dynamic lighting,776 mining enums, GUI/containers, reverse PvP, mixed777, restart persistence, AuthMe/online-mode. No admission changes, commits, pushes or25565 listener.

## Further continuation — dynamic light fixed, admission still closed

The core previously sent light only with the initial chunk. Block edits now recalculate and send full Update Light masks/arrays for the affected chunk and its loaded neighbors in view. A regression checks source14/neighbor13→0 across a chunk boundary through775/776/777 encodings. Primary gameplay rules are unchanged.

| Client / primary | Newly recorded evidence | Full-checklist blocker |
|---|---|---|
| 776 / 26.1.2 | `dynamic-light-v2-776-primary775`:3/3 (login/chunks, relogged inventory, torch14→0). `gameplay-v2-776-primary775`:7 completed checks with775 peer. `mixed777-v3-776-primary775`:7 completed checks with777 peer, including both PvP directions, placement/consumption, death/respawn, chest deposit/close/reopen/withdraw/close | Both full scenarios fail when relogging near the chest: `Chunk(Entity(UnsupportedVersion))` |
| 776 / 26.3 | `light-v2-776-primary777`:3/3; `gameplay-776-primary777`:7 completed checks with777 peer, same actions and PvP20→19 in both directions | Same chest-area relog failure; this is an actual Preview26_3 catalog, not a renamed775 fixture |
| 774 / both | Existing pinned tables/control tests and public rejection traces only | Full Play adapters (items/components, registries, metadata, time/interactions, chunks/light) and gameplay traces remain absent |

Mining is **not proven**: `mining-denial-776-primary775` records the sent action0 and the guarded server rejection; the enum is not enabled by copying777 or assuming equal VarInt schemas. Block-entity-bearing chunks stay denied. Dimensions/recipes, persistent-world restart, AuthMe and online-mode remain unproven for both clients. No public pair is ready; the existing Russian rejection is preserved.

Earlier failed artifacts remain intact. New intermediate failures are retained too: `dynamic-light-776-primary775` is a Python observer scoping error; `dynamic-light-776-primary777` hit the old Play preview guard; `mixed777-776-primary775` hit the fixture's777 admission guard; `mixed777-v2-776-primary775` caught a wrong robot expectation for player entity ID (156 instead of pinned777 ID159). The corrected runner uses pinned peer tables. These failures are not counted as gameplay passes.

The private test fixture can now select the actual primary26.3 catalog and admit a777 partner without changing production admission. The776 chunk path receives the canonical content codec explicitly. Health history retains20→19 even if a later sample regenerates; raw chest window/cursor responses and sent clicks/close packets are recorded. All temporary fixture servers shut down by stdin EOF with exit0; their worlds were removed. No new official-server run, commit, push or25565 listener.

Verification:2910 Rust tests passed (2907 core +3 startup;82 ignored),203 protocol tests (5 ignored),216 Python tests; Clippy `--release --all-targets -- -D warnings` clean. Release build and all four final public rejection cells are recorded in `followup-verification.json`; no full scenario is inferred from its successful individual checks.

## Mining, chest-area relog and disk-backed restart — all admission still closed

`official-actions-776/` retains the single official26.2 run (verified opaque JAR, Java25, copied temporary folder, stdin=/dev/null, ports26188/26189, stopped by PID, exit143). Start0/finish2 produced the authored stone→air update; candidate1 did not break it and candidate3 dropped the held tool. A new login received chest type1 with NBT End. The light observer then rejected that chest-bearing chunk: **the overall run remains failed**, and was not repeated. `test_step1_actions_776.py` checks the raw observations independently; the corrected runner is retained for future use.

The private776 adapter now maps only observed mining0/2 and chest type1 with empty network NBT. Other776 actions, block-entity types and nonempty data remain denied. Mining effects pass the existing typed world-event translator. Production admission and primary gameplay rules are unchanged.

| Client / primary | New trace | Result |
|---|---|---|
| 776 / 26.1.2 | `mining-chest-restart-776-primary775/` | 11/11 PASS |
| 776 / 26.3 | `mining-chest-restart-776-primary777/` | 11/11 PASS |
| 774 / both | No full Play adapter or gameplay/restart/authentication trace | CLOSED |

Both successful traces cover mixed777 player packets/PvP in both directions, placement/consumption, mining to air with one cobblestone and pickaxe damage1, death/respawn, inventory transfers, chest close/reopen/withdraw, relog next to the chest, dynamic light14→0 and a full process restart. The optional fixture uses disk-backed Overworld storage; after restart the client reads the chest and withdraws the same3diamonds. Initial and final fixture processes exit0; temporary worlds are removed, and file hashes remain in each report. Nether/End remain in memory and are not claimed as persistence evidence.

**No pair has passed its full checklist.** 776 dimensions/recipes, all entity/component/block-entity layouts and authentication are incomplete. The private776 fixture explicitly refuses online-mode/password authentication; those guards were not bypassed. 774 still lacks full Play codecs, dynamic registries, gameplay, restart and authentication proof on either primary. All four public cells keep the existing Russian refusal. No commit/push or25565 listener.

Final verification:2910 Rust tests (82 ignored),205 protocol tests (5 ignored),218 Python tests; Clippy `--release --all-targets -- -D warnings` and release build PASS. `admission-mining-chest.json` records8/8 fresh Status/rejection checks with the final production binary. Counts, hashes, logs, all four closed cells and retained failures: `mining-chest-verification.json` / `mining-chest-verification/`.

## Dimensions, recipe book and password dialog — admission remains closed

The private776 path now validates bounded recipe requests and inline Configuration dialogs, including truncation/tail/boolean rejection. It allows recipe ghost responses through the existing typed recipe/item converter. Only the isolated loopback test fixture can enable password auth; production admission and gameplay profiles are unchanged.

| Client / primary | New evidence | Result |
|---|---|---|
| 776 / 26.1.2 | `dimensions-recipes-v3-776-primary775/` | 4/4 PASS: full book update1478 entries; ghost/autofill; one log consumed for four planks; portal Nether→Overworld, chunks/short ACK, server completion barrier |
| 776 / 26.3 | `dimensions-recipes-v2-776-primary777/` | Same4/4 PASS; full primary book update1677 entries, actual Preview26_3 catalog |
| 776 / both | `password-dialog-776-primary{775,777}/` | Registration and relog wrong→correct password PASS; no new player/join before password success, two Clear Dialog envelopes each;5 recorded checks each (the clear check occurs twice) |
| 774 / both | No new full Play/gameplay evidence | CLOSED |

These are packet clients, not graphical-client proof. Crafting checks the authoritative input count2→1, empty grid and four-plank cursor. Recipe decoding is full-book coverage, not live crafting of every recipe. Nether/End persistence and End travel are not claimed. Chat password fallback does not exist in this core and is not implemented here; online-mode remains unproven.776 still supports only five observed entity metadata layouts and empty chest block-entity data; remaining components/actions/payloads are incomplete. **All four public version/primary cells stay closed with the existing Russian refusal.**

Earlier results are retained: `dimensions-recipes-776-primary775/` invoked the production executable instead of the test executable and failed before listening. `dimensions-recipes-v2-776-primary775/` passed transport assertions but predates explicit consumption/completion assertions. `dimensions-recipes-776-primary777/` passed transport assertions, but its server log lacks return completion and ends in Broken pipe; it is not the final completion proof. The retained first Python verification failure exposed this gap; final traces require the return log plus a command barrier. Password traces retain the runner hash from before these portal-only assertion additions.

`official-777-dimensions-continuation/` is only a native777 reference regression, **not a dimension/recipe776 observation**: one copied official26.3 run, Java25, stdin=/dev/null,26194/26195,7/7, stopped by PID(exit143); copied server/world removed. All private fixtures exit0 and their temporary worlds/auth databases are removed. No25565 listener, commit or push.

Final verification:2910 Rust tests (82 ignored),206 protocol tests (5 ignored),221 Python tests; Clippy `--release --all-targets -- -D warnings` and release build PASS. Raw hashes, final admission rejection8/8 and source hashes: `dimensions-recipes-verification.json`; check logs: `dimensions-recipes-verification/`.

Build cache retained outside the worktree at `/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target` (use `CARGO_TARGET_DIR` for the next continuation). Generated Cargo.lock, Python caches, fixture worlds/auth databases and this run's temporary test leftovers were removed; pre-existing `codex.log` and `plugins-examples/target/` were left intact.

## Payload expansion and shared encrypted login — public admission still closed

`generate_payload_776.py` checks the pinned775/776 metadata discriminator and Creative Slot envelope, component schema graphs (with explicit typed ID-remapper boundaries), and common particle payloads. `payload-776-report.json` records hashes:158 schema-based entity layouts and30 typed component kinds, plus the four previously supported nested forms. Creative Slot now checks component lengths; nonempty particle metadata and old-layout world particles translate through typed ID remappers. Primary777 extensions preserve sulfur cubes and translate cushion spawns/metadata to armor stands. This is **schema/synthetic evidence**, not158 live entity observations or complete metadata/component coverage.

Six new Rust codec regressions cover54 complex item cases,10 inventory samples,23 particle effects,11 recorded entity types, all158 layouts' shared flags, nested ID/length rejection, and all four primary777 entity extensions. The existing encrypted-login unit test now runs775/776/777 through the same RSA/session-service implementation, compression and verified identity, checking a separate session UUID for776/777. **Owner decisions applied:**776 chat fallback is not required; this shared-code unit proof satisfies online-mode verification without a live Mojang account. Neither is a remaining admission blocker.

| Client / primary | Fresh private regression | Public admission |
|---|---|---|
| 776 / 26.1.2 | `payload-regression-v2-776-primary775/`:11/11, mixed777 PvP, placement/mining, death/respawn, inventory/chest/relog/light and disk-backed process restart | CLOSED |
| 776 / 26.3 | `payload-regression-776-primary777/`:same11/11 with the actual Preview26_3 catalog | CLOSED |
| 774 / both | No new full Play/gameplay proof | CLOSED |

The first `payload-regression-776-primary775/` run remains failed: a leftover death/mining cobblestone was picked up between inventory clicks (raw SetSlot follows cursor pickup; three diamonds still reach slot37). The corrected robot removes item entities, with a command barrier, before clearing/giving the inventory fixture; no gameplay rule was changed. All fixture processes exit0 and temporary worlds are removed.

Remaining776 blockers: block_dig cancel/drop/release/swap/jab meanings beyond observed0/2; block-entity IDs/data beyond empty chest1; unsupported components/metadata serializer41 and rich text; remaining serverbound GUI/action branches and fresh live coverage for the expanded payloads. End travel and Nether/End persistence are not proven. Full774 codecs, registries, gameplay/restart/authentication remain incomplete. These unresolved checks keep all four public cells closed with the existing Russian message; no old-version gameplay profiles were added. **The requested full admission task is not complete.**

`official-777-payload-continuation/`:one copied official26.3 run, Java25, stdin=/dev/null,26196/26197,7/7 reference checks; stopped by PID(exit143), copied server/world deleted. This is not776 proof. `admission-payload-final.json`:8/8 fresh final-binary Status/refusal checks (the earlier admission check is also retained). The expanded workspace check also exposed and fixed an existing seedmap test's unavailable gameplay-catalog reference and two protocol-test Clippy warnings. Final counts/hashes/logs are in `payload-continuation-verification.json` and `payload-continuation-verification/`; earlier failed checks remain retained. No25565 listener, commit or push.

Final test counts:3133 Rust PASS (2907 core +3 startup +212 protocol +11 seedmap;89 ignored),226 Python PASS. Workspace Clippy `--release --workspace --all-targets -- -D warnings` is clean; required root-only Clippy also passes. The build cache remains at `/home/lenox/.local/share/mcsa-java-26.3-probe/s26-mv5-target`.
Release build PASS; generated Cargo.lock, Python caches and tmp test leftovers removed. Raw gameplay traces precede only test-only lint fixes and indentation; their original test-binary hashes are retained separately from the final build hashes. Pre-existing codex.log and plugins-examples/target remain untouched.

## 776 admission and evidence

The remaining776 work now has live proof on both actual primaries. Public admission is enabled only for776/26.1.2 and776/26.3;774 codecs and primary gameplay profiles were not changed. The existing Russian refusal remains for closed pairs. All earlier failures and their source hashes are retained.

| Existing checklist area | 776 /26.1.2 | 776 /26.3 |
|---|---|---|
| Expanded payloads and remaining actions | `payload-actions-final775/`:196/196 per-payload Pong barriers,6/6 real GUI actions with authoritative outcomes,4/4 statue pose changes | `payload-actions-final777/`:same; actual Preview26_3 catalog |
| Mixed777 PvP, placement/mining, death/respawn, inventory/chest/relog/light and disk restart | `final-gameplay775/`:11/11 | `final-gameplay777/`:11/11 |
| Recipes/crafting, Nether return, password auth | Earlier final traces above retained;4/4 plus registration/relogin before player creation | Earlier final traces above retained;4/4 plus registration/relogin before player creation |
| Online login | Shared RSA/session-service test775/776/777, compression/encryption/identity/session UUID | Same shared implementation |
| Production admission | `public776-final/`:actual release Login/Configuration/Play/chunks/inventory/ACK/barrier | Same release binary; honest Status777 |

The official26.2 client emits0=start,1=cancel,2=finish,3=drop-stack,4=drop-one,5=release,6=swap,7=jab. `action-client-collector775-v3/` first records the ordinary GUI meanings while filtering unenabled actions; the final runs forward every action and verify retained obsidian, exact hand counts/IDs, arrow spawn and spear wear. The first collector's Xvfb/NVIDIA initialization failure and the second collector's short Ctrl+Q/unenhanced spear failure remain separate failed artifacts. The corrected runs use Mesa in a private Xvfb. No packet injection supplies player actions.

`payload-live-776.json` is generated by the ignored Rust export, not handwritten wire data. It includes54 recorded complex items,6 additional simple components, rich text,23 particles, mannequin/profile41,43 fresh reference block-entity samples,5 populated block-entity forms,17 remaining primary-writer components, statue data,11 recorded entity layouts and4 preview projections. These translated fixtures are injected only into the isolated test relay and acknowledged individually by the opaque client. They prove decoding/translation, not every entity's gameplay or every recipe.53 typed component kinds plus4 nested forms cover the current core writer; arbitrary custom forms beyond that writer still fail closed. Item names in shelf/campfire/vault NBT use the content mapper, preserving canonical saved items. Preview Creative Slot uses the existing authoritative inventory guard, as775 already does.

Three live failures corrected assumptions that synthetic roundtrips and equal JSON graphs could not expose:
- `payload-actions-live775-v2/`:87 payloads accepted, then776 rejects NBT End in a block-entity Update. Updates now send an empty compound; optional empty chunk NBT is preserved.
- `payload-actions-instrument775/`:139 accepted, then the instrument payload has one extra byte.776 uses a direct holder; the canonical core writer retains its bool+holder representation, with an explicit typed conversion.
- `payload-actions-bees775/`:146 accepted, then a hive occupant's NBT lacks the preceding entity-type VarInt. The converter now maps that type and keeps the core's named entity data. Non-bee occupants unsupported by the core are refused.

`payload-actions-statue-floor775/` accepted all196 payloads but its GUI scenario failed because the player fell beside the narrow authored platform and clicked the floor. The final fixture authors that floor explicitly. The final775 screenshots include intermediate pose animation frames; final777 screenshots wait for the animation. Both raw traces independently verify all four authoritative pose states. `statue-776-evidence.json` pins the existing official775 observation of type48 and documents the post-bed776 mapping47 plus both live GUI traces; no fresh official776 server observation is claimed.

Exactly one copied official26.3 server ran for this continuation: `official777-blockentities-reference/`, Java25, stdin=/dev/null,26190/26191,43 authored block-entity samples; stopped by PID, copied world removed. Piston and bed are absent from that chunk's block-entity list; bed removal in26.2 is documented at [Minecraft Wiki](https://minecraft.wiki/w/Java_Edition_26.2). Rich text without embedded item/entity registry references follows [Text component format](https://minecraft.wiki/w/Text_component_format). No Mojang code, JAR unpacking, reports, other server cores, proxies or protocol libraries were read.

End travel and Nether/End persistence remain unproven and are documented without adding gates to this run. A live Mojang account and chat password fallback remain excluded by the recorded owner decisions. The53 component kinds/158 layout tables are still schema-derived; the new live traces establish the enumerated payload coverage, not158 distinct entity gameplay observations. This completion concerns the core's supported client payloads and existing checklist, not additional vanilla mechanics or old-version gameplay profiles.

Final test counts, release/Clippy results, all four admission cells, source/fixture/binary hashes and retained verification failures are recorded in `completion-776-verification.json` and `completion-776-verification/`. One full parallel check hit the existing TNT timing assertion; another hit a plugin HUD assertion. No timing bound was weakened. Final checks run serially. Temporary worlds, auth databases, Cargo.lock, Python caches and test leftovers were removed; the external build cache and pre-existing codex.log/plugins-examples/target remain. No25565 listener, commit or push.

Final verification:3145 Rust PASS (2908 core +3 startup +223 protocol +11 seedmap;90 ignored),233 Python PASS. Workspace and root Clippy `--release --all-targets -- -D warnings` and release build PASS. `public776-final/` passes8/8 against the final production binary:776 reaches Play on both primaries,774 keeps the Russian refusal, Status remains honest. The first production observer used a token its inherited marker parser did not recognize; `public776-marker-name/` retains that failed observer result, and the corrected runner uses the existing marker grammar. All196 payload/GUI traces precede only the public-admission switch, its test flag alignment and the preview Creative Slot guard; the final full suite/release/public trace cover those changes.

## 774 admission and 776 regression

774 completes the same existing checklist on both actual primaries. Both774 admission cells are now open;776 remains open. No old-version gameplay profile or new admission gate was added. Unsupported routes keep the existing Russian refusal.

| Existing checklist | 774 /26.1.2 | 774 /26.3 |
|---|---|---|
| Opaque1.21.11 client, payloads/actions/poses | `774-client775-v7/`:196/196 Pong barriers,6/6 GUI actions,4/4 statue poses | `774-client777-final/`:same196/6/4 |
| Mixed777 PvP, place/mine/drop/wear, death/respawn, inventory/chest/relog/light and disk restart | `774-gameplay775-final/`:11/11 | `774-gameplay777-final/`:11/11 |
| Full recipe book, ghost/autofill/input consumption, Nether return with completion barrier | `774-dimensions775-v2/`:4/4 | `774-dimensions777-v1/`:4/4 |
| Password registration, wrong/correct relog before player creation and Clear Dialog | `774-password775-v1/`:5 recorded checks | `774-password777-v1/`:5 recorded checks |
| Encrypted online login | Shared RSA/session-service/compression/identity test now covers774/775/776/777 | Same implementation |

The774↔775 edge uses its own pinned packet, content and metadata tables with typed canonical codecs. It translates the old combined entity interaction/attack envelope, time updates (including set/frozen time across later empty clock updates), count-first recipe/nested slots, recipe display discriminators and IDSet projections, metadata indices/serializers, and single-count chunk sections. Visual projections preserve canonical saved items/states and authoritative inventory. Primary gameplay rules remain unchanged.

Actual774 failures are retained: `774-client775-v1/` and`v2/` failed GLFW/Xvfb startup before connection;`v3/` rejected the26.1 ambient-light attribute and adult/baby wolf-sound wrapper;`v4/` rejected untranslated recipe-display discriminators;`v5/` rejected the double-count chunk header;`v6/` accepted146 payloads then rejected bees without an entity-type prefix. The corrected generator exports the same196 labels, with a typed774 bee ID. Unlike776,774 accepted the canonical instrument bool+holder. Earlier packet-robot/compile/observer failures are retained separately; no failure is relabeled as success.

Sources: pinned`step1-sources-774.json`, existing official775/777 observations and the43 block-entity samples already used for776. [Mob variant history](https://minecraft.wiki/w/Mob_variant_definitions), [26.1 environment attributes](https://minecraft.wiki/w/Java_Edition_26.1), [attribute history](https://minecraft.wiki/w/Attribute) explain the old formats. The774 JSON attribute mapper incorrectly omits four attributes added in1.21; the adapter retains the observed775 list of35, with attack/speed payload acceptance but without claiming35 distinct live774 observations. The schema tables likewise do not prove every entity's gameplay. No Mojang code, JAR unpacking, other cores/proxies/protocol implementations, reports or worldgen dumps were read.

The downloaded1.21.11 client is opaque, SHA1`ba2df812c2d12e0219c489c4cd9a5e1f0760f5bd`; official metadata SHA1`240f9560a4a6bc07faf42a7032a96b225f76e658`. Private Xvfb uses Mesa EGL;774 uses a separately downloaded Debian X11 GLFW library, with package hashes in the verification manifest. This continuation reused existing official-server observations rather than repeating those runs. All fixture worlds/auth databases are temporary and processes stop by owned PID. No25565 listener, commit or push.

End travel, Nether/End persistence, arbitrary custom component forms and exhaustive live entity/attribute semantics remain unproven, without adding checklist gates. The same shared-code online-login proof and excluded chat fallback/live Mojang-account requirements apply to774 under the requested776 checklist. Payload injection proves enumerated decoding/translation; player GUI actions are emitted by the opaque client. The final release/public check and full tests cover the admission switch and the bee fix; raw GUI test-binary hashes are retained separately.

Fresh776 regressions retain the original196 payload bytes byte-for-byte (SHA256`b315a5d410c666ec1b0bc4b8a38635ec38c765693decc46f15ed82752810bab3`). `776-client{775,777}-regression/` each passes196 payloads,6 GUI actions and4 poses;`776-gameplay{775,777}-final/` each passes11/11;`776-dimensions{775,777}-regression/` passes4/4 and`776-password{775,777}-regression/` passes registration/wrong→correct relog. No776 gate or checklist requirement was weakened.

Final counts, source/fixture/binary hashes, release/public admission and retained failures are recorded in`completion-774-verification.json` / `completion-774-verification/`. All public774/776 cells are open; Status continues to report the selected primary honestly. Final verification is recorded after the last codec/admission edits; two Clippy style findings were fixed without suppressions.

Final verification:3152 Rust PASS (2908 core +3 startup +230 protocol +11 seedmap;92 ignored),233 Python PASS. Workspace and required root Clippy `--release --all-targets -- -D warnings` PASS; release build PASS. `public774-776-final/` passes8/8 against the final production binary:774 and776 reach Play/chunks/inventory/ACK/command barrier on both primaries; Status775/777 remains honest. Temporary test worlds/auth databases, Cargo.lock, Python caches and test leftovers were removed. The external build cache and pre-existing codex.log/plugins-examples/target remain untouched.

## 773 follow-up, 774/776 retained

773 now passes the same existing checklist and is admitted on both primaries. Fresh774/776 regressions repeat196 payloads/6GUI/4poses,11gameplay/restart,recipes/Nether,password and supplemental End for all four pairs; original payload files remain unchanged. Final3169 Rust/237 Python,clean Clippy/release and public24/24.770–772 remain closed with the existing Russian refusal; no gameplay profiles or new gates. [Full evidence, version-specific attack equivalent and retained failures](../client-chain-770-773/CONTINUED.md#773-admission-and-774776-regression).

## 771 follow-up, 772/773/774/776 retained

771 now passes the unchanged checklist on both primaries and is admitted through the772 typed path with separate771 tables/views. Fresh772/773/774/776 regressions preserve all eight admission cells and payload bytes.770 stays closed and untouched because its full codecs/integration/live checklist are unfinished.3183 Rust/237 Python,clean Clippy/release and public24/24; no older gameplay profile or new gate. [Raw evidence and limitations](../client-chain-770-773/CONTINUED.md#771-admission-and-772773774776-regression).

## 770 chat-login follow-up, 771–776 retained

770 now uses isolated frozen chat-login limbo on both primaries and is publicly admitted after registration/wrong-password/relogin proof, including the opaque1.21.5 GUI.771–776 keep their Configuration dialogs and the unchanged checklist. No older gameplay profile or new admission gate.769 remains closed because its adjacent adapter/Play/live checklist are unfinished. [Current evidence and final verification](../client-chain-770-773/CONTINUED.md#770-chat-login-admission).
