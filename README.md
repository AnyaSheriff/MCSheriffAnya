<div align="center">

# MCSheriffAnya

A Minecraft-compatible game server written from scratch in Rust.

[![Java Edition 26.1.2](https://img.shields.io/badge/Java_Edition-26.1.2-62B47A?style=for-the-badge&logo=openjdk&logoColor=white)](#what-works)
[![Bedrock 26.10–26.13](https://img.shields.io/badge/Bedrock-26.10–26.13-4E4E4E?style=for-the-badge&logo=xbox&logoColor=white)](#what-works)
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
- A persistent world with its own on-disk format, saved on stop and every
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
  Java players to see; Bedrock players see Java capes. No crafting yet.

Not there yet: mobs, health and damage, containers, packet compression.

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
record both servers at once (`--parallel`). Of 131 scenarios (redstone,
pistons with slime, honey and flying machines, containers, rails and
minecarts, TNT and lightning, sculk, the crafter, fluids, the Nether portal)
128 match the original tick for tick in full. Where vanilla itself is random
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
