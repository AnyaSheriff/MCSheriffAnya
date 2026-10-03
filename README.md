<div align="center">

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
  Java players to see; Bedrock players see Java capes. Crafting, furnaces and the other
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
  `zombie-attack`, `spider-climb` are random and match by structure). A
  hostile mob killed by a player drops 5 experience points plus 1–3 per
  equipment item; Bedrock players see the mobs and arrows and can fight them; main combat sounds
  are sent, the rest of the sounds are not complete (see Status).
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
  arrows, thorns, fire aspect. Not checked against the official server: thorns,
  bow enchantments (power, punch, flame, infinity), arrow pickup, netherite
  knockback resistance. Potion effects work, including strength, weakness and the full potion set.

Not there yet: plugins, multiple protocol versions, ender chests, shulker
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
| Plugins | ❌ | Planned |
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
on the [Releases](https://github.com/AnyaSheriff/MCSheriffAnya/releases) page.
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

Run it from a terminal if you want to type console commands. The first start
creates `config/`, `world/`, `logs/` and `playerdata/`. Connect with a
26.1.2 client to `localhost:25565`.

## Configuration

- `config/server.properties` — the same keys and format as the original
  server. Nothing from it is hard-coded.
- `config/mcsa.properties` (MCSheriffAnya.properties, same `key=value` format as `server.properties`) — MCSheriffAnya's own settings: `debug`; `prefetch-chunks` / `prefetch-at-once` — how far beyond the view distance the world is generated ahead of the player (0 turns it off); `bedrock-port` and `bedrock-online-mode` (if true, only verified Xbox Live accounts join; if false, all join but only verified get operator rights); `autumn-forests` and `pink-cherry-groves` — small additions not found in vanilla (rare autumn patches in forests, pinkish grass and more petals in cherry groves; `false` keeps the world vanilla); `text-info` — a plain-text server summary for `telnet host port`, browsers and curl; `portal-delay` — how many ticks to stand in a portal before changing dimensions in survival mode (80 by default, as in vanilla); `portal-min-width`, `portal-min-height`, `portal-max-width`, `portal-max-height` — which portal openings light (2×3 to 21×21 by default, as in vanilla; each side 1 to 256); `send-error-reports` — enable anonymous error reports sent to developers (false by default; reports contain recent log lines with names, addresses and UUIDs removed).
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
crafter, fluids, the Nether portal); all 137 match the original tick for tick
in full. Where vanilla itself is random
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
