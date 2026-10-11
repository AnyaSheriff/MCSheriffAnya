// Смешанный PvP на НАШЕМ сервере: Java-робот (mineflayer) и Bedrock-клиент (bedrock-protocol) бьют друг друга;
// те же случаи Java против Java — эталон. Числа: урон жертве, отдача, дальность, перезарядка.
// node br-pvp.js [jj|jb|bj|all] [случай,...]  → JSON-строки и сводка в tmp/br/pvp-<режим>.json
//   jj — оба Java (эталон), jb — Java бьёт Bedrock, bj — Bedrock бьёт Java.
const path = require('path'), fs = require('fs');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
const sleep = ms => new Promise(r => setTimeout(r, ms));
const big = (k, v) => typeof v === 'bigint' ? Number(v) : v;
const modes = (process.argv[2] || 'all') === 'all' ? ['jj', 'jb', 'bj'] : process.argv[2].split(',');
const wanted = process.argv[3] ? process.argv[3].split(',') : null;
const JPORT = 25962, BPORT = 19163;
const outDir = path.join(__dirname, '..', '..', 'tmp', 'br'); fs.mkdirSync(outDir, { recursive: true });

// Случаи: setup — команды ($A атакующий, $V жертва), dist — расстояние по z от жертвы, hits — моменты ударов (мс от начала).
const CASES = {
  'sword-full': { give: 'iron_sword', dist: 1.5, hits: [0] },
  'sword-spam': { give: 'iron_sword', dist: 1.5, hits: [0, 250, 500, 750, 1000, 1250, 1500] },
  'sword-paced': { give: 'iron_sword', dist: 1.5, hits: [0, 700, 1400, 2100] },
  'fist': { give: null, dist: 1.5, hits: [0] },
  'axe-full': { give: 'diamond_axe', dist: 1.5, hits: [0] },
  'bow-full': { give: 'bow', extra: 'arrow 16', dist: 8, hits: [0], bow: true, draw: 1300 },
  'bow-half': { give: 'bow', extra: 'arrow 16', dist: 8, hits: [0], bow: true, draw: 450 },
  'shield-front': { give: 'iron_sword', dist: 1.5, hits: [0], shield: true, victimYaw: 0 },
  'shield-back': { give: 'iron_sword', dist: 1.5, hits: [0], shield: true, victimYaw: 180 },
  'shield-axe': { give: 'diamond_axe', dist: 1.5, hits: [0, 1500], shield: true, victimYaw: 0 },
  'armor': { give: 'iron_sword', dist: 1.5, hits: [0], armor: true },
  'strength': { give: 'iron_sword', dist: 1.5, hits: [0], effectA: 'strength 30 1' },
  'weakness': { give: 'iron_sword', dist: 1.5, hits: [0], effectA: 'weakness 30 0' },
  'reach-2.9': { give: 'iron_sword', dist: 2.9, hits: [0] },
  'reach-3.05': { give: 'iron_sword', dist: 3.05, hits: [0] },
  'reach-3.3': { give: 'iron_sword', dist: 3.3, hits: [0] },
  'reach-3.6': { give: 'iron_sword', dist: 3.6, hits: [0] },
  'reach-4.0': { give: 'iron_sword', dist: 4.0, hits: [0] },
  'reach-4.4': { give: 'iron_sword', dist: 4.4, hits: [0] },
  'reach-6.2': { give: 'iron_sword', dist: 6.2, hits: [0] },
  'reach-6.6': { give: 'iron_sword', dist: 6.6, hits: [0] },
};

async function run(mode) {
  const ATT = mode === 'bj' ? 'brrec' : 'pvpa', VIC = mode === 'jb' ? 'brrec' : mode === 'bj' ? 'pvpa' : 'pvpb';
  const log = [];
  const ev = (who, what) => log.push({ t: Date.now() - t0, who, what });
  let t0 = Date.now();
  const opbot = mineflayer.createBot({ host: '127.0.0.1', port: JPORT, username: 'blackbox', version: '26.1', auth: 'offline' });
  await new Promise(r => opbot.once('spawn', r));
  const cmd = async c => { opbot.chat(c.startsWith('/') ? c : '/' + c); await sleep(250); };
  for (const c of ['/gamemode spectator blackbox', '/tp blackbox 0 100 300', '/gamerule spawn_mobs false', '/difficulty normal', '/time set day', '/gamerule advance_time false', '/weather clear', '/gamerule pvp true']) await cmd(c);

  // участники
  const players = {};   // имя → { kind, ... }
  const jbots = {};
  for (const n of ['pvpa', 'pvpb']) {
    if (n !== ATT && n !== VIC && !(mode === 'jj')) continue;
    if (mode === 'jb' && n === 'pvpb') continue;
    if (mode === 'bj' && n === 'pvpb') continue;
    const b = mineflayer.createBot({ host: '127.0.0.1', port: JPORT, username: n, version: '26.1', auth: 'offline' });
    await new Promise(r => b.once('spawn', r));
    jbots[n] = b;
    let prev = 20;
    b.on('health', () => { ev(n, { health: b.health }); prev = b.health; });
    b._client.on('entity_velocity', p => { if (p.entityId === b.entity.id) ev(n, { vel: [p.velocity.x, p.velocity.y, p.velocity.z].map(x => Math.round(x * 1000) / 1000) }); });
  }
  let brYaw = 180, brSneak = false, bc = null, bpos = null, brid = null, bhand = { network_id: 0 }, ids = {}, brtick = 1n;
  if (mode !== 'jj') {
    bc = bedrock.createClient({ host: '127.0.0.1', port: BPORT, username: 'brrec', offline: true, version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet' });
    bc.on('error', e => console.log('bedrock error', e.message));
    bc.on('start_game', p => { brid = p.runtime_entity_id; bpos = p.player_position; });
    bc.on('packet', ({ data }) => {
      const n = data.name, p = data.params;
      if (n === 'add_player') ids[p.username] = p.runtime_id;
      if (n === 'update_attributes' && p.runtime_entity_id == brid) { const h = p.attributes.find(a => a.name === 'minecraft:health'); if (h) { ev('brrec', { health: Math.round(h.current * 100) / 100 }); if (h.current <= 0) setTimeout(() => { try { bc.write('respawn', { position: { x: 0, y: 0, z: 0 }, state: 2, runtime_entity_id: brid }); } catch (e) { /* закрыто */ } }, 600); } }
      if (n === 'set_entity_motion' && p.runtime_entity_id == brid) ev('brrec', { vel: [p.velocity.x, p.velocity.y, p.velocity.z].map(x => Math.round(x * 1000) / 1000) });
      if (n === 'entity_event' && p.runtime_entity_id == brid && p.event_id === 'hurt_animation') ev('brrec', { hurt: p.data });
      if (n === 'inventory_content' && p.window_id === 'inventory') bhand = p.input[0];
      if (n === 'inventory_slot' && p.window_id === 'inventory' && p.slot === 0) bhand = p.item;
      if (n === 'move_player' && p.runtime_id == brid) bpos = p.position;
    });
    await new Promise(r => bc.once('spawn', r));
    try { bc.write('set_local_player_as_initialized', { runtime_entity_id: brid }); } catch (e) { /* уже */ }
    setInterval(() => { try { bc.write('player_auth_input', { pitch: 0, yaw: brYaw, position: bpos, move_vector: { x: 0, z: 0 }, head_yaw: brYaw, input_data: { vertical_collision: true, sneaking: brSneak }, input_mode: 'mouse', play_mode: 'normal', interaction_model: 'classic', interact_rotation: { x: 0, z: 0 }, tick: brtick++, delta: { x: 0, y: 0, z: 0 }, analogue_move_vector: { x: 0, z: 0 }, camera_orientation: { x: 0, y: 0, z: -1 }, raw_move_vector: { x: 0, z: 0 } }); } catch (e) { /* закрыто */ } }, 50);
    await sleep(2500);
  }
  await sleep(1000);

  const results = {};
  const y0 = bpos ? Math.round((bpos.y - 1.62) * 100) / 100 : 4;   // ноги Bedrock-игрока на спавне — запоминаем один раз
  const feetY = () => y0;
  for (const [name, c] of Object.entries(CASES)) {
    if (wanted && !wanted.includes(name)) continue;
    log.length = 0; t0 = Date.now();
    const y = mode === 'jj' ? await javaGroundY(jbots, VIC) : feetY();
    // чистое состояние
    for (const n of [ATT, VIC]) { await cmd(`/clear ${n}`); await cmd(`/effect clear ${n}`); await cmd(`/gamemode survival ${n}`); }
    const vyaw = c.victimYaw === undefined ? 0 : c.victimYaw; brYaw = mode === 'jb' ? vyaw : 180; brSneak = false;
    await cmd(`/tp ${VIC} 0.5 ${y} 0.5 ${vyaw} 0`);
    await cmd(`/tp ${ATT} 0.5 ${y} ${0.5 + c.dist} 180 0`);
    if (c.give) await cmd(`/give ${ATT} minecraft:${c.give}`);
    if (c.extra) await cmd(`/give ${ATT} minecraft:${c.extra}`);
    if (c.effectA) await cmd(`/effect give ${ATT} minecraft:${c.effectA}`);
    if (c.armor) for (const [slot, item] of [['head', 'iron_helmet'], ['chest', 'iron_chestplate'], ['legs', 'iron_leggings'], ['feet', 'iron_boots']]) await cmd(`/item replace entity ${VIC} armor.${slot} with minecraft:${item}`);
    if (c.shield) await cmd(`/item replace entity ${VIC} weapon.offhand with minecraft:shield`);
    await cmd(`/effect give ${VIC} minecraft:instant_health 1 10`);
    await sleep(3500);   // перезарядка выросла, i-кадры ушли
    if (c.shield) {
      // жертва поднимает щит: Java — правая кнопка рукой щита, Bedrock — присед (общий путь тела: Codex)
      if (mode === 'jb') brSneak = true; else await jbots[VIC].activateItem(true);
      await sleep(1000);
    }
    log.length = 0; t0 = Date.now();
    const start = Date.now();
    for (const at of c.hits) {
      await sleep(Math.max(0, at - (Date.now() - start)));
      ev(ATT, { attack: Date.now() - start, held: jbots[ATT] && (jbots[ATT].heldItem ? jbots[ATT].heldItem.name : null) });
      if (c.bow) {
        // выстрел: тянем c.draw мс и отпускаем (Java — Use Item / Release; Bedrock — item_use click_air и item_release)
        if (mode === 'bj') {
          const nothing = { x: 0, y: 0, z: 0 };
          bc.write('inventory_transaction', { transaction: { legacy: { legacy_request_id: 0 }, transaction_type: 'item_use', actions: [], transaction_data: { action_type: 'click_air', trigger_type: 'player_input', block_position: nothing, face: -1, hotbar_slot: 0, held_item: bhand, player_pos: bpos, click_pos: nothing, block_runtime_id: 0, client_prediction: 'success', client_cooldown_state: 'off' } } });
          await sleep(c.draw || 1200);
          bc.write('inventory_transaction', { transaction: { legacy: { legacy_request_id: 0 }, transaction_type: 'item_release', actions: [], transaction_data: { action_type: 'release', hotbar_slot: 0, held_item: bhand, head_pos: bpos } } });
        } else {
          const b = jbots[ATT]; const target = b.players[VIC] && b.players[VIC].entity;
          await b.lookAt(target.position.offset(0, 1.2, 0), true);
          b.activateItem(); await sleep(c.draw || 1200); b.deactivateItem();
        }
        continue;
      }
      if (mode === 'bj') {
        if (ids[VIC] === undefined) { ev(ATT, { error: 'нет runtime id жертвы' }); continue; }
        // игрок Bedrock бьёт: взгляд — на жертву (вдоль -z), предмет — как сообщил сервер
        bc.write('inventory_transaction', { transaction: { legacy: { legacy_request_id: 0 }, transaction_type: 'item_use_on_entity', actions: [], transaction_data: { entity_runtime_id: ids[VIC], action_type: 'attack', hotbar_slot: 0, held_item: bhand, player_pos: bpos, click_pos: { x: 0, y: 0, z: 0 } } } });
      } else {
        const b = jbots[ATT]; const target = b.players[VIC] && b.players[VIC].entity;
        if (!target) { ev(ATT, { error: 'жертвы не видно' }); continue; }
        await b.lookAt(target.position.offset(0, 1.62, 0), true);
        b.attack(target);
      }
    }
    await sleep(1500);
    if (c.shield) { brSneak = false; if (jbots[VIC]) { try { jbots[VIC].deactivateItem(); } catch (e) { /* нечего опускать */ } } }
    // здоровье до удара — ищем по первому событию после ударов
    const hs = log.filter(e => e.who === VIC && e.what.health !== undefined).map(e => e.what.health);
    const vs = log.filter(e => e.who === VIC && e.what.vel !== undefined).map(e => e.what.vel);
    const drops = []; let last = 20; for (const h of hs) { if (last - h >= 0.5) drops.push(Math.round((last - h) * 100) / 100); last = h; }
    results[name] = { drops, velocity: vs.slice(0, 2), hurts: log.filter(e => e.what.hurt !== undefined).length };
    console.log(mode, name, JSON.stringify(results[name]));
  }
  fs.writeFileSync(path.join(outDir, `pvp-${mode}.json`), JSON.stringify(results, big, 1));
  for (const b of Object.values(jbots)) b.quit();
  opbot.quit();
  try { bc && bc.disconnect(); } catch (e) { /* закрыто */ }
  await sleep(2500);
}
async function javaGroundY(jbots, vic) { const b = jbots[vic]; return Math.round(b.entity.position.y * 100) / 100; }

(async () => {
  for (const m of modes) { await run(m); }
  process.exit(0);
})();
process.on('unhandledRejection', e => { console.log('ошибка', e && e.stack); process.exit(1); });
setTimeout(() => { console.log('таймаут'); process.exit(1); }, 900000);
