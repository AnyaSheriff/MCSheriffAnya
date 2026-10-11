// Сравнение записей BDS и нашего сервера по случаям: итоговое состояние каждой сущности
// (add_entity + все set_entity_data), свойства, атрибуты, снаряжение, связи.
//   node br-diff.js bds.jsonl ours.jsonl [метка*]
// Печатает только расхождения. Номера, позиции и случайные значения не сравниваются.
const fs = require('fs');
const [bdsFile, oursFile, only] = [process.argv[2], process.argv[3], process.argv[4] === '-' ? '' : process.argv[4]];
const skipLabels = new Set(['join', 'clean', 'kill', 'end']);
const near = p => Math.abs(p.x - 4000.5) < 14 && Math.abs(p.z - 4005.5) < 14;
// Ключи, зависящие от времени/позиции/случая: их различие ничего не значит.
const noise = new Set(['air', 'max_airdata_max_air', 'player_index', 'player_bed_position', 'interactive_tag', 'eating_counter', 'ambient_sound_interval', 'ambient_sound_interval_range', 'ambient_sound_event_name', 'jump_duration', 'target_eid']);
// Имена флагов, которые меняются от обстановки, а не от вида моба.
const noiseFlags = new Set(['onfire', 'breathing', 'moving', 'sprinting', 'sneaking', 'action']);

function names(v) { return Object.keys(v).filter(k => k !== '_value' && v[k] === true).sort(); }
function val(m) {
  if (m.key === 'flags' || m.key === 'flags_extended') return names(m.value).filter(n => !noiseFlags.has(n)).join(',');
  const v = m.value;
  return v && typeof v === 'object' ? JSON.stringify(v) : String(v);
}
function load(file) {
  const out = new Map(); // метка -> [сущности]
  const rows = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map(l => JSON.parse(l));
  const ids = new Map(); // метка/rid -> сущность
  for (const o of rows) {
    if (skipLabels.has(o.label) || !o.n) continue;
    const p = o.p;
    if (!out.has(o.label)) out.set(o.label, []);
    if (o.n === 'add_entity') {
      if (!near(p.position)) continue;
      const e = { type: p.entity_type, meta: new Map(), props: JSON.stringify(p.properties), attrs: new Map(), equip: new Map(), links: [], events: [], rid: String(p.runtime_id) };
      for (const m of p.metadata) e.meta.set(m.key, val(m));
      for (const a of p.attributes || []) e.attrs.set(a.name, (a.current !== undefined ? a.current : a.value) + '/' + a.max);
      for (const l of p.links || []) e.links.push(`${l.type}:${l.immediate}`);
      out.get(o.label).push(e); ids.set(o.label + '/' + e.rid, e);
      continue;
    }
    const e = ids.get(o.label + '/' + String(p.runtime_entity_id !== undefined ? p.runtime_entity_id : p.runtime_id));
    if (o.n === 'set_entity_link') { const arr = out.get(o.label); (arr.linkLog = arr.linkLog || []).push(JSON.stringify([p.link.type, p.link.immediate, p.link.rider_initiated])); continue; }
    if (!e) continue;
    if (o.n === 'set_entity_data') for (const m of p.metadata) e.meta.set(m.key, val(m));
    else if (o.n === 'update_attributes') for (const a of p.attributes) e.attrs.set(a.name, (a.current !== undefined ? a.current : a.value) + '/' + a.max);
    else if (o.n === 'mob_equipment') e.equip.set('hand-' + p.window_id, (p.item && p.item.network_id) || 0);
    else if (o.n === 'mob_armor_equipment') for (const k of ['helmet', 'chestplate', 'leggings', 'boots', 'body']) e.equip.set(k, (p[k] && p[k].network_id) || 0);
    else if (o.n === 'entity_event') e.events.push(p.event_id);
  }
  return out;
}
// Базовая линия: ключи, значения которых клиент берёт из своих определений (корова BDS).
function baseline(map) {
  const cow = (map.get('d-cow') || [])[0];
  return cow ? cow.meta : new Map();
}
const bds = load(bdsFile), ours = load(oursFile), base = baseline(process.argv[5] ? load(process.argv[5]) : bds);
const skipAttr = a => /underwater_movement|lava_movement|absorption|luck/.test(a);

for (const [label, ours_list] of ours) {
  if (only && !(only.endsWith('*') ? label.startsWith(only.slice(0, -1)) : label === only)) continue;
  const bds_list = bds.get(label) || [];
  const out = [];
  const n = Math.max(bds_list.length, ours_list.length);

  if (bds_list.length !== ours_list.length) out.push(`  количество сущностей: BDS ${bds_list.length} (${bds_list.map(e => e.type).join(' ')}) / наш ${ours_list.length} (${ours_list.map(e => e.type).join(' ')})`);
  for (let i = 0; i < Math.min(bds_list.length, ours_list.length); i++) {
    const b = bds_list[i], o = ours_list[i], tag = `#${i + 1} ${b.type}`;
    if (b.type !== o.type) out.push(`  ${tag}: идентификатор нашего ${o.type}`);
    for (const [k, v] of b.meta) {
      if (noise.has(k)) continue;
      const mine = o.meta.get(k);
      if (mine === undefined) { if (k === 'flags' || base.get(k) !== v) out.push(`  ${tag}: нет ${k} (BDS ${v.slice(0, 160)})`); }
      else if (mine !== v) {
        if (k === 'flags' || k === 'flags_extended') {
          const bs = new Set(v.split(',').filter(Boolean)), os = new Set(mine.split(',').filter(Boolean));
          const miss = [...bs].filter(x => !os.has(x)), extra = [...os].filter(x => !bs.has(x));
          if (miss.length || extra.length) out.push(`  ${tag}: ${k} нет [${miss}] лишние [${extra}]`);
        } else out.push(`  ${tag}: ${k} BDS ${v.slice(0, 120)} / наш ${mine.slice(0, 120)}`);
      }
    }
    for (const [k, v] of o.meta) if (!b.meta.has(k) && !noise.has(k) && base.get(k) !== v) out.push(`  ${tag}: лишний ${k} = ${v.slice(0, 120)}`);
    for (const [k, v] of b.attrs) if (!skipAttr(k) && o.attrs.get(k) !== v) out.push(`  ${tag}: атрибут ${k} BDS ${v} / наш ${o.attrs.get(k)}`);
    for (const k of o.attrs.keys()) if (!skipAttr(k) && !b.attrs.has(k)) out.push(`  ${tag}: лишний атрибут ${k}`);
    if (b.props !== o.props) out.push(`  ${tag}: свойства BDS ${b.props} / наш ${o.props}`);
    for (const [k, v] of b.equip) if (v && o.equip.get(k) !== v) out.push(`  ${tag}: снаряжение ${k} BDS ${v} / наш ${o.equip.get(k)}`);
    for (const [k, v] of o.equip) if (v && !b.equip.get(k)) out.push(`  ${tag}: лишнее снаряжение ${k} ${v}`);
    if (b.links.join() !== o.links.join()) out.push(`  ${tag}: связи BDS ${b.links} / наш ${o.links}`);
  }
  const bl = (bds_list.linkLog || []).join(' '), ol = (ours_list.linkLog || []).join(' ');
  if (bl !== ol) out.push('  связи после появления: BDS ' + bl + ' / наш ' + ol);
  console.log(out.length ? `## ${label}\n${out.join('\n')}` : `## ${label}: совпало`);
}
for (const label of bds.keys()) if (!ours.has(label) && (!only || label === only)) console.log(`## ${label}: у нас нет записи`);
