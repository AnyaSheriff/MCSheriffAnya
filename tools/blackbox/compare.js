// Сравнение двух записей робота: «такт | оригинал | у нас».
// Использование: node compare.js <сценарий> [--record] — с --record сперва
// записывает обе стороны (оригинал на 25566, наш на 25567), потом сравнивает;
// `--files A B` сравнивает две готовые записи (оригинал с самим собой).
'use strict';
const fs = require('fs');
const path = require('path');
const { spawn, spawnSync } = require('child_process');

const name = process.argv[2];
if (!name) { console.error('использование: node compare.js <сценарий> [--record]'); process.exit(2); }
const HERE = __dirname;
const scenario = path.join(HERE, 'scenarios', `${name}.json`);
// Сценарий с частыми действиями («continuous») читается как одно целое:
// отклики на соседние нажатия перекрываются, и делить запись по нажатиям
// нельзя — такты считаются от первого отклика на всю запись.
const scenarioJson = JSON.parse(fs.readFileSync(scenario, 'utf8'));
const continuous = scenarioJson.continuous === true;
// Случайное у оригинала (`random` в сценарии): список правил, каждое со своим
// обоснованием (`why`). Правила действуют только в своём сценарии — общих
// поблажек нет. Что попало в допуск, считается совпавшим, но показывается
// отдельно. Разбор правил — ниже, у `applyRandom`.
const randomRules = Array.isArray(scenarioJson.random) ? scenarioJson.random : [];
// `--ours-port N` — сверять с нашей сборкой на другом порту (например, из
// копии проекта), не трогая сервер чёрного ящика на 25567.
const portFlag = process.argv.indexOf('--ours-port');
const oursPort = portFlag > 0 ? Number(process.argv[portFlag + 1]) : 25567;
// `--vanilla-port N` (или VANILLA_PORT) — оригинал на другом порту.
const vpFlag = process.argv.indexOf('--vanilla-port');
const vanillaPort = vpFlag > 0 ? Number(process.argv[vpFlag + 1]) : Number(process.env.VANILLA_PORT || 25566);
const sides = [
  { title: 'оригинал', port: vanillaPort, file: path.join(HERE, 'reports', `vanilla-${name}.json`) },
  { title: 'у нас',    port: oursPort, file: path.join(HERE, 'reports', `ours-${name}.json`) },
];
// `--files A B` — сравнить две готовые записи, ничего не записывая: так
// сверяются два прогона оригинала между собой (что у него случайно).
const filesFlag = process.argv.indexOf('--files');
if (filesFlag > 0) {
  sides[0].file = path.resolve(process.argv[filesFlag + 1]);
  sides[1].file = path.resolve(process.argv[filesFlag + 2]);
}
if (!fs.existsSync(path.join(HERE, 'reports'))) fs.mkdirSync(path.join(HERE, 'reports'));

// `--lane N` — писать полосой N: робот `blackboxN` на своей площадке
// (bot.js), чтобы несколько сценариев писались разом (lanes.js).
const laneFlag = process.argv.indexOf('--lane');
const lane = laneFlag > 0 ? String(Number(process.argv[laneFlag + 1])) : '1';

// `--parallel` — писать обе стороны разом: серверы разные, роботы друг
// другу не мешают, а полный прогон идёт вдвое быстрее.
const record = async () => {
  const args = (s) => [path.join(HERE, 'bot.js'), '127.0.0.1', String(s.port), scenario, s.file, lane];
  if (!process.argv.includes('--parallel')) {
    for (const s of sides) {
      const r = spawnSync('node', args(s), { stdio: ['ignore', 'inherit', 'pipe'] });
      if (r.status !== 0) { console.error(`запись «${s.title}» не удалась:\n${r.stderr}`); process.exit(1); }
    }
    return;
  }
  const runs = sides.map((s) => new Promise((done) => {
    const child = spawn('node', args(s), { stdio: ['ignore', 'inherit', 'pipe'] });
    let stderr = '';
    child.stderr.on('data', (chunk) => { stderr += chunk; });
    child.on('close', (status) => done({ s, status, stderr }));
  }));
  for (const { s, status, stderr } of await Promise.all(runs)) {
    if (status !== 0) { console.error(`запись «${s.title}» не удалась:\n${stderr}`); process.exit(1); }
  }
};

// Сравнение — после записи.
const compare = () => {
// Событие → строка без лишних дробей; такт — относительно последнего действия.
const describe = (e) => {
  const at = e.at ? `(${e.at.join(',')})` : '';
  switch (e.kind) {
    case 'block': return `блок ${at} ${e.state}${e.batch ? ' [пачкой]' : ''}`;
    case 'action': return `действие ${at} ${e.block} ${e.a}/${e.b}`;
    case 'sound': return `звук ${at} ${e.sound.replace(/^minecraft:/, '')} ${e.volume.toFixed(2)} ~${e.pitch.toFixed(2)}`;
    case 'particle': return `частица ${at} ${e.particle} ${e.count} шт. скорость ${e.speed} (${e.offset})`;
    case 'spawn': return `сущность ${at} ${e.type} ${e.data}${e.extra ? ' ' + e.extra : ''}`;
    case 'destroy': return `убрана сущность`;
    case 'block_entity': return `данные блока ${at} ${e.type} ${e.data}`;
    case 'sign_editor': return `окно правки таблички ${at} ${e.front ? 'спереди' : 'сзади'}`;
    case 'window': return `окно ${e.type}`;
    case 'slots': return `содержимое окна ${e.items}`;
    case 'slot': return `ячейка ${e.window === 'курсор' ? 'курсора' : e.slot}: ${e.item}`;
    case 'cursor': return `курсор: ${e.item}`;
    case 'level': return `событие мира ${at} ${e.effect}/${e.data}`;
    case 'property': return `свойство окна ${e.property}=${e.value}`;
    case 'close': return 'окно закрыто';
    case 'inv': return `инвентарь ${e.slot}: ${e.item}`;
    case 'meta': return `свойства сущности ${e.fields}`;
    case 'cart': return describeCart(e);
    case 'sv': return `робот: ${e.text}`;
    case 'sc': return `табло: ${e.text}`;
    case 'self': return e.what === 'pos' ? `робот: сверка места (${e.at.join(',')}) ${e.flags}` : `робот: скорость ${e.v.join(',')}`;
    case 'explosion': return `взрыв ${at} сила ${e.radius} блоков ${e.blocks} ${e.particle} ${e.sound.replace(/^minecraft:/, '')} толчок ${e.knockback ? 'есть' : 'нет'} дым [${e.smoke}]`;
    default: return JSON.stringify(e);
  }
};
// Пакет о вагонетке: сдвиг (в 1/4096 блока), скорость, перенос, сверка места.
const describeCart = (e) => {
  const ground = e.ground ? ' на земле' : '';
  const who = e.ent || 'вагонетка';
  switch (e.what) {
    case 'move': return `${who} сдвиг ${e.d ? e.d.join(',') : ''}${e.yaw !== undefined ? ` поворот ${e.yaw}/${e.pitch}` : ''}${ground}`;
    case 'rot': return `${who} поворот ${e.yaw}/${e.pitch}${ground}`;
    case 'vel': return `${who} скорость ${e.v.join(',')}`;
    case 'tp': return `${who} перенос (${e.at.join(',')}) скорость ${e.v.join(',')}${ground}`;
    case 'sync': return `${who} сверка (${e.at.join(',')}) скорость ${e.v.join(',')} поворот ${e.yaw}/${e.pitch}${ground}`;
    case 'gone': return e.ent ? `${e.ent} убран` : 'вагонетка убрана';
    case 'status': return `${who} событие ${e.status}`;
    case 'text': return `${who} ${e.text}`;
    default: return JSON.stringify(e);
  }
};
// Что считается «тем же самым» при сопоставлении: у звука высота случайна,
// у пачки — способ доставки не важен.
const key = (e) => {
  // порядок внутри пачки не важен: это одно сообщение

  switch (e.kind) {
    case 'block': return `block ${e.at.join(',')} ${e.state}`;
    case 'action': return `action ${e.at.join(',')} ${e.block} ${e.a}/${e.b}`;
    case 'sound': return `sound ${e.at.join(',')} ${e.sound} ${e.volume.toFixed(2)}`;
    case 'particle': return `частица ${e.at.join(',')} ${e.particle} ${e.count} ${e.speed} (${e.offset})`;
    // Выпавший предмет у оригинала появляется в случайной точке блока
    // (±0,25) — сравниваем по самому блоку, а не по точке.
    case 'spawn': return e.type === 'item'
      ? `spawn item ${e.at.map(Math.floor).join(',')}`
      : `spawn ${e.at.join(',')} ${e.type} ${e.data}${e.extra ? ' ' + e.extra : ''}`;
    case 'block_entity': return `block_entity ${e.at.join(',')} ${e.type}`;
    // Заголовок окна у сервера свой («Chest» или перевод) — сравниваем тип.
    case 'sign_editor': return `sign_editor ${e.at.join(',')} ${e.front}`;
    case 'window': return `window ${e.type}`;
    case 'slots': return `slots ${e.items}`;
    case 'slot': return `slot ${e.window} ${e.slot} ${e.item}`;
    case 'cursor': return `cursor ${e.item}`;
    case 'level': return `level ${e.at.join(',')} ${e.effect} ${e.data}`;
    case 'property': return `property ${e.property} ${e.value}`;
    case 'meta': return `meta ${e.fields}`;
    case 'cart': return `cart ${describeCart(e)}`;
    case 'sv': return `sv ${e.text}`;
    case 'sc': return `sc ${e.text}`;
    case 'self': return `self ${describe(e)}`;
    // Динамит сносит вбок случайный толчок, а блоков взрыв рушит случайно:
    // сверяем клетку центра, силу, вспышку, звук и дым.
    case 'explosion': return `explosion ${e.at.map(Math.floor).join(',')} ${e.radius} ${e.particle} ${e.sound} ${e.smoke}`;
    default: return e.kind;
  }
};

const load = (file) => {
  const saved = JSON.parse(fs.readFileSync(file, 'utf8'));
  // Окно записи (`window` от bot.js): что пришло после такта `last`, робот
  // слушал лишь затем, чтобы не потерять сам последний такт. Такое
  // отбрасывается у обеих сторон одинаково — по такту сервера от начала.
  const last = saved.window ? saved.window.last : null;
  const events = saved.events.filter(e => last === null || e.tick === null || e.tick === undefined || e.tick <= last);
  const out = [];
  let step = -1, base = null;
  // Время события — номер такта сервера, а не часы робота: робот пишет такт
  // по «обновлению времени», и события одного такта получают один номер.
  // Старые записи без такта считаем по миллисекундам, как раньше.
  const at = (e) => (e.tick === null || e.tick === undefined ? e.ms / 50 : e.tick);
  const dtOf = (e, b) => Math.round(at(e) - at(b));
  for (const e of events) {
    if (e.kind === 'do') {
      if (continuous) {
        // Нажатия остаются в общем ряду: по ним видно, что робот нажимал
        // в те же такты на обоих серверах.
        if (!base) { base = e; step = 0; }
        out.push({ step, dt: dtOf(e, base), at: at(e), kind: 'press', text: `▶ ${e.what}`, key: `press ${e.what}` });
        continue;
      }

      step++; base = e; out.push({ step, dt: 0, at: at(e), kind: 'do', text: `▶ ${e.what}` }); continue;
    }
    if (e.kind === 'destroy') continue;
    // Пришедшее до первого действия (очередной отчёт о предмете, что лежит
    // с подготовки) ни к какому действию не относится — не сверяется.
    if (!base) continue;
    const dt = dtOf(e, base);
    out.push({ step, dt, at: at(e), kind: e.kind, text: describe(e), key: key(e), raw: e });
  }
  return out;
};

const [a, b] = sides.map(s => load(s.file));

// Правила случайного (`random` в сценарии). Каждое — объект с `type`, `name`
// (как оно подписано в отчёте) и `why` (почему это случайно у оригинала:
// цитата вики или замер «оригинал против оригинала»). Виды:
//
// - `values`: поле события случайно, но из известного набора. `kind` — вид
//   события, `match` — (необязательно) выражение по его описанию, `field` —
//   поле записи, `values` — допустимые значения или `range` — [от, до].
//   Совпадает всё остальное и такт; значение у нас должно быть допустимым.
// - `delay`: случайная задержка между двумя событиями (запал динамита).
//   `from`, `to` — выражения по ключу события; `to` ищется строго после
//   `from`. Задержка у нас должна лежать в `range` = [от, до] тактов; тогда
//   всё наше от `to` и дальше сверяется со сдвигом на разницу задержек, точно
//   по тактам относительно `to`. `between` — (необязательно) выражение по
//   ключу для событий, что идут каждый такт, пока длится задержка (отсчёт
//   запала): их число зависит от задержки, лишние с любой стороны в допуске,
//   а общие сверяются по тактам относительно `to`.
//   `shift_between: false` — события ожидания не сдвигать, а сверять по
//   тактам от `from` (движение вагонетки после толчка идёт от первого взрыва,
//   а не ко второму).
// - `mask`: часть ключа случайна, но само событие — нет. `kind` — вид
//   события, `pattern` — выражение по ключу, `replace` — на что заменить
//   (урон вагонетки от взрыва случайной силы). Такт и остальное — точно.
// - `count`: события по выражению `match` (по ключу) случайны целиком — их
//   место, громкость, такт (всплески выпавших предметов в воде) или само
//   наличие (семена из травы). Сверяется только их число: у нас оно должно
//   лежать в `range` = [от, до]. Совпавшие по ключу и такту сверяются как
//   обычно, остальные — в допуске. Вне диапазона допуска нет. С `anytick:
//   true` такт у них тоже не сверяется.
// - `window`: событие приходит в случайный такт из небольшого промежутка
//   (оригинал против оригинала расходится на такт-другой от фазы, с какой
//   робот начал бег). `match` — выражение по ключу, `range` = [от, до] — такт
//   от первого отклика шага. Событие с обеих сторон должно лежать в промежутке,
//   тогда оно в допуске; вне него допуска нет.
// - `anytick`: событие случайного такта (гаснущая руда на случайном такте):
//   ключ совпадает, такт любой, или его нет совсем. У нас все такие события
//   одного ключа должны прийти в один такт — иначе допуска нет.
// - `jitter`: событие то же самое, но его такт дрожит на `max` тактов
//   (свойство с частицами эффектов уходит в такте слежения: от места команды
//   в такте на +0 или +1). Ключ должен совпасть целиком; пара со сдвигом не
//   больше `max` — в допуске, событие без пары — как обычно «нет у нас» или
//   «лишнее» (в отличие от `count`, само событие и его содержимое сверяются).
//
// Возвращает предупреждения: если сам оригинал вышел за допуск, правило
// неверно и его надо пересмотреть.
function applyRandom(A, B) {
  const warnings = [];
  for (const e of [...A, ...B]) e.origDt = e.dt;
  for (const rule of randomRules) {
    const name = rule.name || rule.type;
    if (rule.type === 'values') {
      const re = rule.match ? new RegExp(rule.match) : null;
      const allowed = (v) => (rule.values ? rule.values.includes(v) : v >= rule.range[0] && v <= rule.range[1]);
      for (const [side, list] of [['a', A], ['b', B]]) {
        for (const e of list) {
          if (e.kind !== rule.kind || !e.raw || e.raw[rule.field] === undefined || (re && !re.test(e.text))) continue;
          const ok = allowed(e.raw[rule.field]);
          if (!ok && side === 'a') warnings.push(`у оригинала «${e.text}»: ${rule.field}=${e.raw[rule.field]} вне допуска «${name}»`);
          if (!ok && side === 'b') continue;
          const canon = { ...e.raw, [rule.field]: rule.values ? rule.values[0] : rule.range[0] };
          e.key = key(canon);
          e.rule = name;
        }
      }
    } else if (rule.type === 'delay') {
      const from = new RegExp(rule.from), to = new RegExp(rule.to);
      const between = rule.between ? new RegExp(rule.between) : null;
      const steps = new Set([...A, ...B].map(e => e.step));
      for (const st of steps) {
        const find = (list) => {
          const ev = list.filter(e => e.step === st && e.kind !== 'do');
          const i = ev.findIndex(e => from.test(e.key));
          if (i < 0) return null;
          const j = ev.findIndex((e, k) => k > i && e.dt > ev[i].dt && to.test(e.key));
          return j < 0 ? null : { ev, t1: ev[i].dt, t2: ev[j].dt, i, j };
        };
        const fa = find(A), fb = find(B);
        if (!fa || !fb) continue;
        const inRange = (f) => f.t2 - f.t1 >= rule.range[0] && f.t2 - f.t1 <= rule.range[1];
        if (!inRange(fa)) warnings.push(`у оригинала задержка «${name}» ${fa.t2 - fa.t1} такт. вне [${rule.range.join(', ')}]`);
        if (!inRange(fb)) continue;
        const shift = (fa.t2 - fa.t1) - (fb.t2 - fb.t1);
        // Ожидание — всё между `from` и `to` по порядку прихода, включая
        // пакеты такта самого `to`, пришедшие до него.
        const waiting = (e, f) => between && f.ev.indexOf(e) < f.j && between.test(e.key);
        // События ожидания без пары в свой такт в допуске, но в пару
        // с другим тактом не встают: их такты случайны.
        for (const e of fa.ev.slice(fa.i + 1)) if (waiting(e, fa)) { e.free = name; e.rule = name; e.nopair = true; }
        for (const e of fb.ev.slice(fb.i + 1)) {
          const w = waiting(e, fb);
          if (!w && e.dt < fb.t2) continue;
          if (w) { e.free = name; e.nopair = true; }
          if (!w || rule.shift_between !== false) e.dt += shift;
          e.rule = name;
        }
      }
    } else if (rule.type === 'mask') {
      const re = new RegExp(rule.pattern, 'g');
      for (const e of [...A, ...B]) {
        if (e.kind !== rule.kind || !re.test(e.key)) continue;
        re.lastIndex = 0;
        e.key = e.key.replace(re, rule.replace);
        e.rule = name;
      }
    } else if (rule.type === 'count') {
      const re = new RegExp(rule.match);
      const [ma, mb] = [A, B].map(list => list.filter(e => re.test(e.key)));
      const inRange = (n) => n >= rule.range[0] && n <= rule.range[1];
      if (!inRange(ma.length)) warnings.push(`у оригинала «${name}»: ${ma.length} шт., вне [${rule.range.join(', ')}]`);
      if (!inRange(mb.length)) { warnings.push(`у нас «${name}»: ${mb.length} шт., вне [${rule.range.join(', ')}] — без допуска`); continue; }
      for (const e of [...ma, ...mb]) {
        e.free = name; e.rule = name;
        if (rule.anytick) e.anytick = name;
      }
    } else if (rule.type === 'window') {
      const re = new RegExp(rule.match);
      for (const [side, list] of [['a', A], ['b', B]]) {
        for (const e of list) {
          if (!re.test(e.key)) continue;
          const ok = e.dt >= rule.range[0] && e.dt <= rule.range[1];
          if (!ok && side === 'a') warnings.push(`у оригинала «${e.text}»: такт ${e.dt} вне [${rule.range.join(', ')}] («${name}»)`);
          if (!ok) continue;
          e.dt = rule.range[0];
          e.free = name; e.rule = name;
        }
      }
    } else if (rule.type === 'jitter') {
      const re = new RegExp(rule.match);
      for (const e of [...A, ...B]) if (re.test(e.key)) { e.jitter = { name, max: rule.max ?? 1 }; e.rule = name; }
    } else if (rule.type === 'anytick') {
      const re = new RegExp(rule.match);
      for (const e of A) if (re.test(e.key)) { e.anytick = name; e.free = name; e.rule = name; }
      const byKey = new Map();
      for (const e of B) if (re.test(e.key)) byKey.set(e.key, [...(byKey.get(e.key) || []), e]);
      for (const list of byKey.values()) {
        if (new Set(list.map(e => `${e.step}/${e.dt}`)).size > 1) continue;
        for (const e of list) { e.anytick = name; e.free = name; e.rule = name; }
      }
    } else {
      warnings.push(`неизвестное правило случайного «${rule.type}»`);
    }
  }
  return warnings;
}

// Отклик на нажатие приходит через такт-другой, и у робота этот такт
// «плавает»: границы тактов у двух серверов не совпадают, а нажатие ложится
// в случайное место такта. Поэтому внутри шага такты считаются от первого
// ответа сервера, а сама задержка отклика показывается отдельно.
const latency = { a: [], b: [] };
for (const [side, list] of [['a', a], ['b', b]]) {
  const steps = new Set(list.map(e => e.step));
  for (const st of steps) {
    const first = list.find(e => e.step === st && e.kind !== 'do');
    if (!first) continue;
    latency[side][st] = first.dt;
    for (const e of list) if (e.step === st && e.kind !== 'do') e.dt = Math.round(e.at - first.at);
  }
}
const warnings = applyRandom(a, b);
const steps = Math.max(...a.map(e => e.step), ...b.map(e => e.step)) + 1;
const lines = [];
let same = 0, extra = 0, missing = 0, shifted = 0;
// Сколько совпавших вошло лишь по допуску случайного — по правилам.
const tolerated = new Map();
const tolerate = (rule) => { same++; tolerated.set(rule, (tolerated.get(rule) || 0) + 1); return `случайное, в допуске: ${rule}`; };
const pad = (s, n) => s + ' '.repeat(Math.max(0, n - [...s].length));
const W = 62;
lines.push(`# ${name}: оригинал против нас`);
lines.push('');
lines.push(`| такт | ${pad('оригинал', W)} | ${pad('у нас', W)} |`);
lines.push(`|------|${'-'.repeat(W + 2)}|${'-'.repeat(W + 2)}|`);
for (let s = 0; s < steps; s++) {
  const A = a.filter(e => e.step === s), B = b.filter(e => e.step === s);
  const doA = A.find(e => e.kind === 'do'), doB = B.find(e => e.kind === 'do');
  const la = latency.a[s], lb = latency.b[s];
  const note = la === undefined || lb === undefined ? '' : ` (первый отклик через ${la} / ${lb} такт.)`;
  // Такты внутри шага считаются от первого отклика — значит, опоздай весь
  // отклик у нас, сами события бы совпали. Поэтому задержка отклика
  // сверяется отдельно: разница в такт — это ещё дрожь сети (нажатие
  // приходит на границу такта), больше — сдвиг (plate-items: плита
  // отпускалась на 9 тактов позже, а события совпадали).
  const late = la !== undefined && lb !== undefined && Math.abs(la - lb) > 1;
  if (late) shifted++;
  const lateNote = late ? `  ← отклик у нас ${lb > la ? 'позже' : 'раньше'} на ${Math.abs(lb - la)}` : '';
  lines.push(`| | **${doA ? doA.text : ''}**${note} | **${doB ? doB.text : ''}**${lateNote} |`);
  const ea = A.filter(e => e.kind !== 'do'), eb = B.filter(e => e.kind !== 'do');
  // Сопоставление: то же событие в тот же такт — совпало; в другой такт —
  // сдвиг; иначе у нас его нет. Что осталось у нас без пары — лишнее.
  const taken = new Set();
  const rows = [];
  // Сперва пары «тот же ключ, тот же такт», потом остальные: иначе событие,
  // что повторяется (отсчёт запала), встаёт в пару со сдвигом, хотя точная
  // пара у него есть.
  const exact = new Map();
  for (const x of ea) {
    const y = eb.find(e => !taken.has(e) && e.key === x.key && e.dt === x.dt);
    if (y) { exact.set(x, y); taken.add(y); }
  }
  for (const x of ea) {
    let y = exact.get(x);
    let mark = '';
    if (!y) y = eb.find(e => !taken.has(e) && e.key === x.key && !x.nopair && !e.nopair);
    if (y && y.dt === x.dt) {
      // Совпало по ключу и такту. Если правило подменило случайное поле
      // или такт — это совпадение по допуску, его видно отдельно.
      const rule = (x.text !== y.text || x.origDt !== y.origDt) && (y.rule || x.rule);
      if (rule) mark = tolerate(rule); else same++;
    } else {
      // Случайный такт (`anytick`): то же событие в любой такт окна.
      if (y && x.anytick && y.anytick) mark = tolerate(y.anytick);
      else if (y && x.jitter && y.jitter && Math.abs(y.dt - x.dt) <= x.jitter.max) mark = tolerate(x.jitter.name);
      else if (y) { shifted++; mark = y.dt > x.dt ? `у нас позже на ${y.dt - x.dt}` : `у нас раньше на ${x.dt - y.dt}`; }
      else if (x.free) mark = `${tolerate(x.free)} (у нас не было)`;
      else { missing++; mark = 'нет у нас'; }
    }
    if (y) taken.add(y);
    rows.push({ dt: x.dt, left: x.text, right: y ? y.text : '', mark, order: [ea.indexOf(x), y ? eb.indexOf(y) : -1], sameTick: Boolean(y) && y.dt === x.dt });
  }
  for (const y of eb) {
    if (taken.has(y)) continue;
    if (y.free) { rows.push({ dt: y.dt, left: '', right: y.text, mark: `${tolerate(y.free)} (у оригинала не было)`, order: [1e9, eb.indexOf(y)] }); continue; }
    extra++; rows.push({ dt: y.dt, left: '', right: y.text, mark: 'лишнее', order: [1e9, eb.indexOf(y)] });
  }
  rows.sort((p, q) => p.dt - q.dt || p.order[0] - q.order[0]);
  // Порядок внутри такта тоже важен (действие раньше состояния и т. п.).
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i];
    const ok = (q) => !q.mark || q.mark.startsWith('случайное');
    if (ok(r) && r.sameTick) {
      const before = rows.slice(0, i).filter(q => q.dt === r.dt && ok(q) && q.sameTick);
      const batchy = r.left.includes('[пачкой]');
      if (before.some(q => q.order[1] > r.order[1] && !(batchy && q.left.includes('[пачкой]')))) r.mark = r.mark ? `${r.mark}; порядок в такте другой` : 'порядок в такте другой';
    }
    lines.push(`| +${r.dt} | ${pad(r.left, W)} | ${pad(r.right + (r.mark ? `  ← ${r.mark}` : ''), W)} |`);
  }
}
lines.push('');
for (const w of warnings) lines.push(`Внимание: ${w}`);
if (tolerated.size) {
  const total = [...tolerated.values()].reduce((p, q) => p + q, 0);
  lines.push(`Случайное, в допуске: ${total} (входит в «Совпало»): ${[...tolerated].map(([r, n]) => `${r} — ${n}`).join('; ')}.`);
}
lines.push(`Совпало: ${same}. Сдвинуто по тактам: ${shifted}. Нет у нас: ${missing}. Лишнее у нас: ${extra}.`);
const out = lines.join('\n');
console.log(out);
const stamp = new Date().toISOString().slice(0, 16).replace('T', '-').replace(':', '');
fs.writeFileSync(path.join(HERE, 'reports', `${name}-${stamp}.md`), out + '\n');
};

(async () => {
  if (process.argv.includes('--record')) await record();
  compare();
})();
