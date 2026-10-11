// Bedrock-клиент для сверки нашего сервера с BDS (чёрный ящик, только по сети).
// node v-bedrock.js <bds|ours> <name> <script.json> [out.json]
//   bds  -> 127.0.0.1:19160 (прослойка RakNet 10→11 → BDS 19150)
//   ours -> 127.0.0.1:19165
// script.json: {"steps":[{"wait":ms}|{"cmd":"/…"}|{"raw":{name,params}}|{"say":"метка"}], "listen_ms":N,
//               "capture":["level_chunk","container_open",…]}
// Пишет JSON: список пакетов выбранных имён (нормализованных) + разобранные блок-акторы из level_chunk.
const path = require('path');
const fs = require('fs');
const bedrock = require(path.join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const nbt = require(path.join(process.env.HOME, 'node_modules', 'prismarine-nbt'));

const [, , side, name, scriptPath, outPath] = process.argv;
const script = JSON.parse(fs.readFileSync(scriptPath, 'utf8'));
const port = side === 'bds' ? 19160 : 19165;
const capture = new Set(script.capture || ['level_chunk', 'update_block', 'block_entity_data', 'container_open', 'container_close']);
const log = [];
const t0 = Date.now();
const tn = () => Date.now() - t0;

function norm(v) {
  return JSON.parse(JSON.stringify(v, (k, x) => (typeof x === 'bigint' ? x.toString() : (x && x.type === 'Buffer' ? `buf(${x.data.length})` : x))));
}

// payload level_chunk: подчанки (формат 9 с палитрой runtime id), биомы, border, блок-акторы (сетевой NBT).
function parseChunk(p) {
  const buf = p.payload;
  const out = { x: p.x, z: p.z, sub_chunk_count: p.sub_chunk_count, len: buf.length, actors: [] };
  // Блок-акторы идут в конце; ищем их, разбирая NBT с хвоста невозможно — парсим подряд после известных частей.
  // Подчанков/биомов разбираем грубо через пробу: ищем первый корректный сетевой NBT-compound с полем id/x/y/z.
  const actors = [];
  for (let i = 0; i < buf.length - 4; i++) {
    if (buf[i] !== 0x0a || buf[i + 1] !== 0x00) continue;
    try {
      const r = nbt.parseUncompressed(buf.subarray(i), 'littleVarint');
      const s = nbt.simplify(r);
      if (s && typeof s.x === 'number' && typeof s.y === 'number' && typeof s.z === 'number' && s.id) {
        actors.push(norm(s));
        i += nbt.writeUncompressed(r, 'littleVarint').length - 1;
      }
    } catch (e) { /* не NBT */ }
  }
  out.actors = actors;
  return out;
}

const client = bedrock.createClient({ host: '127.0.0.1', port, username: name, offline: true, version: '1.26.10', skipPing: true, raknetBackend: 'jsp-raknet' });
let spawned = false;
let rid = 1n;
client.on('start_game', (p) => { rid = p.runtime_entity_id; log.push({ t: tn(), ev: 'start_game', pos: norm(p.player_position), mode: p.player_gamemode }); });
client.on('kick', (r) => log.push({ t: tn(), ev: 'kick', r: norm(r) }));
client.on('error', (e) => log.push({ t: tn(), ev: 'error', m: String(e.message) }));
client.on('packet', (pk) => {
  const n = pk.data.name;
  if (!capture.has(n) && n !== 'text') return;
  const params = pk.data.params;
  if (n === 'level_chunk') log.push({ t: tn(), pk: n, chunk: parseChunk(params) });
  else log.push({ t: tn(), pk: n, p: norm(params) });
});
client.on('spawn', () => {
  spawned = true;
  client.queue('set_local_player_as_initialized', { runtime_entity_id: rid });
  runSteps();
});

async function runSteps() {
  for (const s of script.steps) {
    if (s.wait) await new Promise((r) => setTimeout(r, s.wait));
    else if (s.cmd) {
      log.push({ t: tn(), ev: 'cmd', c: s.cmd });
      client.write('command_request', { command: s.cmd, origin: { type: 'player', uuid: '00000000-0000-0000-0000-000000000000', request_id: String(Date.now()), player_entity_id: 0 }, internal: false, version: 'latest' });
    } else if (s.raw) {
      client.write(s.raw.name, s.raw.params);
    } else if (s.say) log.push({ t: tn(), ev: 'mark', m: s.say });
  }
  await new Promise((r) => setTimeout(r, script.listen_ms || 3000));
  finish();
}

function finish() {
  fs.writeFileSync(outPath || `${name}.json`, JSON.stringify(log, null, 1));
  console.log('записано', outPath, log.length);
  process.exit(0);
}
setTimeout(() => { log.push({ ev: 'timeout' }); finish(); }, script.timeout_ms || 90000);
