// Сверка окон контейнеров: BDS против нашего. node v-windows.js <bds|ours> <out.json> [java_name,…]
// Клиент ставит блок, жмёт по нему и пишет всё, что сервер присылает до закрытия окна.
const path = require('path');
const fs = require('fs');
const { connect, sleep, jsonSafe } = require('./v-client.js');
const side = process.argv[2];
const out = process.argv[3];
const BLOCKS = JSON.parse(fs.readFileSync(path.join(__dirname, 'v-windows.json'), 'utf8'));
const NAME = 'w' + process.pid;
const ALL = true; const WATCH = new Set(['container_open', 'container_close', 'inventory_content', 'container_set_data', 'inventory_slot', 'item_stack_response', 'block_event', 'level_event', 'level_sound_event', 'text', 'toast_request']);

(async () => {
  const k = await connect(side, NAME, { legacyClick: process.argv[5] === 'legacy' });
  let cur = null; const res = [];
  k.on((n, p) => { if (cur && (ALL || WATCH.has(n))) cur.pk.push({ n, p: jsonSafe(p) }); });
  await sleep(2500);
  let bot = null;
  if (side === 'ours') {
    const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
    bot = mineflayer.createBot({ host: '127.0.0.1', port: 25964, username: 'blackbox', version: '26.1', auth: 'offline' });
    await new Promise((r) => bot.once('spawn', r)); await sleep(1500);
  }
  const run = async (s) => { if (bot) bot.chat(s); else k.cmd(s); await sleep(side === 'bds' ? 250 : 350); };
  const py = -60;
  await run(`/fill -3 ${py - 1} -3 30 ${py - 1} 30 grass_block`);
  await run(`/fill -3 ${py} -3 30 ${py + 3} 30 air`);
  const only = process.argv[4] ? new Set(process.argv[4].split(',')) : null;
  for (let i = 0; i < BLOCKS.length; i++) {
    const [bds, java] = BLOCKS[i];
    if (only && !only.has(java)) continue;
    const x = 4 + 3 * (i % 8), z = 8 + 4 * Math.floor(i / 8);
    await run(`/setblock ${x} ${py} ${z} ${side === 'bds' ? bds : java}`);
    await run(`/tp ${NAME} ${x + 0.5} ${py} ${z - 2}`);
    await sleep(900);
    k.pos = { x: x + 0.5, y: py + 1.62, z: z - 2 }; k.aim(x, py, z, 2); await sleep(300);
    cur = { block: java, pos: k.pos, pk: [] }; res.push(cur);
    k.click(x, py, z, 2);
    await sleep(1500);
    const open = cur.pk.find((p) => p.n === 'container_open');
    if (open) { k.c.write('container_close', { window_id: open.p.window_id, window_type: open.p.window_type, server: false }); await sleep(600); }
    cur = null;
    await run(`/setblock ${x} ${py} ${z} air`);
  }
  fs.writeFileSync(out, JSON.stringify(res, null, 1));
  console.log('готово', out, res.length);
  if (bot) bot.end();
  k.close();
  process.exit(0);
})();
setTimeout(() => { console.log('таймаут'); process.exit(1); }, 280000);
