// Робот Bedrock проверяет стройку: ломает блок под ногами, ставит на его
// место камень и смотрит, что сервер прислал в update_block.
//
// node tools/measure/bedrockbuild.js [порт=19132]
//
// Библиотека bedrock-protocol — чёрный ящик, как mineflayer у Java.
const path = require('path');
const fs = require('fs');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const port = Number(process.argv[2] || 19132);
// Второй аргумент — ник: так роботом можно зайти в уже известного игрока.

const states = JSON.parse(fs.readFileSync(path.join(process.env.HOME,
  '.cache/mcsheriffanya/minecraft-data/bedrock/1.26.10/blockStates.json')));
const stoneRuntime = states.findIndex(state => state.name === 'stone');

const client = bedrock.createClient({
  host: '127.0.0.1', port, username: process.argv[3] || 'bedrockbuild', offline: true,
  version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet',
});

let start = null;
let stoneItem = null;
let target = null;
let acted = 0;
let lastChunk = 0;
let spawned = 0;

client.on('start_game', packet => { start = packet.player_position; });
client.on('item_registry', packet => {
  const entry = (packet.itemstates || packet.items || []).find(item => item.name === 'minecraft:stone');
  stoneItem = entry && entry.runtime_id;
});
client.on('creative_content', packet => console.log('творческий инвентарь:', packet.groups.length, 'вкладок,', packet.items.length, 'предметов, первый', JSON.stringify(packet.items[0])));
client.on('start_game', p => console.log('режим игры', p.player_gamemode));
client.on('inventory_slot', p => console.log('inventory_slot', p.window_id, p.slot, `${p.item.network_id}x${p.item.count || 0}`));
client.on('add_item_entity', p => console.log('add_item_entity', p.runtime_entity_id, p.item.network_id));
client.on('take_item_entity', p => console.log('take_item_entity', p.runtime_entity_id, '→', p.target));
client.on('update_abilities', packet => console.log('способности:', JSON.stringify(packet.abilities[0].enabled)));
client.on('update_block', packet => {
  if (process.env.ALL) console.log('любой update_block', JSON.stringify(packet));
  const p = packet.position;
  if (target && p.x === target.x && p.y === target.y && p.z === target.z) {
    console.log('update_block через', Date.now() - acted, 'мс', JSON.stringify(p), 'номер', packet.block_runtime_id,
      packet.block_runtime_id === stoneRuntime ? '(камень)' : '');
  }
});

function use(action, position, face, item) {
  acted = Date.now();
  try {
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
  } catch (error) { console.log('не записал:', error.message); }
}

client.on('packet', packet => { if (process.env.DUMP && acted) console.log('пакет', Date.now() - acted, packet.data.name, JSON.stringify(packet.data.params.position || '')); });
client.on('level_chunk', () => { lastChunk = Date.now(); });
client.on('spawn', () => {
  spawned = Date.now();
  console.log('появился; камень: предмет', stoneItem, 'блок', stoneRuntime);
  target = { x: Math.floor(start.x), y: Math.floor(start.y - 1.62) - 1, z: Math.floor(start.z) };
  setTimeout(() => { console.log('ломаю', JSON.stringify(target)); use('break_block', target, 1); }, 1000);
  setTimeout(() => {
    const below = { x: target.x, y: target.y - 1, z: target.z };
    console.log('ставлю камень на', JSON.stringify(below));
    use('click_block', below, 1, { network_id: stoneItem, count: 1, metadata: 0, has_stack_id: 0,
      block_runtime_id: stoneRuntime, extra: { has_nbt: false, can_place_on: [], can_destroy: [] } });
  }, 2500);
  // Ступени и пыль — на только что поставленный камень.
  setTimeout(() => {
    const at = { x: target.x, y: target.y, z: target.z };
    const item = (name, network_id) => ({ network_id, count: 1, metadata: 0, has_stack_id: 0,
      block_runtime_id: states.findIndex(state => state.name === name) + 0,
      extra: { has_nbt: false, can_place_on: [], can_destroy: [] } });
    console.log('ставлю ступени и пыль');
    use('click_block', at, 1, item('oak_stairs', 53));
    use('click_block', { x: at.x + 1, y: at.y, z: at.z }, 1, { ...item('air', 405), block_runtime_id: 0 });
  }, 4000);
  setTimeout(() => { console.log('последний чанк через', lastChunk - spawned, 'мс после появления'); client.close(); setTimeout(() => process.exit(0), 500); }, 8000);
});
client.on('kick', reason => console.log('выгнан:', JSON.stringify(reason)));
client.on('error', error => console.log('ошибка:', error.message));
setTimeout(() => { console.log('не дождался'); process.exit(1); }, 30000);
