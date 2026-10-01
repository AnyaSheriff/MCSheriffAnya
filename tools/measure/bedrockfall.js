// Робот Bedrock проверяет здоровье: его роняют с высоты (команда с Java-оператора),
// он сам «падает» — шлёт Player Auth Input с местом по тяжести — и печатает,
// что прислал сервер: атрибуты (health, hunger…), entity_event (hurt/death),
// death_info, respawn. Увидев здоровье 0, жмёт «возродиться» (player_action respawn).
//
// node tools/measure/bedrockfall.js [порт=19132] [ник=bedrockfall] [земля_y=-60]
//
// Ронять надо снаружи: `gamemode survival <ник>`, `tp <ник> x y z` (оператором Java
// или консолью). Библиотека bedrock-protocol — чёрный ящик, как mineflayer у Java.
const path = require('path');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const port = Number(process.argv[2] || 19132);
const ground = Number(process.argv[4] || -60);
const EYES = 1.62;

const client = bedrock.createClient({
  host: '127.0.0.1', port, username: process.argv[3] || 'bedrockfall', offline: true,
  version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet',
});

const show = v => JSON.stringify(v, (k, x) => typeof x === 'bigint' ? x.toString() : x);
let runtime = null;
let pos = null;      // ноги
let vy = 0;
let tick = 0n;
let teleported = false;
let dead = false;
const log = (...a) => console.log(((Date.now() - t0) / 1000).toFixed(2), ...a);
const t0 = Date.now();

client.on('start_game', p => { runtime = p.runtime_entity_id; pos = { x: p.player_position.x, y: p.player_position.y - EYES, z: p.player_position.z }; log('старт, режим', p.player_gamemode, show(pos)); });
client.on('move_player', p => {
  if (String(p.runtime_id) !== String(runtime)) return;
  pos = { x: p.position.x, y: p.position.y - EYES, z: p.position.z };
  vy = 0; teleported = true;
  log('перенос', p.mode, show(pos));
});
client.on('update_attributes', p => {
  const a = p.attributes.filter(x => x.name !== 'minecraft:movement').map(x => `${x.name.replace('minecraft:', '')}=${Number(x.current.toFixed(3))}`).join(' ');
  log('атрибуты', a);
  const health = p.attributes.find(x => x.name === 'minecraft:health');
  if (health && health.current <= 0 && !dead) {
    dead = true;
    setTimeout(() => {
      log('жму «возродиться»');
      client.write('player_action', { runtime_entity_id: runtime, action: 'respawn', position: { x: 0, y: 0, z: 0 }, result_position: { x: 0, y: 0, z: 0 }, face: 0 });
    }, 500);
  }
});
client.on('entity_event', p => { if (String(p.runtime_entity_id) === String(runtime)) log('entity_event', p.event_id, p.data); });
client.on('death_info', p => log('death_info', show(p)));
client.on('respawn', p => { log('respawn', show(p)); if (p.state === 'ready_to_spawn' || p.state === 1) { dead = false; client.write('respawn', { position: p.position, state: 2, runtime_entity_id: runtime }); } });
client.on('text', p => log('чат', p.message));
client.on('kick', r => log('выгнан', show(r)));
client.on('error', e => log('ошибка', e.message));

client.on('spawn', () => {
  log('появился');
  setInterval(() => {
    if (!pos || dead) return;
    // Тяжесть игрока: (v − 0,08)·0,98 за такт, пол — на земле.
    let landed = false;
    if (pos.y > ground + 0.001 || vy > 0) {
      vy = (vy - 0.08) * 0.98;
      pos.y += vy;
      if (pos.y <= ground) { pos.y = ground; vy = 0; landed = true; }
    } else landed = true;
    tick += 1n;
    const flags = { vertical_collision: landed, handled_teleport: teleported };
    teleported = false;
    client.write('player_auth_input', {
      pitch: 0, yaw: 0, position: { x: pos.x, y: pos.y + EYES, z: pos.z }, move_vector: { x: 0, z: 0 }, head_yaw: 0,
      input_data: flags, input_mode: 'mouse', play_mode: 'normal', interaction_model: 'classic',
      interact_rotation: { x: 0, z: 0 }, tick, delta: { x: 0, y: vy, z: 0 },
      analogue_move_vector: { x: 0, z: 0 }, camera_orientation: { x: 0, y: 0, z: 1 }, raw_move_vector: { x: 0, z: 0 },
    });
  }, 50);
});
setTimeout(() => process.exit(0), Number(process.env.SECONDS || 25) * 1000);
