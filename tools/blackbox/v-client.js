// Общий Bedrock-клиент для сверки с BDS: вход, команды, player_auth_input в цикле (BDS при серверной
// авторитетности принимает взаимодействия только внутри него), клики по блокам, ItemStackRequest.
const path = require('path');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const jsonSafe = (v) => JSON.parse(JSON.stringify(v, (k, x) => (typeof x === 'bigint' ? Number(x) : (x && x.type === 'Buffer' ? `buf${x.data.length}` : x))));

async function connect(side, name, opts = {}) {
  const port = side === 'bds' ? 19160 : 19165;
  const c = bedrock.createClient({ host: '127.0.0.1', port, username: name, offline: true, version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet' });
  const st = { c, side, rid: 1n, pos: { x: 0.5, y: 0, z: 0.5 }, tick: 0n, listeners: [], rtid: {}, yaw: 0, pitch: 0, all: [] };
  c.on('error', (e) => console.log('err', e.message));
  c.on('kick', (r) => console.log('kick', JSON.stringify(r)));
  c.on('start_game', (p) => { st.rid = p.runtime_entity_id; st.pos = { ...p.player_position }; st.start = p; st.tick = opts.tick0 ? 0n : BigInt(Array.isArray(p.current_tick) ? p.current_tick[1] : (p.current_tick || 0)); });
  c.on('packet', (pk) => {
    const n = pk.data.name, p = pk.data.params;
    if (n === 'update_block' && !p.layer) st.rtid[`${p.position.x},${p.position.y},${p.position.z}`] = p.block_runtime_id;
    if (n === 'move_player' && p.runtime_id == st.rid) { st.pos = { ...p.position }; if (p.position.y < 10000) st.paused = false; }
    if (n === 'correct_player_movement') st.pos = { ...p.position };
    for (const l of st.listeners) l(n, p);
  });
  await new Promise((r) => c.on('spawn', r));
  c.queue('set_local_player_as_initialized', { runtime_entity_id: st.rid });
  st.paused = side === 'bds';
  st.timer = setInterval(() => { if (!st.paused) st.input(); }, 50);
  st.input = (extra = {}, flags = {}) => {
    st.tick += 1n;
    const base = { pitch: st.pitch, yaw: st.yaw, position: st.pos, move_vector: { x: 0, z: 0 }, head_yaw: st.yaw,
      input_data: { ...flags }, input_mode: 'mouse', play_mode: 'normal', interaction_model: 'classic', interact_rotation: { x: st.pitch, z: st.yaw },
      tick: st.tick, delta: { x: 0, y: 0, z: 0 }, analogue_move_vector: { x: 0, z: 0 }, camera_orientation: { x: 0, y: 0, z: 0 }, raw_move_vector: { x: 0, z: 0 }, ...extra };
    try { c.write('player_auth_input', base); } catch (e) { if (!st.warned) { console.log('auth_input:', e.message); st.warned = true; } }
  };
  st.cmd = (s) => c.write('command_request', { command: s, origin: { type: 'player', uuid: '00000000-0000-0000-0000-000000000000', request_id: String(Math.random()), player_entity_id: 0 }, internal: false, version: 'latest' });
  st.on = (f) => { st.listeners.push(f); return f; };
  st.off = (f) => { st.listeners = st.listeners.filter((x) => x !== f); };
  // Нажатие по блоку: внутри player_auth_input (item_interact) с использованием предмета.
  // Прицеливаемся в центр грани (face), чтобы луч сервера попал в тот же блок/грань.
  st.aim = (x, y, z, face) => {
    const off = [[0.5, 0, 0.5], [0.5, 1, 0.5], [0.5, 0.5, 0], [0.5, 0.5, 1], [0, 0.5, 0.5], [1, 0.5, 0.5]][face];
    const dx = x + off[0] - st.pos.x, dy = y + off[1] - st.pos.y, dz = z + off[2] - st.pos.z;
    st.yaw = (Math.atan2(-dx, dz) * 180) / Math.PI; st.pitch = (-Math.asin(dy / Math.hypot(dx, dy, dz)) * 180) / Math.PI;
    return { x: off[0], y: off[1], z: off[2] };
  };
  st.click = (x, y, z, face = 1, held = { network_id: 0 }) => {
    const cp = st.aim(x, y, z, face);
    const data = { action_type: 'click_block', trigger_type: 'player_input', block_position: { x, y, z }, face, hotbar_slot: 0, held_item: held,
      player_pos: opts.feet ? { x: st.pos.x, y: st.pos.y - 1.62, z: st.pos.z } : { ...st.pos }, click_pos: cp, block_runtime_id: (st.rtid[`${x},${y},${z}`] || 0) >>> 0, client_prediction: opts.pred || 'success', client_cooldown_state: 'off' };
    if (opts.legacyClick) {
      c.write('inventory_transaction', { transaction: { legacy: { legacy_request_id: 0 }, transaction_type: 'item_use', actions: [], transaction_data: data } });
    } else {
      st.input({ transaction: { legacy: { legacy_request_id: 0 }, actions: [], data } }, { item_interact: true });
    }
  };
  st.close = () => { clearInterval(st.timer); try { c.close(); } catch (e) { /* уже закрыт */ } };
  return st;
}
module.exports = { connect, sleep, jsonSafe };
