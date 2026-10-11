// Нормализует записи br-record.js (убирает id, такты, координаты, округляет числа) и сравнивает bds с ours.
// node br-diff-state.js <сценарий> [--full]  → пишет tmp/br/norm-<цель>/<сценарий>.txt и печатает diff.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const root = path.join(__dirname, '..', '..', 'tmp', 'br');
const name = process.argv[2], full = process.argv.includes('--full');
const DROP = new Set(['tick', 'runtime_entity_id', 'runtime_id', 'unique_id', 'entity_unique_id', 'player_unique_id', 'position', 'pitch', 'yaw', 'head_yaw', 'body_yaw', 'seed', 'time', 'origin', 'xuid', 'platform_chat_id']);
function norm(v, k) {
  if (Array.isArray(v)) return v.map(x => norm(x));
  if (v && typeof v === 'object') {
    // битовые флаги метаданных: оставляем только поднятые
    if ('_value' in v) return 'flags:' + Object.keys(v).filter(x => v[x] === true).sort().join(',');
    const o = {};
    for (const key of Object.keys(v).sort()) { if (DROP.has(key)) continue; o[key] = norm(v[key], key); }
    return o;
  }
  if (typeof v === 'number' && !Number.isInteger(v)) return Math.round(v * 1000) / 1000;
  return v;
}
function load(t) {
  const f = path.join(root, t, name + '.jsonl'); if (!fs.existsSync(f)) return null;
  const out = []; const SK = new Set(['inventory_content', 'inventory_slot', 'unlocked_recipes', 'player_hotbar', 'text', 'add_player', 'command_output']);
  for (const l of fs.readFileSync(f, 'utf8').split('\n')) {
    if (!l) continue; const j = JSON.parse(l);
    if (j.step) out.push('## ' + j.step);
    else if (!SK.has(j.n)) {
      let s = JSON.stringify(norm(j.p));
      if (j.n === 'update_attributes' && !full) s = j.p.attributes.map(a => `${a.name.replace('minecraft:', '')}=${Math.round(a.current * 1000) / 1000}[${a.min},${a.max}]${a.modifiers.length ? '+' + a.modifiers.map(m => m.name + ':' + m.amount + '/' + m.operation).join(';') : ''}`).sort().join(' ');
      // метаданные: набор ключей/значений без шума флагов
      out.push(j.n + ' ' + (full ? s : s.slice(0, 260)));
    }
  }
  // подряд идущие одинаковые строки — одной строкой с числом повторов
  const res = [];
  for (const l of out) { const p = res[res.length - 1]; if (p && (p.l === l)) p.n++; else res.push({ l, n: 1 }); }
  return res.map(r => r.n > 1 ? `${r.l}  ×${r.n}` : r.l);
}
const a = load('bds'), b = load('ours');
fs.mkdirSync(path.join(root, 'norm'), { recursive: true });
if (a) fs.writeFileSync(path.join(root, 'norm', name + '.bds.txt'), a.join('\n') + '\n');
if (b) fs.writeFileSync(path.join(root, 'norm', name + '.ours.txt'), b.join('\n') + '\n');
if (a && b) { try { cp.execFileSync('diff', ['-u', '--label', 'bds', '--label', 'ours', path.join(root, 'norm', name + '.bds.txt'), path.join(root, 'norm', name + '.ours.txt')], { stdio: 'inherit' }); console.log('IDENTICAL', name); } catch (e) { /* diff вернул 1 */ } }
