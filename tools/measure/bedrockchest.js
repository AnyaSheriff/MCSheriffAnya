// Робот Bedrock проверяет хранилища и рычаг: ставит два сундука рядом
// (большой сундук), открывает его, кладёт камень из творческого меню в
// ячейку, забирает часть обратно, закрывает; ставит музыкальный блок и
// щёлкает по нему (нота на полтона выше).
// Смотрит, что прислал сервер: block_entity_data, container_open,
// inventory_content, item_stack_response, container_close, block_event,
// update_block.
//
// node tools/measure/bedrockchest.js [порт=19132] [ник=bedrockchest]
//
// Строит на той высоте, где стоит (лучше в суперплоском мире). Библиотека bedrock-protocol —
// чёрный ящик, как mineflayer у Java.
const path = require('path');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const port = Number(process.argv[2] || 19132);

const client = bedrock.createClient({
  host: '127.0.0.1', port, username: process.argv[3] || 'bedrockchest', offline: true,
  version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet',
});

let start = null;
const items = {};
let window = null;

client.on('start_game', p => { start = p.player_position; console.log('режим игры', p.player_gamemode); });
client.on('item_registry', p => {
  for (const item of p.itemstates || p.items || []) items[item.name] = item.runtime_id;
});
client.on('block_entity_data', p => console.log('block_entity_data', JSON.stringify(p.position), JSON.stringify(p.nbt.value)));
client.on('container_open', p => { window = p.window_id; console.log('container_open', JSON.stringify(p, (k, v) => typeof v === 'bigint' ? v.toString() : v)); });
client.on('container_close', p => console.log('container_close', JSON.stringify(p)));
client.on('inventory_content', p => {
  const filled = p.input.map((i, slot) => [slot, i]).filter(([, i]) => i.network_id !== 0);
  console.log('inventory_content окно', p.window_id, 'слотов', p.input.length, 'имя', JSON.stringify(p.container),
    filled.slice(0, 4).map(([slot, i]) => `${slot}:${i.network_id}x${i.count}#${i.stack_id}`).join(' '));
});
client.on('inventory_slot', p => console.log('inventory_slot', p.window_id, p.slot, `${p.item.network_id}x${p.item.count || 0}`));
client.on('item_stack_response', p => console.log('item_stack_response', JSON.stringify(p.responses)));
client.on('block_event', p => console.log('block_event', JSON.stringify(p)));
client.on('update_block', p => { if (p.layer === 0 && process.env.BLOCKS) console.log('update_block', JSON.stringify(p.position), p.block_runtime_id); });

function use(position, face, item) {
  client.write('inventory_transaction', { transaction: {
    legacy: { legacy_request_id: 0 },
    transaction_type: 'item_use',
    actions: [],
    transaction_data: {
      action_type: 'click_block', trigger_type: 'player_input',
      block_position: position, face, hotbar_slot: 0,
      held_item: item || { network_id: 0 },
      player_pos: start, click_pos: { x: 0.5, y: 1, z: 0.5 },
      block_runtime_id: 0, client_prediction: 'success', client_cooldown_state: 'off',
    },
  } });
}

const held = name => ({ network_id: items['minecraft:' + name], count: 1, metadata: 0, has_stack_id: 0,
  block_runtime_id: 0, extra: { has_nbt: false, can_place_on: [], can_destroy: [] } });
const slot = (name, index) => ({ slot_type: { container_id: name }, slot: index, stack_id: 0 });
const request = (request_id, actions) => client.write('item_stack_request', { requests: [{
  request_id, actions, custom_names: [], cause: 'chat_public' }] });

client.on('spawn', () => {
  const x = Math.floor(start.x);
  const z = Math.floor(start.z);
  // Блок под ногами — на нём и строим.
  const ground = Math.floor(start.y - 1.62) - 1;
  console.log('стою на', x, ground, z);
  const steps = [
    () => { console.log('ставлю два сундука'); use({ x: x + 2, y: ground, z }, 1, held('chest')); },
    () => use({ x: x + 3, y: ground, z }, 1, held('chest')),
    () => { console.log('открываю'); use({ x: x + 2, y: ground + 1, z }, 1); },
    () => {
      console.log('кладу 10 камней в ячейку 0');
      request(-1, [
        { type_id: 'craft_creative', item_id: 1, times_crafted: 1 },
        { type_id: 'take', count: 64, source: slot('creative_output', 50), destination: slot('cursor', 0) },
        { type_id: 'place', count: 10, source: slot('cursor', 0), destination: slot('container', 0) },
        { type_id: 'place', count: 54, source: slot('cursor', 0), destination: slot('hotbar_and_inventory', 5) },
      ]);
    },
    () => {
      console.log('забираю 4 в панель, 2 — в ячейку 20');
      request(-3, [
        { type_id: 'take', count: 4, source: slot('container', 0), destination: slot('hotbar_and_inventory', 1) },
        { type_id: 'place', count: 2, source: slot('hotbar_and_inventory', 5), destination: slot('container', 20) },
      ]);
    },
    () => { console.log('закрываю окно', window); client.write('container_close', { window_id: window, window_type: 'container', server: false }); },
    () => { console.log('ставлю музыкальный блок'); use({ x: x - 2, y: ground, z }, 1, held('noteblock')); },
    () => { console.log('щёлкаю по музыкальному блоку'); process.env.BLOCKS = '1'; use({ x: x - 2, y: ground + 1, z }, 1); },
    () => { client.close(); setTimeout(() => process.exit(0), 300); },
  ];
  steps.forEach((step, index) => setTimeout(step, 1500 + index * 1000));
});
client.on('kick', r => console.log('выгнан:', JSON.stringify(r)));
client.on('error', e => console.log('ошибка:', e.message));
setTimeout(() => process.exit(1), 40000);
