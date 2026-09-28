# Acknowledgments

## Sources

- [Minecraft Wiki](https://minecraft.wiki/) and [wiki.vg](https://wiki.vg/) — protocol specification, mechanics, generation algorithms, redstone behavior.
- [minecraft-data](https://github.com/PrismarineJS/minecraft-data) — block properties, item IDs, biome data, dimension types.
- [Minecraft Bug Tracker (Mojira)](https://bugs.mojang.com/) — official issue reports and reproduction cases.
- [mineflayer](https://github.com/PrismarineJS/mineflayer) — client library used only as a test robot in `tools/blackbox/` (black-box comparison with the official server); its code is not part of MCSheriffAnya.
- [Mojang bedrock-protocol-docs](https://github.com/Mojang/bedrock-protocol-docs) — official Bedrock protocol documentation.
- [GeyserMC Documentation](https://wiki.geysermc.org/) — Bedrock notes (documentation only, no code).
- [MineSkin API](https://mineskin.org/) — public API for uploading Bedrock player skins to Mojang.
- Observation of official Minecraft servers over the network.

## Libraries

- **Rust edition 2024** — language and standard library.
- **Tokio** — async runtime.
- **Serde** — serialization/deserialization.
- **TOML** — configuration file format.
- **flate2** — zlib compression for protocol packets.
- **reqwest** — HTTP client for skin downloads.
- **md-5** — MD5 for offline player UUIDs (UUID v3 from the nickname, as vanilla does without Mojang authentication).
- **aws-lc-rs** — cryptographic signatures (RS256, ES384) for Xbox Live authentication verification via OpenID Discovery.
