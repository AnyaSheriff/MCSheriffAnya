# Lower client chain continuation

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
