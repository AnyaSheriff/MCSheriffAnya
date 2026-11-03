# Chat authentication and automatic ViaProxy startup — 2026-10-23

Verified with the default-feature release build and the official ViaProxy 3.4.14
binary, downloaded by the core into a fresh runtime directory and checked against
its pinned SHA-256. No ViaProxy/ViaVersion source was used.

- Single core launch: `viaproxy=true`, backend **127.0.0.1:26120**, public **26121**.
- `auth-enabled=true`, default `auth-mode=dialog`: trusted PROXY connections
  automatically used chat. `auth-premium=false` for offline mineflayer bots;
  premium detection and protected-operator rules also passed the auth unit suite.
- mineflayer **1.20.1** and **1.16.5**: register, world join, disconnect, wrong
  password rejection, `/login`, and 90-second timeout kick all passed.
- Limbo movement corrections and suppression of ordinary chat and non-auth
  commands passed; post-login `/login` was intercepted without logging its secret.
- `auth.db` retained real client IPs **127.0.0.20** and **127.0.0.21**.
- Forced child crash restarted ViaProxy; core shutdown stopped the child and
  released both test ports. Output was prefixed `[ViaProxy]`.
- Console/core/proxy log scan found neither test password.
- `cargo test --release`: **2,500 passed**, **0 failed**, **68 ignored**.
- `cargo clippy --all-targets -- -D warnings`: clean. `git diff --check`: clean.

Client test: `NODE_PATH=/path/to/node_modules node tools/viaproxy/auth-smoke.cjs`.
Dependencies: mineflayer and its normal dependencies, outside this repository.
Runtime evidence on this machine: `/home/lenox/.cache/mcsheriffanya/viaproxy/auto-chat-proof-1792745405` (`bots.log`, `console.log`,
core `logs/`, and ViaProxy `viaproxy/logs/`).
