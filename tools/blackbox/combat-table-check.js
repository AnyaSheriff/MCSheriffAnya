// Мини-«чёрный ящик» для src/combat_table.rs: сверяет числа таблицы с официальным
// сервером 26.1.2, НАБЛЮДАЯ его по сети (генератор отчётов и jar не трогаем).
// Библиотека mineflayer (MIT) используется как есть, её код в проект не попадает.
//
// Использование (сервер оригинала на 127.0.0.1:25566, офлайн, робот blackbox5 — оператор):
//   node combat-table-check.js [--phase attrs,damage,dura,shield] [--port 25566] [--only имя,имя]
//   фазы:
//     attrs  — модификаторы атрибутов: предмет в руку/на броню командой `item replace`,
//              читаем пакеты «обновление атрибутов» (attack_speed, armor, ...);
//     damage — урон оружия: NoAI-свинья с 200 здоровья, удар при полном заряде,
//              урон = падение здоровья в метаданных сущности;
//     dura   — прочность: сначала меряем расход за одно действие (удар, /damage, выстрел),
//              потом предмет с damage=max-расход обязан сломаться, а с max-расход-1 — нет;
//     shield — время отключения щита топором: второй робот (blackbox4) бьёт топором
//              по blackbox5 с поднятым щитом, читаем пакет «перезарядка» (set_cooldown).
//   Работает около x=8000, z=8000 (каменная площадка). Перед стартом берёт замок
//   ~/.claude/jobs/54d01b99/tmp/bb.lock (создание с O_EXCL, ждёт, опрос раз в 20 с),
//   на выходе снимает. Сырой журнал и итоговая таблица — в ~/.claude/jobs/54d01b99/tmp/combat-check/.
'use strict';
const path = require('path');
const fs = require('fs');
const HOME_MODULES = path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(HOME_MODULES, 'mineflayer'));

const args = process.argv.slice(2);
const opt = (name, def) => { const i = args.indexOf(`--${name}`); return i >= 0 ? args[i + 1] : def; };
const PORT = Number(opt('port', 25566));
const PHASES = opt('phase', 'attrs,damage,dura,shield').split(',');
const ONLY = opt('only', '') ? opt('only', '').split(',') : null;
const OUT = path.join(process.env.HOME, '.claude/jobs/54d01b99/tmp/combat-check');
const LOCK = path.join(process.env.HOME, '.claude/jobs/54d01b99/tmp/bb.lock');
const X = 8000, Z = 8000, PY = -59;                 // куда встаёт робот
fs.mkdirSync(OUT, { recursive: true });
const rawLog = fs.createWriteStream(path.join(OUT, 'raw.log'), { flags: 'a' });
const log = (...a) => { const s = `${new Date().toISOString()} ${a.join(' ')}`; console.log(s); rawLog.write(s + '\n'); };
const sleep = (ms) => new Promise(r => setTimeout(r, ms));

// ---- таблица из combat_table.rs (только разбор текста, не сборка) ----
const src = fs.readFileSync(path.join(__dirname, '../../src/combat_table.rs'), 'utf8');
const ITEMS = [];
for (const line of src.split('\n')) {
  const m = line.match(/name: "(\w+)", item: (\d+), max_damage: (\d+), enchantable: \d+, mods: &\[(.*?)\], damage_per_attack: (\d+), disable_blocking_seconds: ([\d.]+), equip_slot: (\d+)/);
  if (!m) continue;
  const mods = [...m[4].matchAll(/attribute: (\d+), operation: (\d+), slot: (\d+), amount: ([-\d.e]+), id: "([^"]+)"/g)]
    .map(x => ({ attribute: +x[1], operation: +x[2], slot: +x[3], amount: +x[4], id: x[5] }));
  ITEMS.push({ name: m[1], item: +m[2], max: +m[3], mods, dpa: +m[5], disable: +m[6], equip: +m[7] });
}
const ATTR_NAME = { 0: 'armor', 1: 'armor_toughness', 2: 'attack_damage', 3: 'attack_knockback', 4: 'attack_speed', 16: 'knockback_resistance' };
const OP_NAME = { 0: 'add_value', 1: 'add_multiplied_base', 2: 'add_multiplied_total' };
const pick = (f) => ITEMS.filter(i => (!ONLY || ONLY.includes(i.name)) && f(i));

const results = [];      // {item, field, table, observed, ok, note}
const add = (item, field, table, observed, ok, note = '') => { results.push({ item, field, table, observed, ok, note }); log(`RESULT ${item} | ${field} | table=${table} | observed=${observed} | ${ok === null ? 'N/A' : ok ? 'OK' : 'MISMATCH'} ${note}`); };

// ---- замок ----
let haveLock = false;
const unlock = () => { if (haveLock) { try { fs.unlinkSync(LOCK); } catch (e) {} haveLock = false; } };
async function lock() {
  for (;;) {
    try { const fd = fs.openSync(LOCK, 'wx'); fs.writeSync(fd, String(process.pid)); fs.closeSync(fd); haveLock = true; return; }
    catch (e) { if (e.code !== 'EEXIST') throw e; log('замок занят, жду 20 с'); await sleep(20000); }
  }
}
process.on('exit', unlock);
process.on('SIGINT', () => { unlock(); process.exit(130); });
process.on('SIGTERM', () => { unlock(); process.exit(143); });

// ---- робот ----
const mk = (name) => mineflayer.createBot({ host: '127.0.0.1', port: PORT, username: name, version: '26.1', auth: 'offline' });
let bot, bot2 = null;
const chats = [];
async function cmd(c, wait = 150) { log('CMD', c); bot.chat('/' + c); await sleep(wait); }

// Слоты окна игрока: 5..8 — голова..ботинки, 36+n — хотбар, 45 — вторая рука.
const slots = {};
function hookBot(b, tag) {
  b._client.on('set_slot', (p) => { if (p.windowId === 0 && b === bot) { slots[p.slot] = p.item; } });
  b._client.on('window_items', (p) => { if (p.windowId === 0 && b === bot) (p.items || []).forEach((it, i) => { slots[i] = it; }); });
  b.on('messagestr', (s) => { chats.push(s); rawLog.write(`${tag} chat: ${s}\n`); });
}
const dmgOf = (it) => {
  if (!it || !it.itemCount) return null;                              // пусто
  const c = (it.components || []).find(x => x.type === 'damage');
  return c ? (typeof c.data === 'number' ? c.data : (c.data && c.data.value)) : 0;
};
const itemId = (it) => (it && it.itemCount ? it.itemId : null);

// пакеты атрибутов для наблюдаемой сущности
const attrLog = [];   // {t, id, key, value, mods}
function hookAttrs(b) {
  b._client.on('entity_update_attributes', (p) => {
    for (const pr of p.properties) attrLog.push({ t: Date.now(), id: p.entityId, key: String(pr.key), value: pr.value, mods: pr.modifiers || [] });
    rawLog.write(`ATTR ${p.entityId} ${JSON.stringify(p.properties)}\n`);
  });
}
const normKey = (k) => k.replace('minecraft:', '').replace('generic.', '');
const normOp = (o) => (typeof o === 'number' ? OP_NAME[o] : String(o)).replace('minecraft:', '');
const modId = (m) => String(m.uuid || m.id || m.name || '');

// ---- подготовка ----
// У робота от прежних сценариев может остаться нестандартная основа attack_damage
// (у blackbox5 было 1000): запоминаем, ставим ванильную 1 на время прогона, в конце возвращаем.
const savedBase = {};
async function baseAttack(name, botObj) {
  const n = chats.length;
  await cmd(`attribute ${name} minecraft:attack_damage base get`, 400);
  const m = chats.slice(n).map(x => x.match(/is ([-\d.e]+)$/)).find(Boolean);
  savedBase[name] = m ? Number(m[1]) : 1;
  log('основа attack_damage', name, savedBase[name]);
  await cmd(`attribute ${name} minecraft:attack_damage base set 1`, 200);
}
async function prepare() {
  await baseAttack('blackbox5');
  await cmd('gamemode survival blackbox5');
  await cmd(`tp blackbox5 ${X + 0.5} ${PY + 3} ${Z + 0.5} 0 0`, 2500);
  await cmd(`fill ${X - 8} ${PY - 1} ${Z - 8} ${X + 8} ${PY - 1} ${Z + 8} stone`, 400);
  await cmd(`fill ${X - 8} ${PY} ${Z - 8} ${X + 8} ${PY + 8} ${Z + 8} air`, 400);
  await cmd(`tp blackbox5 ${X + 0.5} ${PY} ${Z + 0.5} 0 0`, 800);
  await cmd('clear blackbox5');
  await cmd('effect clear blackbox5');
  bot.setQuickBarSlot(0);
  log('на земле:', bot.entity.onGround, 'поз', JSON.stringify(bot.entity.position));
}
async function cleanup() {
  try {
    for (const [n, v] of Object.entries(savedBase)) await cmd(`attribute ${n} minecraft:attack_damage base set ${v}`, 200);
    if (savedDifficulty === 'peaceful') await cmd('difficulty peaceful', 300);
    await cmd('kill @e[tag=cc]', 300);
    await cmd('kill @e[type=arrow,x=' + X + ',y=' + PY + ',z=' + Z + ',distance=..100]', 200);
    await cmd('clear blackbox5');
    await cmd('gamemode creative blackbox5');
    if (bot2) { await cmd('clear blackbox4'); await cmd('gamemode creative blackbox4'); }
    await cmd(`fill ${X - 8} ${PY - 1} ${Z - 8} ${X + 8} ${PY - 1} ${Z + 8} stone`, 200);
  } catch (e) { log('cleanup error', e.message); }
}

// ---- фаза 1: атрибуты ----
async function phaseAttrs() {
  const list = pick(i => i.mods.length > 0);
  for (const it of list) {
    const bodySlot = it.mods.some(m => m.slot === 9);
    const slotName = { 1: 'weapon.mainhand', 4: 'armor.feet', 5: 'armor.legs', 6: 'armor.chest', 7: 'armor.head', 9: 'armor.body' }[it.mods[0].slot];
    if (bodySlot) { await attrsBody(it); continue; }
    await cmd(`item replace entity blackbox5 ${slotName} with minecraft:air`, 400);
    const t0 = Date.now();
    await cmd(`item replace entity blackbox5 ${slotName} with minecraft:${it.name}`, 500);
    const seen = attrLog.filter(a => a.t >= t0 && a.id === bot.entity.id);
    compareMods(it, seen);
    await cmd(`item replace entity blackbox5 ${slotName} with minecraft:air`, 150);
  }
}
function compareMods(it, seen) {
  for (const m of it.mods) {
    const an = ATTR_NAME[m.attribute] || `#${m.attribute}`;
    const rec = seen.filter(a => normKey(a.key) === an || a.key === String(m.attribute)).pop();
    if (!rec) { add(it.name, `${an} modifier`, `${m.amount}`, 'нет пакета', null, m.id === 'minecraft:base_attack_damage' ? 'attack_damage не синхронизируется с клиентом (см. фазу damage)' : 'пакет не пришёл'); continue; }
    const found = rec.mods.find(x => modId(x).replace('minecraft:', '') === m.id.replace('minecraft:', ''));
    if (!found) { add(it.name, `${an} modifier id`, m.id, `нет; есть ${JSON.stringify(rec.mods.map(modId))}`, false); continue; }
    const okAmt = Math.abs(found.amount - m.amount) < 1e-6;
    const okOp = normOp(found.operation) === OP_NAME[m.operation];
    add(it.name, `${an} ${m.id.replace('minecraft:', '')} amount/op`, `${m.amount}/${OP_NAME[m.operation]}`, `${found.amount}/${normOp(found.operation)}`, okAmt && okOp);
  }
}
// броня «тело» — на лошади/волке/наутилусе (у игрока такого слота нет)
async function attrsBody(it) {
  const ent = it.name === 'wolf_armor' ? 'wolf' : it.name.endsWith('_nautilus_armor') ? 'nautilus' : 'horse';
  await cmd('kill @e[tag=cc]', 200);
  const before = new Set(Object.keys(bot.entities));
  await cmd(`summon minecraft:${ent} ${X + 3.5} ${PY} ${Z + 0.5} {NoAI:1b,Silent:1b,Tags:["cc"]${ent === "nautilus" ? ",Tame:1b" : ""}}`, 900);
  const id = Object.keys(bot.entities).map(Number).find(k => !before.has(String(k)) && bot.entities[k].name === ent);
  if (id === undefined) { add(it.name, 'body armor modifiers', '-', `сущность ${ent} не появилась`, null, 'не наблюдаемо'); return; }
  const t0 = Date.now();
  await cmd(`item replace entity @e[tag=cc,limit=1] armor.body with minecraft:${it.name}`, 700);
  compareMods(it, attrLog.filter(a => a.t >= t0 && a.id === id));
  await cmd('kill @e[tag=cc]', 200);
}

// ---- свинья-мишень ----
let pigId = null, pigHealth = null, healthKey = null;
async function spawnPig(dz = 1.6) {
  await cmd('kill @e[tag=cc]', 200);
  const before = new Set(Object.keys(bot.entities));
  await cmd(`summon minecraft:pig ${X + 0.5} ${PY} ${Z + 0.5 + dz} {NoAI:1b,Silent:1b,Tags:["cc"],Rotation:[180f,0f]}`, 900);
  pigId = Object.keys(bot.entities).map(Number).find(k => !before.has(String(k)) && bot.entities[k].name === 'pig');
  if (pigId === undefined) throw new Error('свинья не появилась');
  await cmd('attribute @e[tag=cc,limit=1] minecraft:max_health base set 200', 200);
  await cmd('data merge entity @e[tag=cc,limit=1] {Health:200f}', 500);
  log('pig id', pigId, 'health', pigHealth, 'ключ', healthKey);
}
function hookMeta(b) {
  b._client.on('entity_metadata', (p) => {
    if (p.entityId !== pigId) return;
    rawLog.write(`META ${p.entityId} ${JSON.stringify(p.metadata)}\n`);
    for (const m of p.metadata) {
      if (m.type === 'float' || typeof m.value === 'number' && !Number.isInteger(m.value)) {
        if (healthKey === null && m.value === 200) healthKey = m.key;
        if (m.key === healthKey) pigHealth = m.value;
      }
    }
  });
}
const heal = async () => { await cmd('data merge entity @e[tag=cc,limit=1] {Health:200f}', 400); };
async function lookAtPig() {
  await cmd(`tp blackbox5 ${X + 0.5} ${PY} ${Z + 0.5} 0 0`, 300);   // yaw 0 = на юг (+z), свинья там
}
// Здоровье мишени по команде `data get` (метаданные показали, что настоящее здоровье
// после лечения не 1024, поэтому меряем и до, и после удара).
async function getHealth() {
  const n = chats.length;
  await cmd('data get entity @e[tag=cc,limit=1] Health', 350);
  const m = chats.slice(n).map(x => x.match(/entity data: ([-\d.e]+)f?$/)).find(Boolean);
  return m ? Number(m[1]) : null;
}
// копье бьёт не пакетом use_entity, а действием «укол» (статус 7 в Player Action) с прицелом на мишень
// (мишень мелкая: свинья масштаба 0.25, без прицела укол проходит мимо)
let stabMode = false;
async function hit(it) {
  const e = bot.entities[pigId];
  if (it && it.name.endsWith('_spear')) { await bot.lookAt(e.position.offset(0, 0.1, 0), true); await sleep(250); bot._client.write('block_dig', { status: 7, location: { x: 0, y: 0, z: 0 }, face: 0, sequence: 0 }); bot.swingArm(); }
  else bot.attack(e);
}

// ---- фаза 2: урон ----
async function phaseDamage() {
  await spawnPig();
  const list = pick(i => i.mods.some(m => m.attribute === 2 && m.slot === 1));
  for (const it of list) {
    await cmd(`item replace entity blackbox5 weapon.mainhand with minecraft:${it.name}`, 200);
    await lookAtPig();
    await heal();
    await sleep(3200);                                            // полный заряд: смена предмета сбрасывает счётчик
    const h0 = await getHealth();
    stabMode = false; await hit(it); await sleep(500);
    const h1 = await getHealth();
    const drop = h0 - h1;
    const am = it.mods.find(m => m.attribute === 2).amount;
    const expect = 1 + am;                                        // основа игрока 1 + модификатор предмета
    add(it.name, 'attack damage (hit, full charge)', `${expect} (1 + ${am})`, `${drop}`, Math.abs(drop - expect) < 1e-3, `hp ${h0}->${h1}${it.name.endsWith('_spear') ? ' (укол, действие 7)' : ''}`);
    await sleep(500);
  }
  // контроль: голая рука
  await cmd('item replace entity blackbox5 weapon.mainhand with minecraft:air', 200);
  await heal(); await sleep(3200);
  const h0 = await getHealth(); await hit(null); await sleep(500); const h1 = await getHealth();
  add('(hand)', 'attack damage (fist)', '1', `${h0 - h1} (hp ${h0}->${h1})`, h0 - h1 === 1);
}

// ---- фаза 3: прочность ----
// действие, тратящее прочность предмета; возвращает после того как сервер отреагировал
async function doAction(it) {
  const n = it.name;
  const isMelee = it.mods.some(m => m.attribute === 2 && m.slot === 1);
  if (isMelee) { await heal(); await lookAtPig(); await sleep(3200); await hit(it); await sleep(600); return true; }
  if (n === 'bow') {
    await cmd('give blackbox5 minecraft:arrow 5', 300);
    await cmd(`tp blackbox5 ${X + 0.5} ${PY} ${Z + 0.5} -90 0`, 300);
    bot.activateItem(); await sleep(1500); bot.deactivateItem(); await sleep(700);
    await cmd('clear blackbox5 minecraft:arrow', 100);
    return true;
  }
  if (n === 'elytra') return await glide();
  if (it.equip >= 4 && it.equip <= 7) { await hurtBy(4); return true; }
  if (n === 'shield') { await lookAtPig(); bot.activateItem(true); await sleep(800); await hurtBy(4); bot.deactivateItem(); await sleep(300); return true; }
  if (n === 'elytra') return await glide();
  if (n === 'flint_and_steel') {
    const b = bot.blockAt(bot.entity.position.offset(2, -1, 0));
    await bot.lookAt(b.position.offset(0.5, 1, 0.5)); await sleep(200);
    await bot.activateBlock(b); await sleep(600);
    await cmd(`setblock ${X + 2} ${PY} ${Z} air`, 200); return true;
  }
  return false;
}
async function hurtBy(amount) {
  await cmd('effect give blackbox5 minecraft:instant_health 1 10 true', 300);
  await sleep(800);
  await cmd(`damage blackbox5 ${amount} minecraft:mob_attack by @e[tag=cc,limit=1]`, 500);
}
async function glide() {
  await cmd('effect give blackbox5 minecraft:slow_falling 30 0 true', 200);
  await cmd(`tp blackbox5 ${X + 0.5} ${PY + 260} ${Z + 0.5} 0 0`, 700);
  for (let i = 0; i < 6; i++) { await sleep(300); log('y', bot.entity.position.y, 'vy', bot.entity.velocity.y, 'g', bot.entity.onGround); }
  log('перед полётом: onGround', bot.entity.onGround, 'y', bot.entity.position.y, 'слот груди', JSON.stringify(bot.inventory.slots[6] && bot.inventory.slots[6].name));
  bot._client.write('entity_action', { entityId: bot.entity.id, actionId: 'start_fall_flying', jumpBoost: 0 });   // начать планирование
  await sleep(4500);
  log('после: elytraFlying', bot.entity.elytraFlying, 'y', bot.entity.position.y);
  await cmd(`tp blackbox5 ${X + 0.5} ${PY} ${Z + 0.5} 0 0`, 700);
  await cmd('effect clear blackbox5', 100);
  return true;
}
// Элитра не ломается от полёта: износ растёт раз в секунду и замирает на max-1 (крыло
// перестаёт держать). Ловим последовательность значений damage за один долгий полёт.
async function elytraSequence(it) {
  const start = it.max - 5;
  await cmd('effect give blackbox5 minecraft:slow_falling 60 0 true', 200);
  await withItem(it, start);
  await cmd(`tp blackbox5 ${X + 0.5} ${PY + 260} ${Z + 0.5} 0 0`, 900);
  bot._client.write('entity_action', { entityId: bot.entity.id, actionId: 'start_fall_flying', jumpBoost: 0 });
  const seq = []; let last = null; const t0 = Date.now();
  while (Date.now() - t0 < 9000) {
    const d = itemId(slots[6]) === null ? 'пусто' : dmgOf(slots[6]);
    if (d !== last) { seq.push(d); last = d; }
    await sleep(200);
  }
  await cmd(`tp blackbox5 ${X + 0.5} ${PY} ${Z + 0.5} 0 0`, 700);
  await cmd('effect clear blackbox5', 100);
  const top = Math.max(...seq.filter(x => typeof x === 'number'));
  add(it.name, 'max_damage (износ в полёте замирает на max-1)', it.max, `damage: ${seq.join('->')}`, top === it.max - 1 && !seq.includes('пусто'), 'у оригинала крыло не ломается, а замирает на max-1');
}
const slotFor = (it) => (it.equip >= 4 && it.equip <= 7 ? { 7: 5, 6: 6, 5: 7, 4: 8 }[it.equip] : it.name === 'shield' ? 45 : 36);
const slotCmd = (it) => (it.equip >= 4 && it.equip <= 7 ? { 7: 'armor.head', 6: 'armor.chest', 5: 'armor.legs', 4: 'armor.feet' }[it.equip] : it.name === 'shield' ? 'weapon.offhand' : 'weapon.mainhand');

async function withItem(it, damage) {
  const s = slotFor(it);
  await cmd(`item replace entity blackbox5 ${slotCmd(it)} with minecraft:${it.name}[minecraft:damage=${damage}]`, 500);
  return s;
}
const DURA_SAMPLE = ['wooden_sword', 'stone_sword', 'iron_sword', 'golden_sword', 'diamond_sword', 'netherite_sword', 'copper_sword',
  'iron_axe', 'wooden_pickaxe', 'diamond_shovel', 'golden_hoe', 'trident', 'mace', 'iron_spear', 'bow', 'shield',
  'iron_chestplate', 'leather_helmet', 'turtle_helmet', 'netherite_boots', 'elytra', 'flint_and_steel'];
let savedDifficulty = null;
async function phaseDura() {
  // на «мирном» урон мобов игроку не проходит («Target is invulnerable»): на время прогона — лёгкая сложность
  const n = chats.length;
  await cmd('difficulty', 400);
  const dm = chats.slice(n).map(x => x.match(/difficulty is (\w+)/i)).find(Boolean);
  savedDifficulty = dm ? dm[1].toLowerCase() : null;
  log('сложность была', savedDifficulty);
  if (savedDifficulty === 'peaceful') await cmd('difficulty easy', 300);
  await spawnPig();
  const list = pick(i => i.max > 0 && DURA_SAMPLE.includes(i.name));
  for (const it of list) {
    await lookAtPig();
    const s = slotFor(it);
    if (it.name === 'elytra') { await elytraSequence(it); continue; }
    // а) расход за действие
    await withItem(it, 0);
    await doAction(it);
    const d = dmgOf(slots[s]);
    if (d === null || d === 0) { add(it.name, 'max_damage', it.max, `расход за действие не виден (damage=${d})`, null, 'не наблюдаемо этим действием'); continue; }
    // б) на границе: должен сломаться
    await withItem(it, it.max - d);
    await doAction(it);
    const gone = itemId(slots[s]) === null;
    // в) на шаг раньше: не должен
    await withItem(it, it.max - d - 1);
    await doAction(it);
    const still = itemId(slots[s]) !== null;
    const after = dmgOf(slots[s]);
    add(it.name, `max_damage (расход ${d}/действие)`, it.max, `damage=${it.max - d}: ${gone ? 'сломался' : 'цел'}; damage=${it.max - d - 1}: ${still ? 'цел (damage->' + after + ')' : 'сломался'}`, gone && still);
    await cmd(`item replace entity blackbox5 ${slotCmd(it)} with minecraft:air`, 100);
  }
}

// ---- фаза 4: щит ----
const cooldowns = [];
async function phaseShield() {
  bot2 = mk('blackbox4'); hookBot(bot2, 'b4');
  await new Promise(r => bot2.once('spawn', r));
  await cmd('gamemode survival blackbox4');
  await baseAttack('blackbox4');
  await cmd('kill @e[tag=cc]', 100);
  const list = pick(i => i.disable > 0).concat(pick(i => ['iron_sword', 'mace', 'trident', 'iron_spear'].includes(i.name)));
  for (const it of list) {
    await cmd('clear blackbox4'); await cmd('clear blackbox5');
    await cmd('effect give blackbox5 minecraft:instant_health 1 10 true', 300);
    await cmd(`tp blackbox4 ${X + 0.5} ${PY} ${Z + 0.5} 0 0`, 300);
    await cmd(`tp blackbox5 ${X + 0.5} ${PY} ${Z + 2.0} 180 0`, 500);
    await cmd(`item replace entity blackbox5 weapon.offhand with minecraft:shield`, 200);
    await cmd(`item replace entity blackbox4 weapon.mainhand with minecraft:${it.name}`, 200);
    bot.activateItem(true);
    await sleep(3300);                                             // щит поднят, у нападающего полный заряд
    const t0 = Date.now(), n0 = cooldowns.length;
    bot2.attack(bot2.nearestEntity(e => e.username === 'blackbox5'));
    await sleep(900);
    bot.deactivateItem();
    const got = cooldowns.slice(n0);
    const sec = got.length ? got[0].ticks / 20 : 0;
    add(it.name, 'disable_blocking_seconds', it.disable, `${sec}${got.length ? ' (' + got.map(g => g.group + ':' + g.ticks + 'т').join(',') + ')' : ' (нет set_cooldown)'}`, Math.abs(sec - it.disable) < 1e-6);
    await sleep(6500);                                             // дать перезарядке кончиться
  }
}

// ---- вывод ----
function writeTable() {
  const rows = results.map(r => `| ${r.item} | ${r.field} | ${r.table} | ${r.observed} | ${r.ok === null ? 'n/a' : r.ok ? 'OK' : 'MISMATCH'} |`);
  const md = '| item | field | table | observed | OK? |\n|---|---|---|---|---|\n' + rows.join('\n') + '\n';
  fs.writeFileSync(path.join(OUT, `table-${PHASES.join('+')}.md`), md);
  fs.writeFileSync(path.join(OUT, `results-${PHASES.join('+')}.json`), JSON.stringify(results, null, 1));
  const bad = results.filter(r => r.ok === false);
  log(`ИТОГО: ${results.length} проверок, несовпадений ${bad.length}, не наблюдаемо ${results.filter(r => r.ok === null).length}`);
  bad.forEach(r => log('MISMATCH', r.item, r.field, r.table, r.observed));
}

(async () => {
  await lock();
  log('замок взят, фазы', PHASES.join(','), 'предметов в таблице', ITEMS.length);
  bot = mk('blackbox5'); hookBot(bot, 'b5'); hookAttrs(bot); hookMeta(bot);
  bot.on('error', (e) => log('bot error', e.message));
  bot.on('kicked', (r) => log('kicked', JSON.stringify(r)));
  bot._client.on('set_cooldown', (p) => { cooldowns.push({ t: Date.now(), group: String(p.cooldownGroup ?? p.itemId), ticks: p.cooldownTicks }); rawLog.write(`COOLDOWN ${JSON.stringify(p)}\n`); });
  await new Promise(r => bot.once('spawn', r));
  await sleep(1500);
  try {
    await prepare();
    if (args.includes('--probe')) { await spawnPig(); log('probe done'); }
    else {
      if (PHASES.includes('attrs')) await phaseAttrs();
      if (PHASES.includes('damage')) await phaseDamage();
      if (PHASES.includes('dura')) await phaseDura();
      if (PHASES.includes('shield')) await phaseShield();
    }
  } catch (e) { log('ОШИБКА', e.stack); }
  writeTable();
  await cleanup();
  try { bot.quit(); if (bot2) bot2.quit(); } catch (e) {}
  await sleep(500);
  unlock();
  process.exit(0);
})();
