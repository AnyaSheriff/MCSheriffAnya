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
