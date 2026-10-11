// node v-diff.js bds.json ours.json — построчное сравнение блок-акторов (без isMovable/ключей-шумов)
const [, , fa, fb] = process.argv;
const a = require(require('path').resolve(fa)), b = require(require('path').resolve(fb));
const NOISE = new Set(process.env.V_NOISE ? process.env.V_NOISE.split(',') : []);
function flat(o, p = '', out = {}) {
  if (Array.isArray(o)) { if (!o.length) out[p] = '[]'; o.forEach((v, i) => flat(v, `${p}[${i}]`, out)); }
  else if (o && typeof o === 'object') { const k = Object.keys(o); if (!k.length) out[p] = '{}'; k.forEach((x) => flat(o[x], p ? `${p}.${x}` : x, out)); }
  else out[p] = JSON.stringify(o);
  return out;
}
let same = 0;
for (let i = 0; i < a.rows.length; i++) {
  const ra = a.rows[i], rb = b.rows[i];
  const fa_ = flat(ra.actors), fb_ = flat(rb.actors);
  const keys = new Set([...Object.keys(fa_), ...Object.keys(fb_)]);
  const d = [];
  for (const k of keys) { if ([...NOISE].some((n) => k.endsWith(n))) continue; if (fa_[k] !== fb_[k]) d.push(`   ${k}: bds=${fa_[k]} ours=${fb_[k]}`); }
  if (ra.block !== rb.block) d.unshift(`   BLOCK: bds=${ra.block} ours=${rb.block}`);
  if (!d.length) same++; else console.log(`#${i} ${ra.java} @${ra.rel}\n${d.join('\n')}`);
}
console.log('совпало', same, 'из', a.rows.length);
console.log('лишние bds:', JSON.stringify(a.extra), 'ours:', JSON.stringify(b.extra));
