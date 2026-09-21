# RustCraft

[Русская версия](README_ru.md)

A Minecraft-compatible game server written from scratch in Rust.

RustCraft is an independent implementation of the Java Edition server protocol.
It is not a fork: it is written only from public documentation (the Minecraft
Wiki, protocol descriptions, bug tracker) and observation of the official
server from the outside. It contains no code from Mojang, Paper, Spigot,
Fabric, Forge or any other server core. RustCraft is not affiliated with or
endorsed by Mojang or Microsoft.

- **Client version:** Java Edition 26.1.2 (protocol 775). Older clients work
  through ViaFabricPlus and similar tools.
- **Stack:** Rust (edition 2024), Tokio, Serde, TOML.
- **License:** MIT.

## What works

- Login, world join, several players at once, chat, tab list, player skins
  (Mojang, then Ely.by as a fallback; both can be switched off).
- A persistent world with its own on-disk format, saved on stop and every
  few seconds; player data and inventories are saved per player.
- Survival basics: mining and placing blocks, drops, picking items up,
  dropped items, falling sand and gravel, flowing water and lava.
- Redstone: wire, torches, repeaters, comparators, levers, buttons, pressure
  plates, observers, copper bulbs, lamps, doors, trapdoors, fence gates,
  pistons and sticky pistons with real two-tick movement and client-side
  animation. Timings are verified against the official server tick by tick
  (see below).
- Day and night with sun, moon phases and sky colour; time is kept across
  restarts.
- Operators (`ops.json`), commands with permission levels and tab hints:
  `help`, `list`, `say`, `tp`, `gamemode`, `kick`, `give`, `clear`, `time`,
  `op`, `deop`, `setblock`, `fill`, `stop`.
- A console that looks like the original one; technical detail is hidden
  behind `debug = true` in `config/rustcraft.toml`.

Not there yet: mobs, health and damage, containers, a light engine, packet
compression, world generation beyond flat land.

## Running

```sh
cargo build
./target/debug/rustcraft
```

Run it from a terminal if you want to type console commands. The first start
creates `config/`, `world/`, `logs/` and `playerdata/`. Connect with a
26.1.2 client to `localhost:25565`.

## Configuration

- `config/server.properties` — the same keys and format as the original
  server. Nothing from it is hard-coded.
- `config/rustcraft.toml` — RustCraft's own settings (`debug`).
- `config/skins.toml` — which skin sources to use.
- `ops.json` — operators, the original format.

## Verified against the original

`tools/blackbox/` holds a "black box" harness: a bot joins the official
26.1.2 server and RustCraft, builds the same contraption with commands, flips
a lever like a player would, records every block change, block action and
sound with its tick, and diffs the two recordings. The official server is
only *run* and observed over the network; its jar is never opened. Five
scenarios (piston with a lever, sticky piston pull, two-block push, grass
crushed by a piston, repeater into a lamp) currently match the original
exactly. See `tools/blackbox/README.md`.

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
