# Lower client chain continuation

Latest: [typed773 continuation, still closed](#typed773-continuation--admission-still-closed).

Client translation only. No commits, older gameplay profiles or new admission gates.774/776 remain publicly admitted on26.1.2 and26.3. All eight770–773/primary pairs remain **CLOSED**, with the existing Russian refusal. This continuation does **not** complete lower-client admission.

## Foundations, in requested order

`client-chain-sources.json` pins20 allowed minecraft-data JSON files (protocol, blocks, items, entities and version for each protocol), plus dataPaths and release/protocol provenance. Every URL uses commit`924e26d6b3bb8f97a5d1e73d6190b09879c2e1e2`; byte length and SHA256 are checked. Existing774 sources remain separately pinned.`generate_client_chain.py --cache <client-chain-json> --cache774 <step1-json> --check` reproduces Rust and the schema report;`--fetch` restores only locked JSON.

| Step | Equal/union packet graphs | Remaining work |
|---|---|---|
| 773↔774 | 229/246 |17changed graphs include inventory/recipes/metadata;774→773 lacks17items,4entity types and8components. Full codecs, explicit visual projections and the existing live checklist remain unfinished. |
| 772↔773 | 217/247 | Further registry/movement/Configuration/Play differences; inherits the unfinished773 edge. |
| 771↔772 | 239/239 |772has an extra item despite equal packet graphs; inherits unfinished higher edges. |
| 770↔771 | 207/241 | Further chunk/metadata/dialog/recipe changes; inherits unfinished higher edges. |

Each edge has exact static mappings and a restricted control adapter. The shared chain composes770↔…↔777 and checks every adjacent schema/body. KeepAlive, Ping/Pong, held-slot selection, player-loaded, tick-end, chunk-batch-start and finish-configuration reject invalid values, truncation and tails. Opaque packets, inventory, metadata, recipes and chunks remain refused by this API even with equal graphs. Static IDs preserve MissingTarget/UnknownId; no inferred air/item fallback. Attributes below774 remain unavailable pending correction of the known JSON omission. General item/light/chunk/movement/login codecs explicitly reject these versions. Client release aliases do not enable older primary profiles; configuration validation rejects all six release names.

`adjacent_tests.rs` exhausts adjacent IDs/roundtrips, both-primary ID composition,64control routes and malformed/opaque rejection.`client-chain-report.json` retains each packet's source/target schema hashes and missing IDs. Equal graphs are never labeled live proof.

## Regression and optional End proof

Final results/hashes are in`verification.json`; raw sessions and check logs are retained alongside it. End is supplemental evidence, not a new gate: authored portal blocks, entry/exit by position packets, respawns/chunks/teleport ACKs/server completion logs/command barrier. Nether/End disk persistence and graphical End-client observation are not claimed.

Retained failures:`774-dimensions775-end-v1` selected an obsolete test executable without the ignored server;`v2` exposed the robot repeating its old teleport position after an intentional move.`v3` fixes predicted position and starts beside the Nether portal.`774-gui775` omitted required`--block-entities` and stopped before injection; the corrected run uses a new directory. No failure is relabeled as success.

Existing official-server observations are reused; no fresh official run, JAR unpacking, reports, other cores/proxies/protocol libraries, secrets or gameplay-profile work. GUI runtime uses the same pinned Debian GLFW/xdotool OS packages as the774 completion manifest. Runners clean temporary worlds/auth databases and owned processes. Final production admission is checked separately.

Final verification:3157Rust PASS (2908core+3startup+235protocol+11seedmap;92ignored),237Python PASS, workspace and root Clippy`--release --all-targets -- -D warnings` clean, release build PASS. The full Rust suite ran serially. Both774/776 on both primaries freshly pass196payload Pong barriers,6GUI actions,4poses,11gameplay/restart checks, the original4recipe/Nether checks plus supplemental End return, and5password-dialog recorded checks. Shared encrypted login remains green in the full suite. Original196-payload files are reused byte-for-byte; hashes are in`verification.json`.

`public-final/` passes24/24 against the final release:774/776 reach Play/chunks/inventory/ACK/command barrier on both primaries;770–773 keep the Russian refusal; all12Status responses honestly advertise their primary. All eight lower admission cells remain closed. Their full codec/live-checklist work is unfinished, not inferred from the new table/control tests.

Initial compile failures required explicit unsupported arms for the four new enum variants; the first added rejection test incorrectly included the already supported774 item decoder and was restricted to clients below774. Both failed logs are retained. Final checks follow the last Rust edits; later edits affect only proof runners/docs. Temporary test leftovers, Python caches and generated Cargo.lock are removed; pre-existing codex.log/plugins-examples/target and the external build cache remain.

## Typed773 continuation — admission still closed

The requested full lower-client admission task remains unfinished. Work stayed on773 first;772–770 retain the earlier bounded control/static adapters. All eight lower client/primary pairs remain CLOSED with the existing Russian refusal.774/776 admission and primary gameplay profiles are unchanged; no new admission gate was added.

`client_773::translate_play` (also exposed through `adjacent::STEPS[0]`) now supplies the bounded adjacent773↔774 inventory/recipe/particle/trade/metadata conversion. It parses set-slot, full-window/cursor/player inventory, equipment, untrusted Creative Slot, hashed click predictions, recipe books/displays/advancements, item particles, explosion effects, signed-command checksums and the VarInt/i8 enchant envelope. Shared item components and count-first nested templates use canonical stacks, directional IDs, existing size/depth/duplicate guards and exact consumption; unknown or absent content is refused. The API deliberately rejects preview content and does not enable production connections.

`generate_payload_773.py` checks every source byte length/hash through the existing locks, certifies96 identical shared component field declarations and three equal envelopes, and emits separate canonical-indexed entity layouts and metadata serializer maps. `payload-773-report.json` records eight missing components, four missing entity types and the bee/wolf anger-field rename. Declaration equality is not live proof. `[проверить]` remaining anger ticks versus absolute anger end time has no observed semantic conversion; those fields are refused. Attribute IDs below774 still lack a corrected pinned/observed mapping. The real bee prefix below774 is unobserved, so component77 stays refused rather than inheriting the774 observation.

Nine new Rust regressions cover authoritative component IDs, all inventory envelopes, nested recorded slots,52 shared recorded complex items (the two attribute fixtures explicitly remain refused), recipes/particles, predictions, signed commands/enchant values, independent metadata serializers including mannequin profile37→36, every common empty entity layout, all truncated prefixes/tails and unsupported content/routes. These are synthetic/recorded-source codec checks, not an opaque773 client or a completed primary checklist.

Still needed before either773 admission cell can open: finish the guarded semantic/content cases and full connection/Play integration, then run the unchanged196-payload/GUI/poses/gameplay/restart/recipe/Nether/password checklist on each actual primary, including the shared online-login path. Lower clients inherit that unfinished edge. Existing observations were reused; no fresh official-server run, JAR unpacking, implementation-source access, gameplay-profile changes, commit or push.

The opaque1.21.10 client is prepared for the next live773 stage, not yet executed: metadata SHA1`e8a7596fa677c2b9f0c8aadf600ef0aae2bbe9d3`, client SHA1`d3bdf582a7fa723ce199f3665588dcfe6bf9aca8`. The existing preparation/launcher whitelist now accepts that exact release and its Java21 metadata requirement.4476 files are hash-verified,71 libraries and4403 asset entries; no JAR content was opened. `typed773-verification/client773-download-verification.json` retains the download proof. The first preparation assertion excluded this Java21 release and was corrected; its failed log remains.

Final verification:3166 Rust PASS (2908 core +3 startup +244 protocol +11 seedmap;92 ignored),237 Python PASS. Workspace and required root Clippy`--release --all-targets -- -D warnings` are clean; release build PASS. `typed773-public-final/` passes24/24 against the final release. Fresh774 GUI proof is in`typed773-regression-v2/`;776 GUI and all four gameplay/restart, recipe/Nether plus supplemental End, and password traces are in`typed773-regression-v3/`. Every pair passes196 individual payload Pong barriers,6 GUI actions,4 poses,11 gameplay/restart checks, the original4 recipe/Nether checks plus End, and5 password checks; shared encrypted login passes the full Rust suite. Both original admitted payload hashes are retained byte-for-byte.

Retained live setup/observer failures: first Xvfb loaded NVIDIA EGL and failed before connection; second used the obsolete774 payload fixture without its fixed bee prefix; third applied the774 GLFW override to776, which requires its bundled newer GLFW preedit callback. The resumed runner uses Mesa EGL and applies the external GLFW only to774; already passed774 sessions were preserved. No core behavior or checklist bound was weakened. Counts, source/binary/artifact hashes and all12 admission cells are in`typed773-verification.json`; the verifier and logs are in`typed773-verification/`. Temporary worlds/auth databases, generated Cargo.lock, Python caches and test leftovers are removed; pre-existing codex.log/plugins-examples/target and the external build/client caches remain. No25565 listener, commit or push.
