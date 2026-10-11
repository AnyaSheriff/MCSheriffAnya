# Connection cleanup checks for the verifier

Status: not run here (no server or socket binding). The JSON scenario harness
has no action to stop socket reads, reconnect, or choose the second bot's name.
Its second bot always has a different name, and `bot.js` treats a kick of either
bot as a fatal recording error. These checks need a connection harness extension
or an external client. No executable JSON scenario was added.

Use Java 26.1.2. Use a disposable survival player and record inventory, cursor,
crafting slots, position, player list and the disconnect packet before/after.
No blocks or pad changes are required; remove test items/accounts afterward.

1. Keep reading packets but stop answering Keep Alive. Check disconnect at
   15 seconds after the unanswered request. Wrong IDs and empty responses must
   not extend the deadline. Ordinary matching replies must keep the player in.
2. Stop reading server data while producing enough output to fill the socket.
   Check that the player leaves within the write deadline (15 seconds maximum,
   or the earlier outstanding Keep Alive deadline). Reconnect and check saved
   inventory/position, including items on the cursor and crafting grid.
3. Repeat blocked output, then issue console `kick <name>`. Check prompt removal
   from `list` without resetting the socket. Repeat with command/plugin kick.
4. Log in twice with the same name, including while output to the old client
   is blocked and while it awaits the first teleport confirmation. The old
   readable client receives `multiplayer.disconnect.duplicate_login`; an
   unreadable connection still closes. The new player loads after the old
   save/removal and has exactly one copy of its items. Repeat at max-players.
5. Race three same-name Java logins; only one session may own player data.
   Repeat Java-to-Bedrock and Bedrock-to-Java replacement with the same name.
   Existing edition-specific UUIDs remain unchanged: each UUID keeps its own
   save; cross-edition replacement must not merge inventories.
6. Bedrock: stop all datagrams and check RakNet timeout cleanup. Separately,
   keep sending pings/irrelevant ACKs but stop acknowledging server data;
   retransmissions must not postpone cleanup. Verify ordinary ACKs prevent
   false timeouts. Repeat kick and same-name/same-UUID reconnect.

The disconnect reason cannot be delivered to a peer that does not read.
After a partially written TCP frame, the server must close without appending
another frame or replaying the original buffer.
