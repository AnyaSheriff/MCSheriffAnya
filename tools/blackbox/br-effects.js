// Сводка по эффектам: на каждую метку — id эффекта и поля mob_effect (add/update/remove), метаданные и атрибуты.
// node br-effects.js  → таблица bds vs ours
const fs = require('fs'), path = require('path');
const root = path.join(__dirname, '..', '..', 'tmp', 'br');
function load(t) {
  const res = {}; let tag = null;
  for (const l of fs.readFileSync(path.join(root, t, 'effects.jsonl'), 'utf8').split('\n')) {
    if (!l) continue; const j = JSON.parse(l);
    if (j.step && j.step.startsWith('TAG ')) { tag = j.step.slice(4); res[tag] = []; continue; }
    if (!tag || j.step) continue;
    if (j.n === 'mob_effect') res[tag].push(`${j.p.event_id[0]}${j.p.effect_id}/a${j.p.amplifier}/d${j.p.duration}/${j.p.particles ? 'P' : 'p'}${j.p.ambient ? 'A' : ''}`);
    else if (j.n === 'set_entity_data') res[tag].push('md:' + j.p.metadata.map(m => m.key + '=' + (typeof m.value === 'object' ? JSON.stringify(m.value).slice(0, 40) : m.value)).join(','));
    else if (j.n === 'update_attributes') { const s = j.p.attributes.map(a => a.name.replace('minecraft:', '') + '=' + Math.round(a.current * 100) / 100).join(','); if (!res[tag].includes('at:' + s)) res[tag].push('at:' + s.slice(0, 90)); }
    else if (['level_event', 'level_sound_event', 'entity_event', 'spawn_particle_effect'].includes(j.n)) res[tag].push(j.n + ':' + (j.p.event_id || j.p.sound_id || j.p.particle_name || ''));
  }
  return res;
}
const a = load('bds'), b = load('ours');
// без аргументов — только различия (флаги md:flags сравниваются отдельно, br-diff-state.js); имена через запятую — полная запись
const which = process.argv[2];
const uniq = l => (l || []).filter((v, i, arr) => arr.indexOf(v) === i);
for (const k of Object.keys(a)) {
  if (which && !which.split(',').includes(k)) continue;
  const x = uniq(a[k]).filter(v => which || !v.startsWith('md:flags')).join(' | '), y = uniq(b[k]).filter(v => which || !v.startsWith('md:flags')).join(' | ');
  if (which || x !== y) console.log(k + '\n  bds : ' + x.slice(0, 500) + '\n  ours: ' + y.slice(0, 500));
}
