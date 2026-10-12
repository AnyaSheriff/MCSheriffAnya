# Password dialogs and optional licence detection (Java 26.1.2 / protocol 775)

Status: **[проверить]** on a real client. No server or socket-based test was run
in this workspace. The JSON runner starts in Play and cannot submit Configuration
Custom Click Action packets; use a vanilla 26.1.2 client for the steps below.

Sources, all no later than 26.1.2:

- [Dialog, revision 3512615, 2026-04-02](https://minecraft.wiki/w/Dialog?oldid=3512615):
  confirmation, text inputs, dynamic/custom, wait_for_response.
- [Packets, revision 3501809, 2026-03-27](https://minecraft.wiki/w/Java_Edition_protocol/Packets?oldid=3501809):
  Configuration Show Dialog 0x12 (inline anonymous NBT), Clear Dialog 0x11,
  Custom Click Action 0x08 (identifier then NBT; TAG_END allowed).
- [minecraft-data 26.1 protocol.json](https://github.com/PrismarineJS/minecraft-data/blob/master/data/pc/26.1/protocol.json):
  protocol 775 packet IDs and layouts. There is no separate 26.1.2 JSON table.

1. In an isolated test installation set `online-mode=false`, `auth-enabled=true`,
   `auth-premium=false`. Use an empty test auth.db. Join as a non-op: expect a
   gold MCSheriffAnya registration title, two 64-character text inputs, green
   registration button and red exit button. Escape must not enter the world.
   Test mismatched passwords, lengths 3/4/64, Cyrillic and supplementary Unicode.
   Valid registration sends Clear Dialog then Finish Configuration; no Play,
   player save, join event or op permission is available before this point.
2. Reconnect (also with different name casing): one password field, login/exit.
   Two wrong passwords show red errors and 2/1 attempts left. Third disconnects.
   Five failures across connections block that IP for the rolling 10-minute
   window. A different IP has its own counter. Wait 90 seconds without completing
   configuration/login: disconnect. Clicking Exit also disconnects.
3. Register three names from one IP; fourth is refused for a rolling 24 hours.
   Unregistering and restarting must not reset the registration quota.
4. Verify `auth setpassword <name> <password>`, `auth list`, `auth unregister <name>`
   from the console. From Play verify `/changepassword <old> <new>`; quoted JSON
   strings support spaces. Wrong old password must not change the account.
   With debug enabled inspect logs and plugin command hooks: no credential
   arguments, hashes or salts. Commands named auth are console-only.
5. Use SheriffAnya's supplied online UUID in ops.json. Without an auth.db account,
   cracked registration is refused with the console-password message. Console
   setpassword must preserve that op UUID. No old playerdata copying or Ops::adopt
   may occur while auth is enabled. Reconnect with that password: rights come
   from the account UUID only.
6. Enable `auth-premium=true`. Test licensed success (online UUID, signed textures),
   a cracked client using the same premium-looking name (first licence failure,
   second connection within five minutes skips licence authentication), a
   non-premium name, and an unavailable Mojang profile API (direct cracked path).
   The fallback key includes both lowercase name and connecting IP. After five
   minutes a new licence attempt is allowed; profile lookups are cached 30 minutes.
7. Repeat those four paths with a new account, an existing licensed account,
   and an existing cracked account; repeat for op/non-op names. Every successful
   path still requires the password. Existing accounts retain their saved UUID.
   Verified licence + existing cracked account logs that its offline UUID stayed.
   New op registration is allowed only for the verified matching online UUID.
8. Check Login Success UUID and subsequent player list/playerdata UUID agree.
   Check signed skins and Essential cosmetics with a licensed owner. **[проверить]**
   Essential compatibility cannot be established by packet tests alone.
9. Repeat transport checks with compression -1, 0 and 256; licensed connections
   remain encrypted across Configuration and Play. Cracked fallback follows the
   existing unencrypted offline transport. Dialog `minecraft:text` has no masked
   password-field setting in the pinned schema, so typed passwords are visible.
10. With Java `online-mode=true`, no password dialog. With
    `bedrock-online-mode=true`, Xbox login remains required and skips Java auth;
    an Xbox player named SheriffAnya must have level 0 unless their distinct
    XUID-derived UUID is explicitly in ops.json. No name adoption for Bedrock.
11. After authenticating a test operator run `auth-dialog-play-smoke.json` to
    verify world admission and block updates. Its cleanup removes its entire pad.

Use disposable passwords only. These documents contain no credentials.
