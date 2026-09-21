// Робот-записыватель: заходит на сервер, строит сценарий командами и пишет всё,
// что сервер прислал о мире, с привязкой к тактам. Библиотека mineflayer (MIT)
// используется как есть, её код в RustCraft не попадает.
'use strict';
const path = require('path');
const fs = require('fs');
const HOME_MODULES = path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(HOME_MODULES, 'mineflayer'));
const mcData = require(path.join(HOME_MODULES, 'minecraft-data'))('26.1');
const Vec3 = require(path.join(HOME_MODULES, 'vec3')).Vec3;
// Номера звуков → имена: таблица снята с официального сервера его же генератором отчётов.
const SOUNDS = JSON.parse(fs.readFileSync(path.join(__dirname, 'data', 'sounds.json'), 'utf8'));

const [,, host, portArg, scenarioFile, outFile] = process.argv;
if (!outFile) {
  console.error('использование: node bot.js <host> <port> <scenario.json> <out.json>');
  process.exit(2);
}
const port = Number(portArg);
const scenario = JSON.parse(fs.readFileSync(scenarioFile, 'utf8'));
const ORIGIN = { x: 100, y: -59, z: 100 }; // площадка вдали от точки появления
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

const bot = mineflayer.createBot({ host, port, username: 'blackbox', version: '26.1', auth: 'offline' });
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
bot._client.on('spawn_entity', (p) => {
  const pos = { x: p.x, y: p.y, z: p.z };
  if (inPad(pos)) note('spawn', { at: rel(pos).map(v => Math.round(v * 100) / 100), type: (mcData.entities[p.type] || {}).name || p.type, data: p.objectData, id: p.entityId });
});
bot._client.on('entity_destroy', (p) => note('destroy', { ids: p.entityIds }));
bot._client.on('tile_entity_data', (p) => {
  if (inPad(p.location)) note('block_entity', { at: rel(p.location), type: p.action, data: JSON.stringify(p.nbtData).slice(0, 200) });
});

// Момент действия отмечаем не когда решили нажать, а когда пакет нажатия
// действительно ушёл серверу: робот сперва поворачивает голову, и это долго.
let pendingDo = null;
{
  const write = bot._client.write.bind(bot._client);
  bot._client.write = (name, params) => {
    if (process.env.BB_DEBUG && name !== 'position' && name !== 'position_look' && name !== 'look' && name !== 'keep_alive') console.error('→', name, JSON.stringify(params, (k, v) => typeof v === 'bigint' ? String(v) : v).slice(0, 160));
    if (pendingDo && (name === 'block_place' || name === 'chat_command' || name === 'chat_command_signed')) {
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
const clickBlock = (block) => {
  bot._client.write('block_place', {
    location: block.position,
    direction: 1,              // жмём сверху
    hand: 0,
    cursorX: 0.5, cursorY: 0.5, cursorZ: 0.5,
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

bot.once('spawn', async () => {
  try {
    await sleepTicks(100); // дать роботу разобрать присланные чанки
    await say(`gamemode creative`);
    await say(`tp blackbox ${ORIGIN.x + 3} ${ORIGIN.y + 1} ${ORIGIN.z + 3}`);
    await sleepTicks(40);
    await say(`fill ${ORIGIN.x - PAD} ${ORIGIN.y - 1} ${ORIGIN.z - PAD} ${ORIGIN.x + PAD} ${ORIGIN.y + PAD} ${ORIGIN.z + PAD} minecraft:air`);
    await say(`fill ${ORIGIN.x - PAD} ${ORIGIN.y - 1} ${ORIGIN.z - PAD} ${ORIGIN.x + PAD} ${ORIGIN.y - 1} ${ORIGIN.z + PAD} minecraft:stone`);
    await sleepTicks(20);
    // Сетку тактов снимаем до постройки: калибровка ставит и убирает блок
    // в углу площадки, и этот угол потом затирается заливкой.
    for (let i = 0; clockSamples.length < 2 && i < 60; i++) await sleepTicks(20);
    if (clockSamples.length < 2) throw new Error('не удалось сверить часы с сервером: нечем привязывать такты');
    await calibrate();
    await say(`setblock ${ORIGIN.x + PAD} ${ORIGIN.y} ${ORIGIN.z + PAD} minecraft:air`);

    for (const b of scenario.blocks) await setBlock(b);
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
    const baseTick = tickNow() + 3;   // запас, чтобы первое нажатие не опоздало
    for (const a of scenario.actions) {
      // Граница счётного такта — это ровно середина между приходами пакетов,
      // то есть середина настоящего такта сервера: дальше всего от его краёв,
      // куда бы отправка ни дрогнула.
      const at = baseTick + (a.after || 0);
      await waitTick(at, 0);
      const what = a.set ? `set ${a.set.at.join(',')} ${a.set.state}` : a.click ? `click ${a.click.join(',')}` : JSON.stringify(a);
      pendingDo = { what, tick: at };
      if (a.set) bot.chat(`/setblock ${ORIGIN.x + a.set.at[0]} ${ORIGIN.y + a.set.at[1]} ${ORIGIN.z + a.set.at[2]} ${a.set.state}`);
      if (a.click) {
        const block = bot.blockAt(new Vec3(ORIGIN.x + a.click[0], ORIGIN.y + a.click[1], ORIGIN.z + a.click[2]));

        // Повернуться к цели надо один раз: поворот занимает такты, и при
        // частых нажатиях он бы съедал промежутки между ними.
        if (!aimedAt || !aimedAt.equals(block.position)) {
          await bot.lookAt(block.position.offset(0.5, 0.5, 0.5), true);
          await sleepTicks(5);
          aimedAt = block.position;
        }

        clickBlock(block);
      }
    }
    await waitTick(baseTick + scenario.record_ticks + 10);
    recording = false;
    fs.writeFileSync(outFile, JSON.stringify({ scenario: scenario.name, host, port, events }, null, 1));
    console.log(`записано событий: ${events.length} → ${outFile}`);
    bot.quit();
    setTimeout(() => process.exit(0), 500);
  } catch (e) { console.error(e); process.exit(1); }
});
bot.on('kicked', (r) => { console.error('кик:', JSON.stringify(r)); process.exit(1); });
bot.on('error', (e) => { console.error('ошибка:', e.message); process.exit(1); });
bot.on('message', (m) => { const s = m.toString(); if (s && !/blackbox/.test(s)) console.error('  чат:', s); });
setTimeout(() => { console.error('время вышло'); process.exit(1); }, 120000);
