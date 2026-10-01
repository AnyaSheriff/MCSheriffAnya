// Робот Bedrock проверяет рамки для предметов: вешает рамку и светящуюся
// рамку на верх блока, кладёт в первую камень, поворачивает, ломает обе —
// и печатает, что прислал сервер: update_block (номер состояния блока рамки)
// и block_entity_data (ItemFrame / GlowItemFrame с Item и ItemRotation).
//
// node tools/measure/bedrockframe.js [порт=19132] [ник=bedrockframe]
//
// В творческом режиме. Опорные камни бот ставит сам на высоте ног, так что
// земля под ним не нужна. Библиотека
// bedrock-protocol — чёрный ящик, как mineflayer у Java.
const path = require('path');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const port = Number(process.argv[2] || 19132);

const client = bedrock.createClient({
  host: '127.0.0.1', port, username: process.argv[3] || 'bedrockframe', offline: true,
  version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet',
});

let start = null;
const items = {};
const show = v => JSON.stringify(v, (k, x) => typeof x === 'bigint' ? x.toString() : x);

client.on('start_game', p => { start = p.player_position; });
client.on('item_registry', p => {
  for (const item of p.itemstates || p.items || []) items[item.name] = item.runtime_id;
});
client.on('block_entity_data', p => console.log('block_entity_data', show(p.position), show(p.nbt.value)));
client.on('update_block', p => { if (p.layer === 0 && process.env.BLOCKS) console.log('update_block', show(p.position), p.block_runtime_id); });

function use(action, position, face, item) {
  client.write('inventory_transaction', { transaction: {
    legacy: { legacy_request_id: 0 },
    transaction_type: 'item_use',
    actions: [],
    transaction_data: {
      action_type: action, trigger_type: 'player_input',
      block_position: position, face, hotbar_slot: 0,
      held_item: item || { network_id: 0 },
      player_pos: start, click_pos: { x: 0.5, y: 1, z: 0.5 },
      block_runtime_id: 0, client_prediction: 'success', client_cooldown_state: 'off',
    },
  } });
}

const held = name => ({ network_id: items['minecraft:' + name], count: 1, metadata: 0, has_stack_id: 0,
  block_runtime_id: 0, extra: { has_nbt: false, can_place_on: [], can_destroy: [] } });

client.on('spawn', () => {
  const x = Math.floor(start.x);
  const z = Math.floor(start.z);
  const feet = Math.round(start.y - 1.62);
  const ground = feet + 1; // верх опорных камней
  const first = { x: x + 2, y: ground + 1, z };
  const second = { x: x + 4, y: ground + 1, z };
  console.log('стою на', x, ground, z, 'рамка в клетке', show(first));
  process.env.BLOCKS = '1';
  const steps = [
    () => { console.log('ставлю два опорных камня'); use('click_block', { x: x + 2, y: feet, z }, 1, held('stone')); },
    () => use('click_block', { x: x + 4, y: feet, z }, 1, held('stone')),
    () => { console.log('вешаю рамку на верх блока'); use('click_block', { x: x + 2, y: ground, z }, 1, held('frame')); },
    () => { console.log('вешаю светящуюся рамку'); use('click_block', { x: x + 4, y: ground, z }, 1, held('glow_frame')); },
    () => { console.log('кладу камень в первую'); use('click_block', first, 1, held('stone')); },
    () => { console.log('поворачиваю'); use('click_block', first, 1, held('stone')); },
    // KEEP=1 — рамки оставить: второй вход должен получить их с чанком.
    () => { if (!process.env.KEEP) { console.log('ломаю первую'); use('break_block', first, 1); } },
    () => { if (!process.env.KEEP) { console.log('ломаю вторую'); use('break_block', second, 1); } },
    () => { client.close(); setTimeout(() => process.exit(0), 300); },
  ];
  steps.forEach((step, index) => setTimeout(step, 1500 + index * 1000));
});
client.on('kick', r => console.log('выгнан:', JSON.stringify(r)));
client.on('error', e => console.log('ошибка:', e.message));
setTimeout(() => process.exit(1), 40000);
