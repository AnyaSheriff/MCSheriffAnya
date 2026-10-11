// Сверка блок-акторов в LevelChunk: BDS против нашего сервера.
// node v-blocks.js <bds|ours> <out.json>
// Клиент A (Bedrock) даёт позицию (и на BDS ставит блоки командами; на нашем сервере Bedrock-игрок без
// Xbox не оператор — там блоки ставит Java-бот blackbox). Клиент B заходит заново и собирает level_chunk.
const path = require('path');
const fs = require('fs');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const nbt = require(path.join(process.env.HOME, 'node_modules', 'prismarine-nbt'));
const side = process.argv[2];
const out = process.argv[3];
const PORT = side === 'bds' ? 19160 : 19165;
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

// [имя на BDS (с состояниями), имя в Java (с состояниями)]
const PAD = JSON.parse(fs.readFileSync(process.argv[4] ? path.resolve(process.argv[4]) : path.join(__dirname, 'v-pad.json'), 'utf8'));

function join(name) {
  return new Promise((resolve) => {
    const c = bedrock.createClient({ host: '127.0.0.1', port: PORT, username: name, offline: true, version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet' });
    const st = { c, pos: null, chunks: [], subs: [], pk: [] };
    let rid = 1n;
    c.on('start_game', (p) => { st.pos = p.player_position; rid = p.runtime_entity_id; });
    c.on('error', (e) => console.log('err', e.message));
    c.on('kick', (r) => console.log('kick', JSON.stringify(r)));
    c.on('packet', (pk) => {
      const n = pk.data.name;
      if (n === 'level_chunk') st.chunks.push(pk.data.params);
      else if (n === 'subchunk') st.subs.push(pk.data.params);
      else if (n === 'block_entity_data' || n === 'update_block') st.pk.push(pk.data);
    });
    c.on('spawn', () => { c.queue('set_local_player_as_initialized', { runtime_entity_id: rid }); resolve(st); });
  });
}
function cmd(c, s) {
  c.write('command_request', { command: s, origin: { type: 'player', uuid: '00000000-0000-0000-0000-000000000000', request_id: String(Math.random()), player_entity_id: 0 }, internal: false, version: 'latest' });
}
function types(v, depth = 0) {
  const o = {};
  for (const k of Object.keys(v)) {
    if (k === 'x' || k === 'y' || k === 'z') continue;
    const t = v[k].type; o[k] = t;
    if (t === 'compound' && depth < 2) o[k] = types(v[k].value, depth + 1);
  }
  return o;
}
function actorsOf(p) {
  const buf = p.payload; const res = [];
  for (let i = 0; i < buf.length - 4; i++) {
    if (buf[i] !== 0x0a || buf[i + 1] !== 0x00) continue;
    try {
      const r = nbt.parseUncompressed(buf.subarray(i), 'littleVarint');
      const s = nbt.simplify(r);
      if (s && typeof s.x === 'number' && typeof s.z === 'number' && s.id) { s._t = types(r.value); res.push(s); }
    } catch (e) { /* не NBT */ }
  }
  return res;
}
const jsonSafe = (v) => JSON.parse(JSON.stringify(v, (k, x) => (typeof x === 'bigint' ? Number(x) : x)));

(async () => {
  const A = await join(side === 'bds' ? 'padA' : 'padA');
  await sleep(2500);
  const cx = 0, cz = 0, py = -60; // плоский мир: трава на y=-61, площадка на y=-60 (общая для BDS и нашего)
  const place = [];
  PAD.forEach((e, i) => {
    const x = cx + 1 + (i % 14), z = cz + 2 + 2 * Math.floor(i / 14);
    place.push({ x, y: py, z, bds: e[0], java: e[1], i });
  });
  console.log('pad origin', cx, py, cz, 'blocks', place.length);
  if (side === 'bds') {
    cmd(A.c, `/fill 0 ${py} 0 15 ${py + 1} 15 air`);
    await sleep(500);
    for (const p of place) { cmd(A.c, `/setblock ${p.x} ${p.y} ${p.z} ${p.bds}`); await sleep(120); }
    await sleep(1500);
  } else {
    const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
    const bot = mineflayer.createBot({ host: '127.0.0.1', port: 25964, username: 'blackbox', version: '26.1', auth: 'offline' });
    await new Promise((r) => bot.once('spawn', r));
    await sleep(1500);
    bot.chat(`/fill 0 ${py - 1} 0 15 ${py - 1} 15 grass_block`);
    await sleep(500);
    // Стенки от затекающей воды нашего «плоского» мира (там вода на уровне площадки).
    for (const w of [[-1, -1, 16, -1], [-1, 16, 16, 16], [-1, -1, -1, 16], [16, -1, 16, 16]]) { bot.chat(`/fill ${w[0]} ${py} ${w[1]} ${w[2]} ${py + 2} ${w[3]} stone`); await sleep(300); }
    bot.chat(`/fill 0 ${py} 0 15 ${py + 1} 15 air`);
    await sleep(500);
    for (const p of place) { bot.chat(`/setblock ${p.x} ${p.y} ${p.z} ${p.java}`); await sleep(150); }
    await sleep(2000);
    bot.end();
  }
  A.c.close();
  await sleep(8000);
  const B = await join('padB');
  await sleep(6000);
  if (B.chunks.some((c) => c.sub_chunk_count < 0)) {
    // BDS отдаёт чанки в режиме опроса подчанков: блок-акторы приходят в пакете subchunk.
    B.c.write('subchunk_request', { dimension: 0, origin: { x: 0, y: -4, z: 0 }, requests: [{ dx: 0, dy: 0, dz: 0 }, { dx: 0, dy: 1, dz: 0 }] });
    await sleep(3000);
  }
  const acts = [];
  for (const ch of B.chunks) if (ch.sub_chunk_count >= 0) for (const a of actorsOf(ch)) acts.push(a);
  for (const s of B.subs) for (const e of s.entries) if (e.payload) for (const a of actorsOf({ payload: e.payload })) acts.push(a);
  // Сами блоки: у BDS номера — хэши состояний, у нас — индексы block_nbt.bin; сводим к описанию.
  const S = require('./v-states.js'); const table = S.load(); const byHash = new Map(table.map((t) => [t.hash, t.desc]));
  let sub = null;
  try {
    if (side === 'bds') sub = S.subchunk(B.subs[0].entries.find((e) => e.dy === 0 || e.dy === undefined).payload);
    else sub = S.subchunk(B.chunks.find((c) => c.x === 0 && c.z === 0).payload);
  } catch (e) { console.log('подчанк не разобран', e.message); }
  const blockAt = (x, y, z) => { if (!sub) return '?'; const id = sub.ids[(x & 15) * 256 + (z & 15) * 16 + (y & 15)]; return side === 'bds' ? (byHash.get(id) || `hash:${id}`) : (table[id] ? table[id].desc : `rid:${id}`); };
  const rows = place.map((p) => {
    const a = acts.filter((x) => x.x === p.x && x.y === p.y && x.z === p.z);
    return { i: p.i, block: blockAt(p.x, p.y, p.z), java: p.java, bds: p.bds, rel: [p.x - cx, p.y - py, p.z - cz], actors: jsonSafe(a.map((x) => { const y = { ...x }; delete y.x; delete y.y; delete y.z; return y; })) };
  });
  const extra = acts.filter((a) => !place.some((p) => p.x === a.x && p.y === a.y && p.z === a.z)).map((a) => ({ id: a.id, pos: [a.x - cx, a.y - py, a.z - cz] }));
  const c00 = B.chunks.filter((c) => c.x === 0 && c.z === 0).map((c) => ({ n: c.sub_chunk_count, len: c.payload.length, cache: c.cache_enabled, tail: c.payload.subarray(Math.max(0, c.payload.length - 64)).toString('hex') }));
  fs.writeFileSync(out, JSON.stringify({ origin: [cx, py, cz], c00, chunks: B.chunks.length, rows, extra }, null, 1));
  console.log('готово', out, 'чанков', B.chunks.length, 'акторов', acts.length);
  process.exit(0);
})();
setTimeout(() => { console.log('таймаут'); process.exit(1); }, 150000);
