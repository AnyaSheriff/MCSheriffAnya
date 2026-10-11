// Хуки плагинов (protect-spawn, chat-filter): игрок Java и игрок Bedrock (оба не операторы) делают одно и то же;
// состояние мира смотрит оператор Java. Только наш сервер (в BDS плагинов нет).
// Нужны плагины в tmp/v-ours: bash tools/blackbox/v-plug.sh && bash tools/blackbox/v-restart-ours.sh
const path = require('path');
const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
const { Vec3 } = require(path.join(process.env.HOME, 'node_modules', 'vec3'));
const { connect, sleep } = require('./v-client.js');
const out = [];
const log = (...a) => { const s = a.join(' '); out.push(s); console.log(s); };
const JB = (n) => mineflayer.createBot({ host: '127.0.0.1', port: 25964, username: n, version: '26.1', auth: 'offline' });

(async () => {
  const op = JB('blackbox'); await new Promise((r) => op.once('spawn', r));
  const BN = 'pb' + (process.pid % 1000), JN = 'pj' + (process.pid % 1000);
  const run = async (s) => { op.chat(s); await sleep(350); };
  const near = { x: 4, z: 4 }, far = { x: 60, z: 4 };
  for (const s of [near, far]) {
    await run(`/fill ${s.x - 2} -61 ${s.z - 2} ${s.x + 6} -61 ${s.z + 6} stone`);
    await run(`/fill ${s.x - 2} -60 ${s.z - 2} ${s.x + 6} -58 ${s.z + 6} air`);
    await run(`/setblock ${s.x + 3} -60 ${s.z + 3} stone`); // что ломать
  }
  const k = await connect('ours', BN); // Bedrock
  const chat = [];
  let inv = null;
  k.on((n, p) => {
    if (n === 'text') chat.push(`bedrock<-${p.message}`);
    if (n === 'inventory_content' && p.window_id === 'inventory') inv = p.input;
    if (n === 'inventory_slot' && p.window_id === 'inventory') { inv = inv || []; inv[p.slot] = p.item; }
  });
  const jv = JB(JN); await new Promise((r) => jv.once('spawn', r));
  jv.on('messagestr', (m) => chat.push(`java<-${m}`));
  await sleep(1500);
  await run(`/gamemode creative ${BN}`); await run(`/gamemode creative ${JN}`);
  await run(`/give ${BN} stone 20`); await run(`/give ${JN} stone 20`);
  await sleep(1000);
  const held = (inv || []).find((i) => i && i.network_id) || { network_id: 0 };
  const blockAt = (x, y, z) => { const b = op.blockAt(new Vec3(x, y, z)); return b ? b.name : '?'; };
  for (const [label, s] of [['near', near], ['far', far]]) {
    // --- Java
    await run(`/tp ${JN} ${s.x + 0.5} -60 ${s.z + 0.5}`); await sleep(800);
    chat.length = 0;
    const jtarget = jv.blockAt(new Vec3(s.x + 3, -60, s.z + 3));
    try { await jv.dig(jtarget); } catch (e) { log('java dig error', e.message); }
    await sleep(800);
    log(`${label} java break ->`, blockAt(s.x + 3, -60, s.z + 3), JSON.stringify(chat));
    await run(`/setblock ${s.x + 3} -60 ${s.z + 3} stone`);
    chat.length = 0;
    try { await jv.equip(jv.registry.itemsByName.stone.id, 'hand'); await jv.placeBlock(jv.blockAt(new Vec3(s.x + 1, -61, s.z + 1)), new Vec3(0, 1, 0)); } catch (e) { log('java place error', e.message); }
    await sleep(800);
    log(`${label} java place ->`, blockAt(s.x + 1, -60, s.z + 1), JSON.stringify(chat));
    await run(`/setblock ${s.x + 1} -60 ${s.z + 1} air`);
    // --- Bedrock
    await run(`/tp ${BN} ${s.x + 0.5} -60 ${s.z + 0.5}`); await sleep(800);
    k.pos = { x: s.x + 0.5, y: -60 + 1.62, z: s.z + 0.5 };
    chat.length = 0;
    k.aim(s.x + 3, -60, s.z + 3, 1);
    k.input({ block_action: [{ action: 'start_break', position: { x: s.x + 3, y: -60, z: s.z + 3 }, face: 1 }] }, { block_action: true });
    await sleep(1000);
    log(`${label} bedrock break ->`, blockAt(s.x + 3, -60, s.z + 3), JSON.stringify(chat));
    await run(`/setblock ${s.x + 3} -60 ${s.z + 3} stone`);
    chat.length = 0;
    k.aim(s.x + 1, -61, s.z + 1, 1);
    await sleep(300);
    k.click(s.x + 1, -61, s.z + 1, 1, held);
    await sleep(1000);
    log(`${label} bedrock place ->`, blockAt(s.x + 1, -60, s.z + 1), JSON.stringify(chat));
    await run(`/setblock ${s.x + 1} -60 ${s.z + 1} air`);
  }
  // --- чат
  for (const msg of ['hello normal', 'this is SPAM here', 'THIS IS LOUD']) {
    chat.length = 0; jv.chat(msg); await sleep(700);
    log('java says', JSON.stringify(msg), '->', JSON.stringify(chat));
    chat.length = 0;
    k.c.write('text', { type: 'chat', needs_translation: false, source_name: BN, xuid: '', platform_chat_id: '', filtered_message: '', message: msg }); await sleep(700);
    log('bedrock says', JSON.stringify(msg), '->', JSON.stringify(chat));
  }
  // --- команда (хук player_command): неоператор Bedrock
  chat.length = 0; k.cmd('/me hi'); await sleep(700); log('bedrock cmd /me ->', JSON.stringify(chat));
  chat.length = 0; jv.chat('/me hi'); await sleep(700); log('java cmd /me ->', JSON.stringify(chat));
  k.close(); jv.end(); op.end();
  process.exit(0);
})();
setTimeout(() => { console.log('таймаут'); process.exit(1); }, 120000);
