// Робот Bedrock проверяет инвентарь, которым распоряжается сервер: берёт
// камень из творческого меню (создать → в курсор → в панель), ставит его и
// выбрасывает одну штуку.
//
// node tools/measure/bedrockinv.js [порт=19132] [ник=bedrockinv]
//
// Библиотека bedrock-protocol — чёрный ящик, как mineflayer у Java.
const path = require('path');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const port = Number(process.argv[2] || 19132);
const username = process.argv[3] || 'bedrockinv';

const client = bedrock.createClient({
  host: '127.0.0.1', port, username, offline: true,
  version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet',
});

let start = null;
client.on('start_game', p => { start = p.player_position; console.log('режим игры', p.player_gamemode); });
client.on('inventory_content', p => {
  const filled = p.input.filter(i => i.network_id !== 0);
  console.log('inventory_content окно', p.window_id, 'слотов', p.input.length, 'занято', filled.length,
    filled.slice(0, 3).map(i => `${i.network_id}x${i.count}#${i.stack_id}`).join(' '));
});
client.on('inventory_slot', p => console.log('inventory_slot', p.window_id, p.slot, `${p.item.network_id}x${p.item.count || 0}`));
client.on('item_stack_response', p => console.log('item_stack_response', JSON.stringify(p.responses)));
client.on('add_item_entity', p => console.log('add_item_entity', p.runtime_entity_id, p.item.network_id));
client.on('take_item_entity', p => console.log('take_item_entity', p.runtime_entity_id, '→', p.target));

const slot = (name, index) => ({ slot_type: { container_id: name }, slot: index, stack_id: 0 });

client.on('spawn', () => {
  setTimeout(() => {
    console.log('беру камень из меню');
    client.write('item_stack_request', { requests: [{
      request_id: -1,
      actions: [
        { type_id: 'craft_creative', item_id: 1, times_crafted: 1 },
        { type_id: 'take', count: 64, source: slot('creative_output', 50), destination: slot('cursor', 0) },
        { type_id: 'place', count: 64, source: slot('cursor', 0), destination: slot('hotbar_and_inventory', 0) },
      ],
      custom_names: [], cause: 'chat_public',
    }] });
  }, 2500);
  setTimeout(() => {
    console.log('выбрасываю одну штуку');
    client.write('item_stack_request', { requests: [{
      request_id: -3,
      actions: [{ type_id: 'drop', count: 1, source: slot('hotbar_and_inventory', 0), randomly: false }],
      custom_names: [], cause: 'chat_public',
    }] });
  }, 4000);
  setTimeout(() => { client.close(); setTimeout(() => process.exit(0), 300); }, 8000);
});
client.on('kick', r => console.log('выгнан:', JSON.stringify(r)));
client.on('error', e => console.log('ошибка:', e.message));
setTimeout(() => process.exit(1), 30000);
