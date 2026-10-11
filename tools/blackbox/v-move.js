// Перенос предметов в окнах нашего сервера через ItemStackRequest (в BDS клики не удалось провести — см. REPORT-VERIFY.md).
// node v-move.js — для каждого контейнера: открыть, переложить 5 камней из панели в ячейку 0, затем обратно, показать ответы.
const path = require('path');
const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
const { connect, sleep, jsonSafe } = require('./v-client.js');
const CASES = [['copper_chest', 'container'], ['hopper', 'container'], ['shulker_box', 'shulker'], ['crafter', 'crafter'], ['ender_chest', 'container'], ['barrel', 'barrel'], ['furnace', 'furnace_ingredient'], ['trapped_chest', 'container']];

(async () => {
  const NAME = 'm' + (process.pid % 1000);
  const op = mineflayer.createBot({ host: '127.0.0.1', port: 25964, username: 'blackbox', version: '26.1', auth: 'offline' });
  await new Promise((r) => op.once('spawn', r)); await sleep(1000);
  const run = async (s) => { op.chat(s); await sleep(350); };
  const k = await connect('ours', NAME);
  let inv = [];
  let log = [];
  k.on((n, p) => {
    if (n === 'inventory_content' && p.window_id === 'inventory') inv = p.input;
    if (n === 'inventory_slot' && p.window_id === 'inventory') inv[p.slot] = p.item;
    if (n === 'item_stack_response') log.push(`response ${JSON.stringify(jsonSafe(p.responses)).slice(0, 400)}`);
    if (n === 'container_open') log.push(`open type=${p.window_type} id=${p.window_id}`);
    if (n === 'inventory_slot' && p.window_id !== 'inventory') log.push(`slot w=${p.window_id} ${p.slot} -> ${p.item.network_id}x${p.item.count}`);
  });
  await sleep(2500);
  await run('/fill -3 -61 -3 30 -61 30 grass_block'); await run('/fill -3 -60 -3 30 -57 30 air');
  await run(`/gamemode survival ${NAME}`);
  let rid = -1;
  for (let i = 0; i < CASES.length; i++) {
    const [block, slotType] = CASES[i];
    const x = 4 + 3 * i, z = 8;
    log = [];
    await run(`/setblock ${x} -60 ${z} ${block}`);
    await run(`/clear ${NAME}`); await run(`/give ${NAME} stone 10`); await sleep(600);
    await run(`/tp ${NAME} ${x + 0.5} -60 ${z - 2}`); await sleep(500);
    k.pos = { x: x + 0.5, y: -60 + 1.62, z: z - 2 }; k.aim(x, -60, z, 2); await sleep(300);
    k.click(x, -60, z, 2, { network_id: 0 });
    await sleep(1200);
    const slot = inv.findIndex((it, idx) => idx < 9 && it && it.network_id);
    const stack = inv[slot] || {};
    const mv = (from, to, count) => { rid -= 2; k.c.write('item_stack_request', { requests: [{ request_id: rid, actions: [{ type_id: 'place', count, source: from, destination: to }], custom_names: [], cause: -1 }] }); };
    const hot = { slot_type: { container_id: 'hotbar' }, slot, stack_id: stack.stack_id || 0 };
    const dst = { slot_type: { container_id: slotType }, slot: 0, stack_id: 0 };
    log.push(`hotbar slot ${slot} item ${stack.network_id}x${stack.count} stack_id ${stack.stack_id}`);
    mv(hot, dst, 5); await sleep(800);
    log.push('-- обратно');
    mv({ ...dst, stack_id: 0 }, { slot_type: { container_id: 'hotbar' }, slot, stack_id: 0 }, 5); await sleep(800);
    k.c.write('container_close', { window_id: 2 + i, window_type: 'none', server: false }); await sleep(300);
    console.log(`${block}:\n  ${log.join('\n  ')}`);
    await run(`/setblock ${x} -60 ${z} air`);
  }
  k.close(); op.end(); process.exit(0);
})();
setTimeout(() => { console.log('таймаут'); process.exit(1); }, 150000);
