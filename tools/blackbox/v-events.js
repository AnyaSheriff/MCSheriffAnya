// События клика на нашем сервере: какие level_event / звуки / блок-пакеты получает Bedrock-клиент (BDS не удалось провести — см. REPORT-VERIFY.md).
const path = require('path');
const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
const { connect, sleep, jsonSafe } = require('./v-client.js');
// [название, блок, предмет в руке (Java), грань]
const CASES = [
  ['bone meal on wheat', 'wheat[age=2]', 'bone_meal', 1],
  ['bone meal on grass', 'grass_block', 'bone_meal', 1],
  ['honeycomb on copper', 'copper_block', 'honeycomb', 1],
  ['axe scrapes copper', 'exposed_copper', 'iron_axe', 1],
  ['axe unwax', 'waxed_copper_block', 'iron_axe', 1],
  ['bell ring (hand)', 'bell[attachment=floor,facing=north]', null, 2],
  ['note block (hand)', 'note_block', null, 1],
  ['brush on suspicious sand', 'suspicious_sand', 'brush', 1],
  ['flint and steel on tnt', 'tnt', 'flint_and_steel', 1],
  ['shovel on campfire', 'campfire[lit=true]', 'iron_shovel', 1],
  ['hoe on dirt', 'dirt', 'iron_hoe', 1],
  ['sponge-like: water bucket', 'stone', 'water_bucket', 1]
];
(async () => {
  const NAME = 'e' + (process.pid % 1000);
  const op = mineflayer.createBot({ host: '127.0.0.1', port: 25964, username: 'blackbox', version: '26.1', auth: 'offline' });
  await new Promise((r) => op.once('spawn', r)); await sleep(1000);
  const run = async (s) => { op.chat(s); await sleep(350); };
  const k = await connect('ours', NAME);
  let inv = [], log = [];
  k.on((n, p) => {
    if (n === 'inventory_content' && p.window_id === 'inventory') inv = p.input;
    if (n === 'inventory_slot' && p.window_id === 'inventory') inv[p.slot] = p.item;
    if (n === 'update_block' && p.position.y === 1) return; if (n === 'block_entity_data' && p.position.y === 1) return;
    if (['level_event', 'level_sound_event', 'block_event', 'spawn_particle_effect', 'play_sound', 'update_block', 'block_entity_data', 'update_blocks_sync'].includes(n)) {
      const q = jsonSafe(p);
      log.push(n === 'update_block' ? `update_block ${JSON.stringify(q.position)} rid=${q.block_runtime_id} layer=${q.layer}` : n === 'level_event' ? `level_event ${q.event} ${JSON.stringify(q.position)} data=${q.data}` : n === 'level_sound_event' ? `sound ${q.sound_id} extra=${q.extra_data}` : `${n} ${JSON.stringify(q).slice(0, 110)}`);
    }
  });
  await sleep(2500);
  await run('/fill -3 -61 -3 40 -61 40 grass_block'); await run('/fill -3 -60 -3 40 -57 40 air');
  for (let i = 0; i < CASES.length; i++) {
    const [title, block, item, face] = CASES[i]; if (process.argv[2] && !title.includes(process.argv[2])) continue;
    const x = 4 + 3 * i, z = 8;
    if (block === 'grass_block') { await run(`/setblock ${x} -61 ${z} grass_block`); } else await run(`/setblock ${x} -60 ${z} ${block}`);
    await run(`/clear ${NAME}`);
    if (item) { await run(`/give ${NAME} ${item} 3`); await sleep(500); }
    await run(`/tp ${NAME} ${x + 0.5} -60 ${z - 2}`); await sleep(500);
    const ty = block === 'grass_block' ? -61 : -60;
    k.pos = { x: x + 0.5, y: -60 + 1.62, z: z - 2 }; k.aim(x, ty, z, face); await sleep(300);
    const slot = inv.findIndex((it, idx) => idx < 9 && it && it.network_id);
    const held = slot >= 0 ? inv[slot] : { network_id: 0 };
    log = [];
    k.click(x, ty, z, face, held);
    await sleep(1500);
    console.log(`${title}: ${log.filter((l) => !l.includes('layer=1')).slice(0, 14).join(' | ')}`);
    await run(`/setblock ${x} -60 ${z} air`);
  }
  k.close(); op.end(); process.exit(0);
})();
setTimeout(() => { console.log('таймаут'); process.exit(1); }, 150000);
