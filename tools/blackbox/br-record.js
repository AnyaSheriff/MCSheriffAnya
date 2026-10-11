// Записывает, что сервер (BDS через прослойку или наш) шлёт Bedrock-клиенту при заданных действиях.
// node br-record.js <bds|ours> <сценарий|all> [--out dir]
// Сценарий — список шагов: ['/команда'], {w: мс}, {attack: 'zombie'}, {raw: пакет, p: поля}, {tag: 'метка'}.
// Клиент — bedrock-protocol как чёрный ящик. Результат — JSONL в tmp/br/<цель>/<сценарий>.jsonl.
const path = require('path'), fs = require('fs');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const target = process.argv[2], which = process.argv[3] || 'all';
const PORT = target === 'bds' ? 19160 : 19163;
const outDir = path.join(__dirname, '..', '..', 'tmp', 'br', target);
fs.mkdirSync(outDir, { recursive: true });
const SC = require('./br-scenarios.js');
const names = which === 'all' ? Object.keys(SC) : which.split(',');
const sleep = ms => new Promise(r => setTimeout(r, ms));
const SKIP = new Set(['level_chunk', 'subchunk', 'network_chunk_publisher_update', 'move_player', 'move_entity_delta', 'move_entity', 'set_time', 'update_block', 'update_subchunk_blocks', 'client_cache_miss_response', 'player_auth_input', 'tick_sync', 'available_commands', 'biome_definition_list', 'available_entity_identifiers', 'item_registry', 'creative_content', 'crafting_data', 'start_game', 'update_block_synced', 'update_client_input_locks', 'level_event_generic', 'event', 'current_structure_feature']);
const big = (k, v) => typeof v === 'bigint' ? Number(v) : v;
(async () => {
  const client = bedrock.createClient({ host: '127.0.0.1', port: PORT, username: 'brrec', offline: true, version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet' });
  let hand = { network_id: 0 }, cur = null, rid = null, pos = null, ents = {};
  client.on('start_game', p => { rid = p.runtime_entity_id; pos = p.player_position; });
  client.on('error', e => { console.log('error', e.message); process.exit(2); });
  client.on('kick', r => console.log('kick', JSON.stringify(r, big)));
  client.on('packet', ({ data }) => {
    if (!data || !data.name) return;
    const n = data.name, p = data.params;
    if (n === 'add_entity' || n === 'add_player' || n === 'add_item_entity') { ents[n === 'add_player' ? 'player' : (p.entity_type || n)] = p.runtime_id || p.runtime_entity_id; }
    if (n === 'respawn' && p.position && p.position.y < 1000) pos = p.position;
    if (n === 'inventory_content' && p.window_id === 'inventory') hand = p.input[0];
    if (n === 'inventory_slot' && p.window_id === 'inventory' && p.slot === 0) hand = p.item;
    if (!cur || SKIP.has(n)) return;
    cur.write(JSON.stringify({ t: Date.now() - cur.t0, n, p }, big) + '\n');
  });
  await new Promise(r => client.once('spawn', r));
  try { client.write('set_local_player_as_initialized', { runtime_entity_id: rid }); } catch (e) { console.log('init:', e.message.slice(0, 100)); }
  await sleep(3000);
  // Бездействующий клиент: пульс player_auth_input раз в такт (сервер с авторитарным движением без него бьёт мимо)
  let tick = 1n;
  const pulse = setInterval(() => {
    try {
      client.write('player_auth_input', { pitch: 25, yaw: 0, position: pos, move_vector: { x: 0, z: 0 }, head_yaw: 0, input_data: {}, input_mode: 'mouse', play_mode: 'normal', interaction_model: 'classic', interact_rotation: { x: 0, z: 0 }, tick: tick++, delta: { x: 0, y: 0, z: 0 }, analogue_move_vector: { x: 0, z: 0 }, camera_orientation: { x: 0, y: 0, z: 1 }, raw_move_vector: { x: 0, z: 0 } });
    } catch (e) { if (!pulse.warned) { pulse.warned = true; console.log('auth_input:', e.message.slice(0, 200)); } }
  }, 50);
  let jbot = null;
  if (target === 'ours') {
    // Команды Bedrock-игроку нашего сервера недоступны (вход не подтверждён Xbox Live, world.rs), поэтому
    // обстановку ставит оператор-робот Java по имени brrec, а Bedrock-клиент только смотрит и бьёт.
    const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
    jbot = mineflayer.createBot({ host: '127.0.0.1', port: 25962, username: 'blackbox', version: '26.1', auth: 'offline' });
    await new Promise(r => jbot.once('spawn', r));
    jbot.on('message', m => { if (process.argv.includes('--debug')) console.log('java:', m.toString()); });
    await sleep(500);
    for (const c of ['/gamemode spectator blackbox', '/gamerule spawn_mobs false', '/gamerule mob_spawning false', '/gamerule do_mob_spawning false', '/gamerule doMobSpawning false', '/time set day', '/gamerule advance_time false', '/weather clear', '/kill @e[type=!player]', '/tp blackbox 0 100 300']) { jbot.chat(c); await sleep(300); }
  }
  // Bedrock-синтаксис → Java-синтаксис для нашего сервера (команды идут в общий разбор)
  const J = c => {
    let m;
    if ((m = c.match(/^\/effect @s (\w+) (\d+) (\d+)( true)?$/))) return `/effect give @s minecraft:${m[1] === "village_hero" ? "hero_of_the_village" : m[1]} ${m[2]} ${m[3]}${m[4] || ''}`;
    if (c === '/effect @s clear') return '/effect clear @s';
    if ((m = c.match(/^\/xp (-?\d+)(L)? @s$/))) return m[1].startsWith('-') ? `/xp set @s 0 ${m[2] ? 'levels' : 'points'}` : `/xp add @s ${m[1]} ${m[2] ? 'levels' : 'points'}`;
    if ((m = c.match(/^\/damage (\S+) (\d+) (\w+)$/))) return `/damage ${m[1]} ${m[2]} minecraft:${{ entity_attack: "mob_attack", fire: "in_fire", void: "out_of_world", contact: "cactus" }[m[3]] || m[3]}`;
    return c;
  };
  const cmd = c => {
    if (jbot) {
      let j = J(c).replace(/@s(?! run)/g, 'brrec');
      if (j.includes('~')) j = '/execute as brrec at @s run ' + j.slice(1);
      jbot.chat(j);
    } else client.write('command_request', { command: c, origin: { type: 'player', uuid: '00000000-0000-0000-0000-000000000000', request_id: '', player_entity_id: 0 }, internal: false, version: 'latest' });
  };
  for (const name of names) {
    cur = fs.createWriteStream(path.join(outDir, name + '.jsonl')); cur.t0 = Date.now(); ents = {};
    for (const s of SC[name]) {
      if (typeof s === 'string') { cur.write(JSON.stringify({ t: Date.now() - cur.t0, step: s }) + '\n'); cmd(s); await sleep(450); }
      else if (s.w) await sleep(s.w);
      else if (s.attack) {
        const id = ents[s.attack] || ents.player;
        if (id === undefined) { cur.write(JSON.stringify({ step: 'attack: цель не найдена' }) + '\n'); continue; }
        cur.write(JSON.stringify({ step: 'attack' }) + '\n'); if (process.argv.includes('--debug')) console.log('attack', id, JSON.stringify(hand, big).slice(0, 200), JSON.stringify(pos));
        client.write('inventory_transaction', { transaction: { legacy: { legacy_request_id: 0 }, transaction_type: 'item_use_on_entity', actions: [], transaction_data: { entity_runtime_id: id, action_type: 'attack', hotbar_slot: s.slot || 0, held_item: hand, player_pos: pos, click_pos: { x: 0, y: 0, z: 0 } } } });
        await sleep(s.after || 500);
      } else if (s.raw) { client.write(s.raw, s.p); await sleep(s.after || 300); }
      else if (s.tag) cur.write(JSON.stringify({ t: Date.now() - cur.t0, step: 'TAG ' + s.tag }) + '\n');
    }
    await new Promise(r => cur.end(r)); cur = null;
    console.log('записан', name);
  }
  if (jbot) jbot.quit();
  try { client.disconnect(); } catch (e) { /* уже закрыто */ }
  await sleep(1500);
  process.exit(0);
})();
process.on('unhandledRejection', e => { console.log('ошибка', e && e.message); process.exit(1); });
setTimeout(() => { console.log('таймаут'); process.exit(1); }, 600000);
