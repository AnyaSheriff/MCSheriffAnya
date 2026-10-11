// Запись того, что клиент Bedrock получает при призыве сущностей.
//   node br-capture.js bds  <порт BDS-прослойки> out.jsonl cases.js
//   node br-capture.js ours <порт нашего Bedrock> out.jsonl cases.js <порт нашей Java>
// В режиме bds команды из case.bds выполняет сам клиент Bedrock (он оператор),
// в режиме ours команды из case.java выполняет Java-бот-оператор (mineflayer).
// Клиент Bedrock — только чёрный ящик (пакеты по сети), код bedrock-protocol не читался.
const path = require('path'), fs = require('fs');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const mode = process.argv[2], port = +process.argv[3], out = process.argv[4];
const cases = require(path.resolve(process.argv[5])).cases;
const javaPort = +process.argv[6] || 0;
// ONLY=метка1,метка2 — прогнать только эти случаи; метка с «*» на конце — по началу.
const only = process.env.ONLY ? process.env.ONLY.split(',') : null;
const wanted = l => !only || only.some(x => x.endsWith('*') ? l.startsWith(x.slice(0, -1)) : l === x);
const fd = fs.openSync(out, 'w');
const J = o => JSON.stringify(o, (k, v) => typeof v === 'bigint' ? v.toString() : (v && v.type === 'Buffer' && v.data ? '<buf ' + v.data.length + '>' : v));
const client = bedrock.createClient({ host: '127.0.0.1', port, username: 'entprobe', offline: true, version: '1.26.10', skipPing: false, raknetBackend: 'jsp-raknet', useRaknetWorkers: false });
const want = new Set(['add_entity', 'set_entity_data', 'remove_entity', 'add_item_entity', 'entity_event', 'set_entity_motion', 'sync_entity_property', 'update_attributes', 'mob_effect', 'set_entity_link', 'mob_equipment', 'mob_armor_equipment', 'boss_event', 'add_painting', 'take_item_entity', 'level_event', 'spawn_particle_effect', 'animate', 'command_output', 'text']);
let t0 = Date.now(), label = 'join', rid = null, pos = null, tick0 = 0n, ts0 = 0, ackTp = false;

function log(o) { o.t = Date.now() - t0; o.label = label; fs.writeSync(fd, J(o) + '\n'); }
client.on('start_game', p => { rid = p.runtime_entity_id; pos = p.player_position; tick0 = BigInt(p.current_tick[0]) * 4294967296n + BigInt(p.current_tick[1]); ts0 = Date.now(); });
client.on('respawn', p => { if (p.state === 1) { pos = p.position; client.write('respawn', { position: p.position, state: 2, runtime_entity_id: rid }); } });
client.on('move_player', p => { if (rid !== null && BigInt(p.runtime_id) === BigInt(rid)) { pos = p.position; ackTp = true; } });
client.on('packet', pk => { const n = pk.data.name, p = pk.data.params; if (want.has(n)) log({ n, p }); });
client.on('kick', r => { log({ ev: 'kick', r }); console.log('kick', J(r)); process.exit(1); });

// Предмет в руке (слот 0) и номера сущностей — для взаимодействия как у игрока.
let held = { network_id: 0 };
const seen = [];
client.on('inventory_slot', p => { if (p.window_id === 'inventory' && p.slot === 0) held = p.item; });
client.on('inventory_content', p => { if (p.window_id === 'inventory' && p.input[0] && p.input[0].network_id) held = p.input[0]; });
client.on('add_entity', p => { if (Math.abs(p.position.x - 4000.5) < 8 && Math.abs(p.position.z - 4010.5) < 8) seen.push([p.entity_type, p.runtime_id]); });
function useEntity(type, attack) {
  const hit = seen.filter(e => e[0] === type).pop();
  log({ ev: 'use', type, found: !!hit, held_id: held && held.network_id });
  if (!hit) return;
  client.write('inventory_transaction', { transaction: { legacy: { legacy_request_id: 0 }, transaction_type: 'item_use_on_entity', actions: [], transaction_data: { entity_runtime_id: BigInt(hit[1]), action_type: attack ? 'attack' : 'interact', hotbar_slot: 0, held_item: held, player_pos: pos, click_pos: { x: 0, y: 0, z: 0 } } } });
}
function bcmd(c) { client.write('command_request', { command: c, origin: { type: 'player', uuid: '00000000-0000-0000-0000-000000000000', request_id: '', player_entity_id: 0n }, internal: false, version: 'latest' }); }
function sendInput() {
  const f = {}; if (ackTp) { f.handled_teleport = true; ackTp = false; }
  client.write('player_auth_input', { pitch: 0, yaw: 0, position: pos, move_vector: { x: 0, z: 0 }, head_yaw: 0, input_data: f, input_mode: 'mouse', play_mode: 'normal', interaction_model: 'classic', interact_rotation: { x: 0, z: 0 }, tick: tick0 + BigInt(Math.floor((Date.now() - ts0) / 50)), delta: { x: 0, y: 0, z: 0 }, analogue_move_vector: { x: 0, z: 0 }, camera_orientation: { x: 0, y: 0, z: 0 }, raw_move_vector: { x: 0, z: 0 } });
}
const wait = ms => new Promise(r => setTimeout(r, ms));

client.on('spawn', () => {
  client.write('serverbound_loading_screen', { type: 1 });
  client.write('serverbound_loading_screen', { type: 2 });
  setInterval(sendInput, 50);
  (async () => {
    let jcmd = bcmd;
    await wait(1500);

    if (mode === 'ours') {
      const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
      const bot = mineflayer.createBot({ host: '127.0.0.1', port: javaPort, username: 'blackbox', version: '26.1', auth: 'offline' });
      bot.on('error', e => console.error('java:', e.message));
      bot.on('messagestr', m => log({ ev: 'chat', m }));
      await new Promise(r => bot.once('spawn', r));
      await wait(1500);
      jcmd = c => bot.chat('/' + c);
    }

    const run = (c) => { jcmd(c); return wait(150); };

    await run('gamerule domobspawning false');
    await run('gamerule dodaylightcycle false');
    await run('time set 6000');
    await run(mode === 'ours' ? 'tp entprobe 4000.5 -60 4000.5' : 'tp @s 4000.5 -60 4000.5');
    if (mode === 'ours') await run('tp blackbox 4000.5 -60 4006.5');
    await wait(2500);
    label = 'clean'; await run('kill @e[type=!player]'); await wait(800);

    for (const c of cases) {
      if (!wanted(c.label)) continue;
      const list = mode === 'ours' ? c.java : c.bds;
      if (!list) { log({ ev: 'skip', why: 'no ' + mode + ' commands' }); continue; }
      label = c.label;
      for (const line of list) {
        if (typeof line === 'number') await wait(line);
        else if (typeof line === 'object') {
          // {give: 'shears'} — выдать игроку Bedrock предмет в слот 0; {use: 'minecraft:wolf', times: n, attack: true}
          if (line.give) { await run(mode === 'ours' ? 'clear entprobe' : 'clear @s'); await wait(300); await run(mode === 'ours' ? 'give entprobe minecraft:' + line.give : 'give @s ' + line.give); await wait(700); }
          if (line.throw) for (let i = 0; i < line.throw; i++) { log({ ev: 'throw', held_id: held && held.network_id }); client.write('inventory_transaction', { transaction: { legacy: { legacy_request_id: 0 }, transaction_type: 'item_use', actions: [], transaction_data: { action_type: 'click_air', trigger_type: 'player_input', block_position: { x: 0, y: 0, z: 0 }, face: 0, hotbar_slot: 0, held_item: held, player_pos: pos, click_pos: { x: 0, y: 0, z: 0 }, block_runtime_id: 0, client_prediction: 'success', client_cooldown_state: 'off' } } }); await wait(400); }
          if (line.use) for (let i = 0; i < (line.times || 1); i++) { useEntity(line.use, line.attack); await wait(line.gap || 350); }
        } else await run(line);
      }
      await wait(c.wait || 900);
      label = 'kill'; await run('kill @e[type=!player]'); await wait(500);
    }
    label = 'end'; await wait(500);
    process.exit(0);
  })();
});
setTimeout(() => { console.log('timeout'); process.exit(1); }, 600000);
