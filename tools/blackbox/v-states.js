// Таблица состояний блоков Bedrock из src/bedrock/block_nbt.bin (номер → {name, states}) и хэши BDS
// (FNV-1a 32 по NBT {name, states} в little-endian — сверено по воздуху: -604749536) + разбор подчанка.
const fs = require('fs');
const path = require('path');
const nbt = require(path.join(process.env.HOME, 'node_modules', 'prismarine-nbt'));

function fnv1a(buf) { let h = 0x811c9dc5 >>> 0; for (const b of buf) { h = (h ^ b) >>> 0; h = Math.imul(h, 0x01000193) >>> 0; } return h | 0; }

function load() {
  const B = fs.readFileSync(path.join(__dirname, '..', '..', 'src', 'bedrock', 'block_nbt.bin'));
  let at = 0;
  const strings = () => { const n = B.readUInt16LE(at); at += 2; const l = []; for (let i = 0; i < n; i++) { const len = B[at]; l.push(B.toString('utf8', at + 1, at + 1 + len)); at += 1 + len; } return l; };
  const names = strings(), keys = strings(), texts = strings();
  const count = B.readUInt32LE(at); at += 4;
  const states = [];
  for (let i = 0; i < count; i++) {
    const name = `minecraft:${names[B.readUInt16LE(at)]}`;
    const n = B[at + 2];
    const st = {};
    for (let p = 0; p < n; p++) {
      const o = at + 3 + p * 3; const key = keys[B[o]]; const v = B[o + 2];
      st[key] = B[o + 1] === 1 ? nbt.byte(v) : B[o + 1] === 3 ? nbt.int(v) : nbt.string(texts[v]);
    }
    at += 3 + 3 * n;
    const tag = nbt.comp({ name: nbt.string(name), states: nbt.comp(st) }, '');
    const desc = `${name}[${Object.entries(st).map(([k, v]) => `${k}=${v.value}`).join(',')}]`;
    states.push({ hash: fnv1a(nbt.writeUncompressed(tag, 'little')), desc });
  }
  return states;
}

// Подчанк (версия 8/9): возвращает {y, ids: Int32Array(4096) по индексу x*256+z*16+y} для слоя 0.
function readVarint(buf, st) { let r = 0, s = 0, b; do { b = buf[st.at++]; r |= (b & 0x7f) << s; s += 7; } while (b & 0x80); return r >>> 0; }
function subchunk(buf) {
  const st = { at: 0 };
  const version = buf[st.at++];
  const layers = buf[st.at++];
  let y = null;
  if (version === 9) y = (buf[st.at++] << 24) >> 24;
  let ids = null;
  for (let l = 0; l < layers; l++) {
    const flags = buf[st.at++]; const bits = flags >> 1;
    const per = Math.floor(32 / bits), words = Math.ceil(4096 / per);
    const start = st.at; st.at += words * 4;
    const pn = (v) => (v >>> 1) ^ -(v & 1);
    const psize = pn(readVarint(buf, st));
    const palette = []; for (let i = 0; i < psize; i++) palette.push(pn(readVarint(buf, st)));
    if (l === 0) {
      ids = new Int32Array(4096);
      for (let i = 0; i < 4096; i++) { const w = buf.readUInt32LE(start + 4 * Math.floor(i / per)); ids[i] = palette[(w >>> ((i % per) * bits)) & ((1 << bits) - 1)]; }
    }
  }
  return { y, ids, end: st.at };
}
module.exports = { load, subchunk, fnv1a };
