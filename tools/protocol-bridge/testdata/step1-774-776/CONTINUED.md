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
