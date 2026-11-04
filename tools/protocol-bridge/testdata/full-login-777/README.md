# Public 777 login, 2026-10-23

`core.json`: release binary, explicit primary 26.3, real persistent world, no test entry point.
Eight checks pass: login/chunks, one bare-hand PvP hit (20 → 19), death/respawn,
placement (2 → 1), mining (one cobblestone, pickaxe damage 1), inventory transfer,
logout/login, process restart with slab3 and the unchanged top slab on disk.
Both core processes exited 0. Six saved player/region files have recorded hashes.
All catalog block states also survive a real World::open_with_catalog disk reopen in the Rust suite.

`official-failed.json`: the single authorized official run on 26182, copied JAR,
Java25, stdin=/dev/null, stopped by its PID (SIGTERM, exit143).
Login succeeded. The initial runner mistakenly sent the old use_entity attack form;
the pinned schema requires packet_attack/entityId. Official parity after login is unproven.
The corrected runner was not rerun on the official server.

Core runner preparation failures were corrected before the successful run:
wrong properties location (brief isolated default25565 listener, no client joined;
child stopped), fixed-pose assertion during teleport, missing respawn-position wait,
and initial-inventory timing on reconnect. One earlier PvP check ran before the peer
caught up; the final runner waits for inventory and the victim health packet.
These failed attempts are not counted as passes.

Reproduce with a fresh output directory and free ports26184/26185:
`python3 tools/protocol-bridge/probe_full_login_777.py --binary target/release/mcsheriffanya --data /home/lenox/.local/share/mcsa-java-26.3-probe/candidate-json --output /tmp/full-login-new --port 26184`

Limits: no graphical-client proof, exhaustive packet parity, online-mode/AuthMe,
complete PvP matrix, dimension travel or crash-atomic saves. Client775 remains
available in the unchanged native26.1.2 profile, but cannot join the new26.3 world.
Existing-world profile conversion is deliberately refused; terrain work belongs to another helper.

Validation: cargo test --release: 2884 unit +3 integration passed,77 ignored;
213 Python tests passed; cargo clippy --release --all-targets -- -D warnings and
cargo build --release passed. No source/decompiled code or JAR contents were read.
