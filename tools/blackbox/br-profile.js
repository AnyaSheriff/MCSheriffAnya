// Из записей BDS (br-capture.js) собирает «профиль» вида — то, что BDS шлёт клиенту
// при появлении взрослого моба и детёныша, — и пишет таблицу Rust:
//   node br-profile.js src/bedrock/bds_profile_table.rs bds1.jsonl [bds2.jsonl ...]
// Взрослый — первая запись вида без флага baby (метки d-*, x-*); детёныш — первая с флагом
// (метки b-*, d-*). Номера ключей и битов флагов — таблицы minecraft-data (types.yml).
const fs = require('fs');
const types = fs.readFileSync('/home/lenox/mine-bot/node_modules/minecraft-data/minecraft-data/data/bedrock/1.26.10/types.yml', 'utf8');
const keyNumber = {};
for (const m of types.slice(types.indexOf('MetadataDictionary:'), types.indexOf('MetadataFlags1:')).matchAll(/^\s+(\d+): (\w+)/gm)) keyNumber[m[2]] = +m[1];
const flagBlock = types.slice(types.indexOf('MetadataFlags1:'));
const flagNames = [...flagBlock.slice(flagBlock.indexOf('"flags": [') + 10, flagBlock.indexOf(']')).matchAll(/"(\w+)"/g)].map(m => m[1]);
const flagBit = n => BigInt(flagNames.indexOf(n));
// Состояния, а не свойства вида: код считает их сам из общего Mob.
const situational = ['baby', 'onfire', 'breathing', 'sitting', 'sheared', 'inlove', 'resting', 'moving', 'sneaking', 'sprinting', 'saddled', 'chested', 'powered', 'angry', 'tempted', 'action', 'can_show_nametag', 'invisible', 'riding', 'stunned', 'eating', 'charged', 'lit', 'wasd_controlled', 'can_power_jump', 'showbase', 'rearing', 'dancing'];
// container_private — постоянный флаг (панда, житель), в список состояний не входит.
let situationalMask = 0n;
for (const n of situational) if (flagNames.includes(n)) situationalMask |= 1n << flagBit(n);

let thingMask = 0n;
for (const n of ['onfire', 'sneaking', 'sprinting', 'moving', 'breathing', 'action', 'ignited', 'invisible', 'riding', 'critical', 'enchanted']) if (flagNames.includes(n)) thingMask |= 1n << flagBit(n);
const near = p => Math.abs(p.x - 4000.5) < 8 && Math.abs(p.z - 4010.5) < 8;
// Ключи, которые код ставит сам или которые случайны у вида.
const skip = new Set(['air', 'max_airdata_max_air', 'player_index', 'player_bed_position', 'interactive_tag', 'eating_counter', 'ambient_sound_interval', 'ambient_sound_interval_range', 'ambient_sound_event_name', 'jump_duration', 'target_eid', 'health', 'nametag', 'owner_eid', 'lead_holder_eid', 'hurt_time', 'hurt_direction', 'scale', 'boundingbox_width', 'boundingbox_height', 'flags', 'flags_extended', 'color', 'color_2', 'strength', 'max_strength', 'variant', 'mark_variant', 'enderman_held_runtime_id', 'shulker_attach_pos', 'hitbox', 'buoyancy_data', 'update_properties', 'wither_invulnerable_ticks', 'spawning_frames', 'heartbeat_interval_ticks', 'visible_mob_effects', 'goat_horn_count', 'pufferfish_size', 'entity_age']);
// Вид, у которого эти «случайные» ключи постоянны (проверено несколькими призывами).
// Постоянные значения «случайных» ключей у отдельных видов (BDS: мухомор -1, лама торговца 1).
const fixedKeys = { 'minecraft:mooshroom': ['mark_variant'], 'minecraft:trader_llama': ['mark_variant'] };
const fixedVariant = new Set(['minecraft:stray', 'minecraft:bogged', 'minecraft:husk', 'minecraft:wither_skeleton', 'minecraft:piglin_brute', 'minecraft:shulker', 'minecraft:zombie_pigman', 'minecraft:drowned']);
const names = v => Object.keys(v).filter(k => k !== '_value' && v[k] === true);
const f32 = x => { if (x === undefined) return 'f32::NAN'; for (let p = 1; p < 10; p++) { const s = Number(x).toPrecision(p); if (Math.fround(parseFloat(s)) === Math.fround(x)) { let t = String(parseFloat(s)); if (!/[.e]/.test(t)) t += '.0'; return t; } } return String(x); };

const adult = {}, baby = {}, things = {};
// Для снарядов и транспорта ключи со временем/позицией пропускаются, остальное (размер, здоровье, контейнеры) берётся.
const skipThing = new Set(['air', 'max_airdata_max_air', 'player_index', 'player_bed_position', 'interactive_tag', 'eating_counter', 'target_eid', 'owner_eid', 'lead_holder_eid', 'hurt_time', 'hurt_direction', 'nametag', 'flags', 'flags_extended', 'scale', 'boundingbox_width', 'boundingbox_height', 'fuse_length', 'minecart_display_block', 'buoyancy_data', 'hitbox', 'update_properties', 'variant', 'potion_aux_value', 'enderman_held_runtime_id']);
// Флаги, которые BDS ставит видам вторым пакетом, а не при появлении.
const lateStatic = ['tamed'];
const ridMap = new Map();
const out = process.argv[2];
// Базовая линия — корова из любой записи, где она есть.
let globalBase = new Map();
for (const file of process.argv.slice(3)) {
  for (const l of fs.readFileSync(file, 'utf8').split('\n').filter(Boolean)) {
    const o = JSON.parse(l);
    if (o.label === 'd-cow' && o.n === 'add_entity' && near(o.p.position)) { globalBase = new Map(o.p.metadata.map(m => [m.key, JSON.stringify(m.value)])); break; }
  }
  if (globalBase.size) break;
}
for (const file of process.argv.slice(3)) {
  const rows = fs.readFileSync(file, 'utf8').split('\n').filter(Boolean).map(l => JSON.parse(l));
  let base = null;
  for (const l of rows) if (l.label === 'd-cow' && l.n === 'add_entity' && near(l.p.position)) { base = new Map(l.p.metadata.map(m => [m.key, JSON.stringify(m.value)])); break; }
  if (!base || !base.size) base = globalBase;
  for (const o of rows) {
    if (o.n === 'set_entity_data') {
      const hit = ridMap.get(o.label + '/' + o.p.runtime_entity_id), fl = o.p.metadata.find(m => m.key === 'flags');
      if (hit && fl) for (const n of lateStatic) if (fl.value[n] === true) hit.flags |= 1n << flagBit(n);
      continue;
    }
    if (o.n !== 'add_entity' || !(near(o.p.position) || (/^(p|v)-/.test(o.label) && Math.abs(o.p.position.x - 4000.5) < 14 && Math.abs(o.p.position.z - 4005.5) < 14))) continue;
    const p = o.p, get = k => (p.metadata.find(m => m.key === k) || {}).value, type = p.entity_type;
    if (/^(p|v)-/.test(o.label)) {
      if (things[type] || adult[type] || /chicken|lightning/.test(type)) continue;
      const ex = [];
      for (const m of p.metadata) {
        if (skipThing.has(m.key) || !(m.key in keyNumber) || !['byte', 'short', 'int', 'float', 'long'].includes(m.type)) continue;
        if (base.get(m.key) === JSON.stringify(m.value) && !['always_show_nametag'].includes(m.key)) continue;
        ex.push([keyNumber[m.key], m.type, m.value]);
      }
      things[type] = { flags: BigInt(get('flags')._value) & ~thingMask, flags2: BigInt(get('flags_extended')._value), scale: 1, width: get('boundingbox_width'), height: get('boundingbox_height'), speed: undefined, extra: ex };
      continue;
    }
    const wanted = o.label.replace(/^[a-z]+-/, '').replace(/-.*/, '');
    const short = type.replace('minecraft:', '');
    if (!(short === wanted || short === ({ zombified_piglin: 'zombie_pigman', evoker: 'evocation_illager', tropical_fish: 'tropicalfish', end_crystal: 'ender_crystal', villager: 'villager_v2', zombie_villager: 'zombie_villager_v2' })[wanted])) continue;
    const isBaby = names(get('flags')).includes('baby');
    const bucket = isBaby ? baby : adult;
    if (bucket[type] || !/^(d|b|x)-/.test(o.label)) continue;
    const extra = [];
    for (const m of p.metadata) {
      if (skip.has(m.key) && !(m.key === 'variant' && fixedVariant.has(type)) && !(fixedKeys[type] || []).includes(m.key)) continue;
      if (base.get(m.key) === JSON.stringify(m.value) || !(m.key in keyNumber)) continue;
      if (!['byte', 'short', 'int', 'float', 'long'].includes(m.type)) continue;
      extra.push([keyNumber[m.key], m.type, m.value]);
    }
    const attr = n => { const a = p.attributes.find(x => x.name === n) || {}; return a.current !== undefined ? a.current : a.value; };
    bucket[type] = {
      flags: BigInt(get('flags')._value) & ~situationalMask,
      flags2: BigInt(get('flags_extended')._value),
      scale: get('scale'), width: get('boundingbox_width'), height: get('boundingbox_height'),
      speed: attr('minecraft:movement'), props: p.properties.ints.map(i => i.value), extra,
    };
    ridMap.set(o.label + '/' + p.runtime_id, bucket[type]);
  }
}

// У видов, которых BDS даёт только детёнышами (головастик), детёныш и есть вид.
for (const t of Object.keys(baby)) if (!adult[t] && t === 'minecraft:tadpole') { adult[t] = baby[t]; delete baby[t]; }
const rustVal = (t, v) => ({ byte: `V::Byte(${v})`, short: `V::Short(${v})`, int: `V::Int(${v})`, long: `V::Long(${v})`, float: `V::Float(${f32(v)})` })[t];
let src = '// Создано tools/blackbox/br-profile.js из записей официального BDS 1.26.13.1 (наблюдение по сети,\n// tools/blackbox/br-cases.js). Не править руками: перезапись — запуском генератора.\n';
src += 'pub static PROFILES: &[Profile] = &[\n';
for (const type of Object.keys(adult).sort()) {
  const a = adult[type], b = baby[type];
  const babyText = b ? `Some(Baby { width: ${f32(b.width)}, height: ${f32(b.height)}, scale: ${f32(b.scale)}, flags: 0x${b.flags.toString(16)} })` : 'None';
  src += `    Profile { id: "${type}", flags: 0x${a.flags.toString(16)}, flags2: 0x${a.flags2.toString(16)}, width: ${f32(a.width)}, height: ${f32(a.height)}, scale: ${f32(a.scale)}, speed: ${f32(a.speed)}, extra: &[${a.extra.map(e => `(${e[0]}, ${rustVal(e[1], e[2])})`).join(', ')}], baby: ${babyText} },\n`;
}
src += '];\n';
src += 'pub static THINGS: &[Profile] = &[\n';
for (const type of Object.keys(things).sort()) {
  const a = things[type];
  src += `    Profile { id: "${type}", flags: 0x${a.flags.toString(16)}, flags2: 0x${a.flags2.toString(16)}, width: ${f32(a.width)}, height: ${f32(a.height)}, scale: 1.0, speed: f32::NAN, extra: &[${a.extra.map(e => `(${e[0]}, ${rustVal(e[1], e[2])})`).join(', ')}], baby: None },\n`;
}
src += '];\n';
fs.writeFileSync(out, src);
console.log('видов:', Object.keys(adult).length, 'детёнышей:', Object.keys(baby).length, 'без взрослого:', Object.keys(baby).filter(t => !adult[t]).join(' '));
