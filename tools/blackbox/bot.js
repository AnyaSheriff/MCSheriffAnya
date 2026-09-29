// Робот-записыватель: заходит на сервер, строит сценарий командами и пишет всё,
// что сервер прислал о мире, с привязкой к тактам. Библиотека mineflayer (MIT)
// используется как есть, её код в MCSheriffAnya не попадает.
'use strict';
const path = require('path');
const fs = require('fs');
const HOME_MODULES = path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(HOME_MODULES, 'mineflayer'));
const mcData = require(path.join(HOME_MODULES, 'minecraft-data'))('26.1');
const Vec3 = require(path.join(HOME_MODULES, 'vec3')).Vec3;
// Номера звуков → имена: таблица снята с официального сервера его же генератором отчётов.
const SOUNDS = JSON.parse(fs.readFileSync(path.join(__dirname, 'data', 'sounds.json'), 'utf8'));

const [,, host, portArg, scenarioFile, outFile, laneArg] = process.argv;
if (!outFile) {
  console.error('использование: node bot.js <host> <port> <scenario.json> <out.json> [полоса]');
  process.exit(2);
}
const port = Number(portArg);
const scenario = JSON.parse(fs.readFileSync(scenarioFile, 'utf8'));
// Полоса (1…5): несколько роботов пишут разные сценарии разом, каждый под
// своим именем и на своей площадке. Полоса 1 — прежние имя и место, так
// старые записи остаются сравнимыми. Площадки полос — через 512 блоков по x
// (целая область файла мира): при дальности обзора 4 чанка ни блоки, ни
// звуки, ни сущности одной полосы до робота другой не доходят.
const LANE = Number(laneArg || process.env.BB_LANE || 1);
if (!Number.isInteger(LANE) || LANE < 1) { console.error(`плохая полоса: ${laneArg || process.env.BB_LANE}`); process.exit(2); }
const LANE_STEP = 512;
const USER = LANE === 1 ? 'blackbox' : `blackbox${LANE}`;
// Площадка вдали от точки появления. Сценарий может задать свою (`origin`):
// порядок обновлений пыли у оригинала зависит от места в мире. Такое место
// задано ради самого места, поэтому полоса его не сдвигает — прогон полосами
// гоняет такие сценарии отдельно, по одному (см. lanes.js).
const ORIGIN = scenario.origin ? { x: scenario.origin[0], y: scenario.origin[1], z: scenario.origin[2] } : { x: 100 + (LANE - 1) * LANE_STEP, y: -59, z: 100 };
const PAD = 8;
const TICK_MS = 50;

const stateName = (id) => {
  const b = mcData.blocksByStateId[id];
  if (!b) return `#${id}`;
  if (b.minStateId === b.maxStateId || !b.states || b.states.length === 0) return b.name;
  // разложить номер состояния в свойства (порядок свойств — как в minecraft-data)
  let n = id - b.minStateId;
  const props = [];
  for (let i = b.states.length - 1; i >= 0; i--) {
    const s = b.states[i];
    const num = s.num_values;
    const v = n % num; n = Math.floor(n / num);
    let val;
    if (s.type === 'bool') val = v === 0 ? 'true' : 'false';
    else if (s.values) val = s.values[v];
    else val = String(v);
    props.unshift(`${s.name}=${val}`);
  }
  return `${b.name}[${props.join(',')}]`;
};
const rel = (p) => [p.x - ORIGIN.x, p.y - ORIGIN.y, p.z - ORIGIN.z];
const inPad = (p) => Math.abs(p.x - ORIGIN.x) <= PAD && Math.abs(p.y - ORIGIN.y) <= PAD && Math.abs(p.z - ORIGIN.z) <= PAD;

const bot = mineflayer.createBot({ host, port, username: USER, version: '26.1', auth: 'offline' });
const events = [];
let serverAge = null, ageStamp = 0;       // возраст мира из последнего «обновления времени»
let recording = false, t0 = null;         // t0 — момент первого действия (мс)

// Часы сервера. «Обновление времени» приходит раз в секунду на границе такта
// и несёт возраст мира — значит, каждый такой пакет даёт оценку момента, когда
// у сервера был такт 0. Пакеты задерживаются (сеть, разбор), но никогда не
// приходят раньше срока, поэтому из окна последних измерений берём самое
// раннее: оно меньше всех испорчено задержкой. Окно короткое — сервер может
// подтормаживать, и тогда старые измерения врут.
const CLOCK_WINDOW = 6;
const clockSamples = [];                  // только измерения, прошедшие проверку
let roughBase = null;                     // грубая оценка до первой проверки
let lastClock = null;                     // предыдущее измерение, с чем сверяться
let tickBase = null;                      // мс по часам робота, когда у сервера был такт 0
// На время записи база замирает: она уточняется каждую секунду на несколько
// миллисекунд, и если считать по живой, соседние ожидания разъезжаются —
// нажатие уходит то на такт раньше, то на такт позже. Записи идут пару секунд,
// за них часы не успевают разойтись.
let fixedBase = null;
// Насколько начало такта сервера отстоит от нашей оценки. «Обновление
// времени» каждый сервер шлёт в свой момент такта, поэтому сетка по нему
// сдвинута — поправку измеряем отдельно, по откликам (см. calibrate).
let clockOffset = 0;
const baseNow = () => (fixedBase === null ? (tickBase === null ? null : tickBase + clockOffset) : fixedBase);
const tickNow = () => (baseNow() === null ? null : Math.floor((Date.now() - baseNow()) / TICK_MS));
const tickStartMs = (t) => baseNow() + t * TICK_MS;
// Спать до доли `phase` такта `target` по часам сервера. Шагами по 20 мс,
// чтобы подхватывать поправку часов, пришедшую пока мы спали.
const waitTick = async (target, phase = 0.5) => {
  for (;;) {
    const left = tickStartMs(target) + phase * TICK_MS - Date.now();
    if (left <= 0) return;
    await new Promise(r => setTimeout(r, Math.min(left, 20)));
  }
};
// Пакеты одного такта приходят одной пачкой, за доли миллисекунды. Если
// между соседними событиями меньше 15 мс, это один такт — даже если по часам
// робота граница такта пришлась между ними.
let lastNoteMs = null, lastNoteTick = null;
let calibHook = null;        // во время калибровки ловит момент первого отклика
const note = (kind, data) => {
  if (calibHook) calibHook(Date.now());
  if (!recording) return;
  const now = Date.now();
  let tick = tickNow();
  if (lastNoteMs !== null && now - lastNoteMs < 15 && lastNoteTick !== null) tick = lastNoteTick;
  lastNoteMs = now; lastNoteTick = tick;
  events.push({ tick, ms: now - t0, kind, ...data });
};

bot._client.on('update_time', (p) => {
  // Часы приходят раз в секунду. Если чаще — робот не успевает разбирать
  // пакеты (например, ещё грызёт чанки), и времена в записи врут.
  if (serverAge !== null && Date.now() - ageStamp < 800) console.error('внимание: робот отстаёт от сервера, времена в записи неточные');
  serverAge = Number(p.age); ageStamp = Date.now();
  // Первый пакет приходит при входе, вне общего порядка, и врёт на сотни
  // миллисекунд. Измерению верим, только если от предыдущего прошло столько
  // же времени, сколько тактов насчитал сервер.
  const sample = ageStamp - serverAge * TICK_MS;
  const sane = lastClock !== null
    && Math.abs((ageStamp - lastClock.at) - (serverAge - lastClock.age) * TICK_MS) < 30;
  lastClock = { age: serverAge, at: ageStamp };
  if (roughBase === null) roughBase = sample;
  if (sane) {
    clockSamples.push(sample);
    if (clockSamples.length > CLOCK_WINDOW) clockSamples.shift();
  }
  tickBase = clockSamples.length ? Math.min(...clockSamples) : roughBase;
});
bot._client.on('block_change', (p) => {
  if (process.env.BB_DEBUG) console.error('bc', JSON.stringify(p));
  if (inPad(p.location)) note('block', { at: rel(p.location), state: stateName(p.type) });
});
bot._client.on('multi_block_change', (p) => {
  if (process.env.BB_DEBUG) console.error('mbc', JSON.stringify(p));
  const c = p.chunkCoordinates;
  for (const r of p.records) {
    const n = BigInt(r);
    const state = Number(n >> 12n);
    const local = Number(n & 0xfffn);
    const pos = { x: c.x * 16 + (local >> 8), z: c.z * 16 + ((local >> 4) & 15), y: c.y * 16 + (local & 15) };
    if (inPad(pos)) note('block', { at: rel(pos), state: stateName(state), batch: true });
  }
});
bot._client.on('block_action', (p) => {
  if (inPad(p.location)) note('action', { at: rel(p.location), a: p.byte1, b: p.byte2, block: (mcData.blocks[p.blockId] || {}).name || p.blockId });
});
bot._client.on('sound_effect', (p) => {
  const pos = { x: p.x / 8, y: p.y / 8, z: p.z / 8 };
  if (!inPad(pos)) return;
  const s = p.sound || {};
  // Оригинал шлёт номер из реестра, мы — имя прямо в пакете; приводим к имени.
  const name = s.soundId !== undefined ? (SOUNDS[String(s.soundId)] || `#${s.soundId}`)
    : s.data && s.data.soundName ? s.data.soundName
    : JSON.stringify(s);
  note('sound', { at: rel(pos).map(v => Math.round(v * 8) / 8), sound: name, category: p.soundCategory, volume: p.volume, pitch: p.pitch });
});
bot._client.on('world_particles', (p) => {
  const pos = { x: p.x, y: p.y, z: p.z };
  if (!inPad(pos)) return;
  const kind = p.particle && (p.particle.type !== undefined ? p.particle.type : p.particle);
  note('particle', {
    at: rel(pos).map(v => Math.round(v * 1000) / 1000,),
    particle: typeof kind === 'object' ? JSON.stringify(kind) : String(kind),
    count: p.particles,
    speed: Math.round((p.particleData ?? 0) * 1000) / 1000,
    offset: [p.offsetX, p.offsetY, p.offsetZ].map(v => Math.round((v ?? 0) * 1000) / 1000).join(','),
    long: p.longDistance
  });
});
// Динамит на площадке: его свойства (запал, блок) пишем отдельно — по ним
// видно, с каким запалом он появился.
const padTnt = new Set();
// Вагонетки на площадке: их движение пишем пакет за пакетом — сдвиг, скорость,
// перенос, сверка места. По ним видно, как часто и чем сервер рассказывает о
// вагонетке. Сдвиг — в 1/4096 блока, как в пакете; место — от площадки.
const padCarts = new Set();
// Где вагонетка по присланному (появление, сдвиги, сверки): пакеты
// пишутся, пока она на площадке, — увезённая командой прочь не в счёт.
const cartAt = new Map();
// Как называть сущность в записи движения: по умолчанию — вагонетка.
const entKind = new Map();
const CART_TYPES = new Set(['minecart', 'chest_minecart', 'hopper_minecart', 'tnt_minecart', 'furnace_minecart']);
bot._client.on('spawn_entity', (p) => {
  const pos = { x: p.x, y: p.y, z: p.z };
  const type = (mcData.entities[p.type] || {}).name || p.type;
  if (inPad(pos) && type === 'tnt') padTnt.add(p.entityId);
  // Только появившиеся за запись: оставшиеся от прошлых прогонов не в счёт.
  if (recording && inPad(pos) && CART_TYPES.has(type)) { padCarts.add(p.entityId); cartAt.set(p.entityId, pos); }
  // `track_items` в сценарии: движение выпавших предметов пишется так же,
  // как у вагонеток (поршень толкает предметы). Предметы, лежавшие до записи,
  // тоже в счёт: их кладут командой перед записью.
  if (scenario.track_items && inPad(pos) && type === 'item') { padCarts.add(p.entityId); cartAt.set(p.entityId, pos); entKind.set(p.entityId, 'предмет'); }
  if (inPad(pos)) note('spawn', { at: rel(pos).map(v => Math.round(v * 100) / 100), type, data: p.objectData, id: p.entityId, velocity: p.velocity });
});
bot._client.on('entity_metadata', (p) => {
  if (!padTnt.has(p.entityId) && !(padCarts.has(p.entityId) && inPad(cartAt.get(p.entityId)))) return;
  const fields = (p.metadata || []).map(m => `${m.key}=${typeof m.value === 'object' ? JSON.stringify(m.value) : m.value}`);
  if (fields.length) note('meta', { id: p.entityId, fields: fields.join(' ') });
});
const round3 = (v) => Math.round(v * 1000) / 1000;
const vec = (v) => (v ? [v.x, v.y, v.z].map(round3) : null);
for (const name of ['rel_entity_move', 'entity_move_look', 'entity_look']) {
  bot._client.on(name, (p) => {
    if (!padCarts.has(p.entityId)) return;
    const d = p.dX === undefined ? null : [p.dX, p.dY, p.dZ];
    const was = cartAt.get(p.entityId);
    if (d && was) cartAt.set(p.entityId, { x: was.x + d[0] / 4096, y: was.y + d[1] / 4096, z: was.z + d[2] / 4096 });
    if (!inPad(cartAt.get(p.entityId) || was)) return;
    note('cart', { ent: entKind.get(p.entityId), what: name === 'entity_look' ? 'rot' : 'move', id: p.entityId, d, yaw: p.yaw, pitch: p.pitch, ground: p.onGround });
  });
}
// Событие сущности у вагонетки: 10 — вагонетку с динамитом подожгли.
bot._client.on('entity_status', (p) => {
  if (padCarts.has(p.entityId) && inPad(cartAt.get(p.entityId))) note('cart', { ent: entKind.get(p.entityId), what: 'status', id: p.entityId, status: p.entityStatus });
});
bot._client.on('entity_velocity', (p) => {
  if (padCarts.has(p.entityId) && inPad(cartAt.get(p.entityId))) note('cart', { ent: entKind.get(p.entityId), what: 'vel', id: p.entityId, v: vec(p.velocity) });
});
for (const name of ['entity_teleport', 'sync_entity_position']) {
  bot._client.on(name, (p) => {
    if (!padCarts.has(p.entityId)) return;
    cartAt.set(p.entityId, { x: p.x, y: p.y, z: p.z });
    if (!inPad(p)) return;
    note('cart', { ent: entKind.get(p.entityId), what: name === 'entity_teleport' ? 'tp' : 'sync', id: p.entityId, at: rel(p).map(round3), v: [p.dx, p.dy, p.dz].map(round3), yaw: round3(p.yaw), pitch: round3(p.pitch), ground: p.onGround });
  });
}
// Взрыв приходит своим пакетом: звук и вспышка — в нём же. Число блоков
// случайно, сравнивается только место, сила, вспышка и звук.
bot._client.on('explosion', (p) => {
  const c = p.center;
  if (!inPad(c)) return;
  const s = p.sound || {};
  const sound = s.soundId !== undefined ? (SOUNDS[String(s.soundId)] || `#${s.soundId}`)
    : s.data && s.data.soundName ? s.data.soundName
    : JSON.stringify(s);
  const smoke = (p.blockParticles || []).map(b => `${b.data.particle.type}:${b.data.scaling}/${b.data.speed}x${b.weight}`).join(' ');
  note('explosion', { at: rel(c).map(v => Math.round(v * 1000) / 1000), radius: p.radius, blocks: p.blockCount, knockback: Boolean(p.playerKnockback), particle: String(p.explosionParticle && p.explosionParticle.type), sound, smoke });
});
// `track_self` в сценарии: что сервер говорит роботу о нём самом — сверка
// места (Synchronize Player Position) и скорость (поршень толкает игрока).
bot._client.on('position', (p) => {
  if (!scenario.track_self || !recording) return;
  note('self', { what: 'pos', at: rel({ x: p.x, y: p.y, z: p.z }).map(round3), flags: JSON.stringify(p.flags) });
});
bot._client.on('entity_velocity', (p) => {
  if (scenario.track_self && bot.entity && p.entityId === bot.entity.id) note('self', { what: 'vel', v: vec(p.velocity) });
});
bot._client.on('entity_destroy', (p) => note('destroy', { ids: p.entityIds }));
bot._client.on('entity_destroy', (p) => {
  for (const id of p.entityIds) {
    if (!padCarts.has(id)) continue;
    padCarts.delete(id);
    if (inPad(cartAt.get(id))) note('cart', { ent: entKind.get(id), what: 'gone', id });
  }
});
// Окна хранилищ: робот открывает сундук или воронку щелчком и видит каждое
// изменение содержимого — сервер шлёт его открытому окну в тот же такт.
const itemName = (it) => {
  if (!it || !it.itemCount) return null;
  const id = it.itemId ?? it.id;
  return `${(mcData.items[id] || {}).name || id}×${it.itemCount}`;
};
let openWindow = null;
bot._client.on('open_window', (p) => { openWindow = Number(p.windowId); });
// Заголовок окна — ключ перевода, если он есть: у окна вагонетки заголовок
// несёт ещё и её опознаватель, а он у каждого сервера свой.
const titleOf = (t) => { const j = JSON.stringify(t ?? ''); const m = /"translate":\{"type":"string","value":"([^"]+)"/.exec(j) || /"translate":"([^"]+)"/.exec(j); return m ? m[1] : j.slice(0, 60); };
bot._client.on('open_window', (p) => note('window', { type: String(p.inventoryType ?? p.windowType), title: titleOf(p.windowTitle ?? p.title) }));
bot._client.on('window_items', (p) => {
  if (Number(p.windowId) === 0) return;
  const slots = (p.items || []).map((it, i) => [i, itemName(it)]).filter(([, n]) => n).map(([i, n]) => `${i}:${n}`);
  note('slots', { items: slots.join(' ') });
});
let stateId = 0;
bot._client.on('window_items', (p) => { stateId = p.stateId; });
bot._client.on('set_slot', (p) => {
  stateId = p.stateId;
  if (Number(p.windowId) === 0) return;
  note('slot', { window: Number(p.windowId) === -1 ? 'курсор' : 'окно', slot: p.slot, item: itemName(p.item) || 'пусто' });
});
// `inventory: true` в сценарии — писать и ячейки инвентаря самого робота
// (полка меняет местами панель быстрого доступа).
bot._client.on('set_slot', (p) => {
  if (scenario.inventory && Number(p.windowId) === 0) note('inv', { slot: p.slot, item: itemName(p.item) || 'пусто' });
});
bot._client.on('window_items', (p) => {
  if (!scenario.inventory || Number(p.windowId) !== 0) return;
  const slots = (p.items || []).map((it, i) => [i, itemName(it)]).filter(([, n]) => n).map(([i, n]) => `${i}:${n}`);
  note('inv', { slot: 'все', item: slots.join(' ') });
});
bot._client.on('set_cursor_item', (p) => note('cursor', { item: itemName(p.contents) || 'пусто' }));
// События мира (звук и дым выбрасывателя, шаги и т. п.) — клиент играет их сам.
bot._client.on('world_event', (p) => {
  if (inPad(p.location)) note('level', { at: rel(p.location), effect: p.effectId, data: p.data });
});
bot._client.on('craft_progress_bar', (p) => note('property', { property: p.property, value: p.value }));
bot._client.on('close_window', () => note('close', {}));
bot._client.on('tile_entity_data', (p) => {
  // Данные — без обёрток типов (prismarine-nbt `simplify`): так видно
  // содержимое полки целиком, а не первые байты.
  let data;
  try { data = JSON.stringify(require('prismarine-nbt').simplify(p.nbtData)); } catch (e) { data = JSON.stringify(p.nbtData); }
  if (inPad(p.location)) note('block_entity', { at: rel(p.location), type: p.action, data: data.slice(0, 400) });
});

// Момент действия отмечаем не когда решили нажать, а когда пакет нажатия
// действительно ушёл серверу: робот сперва поворачивает голову, и это долго.
let pendingDo = null;
{
  const write = bot._client.write.bind(bot._client);
  bot._client.write = (name, params) => {
    if (process.env.BB_DEBUG && name !== 'position' && name !== 'position_look' && name !== 'look' && name !== 'keep_alive') console.error('→', name, JSON.stringify(params, (k, v) => typeof v === 'bigint' ? String(v) : v).slice(0, 160));
    if (pendingDo && (name === 'block_place' || name === 'chat_command' || name === 'chat_command_signed' || name === 'close_window' || name === 'window_click' || name === 'enchant_item' || name === 'set_slot_state' || name === 'attack' || name === 'held_item_slot')) {
      // Такт нажатия берём из плана, а не из часов: в какой такт целились,
      // в тот оно и уходит. Часы на этом месте врать не должны, но нажатие
      // ложится ровно на границу счёта, и округление могло бы дрогнуть.
      // phase — в какую долю такта сервера нажатие ушло на самом деле.
      const phase = baseNow() === null ? null : Math.round(((Date.now() - baseNow()) % TICK_MS) / TICK_MS * 100) / 100;
      events.push({ tick: pendingDo.tick, ms: Date.now() - t0, kind: 'do', what: pendingDo.what, phase });
      pendingDo = null;
    }
    return write(name, params);
  };
}

// Нажатие шлём сами. У mineflayer `activateBlock` сперва плавно поворачивает
// голову и ждёт своего физического такта — это до 50 мс, и нажатие уезжает
// в соседний такт сервера. Робот прицелен заранее, поворачивать нечего.
let placeSeq = 0;
// `face` и `cursor` в действии — по какой грани и в какую точку щёлкнуть
// (полка и резной книжный шкаф выбирают ячейку по точке щелчка).
const clickBlock = (block, face = 1, cursor = [0.5, 0.5, 0.5]) => {
  bot._client.write('block_place', {
    location: block.position,
    direction: face,           // по умолчанию жмём сверху
    hand: 0,
    cursorX: cursor[0], cursorY: cursor[1], cursorZ: cursor[2],
    insideBlock: false,
    sequence: placeSeq++,
    worldBorderHit: false
  });
  bot.swingArm();
};

// Где у сервера начинается такт. По «обновлению времени» этого не узнать:
// оригинал шлёт его в одном месте такта, мы — в другом, и сетка, снятая по
// нему, у каждого сдвинута по-своему. А вот отклик на нажатие сервер шлёт
// сразу, как его обработал, то есть в самом начале такта. Нажимаем несколько
// раз вхолостую и смотрим, в какую долю нашей сетки приходят ответы: самый
// ранний ответ и показывает начало такта. Без этого робот целится в середину
// такта у одного сервера и на границу — у другого, и записи не сравнить.
const calibrate = async (times = 6) => {
  const phases = [];
  // Меряем откликом на команду, а не нажатием по схеме: схему трогать нельзя,
  // иначе к записи она придёт не в исходном виде — в `grass-crush` поршень
  // успел бы раздавить траву ещё до начала.
  const spot = { x: ORIGIN.x + PAD, y: ORIGIN.y, z: ORIGIN.z + PAD };

  for (let i = 0; i < times; i++) {
    let got = null;
    calibHook = (t) => { if (got === null) got = t; };
    await waitTick(tickNow() + 3, 0.5);
    bot.chat(`/setblock ${spot.x} ${spot.y} ${spot.z} ${i % 2 ? 'minecraft:air' : 'minecraft:stone'}`);
    await sleepTicks(8);
    calibHook = null;
    if (got !== null) phases.push(((got - tickBase) % TICK_MS + TICK_MS) % TICK_MS);
  }

  if (phases.length < 3) throw new Error('калибровка не удалась: сервер не отвечает на команды');

  // Доли такта лежат по кругу: разворачиваем их вокруг первой, иначе 49 мс
  // и 1 мс выглядят как разные концы, хотя это соседние мгновения.
  const f0 = phases[0];
  const around = phases.map(f => {
    let d = f - f0;
    if (d > TICK_MS / 2) d -= TICK_MS;
    if (d < -TICK_MS / 2) d += TICK_MS;
    return f0 + d;
  });
  // Пакеты сервера приходят вплотную к началу его такта, и если ставить
  // границу счёта туда же, соседние прогоны раскладывают одни и те же
  // события по разным тактам: хватает пары миллисекунд. Поэтому границу
  // счёта отодвигаем на полтакта — тогда приходы ложатся ровно в середину
  // такта, дальше всего от края.
  const start = Math.min(...around);
  clockOffset = start + TICK_MS / 2;
  console.error(`калибровка: начало такта на ${start.toFixed(0)} мс от сетки часов, доли ${phases.map(f => f.toFixed(0)).join(' ')}`);
};

const sleepTicks = (n) => new Promise(r => setTimeout(r, n * TICK_MS));
const say = async (cmd) => { bot.chat('/' + cmd); await sleepTicks(2); };
const setBlock = (b) => say(`setblock ${ORIGIN.x + b.at[0]} ${ORIGIN.y + b.at[1]} ${ORIGIN.z + b.at[2]} ${b.state}`);
// Команда сценария: `$(dx,dy,dz)` заменяется настоящими координатами точки
// площадки — так в `cmd` можно звать summon, data, item и прочее. Дробные
// смещения тоже годятся: `$(0.5,0,0.5)` — середина клетки (summon). Сумма
// округляется до знаков смещения: 612 + 6.93 в двоичных дробях дало бы хвост.
// `$(bot)` — имя робота этой полосы (tp, give, clear, item replace).
const shift = (o, d) => String(Number((o + Number(d)).toFixed((d.split('.')[1] || '').length)));
const expand = (cmd) => cmd
  .replace(/\$\(bot\)/g, USER)
  .replace(/\$\((-?[\d.]+),(-?[\d.]+),(-?[\d.]+)\)/g, (_, x, y, z) => `${shift(ORIGIN.x, x)} ${shift(ORIGIN.y, y)} ${shift(ORIGIN.z, z)}`);

bot.once('spawn', async () => {
  try {
    await sleepTicks(100); // дать роботу разобрать присланные чанки
    await say(`gamemode creative`);
    // Инвентарь робота у каждого сервера свой и переживает перезаходы:
    // очищаем, чтобы окна хранилищ показывали одно и то же.
    await say(`clear ${USER}`);
    // Выбранная ячейка руки тоже переживает перезаходы: другой сценарий мог
    // оставить не первую, и тогда выданная в первую ячейку пластинка
    // оказывается не в руке (`jukebox-disc` у оригинала через раз не играл).
    bot.setQuickBarSlot(0);
    await say(`tp ${USER} ${ORIGIN.x + 3} ${ORIGIN.y + 1} ${ORIGIN.z + 3}`);
    // Висеть на месте, пока строится пол: у нашего ровного мира земля на
    // нуле, и площадка на ORIGIN.y оказывается в пустоте под ней. Упавшего
    // робота сервер не пустит к рычагу — слишком далеко.
    bot.creative.startFlying();
    await sleepTicks(40);
    // Площадка новой полосы в первый раз лежит в ещё не созданных чанках:
    // заливка по незагруженному месту не сработает. Ждём, пока сервер пришлёт
    // все четыре угла площадки (у обжитой площадки они уже есть).
    const corners = [[-PAD, -PAD], [PAD, -PAD], [-PAD, PAD], [PAD, PAD]].map(([dx, dz]) => new Vec3(ORIGIN.x + dx, ORIGIN.y, ORIGIN.z + dz));
    for (let i = 0; corners.some(c => !bot.blockAt(c)) && i < 200; i++) await sleepTicks(1);
    if (corners.some(c => !bot.blockAt(c))) throw new Error('чанки площадки так и не пришли');
    await say(`fill ${ORIGIN.x - PAD} ${ORIGIN.y - 1} ${ORIGIN.z - PAD} ${ORIGIN.x + PAD} ${ORIGIN.y + PAD} ${ORIGIN.z + PAD} minecraft:air`);
    await say(`fill ${ORIGIN.x - PAD} ${ORIGIN.y - 1} ${ORIGIN.z - PAD} ${ORIGIN.x + PAD} ${ORIGIN.y - 1} ${ORIGIN.z + PAD} minecraft:stone`);
    // Чужие сущности вокруг площадки — летучие мыши из пещер, предметы от
    // прошлых взрывов — шумят в записи: убрать и мобов больше не пускать.
    await say(`gamerule spawn_mobs false`);
    // Дождь у оригинала гасит небесный свет: датчик света в полдень давал 12
    // вместо 15 (daylight-*, у нас погоды нет). Перед записью — ясно.
    await say(`weather clear`);
    await say(`kill @e[type=!player,distance=..48]`);
    await sleepTicks(20);
    // Сетку тактов снимаем до постройки: калибровка ставит и убирает блок
    // в углу площадки, и этот угол потом затирается заливкой.
    for (let i = 0; clockSamples.length < 2 && i < 60; i++) await sleepTicks(20);
    if (clockSamples.length < 2) throw new Error('не удалось сверить часы с сервером: нечем привязывать такты');
    await calibrate();
    await say(`setblock ${ORIGIN.x + PAD} ${ORIGIN.y} ${ORIGIN.z + PAD} minecraft:air`);

    for (const b of scenario.blocks) await setBlock(b);
    for (const c of scenario.setup || []) await say(expand(c));
    await sleepTicks(40);                      // тишина перед записью
    // Прицелиться заранее, до начала записи: иначе поворот головы съест
    // первые такты сценария.
    let aimedAt = null;
    const first = scenario.actions.find(a => a.click);

    if (first) {
      const block = bot.blockAt(new Vec3(ORIGIN.x + first.click[0], ORIGIN.y + first.click[1], ORIGIN.z + first.click[2]));
      await bot.lookAt(block.position.offset(0.5, 0.5, 0.5), true);
      await sleepTicks(10);
      aimedAt = block.position;
    }

    // Ждём часы сервера: без них не привязаться к его тактам. Нужны
    // проверенные измерения, а не первое попавшееся.
    for (let i = 0; clockSamples.length < 2 && i < 60; i++) await sleepTicks(20);
    if (clockSamples.length < 2) throw new Error('не удалось сверить часы с сервером: нечем привязывать такты');

    fixedBase = tickBase + clockOffset;
    recording = true; t0 = Date.now();
    // Нажатия расставляем по тактам сервера, а не по часам робота: `after` —
    // это такты от первого нажатия. Целимся в середину такта, чтобы промах
    // на десяток миллисекунд не перекидывал нажатие в соседний такт.
    let baseTick = tickNow() + 3;   // запас, чтобы первое нажатие не опоздало
    // `align` — начать сценарий в такте, кратном этому числу по возрасту
    // мира: то, что сервер делает раз в 20 тактов (датчик света), иначе
    // попадает в случайную фазу относительно действий, и записи двух
    // серверов не сравнить.
    if (scenario.align) baseTick = Math.ceil(baseTick / scenario.align) * scenario.align;
    for (const a of scenario.actions) {
      // Граница счётного такта — это ровно середина между приходами пакетов,
      // то есть середина настоящего такта сервера: дальше всего от его краёв,
      // куда бы отправка ни дрогнула.
      const at = baseTick + (a.after || 0);
      await waitTick(at, 0);
      const what = a.set ? `set ${a.set.at.join(',')} ${a.set.state}` : a.click ? `click ${a.click.join(',')}` : a.cmd ? `cmd ${a.cmd}` : a.close ? 'close' : a.window_click ? `window_click ${JSON.stringify(a.window_click)}` : a.attack ? 'attack' : a.interact ? 'interact' : a.button !== undefined ? `button ${a.button}` : a.slot_state ? `slot_state ${a.slot_state.slot} ${a.slot_state.state}` : a.hotbar !== undefined ? `hotbar ${a.hotbar}` : JSON.stringify(a);
      pendingDo = { what, tick: at };
      if (a.set) bot.chat(`/setblock ${ORIGIN.x + a.set.at[0]} ${ORIGIN.y + a.set.at[1]} ${ORIGIN.z + a.set.at[2]} ${a.set.state}`);
      if (a.cmd) bot.chat('/' + expand(a.cmd));
      // Закрыть открытое окно, как это делает игрок клавишей Esc.
      // Щелчок в открытом окне: слот, кнопка, режим. Что изменилось, робот
      // серверу не подсказывает — пусть сервер сам пришлёт итог.
      if (a.window_click && openWindow !== null) {
        const w = a.window_click;
        bot._client.write('window_click', { windowId: openWindow, stateId, slot: w.slot, mouseButton: w.button || 0, mode: w.mode || 0, changedSlots: [], cursorItem: undefined });
      }
      // Кнопка в открытом окне (кафедра листает, стол зачарования): пакет
      // Click Container Button, в mineflayer он зовётся enchant_item.
      if (a.button !== undefined && openWindow !== null) bot._client.write('enchant_item', { windowId: openWindow, enchantment: a.button });
      // Щелчок по пустой ячейке крафтера: включить или выключить её
      // (пакет Change Container Slot State, в mineflayer — set_slot_state).
      if (a.slot_state && openWindow !== null) bot._client.write('set_slot_state', { slot_id: a.slot_state.slot, window_id: openWindow, state: a.slot_state.state });
      // Выбрать ячейку панели быстрого доступа (пакет Set Held Item).
      if (a.hotbar !== undefined) bot._client.write('held_item_slot', { slotId: a.hotbar });
      if (a.close && openWindow !== null) { bot._client.write('close_window', { windowId: openWindow }); openWindow = null; }
      // Щелчок (правой кнопкой) по последней появившейся на площадке
      // вагонетке — основной рукой, в середину её ящика.
      if (a.interact) {
        const id = [...padCarts].pop();
        if (id !== undefined) { bot._client.write('use_entity', { target: id, hand: 0, location: { x: 0, y: 0.35, z: 0 }, sneaking: false }); bot.swingArm(); }
      }
      // Удар по последней появившейся на площадке вагонетке.
      if (a.attack) {
        const id = [...padCarts].pop();
        if (id !== undefined) { bot._client.write('attack', { entityId: id }); bot.swingArm(); }
      }
      if (a.click) {
        const block = bot.blockAt(new Vec3(ORIGIN.x + a.click[0], ORIGIN.y + a.click[1], ORIGIN.z + a.click[2]));

        // Повернуться к цели надо один раз: поворот занимает такты, и при
        // частых нажатиях он бы съедал промежутки между ними.
        if (!aimedAt || !aimedAt.equals(block.position)) {
          await bot.lookAt(block.position.offset(0.5, 0.5, 0.5), true);
          await sleepTicks(5);
          aimedAt = block.position;
        }

        clickBlock(block, a.face ?? 1, a.cursor ?? [0.5, 0.5, 0.5]);
      }
    }
    // Окно записи — до такта `last` включительно. События такта приходят
    // в его середину, ровно туда, где кончалось бы ожидание, — и последний
    // такт то попадал в запись, то нет (minecart-slope: сдвиг вагонетки на
    // +150 у одного сервера есть, у другого нет). Поэтому слушаем ещё три
    // такта, а всё позже `last` compare.js отбрасывает у обеих сторон.
    const last = baseTick + scenario.record_ticks + 10;
    await waitTick(last + 3);
    recording = false;
    // Уборка после записи (`cleanup`): вагонетки и прочие сущности иначе
    // остались бы на площадке и мешали следующим сценариям.
    for (const c of scenario.cleanup || []) await say(expand(c));
    fs.writeFileSync(outFile, JSON.stringify({ scenario: scenario.name, host, port, lane: LANE, window: { first: baseTick, last }, events }, null, 1));
    console.log(`записано событий: ${events.length} → ${outFile}`);
    bot.quit();
    setTimeout(() => process.exit(0), 500);
  } catch (e) { console.error(e); process.exit(1); }
});
bot.on('kicked', (r) => { console.error('кик:', JSON.stringify(r)); process.exit(1); });
bot.on('error', (e) => { console.error('ошибка:', e.message); process.exit(1); });
bot.on('message', (m) => { const s = m.toString(); if (s && !/blackbox/.test(s)) console.error('  чат:', s); });
setTimeout(() => { console.error('время вышло'); process.exit(1); }, 120000);
