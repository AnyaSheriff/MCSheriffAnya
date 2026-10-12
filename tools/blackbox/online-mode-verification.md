# Java online-mode verification (26.1.2 only)

Status: **[проверить]**, written for an external verifier. No server was run here.
Use a legitimate Java session and vanilla 26.1.2 client. The scenario runner must
also authenticate with a legitimate session before using the JSON play scenario.

1. Set `online-mode=true`, `prevent-proxy-connections=false`, compression 256.
   Update the tester's `ops.json` entry to the verified online UUID first.
   Log in: check the logged dashed UUID and canonical name against the account,
   signed skin from another client, configuration completion, chunk loading,
   movement, chat, inventories, and reconnect. Run `online-mode-play-smoke.json`.
2. Repeat with compression -1 and 0. Encryption must continue through
   configuration/play and every packet, including its length.
3. Repeat with `prevent-proxy-connections=true` over a direct connection.
   Authentication should use the connecting client's IP.
4. Join with an unverified session: expect
   `multiplayer.disconnect.unverified_username`, no player admission or save.
   Simulate the session service being unreachable in the verifier's isolated
   environment: expect `multiplayer.disconnect.authservers_down` within 10 s
   of starting the HTTP request (plus the existing bounded disconnect write).
5. Check UUID-based whitelist, bans, and OP using the authenticated UUID.
   A Login Start UUID must not override that identity or migrate permissions.
6. Set `online-mode=false`: verify the existing OfflinePlayer UUID, plaintext
   connection, skin lookup, and compression behavior. Restore test settings.

The JSON scenario cleans its pad. Login failures cannot be represented by the
existing post-login block-action scenario format; they are covered by in-memory
Rust tests and the manual checks above. No credentials belong in these files.
