<div align="center">

<img src="assets/logo.png" alt="MCSheriffAnya" width="160">

# MCSheriffAnya

A Minecraft-compatible game server written from scratch in Rust.

[![Java Edition 26.1.2](https://img.shields.io/badge/Java_Edition-26.1.2-62B47A?style=for-the-badge&logo=openjdk&logoColor=white)](#what-works)
[![Bedrock 26.10–26.13](https://img.shields.io/badge/Bedrock_26.10–26.13-partial-F28C28?style=for-the-badge&logo=xbox&logoColor=white)](#status)
[![Rust 2024](https://img.shields.io/badge/Rust-2024-CE422B?style=for-the-badge&logo=rust&logoColor=white)](https://www.rust-lang.org)
[![MIT](https://img.shields.io/badge/License-MIT-3DA639?style=for-the-badge)](LICENSE)

[![Last commit](https://img.shields.io/github/last-commit/AnyaSheriff/MCSheriffAnya?style=flat-square&label=last%20commit)](https://github.com/AnyaSheriff/MCSheriffAnya/commits/main)
[![Stars](https://img.shields.io/github/stars/AnyaSheriff/MCSheriffAnya?style=flat-square&label=stars)](https://github.com/AnyaSheriff/MCSheriffAnya/stargazers)
[![Issues](https://img.shields.io/github/issues/AnyaSheriff/MCSheriffAnya?style=flat-square&label=issues)](https://github.com/AnyaSheriff/MCSheriffAnya/issues)

[![English](https://img.shields.io/badge/English-selected-0969DA?style=flat-square)](README.md)
[![Русский](https://img.shields.io/badge/Русский-читать-555555?style=flat-square)](README_ru.md)

</div>

MCSheriffAnya is an independent implementation of the Java Edition server protocol.
It is not a fork: it is written only from public documentation (the Minecraft
Wiki, protocol descriptions, bug tracker) and observation of the official
server from the outside. It contains no code from Mojang, Paper, Spigot,
Fabric, Forge or any other server core. MCSheriffAnya is not affiliated with or
endorsed by Mojang or Microsoft.

- **Client version:** Java Edition 26.1.2 (protocol 775). Older clients work
  through ViaFabricPlus and similar tools. Player IDs are generated offline from
  nicknames (UUID v3 "OfflinePlayer:<name>"), old player data is migrated
  automatically.
- **Bedrock support:** Bedrock Edition 26.10–26.13 join the core directly with
  native account verification and cross-play features.
- **Stack:** Rust (edition 2024), Tokio, Serde, TOML.
- **License:** MIT.

## What works

- Login, world join, several players at once, chat, tab list, player skins
  ([chat colors, HEX and console colors](docs/colors.md))
  (Mojang, then Ely.by as a fallback; both can be switched off).
- A persistent world with its own compact on-disk format (`region/r.X.Z.mcs`,
  zstd-compressed palettes, about half the size of vanilla Anvil; old `.rcw`
  regions are converted on startup), saved on stop and every
  few seconds; player data and inventories are saved per player.
- Our own world generation, fitted to measurements of vanilla worlds:
  terrain, caves, aquifers, rivers, oceans, biomes, trees and plants; sky
  and block light. World types: normal, superflat and "Super smooth".
- Survival basics: mining and placing blocks, drops, picking items up,
  dropped items, falling sand and gravel (through two ticks, through water),
  flowing water and lava (water washes away grass, flowers, torches, wires,
  carpets; lava solidifies to cobblestone on contact with water); bucket
  fills water by sight; water currents carry dropped items and splashes on
  entry. In the Nether, lava flows farther and faster; water evaporates.
- The Nether as a second dimension: caves, lava sea, bedrock ceiling, five
  biomes (wastelands, crimson and warped forests, soul valley, basalt deltas)
  with surface blocks, ores, fungi, vines, pillars and glowstone. Portal frames
  (2×3 to 21×21), ignition with flint and steel (Java and Bedrock), automatic
  portal finding or building and linking. Bedrock players see their own Nether
  biome set.
- The End as a third dimension: the main island with ten obsidian pillars
  (two caged) and the exit portal podium, the void gap, and outer islands from
  ~1000 blocks with chorus plants, all fitted to a measured vanilla End. End
  portal frames (twelve eyes open the portal, as in vanilla), the obsidian
  arrival platform at 100, 49, 0, and the way back. The ender dragon fight with
  the end crystals works.
- Redstone: wire, torches, repeaters, comparators, levers, buttons, pressure
  plates, observers, copper bulbs, lamps, doors, trapdoors, fence gates,
  pistons and sticky pistons with real two-tick movement and client-side
  animation. Timings are verified against the official server tick by tick
  (see below).
- Day and night with sun, moon phases and sky colour; time is kept across
  restarts.
- Operators (`ops.json`), commands with permission levels and tab hints:
  `help`, `list`, `say`, `tp` (with dimension switching), `gamemode`, `kick`,
  `give`, `clear`, `time`, `op`, `deop`, `setblock`, `fill`, `stop`. The `tp`
  command accepts player name or coordinates (including yaw and pitch from
  F3+C). `execute in <dimension> run tp` teleports to another dimension.
- Custom TNT for operators: `tnt give <player> <mega|nuke|cluster|safe>
  [count]` and `tnt list` (permission node `mcsheriffanya.tnt.give`, for now
  granted to ops). Mega — power 8, nuke — power 16 with an 8-second fuse,
  cluster — throws 8 small charges, safe — pushes but breaks no blocks. The
  item keeps its kind in inventories, chests and on the ground; a placed block
  remembers it in the chunk. Anyone holding it can place and light it.
- A console that looks like the original one; technical detail is hidden
  behind `debug=true` in `config/mcsa.properties`.
- Bedrock Edition 26.10–26.13 players join the core directly, no proxy (UDP
  port `bedrock-port`, 19132 by default): the world, Java players and their
  skins, chat, building in creative and survival, the inventory (opened with E,
  kept in the same player file as for Java), drops and item pickup, commands,
  cross-dimension travel via portals. Crouching, hand waving, nametags and
  held items are visible between Java and Bedrock. Xbox Live account
  verification (signature check via OpenID discovery); an unverified player can
  only use `help`, `list`, and `say` commands. Flight is disabled in survival
  mode. Player skins from Bedrock can be uploaded to Mojang via MineSkin for
  Java players to see them; Bedrock players see Java capes. Crafting, furnaces and the other
  workstations work, see [Status](#status).

- Farm animals: cows, pigs, sheep and chickens (`summon cow|pig|sheep|chicken
  <x> <y> <z> {NoAI:1b,Age:-24000,…}`). They fall, take fall damage, wander,
  look at players, follow wheat/carrots/seeds, breed into babies, panic when
  hit, drop meat, leather, feathers and wool; sheep are sheared with shears and
  eat grass, chickens lay eggs. They also settle in newly generated chunks
  (rule `spawn_mobs`), press plates, are saved with the chunk and are visible
  to Bedrock players (they can hit, feed, breed and shear them). Physics, damage, drops
  and the packet stream are checked against the official server with the black
  box (`cow-physics`, `cow-hit`, `sheep-shear`, `mob-plate`, `chicken-fall`,
  `pig-baby`, `cow-feed`); the random behaviour (wandering, breeding walks,
  spawn choices) is checked by unit tests only. An animal killed by a player
  drops experience orbs (1–3).

- Hostile mobs: zombie, skeleton, creeper and spider (`summon zombie|skeleton|
  creeper|spider <x> <y> <z> {NoAI:1b,…}`). They hunt the nearest survival
  player in line of sight (creative and spectators are ignored); zombies and
  spiders strike every 20 ticks with knockback, skeletons draw a bow and shoot
  arrow entities (gravity 0.05, drag 0.99, stick in blocks, hit players, an
  arrow stays in the body), creepers swell for 30 ticks near a player and
  explode with power 3 (the blast also hurts nearby mobs), spiders climb walls
  and stay neutral in daylight unless hit. Zombies and skeletons burn in the
  sun (not with a helmet or in water). They spawn in the dark in the overworld
  (block light 0, monster cap 70, at least 24 blocks from players) and despawn
  far away. Damage, armor, drops, burning, fuse, explosion and arrow packets
  are checked against the official server with the black box (`zombie-hit`,
  `zombie-burn`, `creeper-explode`, `skeleton-stand`; `skeleton-arrow`,
  `zombie-attack`, `spider-climb` are random and match by structure).
- Experience orbs: real entities that fly to the nearest player and are picked
  up one per 2 ticks; the value is split greedily into 7, 3 and 1 (checked
  against the official server).
- Containers: chests (including trapped and copper), barrels, hoppers,
  droppers, dispensers, jukebox, lectern, crafter, shelf and chiseled
  bookshelf; checked with the black box.
- Player combat, checked against the official server with the black box
  (`pvp-*` scenarios, two robots): entity attributes with modifiers (weapon
  damage and attack speed, armor, toughness, knockback resistance, sprint
  speed) sent with Update Attributes; item components `damage`, `enchantments`
  and `attribute_modifiers` (`give @p diamond_sword[enchantments={sharpness:5}]`);
  attack cooldown, crits, sprint knockback, sweeping, armor and protection
  enchantments, durability, shields (blocking arc, axes disable them), bows and
  arrows, thorns, fire aspect. Thorns and bow enchantments (power, punch,
  flame, infinity) are also checked by the `pvp-*` scenarios. Potion effects
  work, including strength, weakness and the full potion set.
- Enchantments: spear Lunge is implemented; Frost Walker and Soul Speed are
  expanded; Impaling, Looting, Mending, Silk Touch/Fortune and Bedrock Binding
  are corrected. Mobs persist six equipment slots with enchantments and drop
  chances, including weapon effects, Thorns and enchanted pillager crossbows.
  The `enchant-*` scenarios measure individual effects against vanilla 26.1.2.
  Full parity is not claimed: packet timing/order, general AI movement and
  statistically measured random distributions still have known differences
  or require further verification.

Not there yet: Java plugins, multiple protocol versions, ender chests, shulker
boxes, packet compression. See the full list below.

## Status

✅ works · 🟡 partial · ❌ not yet

**Java Edition 26.1.2** — verified tick by tick against the official server
with the black box (about 300 scenarios).

| Area | State | Notes |
| --- | --- | --- |
| World generation | ✅ | Overworld, Nether, End with the dragon fight, strongholds, villages, end cities |
| Redstone | ✅ | All components, pistons, containers, comparators |
| Combat and PvP | ✅ | Melee, sweep, crits, shields, bow, crossbow, trident, mace |
| Survival | ✅ | Hunger, XP, potions, beds, respawn anchor, farming, furnaces, brewing, enchanting, anvil, grindstone, smithing table, villagers and trading |
| Mobs | ✅ | |
| Commands | ✅ | Vanilla commands, command blocks, datapack functions, scoreboards |
| Weather | ✅ | |
| Known differences | 🟡 | Enderman stare, shulker bullets, riptide side effects, mace wind burst particles |
| Plugins | 🟡 | WebAssembly plugins and modules, hot `/plugman`, API `mcsa:api` (WIT) — [docs/plugins.md](docs/plugins.md). Java API (TeaVM, Jar2WASM) planned |
| Multiple protocol versions | ❌ | Planned |
| Ender chests, shulker boxes | ❌ | |
| Packet compression | ❌ | |

**Bedrock Edition 26.10–26.13**

| Area | State | Notes |
| --- | --- | --- |
| Join without a proxy, world and chunks | ✅ | |
| Nether and End travel, chat | ✅ | |
| Skins | ✅ | Both ways |
| Java players | ✅ | Seeing and fighting them, PvP both ways |
| Mobs | ✅ | Hit, feed, breed, shear; variants |
| Inventory, placing and breaking | ✅ | |
| Redstone blocks | ✅ | Levers, buttons, doors, pressure plates, pistons |
| Containers | ✅ | Chests, barrels, hoppers, droppers, dispensers, crafter |
| Workstations | ✅ | Furnaces, brewing stand, crafting table and 2×2 grid, enchanting table, anvil, grindstone, smithing table |
| Villager trading, minecart windows | ✅ | Minecart with chest and with hopper |
| Items | ✅ | Bow, crossbow, eating and drinking, bone meal, hoe, buckets, spawn eggs, thrown potions, fireworks |
| World objects | ✅ | TNT, falling blocks, XP orbs |
| Sounds | 🟡 | Main combat and block sounds work; crit and sweep particles and station sounds are missing |
| Fireworks colours, piston animation | 🟡 | Colours are not complete; the piston arm does not move smoothly |
| Shift-click trading | 🟡 | |
| Screens | 🟡 | Many are checked only against the official Bedrock server's network traffic, not yet by players |
| Xbox-only features | ❌ | Such as Realms; nothing else known to be missing |

## Download

Ready-made builds for Linux, Windows and macOS (Apple Silicon and Intel) are
on the [Releases](https://github.com/MCSheriffAnya-Rust/MCSheriffAnya/releases) page.
Download the archive for your system, unpack it and run `mcsheriffanya`
(`mcsheriffanya.exe` on Windows) from a terminal. No Rust is needed. The first
start creates the config and the world next to the binary. Ports: `25565`
(Java) and `19132/UDP` (Bedrock). On Windows the console reads whole lines
(no live input line); on Linux and macOS it has the full line editor.

## Running

```sh
cargo build
./target/debug/mcsheriffanya
```

WASM plugins (Wasmtime + Cranelift JIT) are included by default. For an optimized
binary, use `cargo build --release`; to omit the plugin host, use
`cargo build --release --no-default-features`. See [plugins](docs/plugins.md).

Run it from a terminal if you want to type console commands. The first start
creates `config/`, `world/`, `logs/` and `playerdata/`. Connect with a
26.1.2 client to `localhost:25565`.

## Docker

The image `sheriffanya/mcsa:latest` supports Linux amd64 and arm64, including WASM plugins:

```sh
docker run -it --name mcsa --restart unless-stopped --stop-timeout 120 \
  -p 25565:25565/tcp -p 19132:19132/udp \
  -v mcsa-data:/data sheriffanya/mcsa:latest
```

Detach with **Ctrl+P, Ctrl+Q**; reconnect with `docker attach mcsa` to enter
console commands. `stop`, Ctrl+C, or `docker stop mcsa` saves players and worlds.
Allow more than 120 seconds for very large worlds if needed. For a detached start
use `-dit` instead of `-it`, keeping stdin open for later attachment.

Or use the included `docker-compose.yml`:

```sh
docker compose up -d
docker attach "$(docker compose ps -q mcsa)"
docker compose stop
```

The named volume at `/data` keeps `world/` (including `world/playerdata/`),
`config/`, `ops.json`, `auth.db`, `logs/`, `skins/`, `plugins/`, and `modules/`.
Compose creates its own project-prefixed volume; `docker compose down` preserves
it, while `docker compose down -v` deletes it.

The first start creates `/data/config/server.properties` and
`/data/config/mcsa.properties`. Stop the server before editing them in the volume,
then use `docker start mcsa` or `docker compose up -d`. For convenient host editing,
replace `mcsa-data:/data` with `./mcsa-data:/data` (an absolute host path with
`docker run`), create that directory first, and make it writable by UID/GID
`10001:10001`. Edit `mcsa-data/config/server.properties` for Java settings and
`mcsa-data/config/mcsa.properties` for MCSA settings, including `bedrock-port`.
Java uses **25565/TCP**; Bedrock needs **19132/UDP** published and allowed through
the firewall. If you change an internal port, update the mapping too.

Build locally with `docker build --build-arg VERSION=dev -t sheriffanya/mcsa:dev .`.
An existing `Cargo.lock` is used; otherwise the image generates one before the
locked release build. Tag pushes (`v*`) publish the version without `v` and
`latest`; configure repository secrets `DOCKERHUB_USERNAME` and `DOCKERHUB_TOKEN`.

## Configuration

- `config/server.properties` — the same keys and format as the original
  server. Nothing from it is hard-coded.
- `config/mcsa.properties` (MCSheriffAnya.properties, same `key=value` format as `server.properties`) — MCSheriffAnya's own settings: `debug`; `prefetch-chunks` / `prefetch-at-once` — how far[...]
- `config/skins.toml` — which skin sources to use (Mojang, Ely.by) and whether to upload Bedrock player skins to Mojang via MineSkin so Java players can see them.
- `ops.json` — operators, the original format.

## Verified against the original

`tools/blackbox/` holds a "black box" harness: a bot joins the official
26.1.2 server and MCSheriffAnya, builds the same contraption with commands, flips
a lever like a player would, records every block change, block action and
sound with its tick, and diffs the two recordings. The official server is
only *run* and observed over the network; its jar is never opened. The bot can
record both servers at once (`--parallel`). The whole suite is about 300
scenarios. Redstone alone is 137 of them (pistons with slime, honey and flying
machines, containers, rails and minecarts, TNT and lightning, sculk, the
crafter, fluids; all 137 match the original tick for tick in full. Where vanilla itself is random
(blast spread, fuse from an explosion, particle colours), the scenario says so
in a `random` rule backed by a wiki quote or vanilla-against-vanilla
recordings, and everything else is still compared exactly. See `tools/blackbox/README.md`.

## Contributing

Rules of the project, to keep it legally clean:

- No code from the game or from any server core, proxy or protocol library.
  Any wiki, documentation, bug report or observed behaviour is fine.
- Settings that exist in the original `server.properties` are never
  hard-coded.
- Behaviour should match the original tick for tick; when in doubt, write a
  black-box scenario first.

Before sending changes: `cargo test` and `cargo clippy --all-targets` must
be clean.

### Offline Java password accounts

`config/mcsa.properties` has `auth-enabled=true` and `auth-premium=true` by
default. With Java `online-mode=false`, all players authenticate in a Russian
Configuration-phase dialog before entering the world. Online-mode Java and
Bedrock skip this gate. Bedrock's Xbox-only policy is controlled separately by
`bedrock-online-mode=true`.

Premium detection is optional: a Mojang name lookup has a 3-second timeout and
30-minute cache. A successful licence check supplies the online UUID and signed
textures, but still requires the password. If licence authentication fails,
reconnect with the same name and IP within five minutes to use the cracked path.
Non-premium names and unavailable profile APIs go directly to that path.

Accounts are keyed by lowercase name. Registration requires matching passwords
of 4–64 characters. An account keeps its original UUID across both paths. An
unregistered op name is protected unless a verified licence matches its ops.json
UUID. With auth enabled, legacy name-based identity/op adoption is disabled.

Console: `auth setpassword <name> <password>`, `auth unregister <name>`, `auth list`.
A new console-created op account uses its existing ops.json UUID; a new non-op
account uses its offline UUID. Existing accounts retain their UUID. In game:
`/changepassword <old> <new>`. Use JSON-quoted arguments for passwords with spaces.
Unregister removes the account, not playerdata or ops; it does not kick an
already admitted player. Direct password commands bypass plugin hooks and their
arguments are not logged.

Since **0.2.8-alpha** passwords always use Argon2id with a secret pepper built into the
core (the former MCSA Guard module; no download, no `auth-provider` setting). Old
PBKDF2 accounts are upgraded on a successful login. Back up `auth.db` together with
`mcsa-guard/` (see [docs/auth.md](docs/auth.md), in Russian).

`auth.db` is an atomically rewritten file database (SQLite was unavailable in the
offline dependency cache). It stores Argon2id + HMAC-pepper records (legacy PBKDF2 records are
upgraded at login). Back up this file together with `mcsa-guard/`, playerdata
and ops.json. Corrupt storage fails closed. Registration quotas survive restart;
login failure counters and premium caches are in memory. Limits: three wrong
passwords per connection, five failures/IP per rolling ten minutes, three
registrations/IP per rolling day, and 90 seconds for configuration/password entry.

Vanilla text inputs display their contents; this dialog API has no masked input.
Cracked connections retain the existing offline transport without encryption.
Licensed connections use the existing encrypted online handshake. Real-client,
signed-skin and Essential checks are listed in
[the verification checklist](tools/blackbox/auth-verification.md).
