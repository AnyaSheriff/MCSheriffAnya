# Step 1 preparation — full gameplay remains incomplete
No client774/776 route is opened on either primary. There is no claim of playable support.
`official-777.json`: the single official26.3 run, ports26184/26185, copied opaque JAR, Java25, stdin=/dev/null, stopped by PID (143);7/7 checks, PvP20→19.
`private-776.json`: one existing bounded test server on26028, primary775;5 checks: initial login/chunks, stone inventory, stale click/resync, command/chat and teleport ACK. Shutdown0 with stdin EOF.
`admission.json`: raw Status and rejection frames for774/776 on both primaries; this checks the admission guard, not Play.
Generated774↔775 static tables and packet schema equality are pinned in `step1-sources-774.json`; `generate_step1.py --cache <step1-json> --baseline <candidate-json> --check` reproduces them.
`chain::translate_control` checks every adjacent edge774↔775↔776↔777 for a small typed control allowlist; it rejects content packets. Existing public775/777 routes retain their dispatchers.
Content projection keeps shared26.2 objects native on776, uses explicit775 substitutes otherwise, and projects774 flowers/pots/note instruments by names/properties. This is not a complete entity/item codec.
Remaining:774 time/interaction layouts, item components, dynamic registries, complete metadata, and login/chunks/light/entities/inventory/PvP/death/respawn/relogin proof for each client × primary. GUI/audio, AuthMe/online-mode remain unproven.
No JAR inspection, additional official runs, commits or publishing; no25565 listener. The requested full client gameplay step is not complete.

Latest continuation:776 mining, chest-area relog and disk-backed process restart pass11/11 on each primary; all four public admission cells remain closed. Full774 Play/gameplay and authentication are still unproven. See `CONTINUED.md` and `mining-chest-verification.json` for the final matrix, counts and raw trace hashes.
