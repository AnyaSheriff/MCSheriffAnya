# Step 1 continuation — admission remains closed

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
