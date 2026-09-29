// Сравнение двух записей робота: «такт | оригинал | у нас».
// Использование: node compare.js <сценарий> [--record] — с --record сперва
// записывает обе стороны (оригинал на 25566, наш на 25567), потом сравнивает.
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
const continuous = JSON.parse(fs.readFileSync(scenario, 'utf8')).continuous === true;
// `--ours-port N` — сверять с нашей сборкой на другом порту (например, из
// копии проекта), не трогая сервер чёрного ящика на 25567.
const portFlag = process.argv.indexOf('--ours-port');
const oursPort = portFlag > 0 ? Number(process.argv[portFlag + 1]) : 25567;
const sides = [
  { title: 'оригинал', port: 25566, file: path.join(HERE, 'reports', `vanilla-${name}.json`) },
  { title: 'у нас',    port: oursPort, file: path.join(HERE, 'reports', `ours-${name}.json`) },
];
if (!fs.existsSync(path.join(HERE, 'reports'))) fs.mkdirSync(path.join(HERE, 'reports'));

// `--parallel` — писать обе стороны разом: серверы разные, роботы друг
// другу не мешают, а полный прогон идёт вдвое быстрее.
const record = async () => {
  const args = (s) => [path.join(HERE, 'bot.js'), '127.0.0.1', String(s.port), scenario, s.file];
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
    case 'spawn': return `сущность ${at} ${e.type} ${e.data}`;
    case 'destroy': return `убрана сущность`;
    case 'block_entity': return `данные блока ${at} ${e.type} ${e.data}`;
    case 'window': return `окно ${e.type}`;
    case 'slots': return `содержимое окна ${e.items}`;
    case 'slot': return `ячейка ${e.window === 'курсор' ? 'курсора' : e.slot}: ${e.item}`;
    case 'cursor': return `курсор: ${e.item}`;
    case 'level': return `событие мира ${at} ${e.effect}/${e.data}`;
    case 'property': return `свойство окна ${e.property}=${e.value}`;
    case 'close': return 'окно закрыто';
    case 'meta': return `свойства сущности ${e.fields}`;
    case 'cart': return describeCart(e);
    case 'explosion': return `взрыв ${at} сила ${e.radius} блоков ${e.blocks} ${e.particle} ${e.sound.replace(/^minecraft:/, '')} толчок ${e.knockback ? 'есть' : 'нет'} дым [${e.smoke}]`;
    default: return JSON.stringify(e);
  }
};
// Пакет о вагонетке: сдвиг (в 1/4096 блока), скорость, перенос, сверка места.
const describeCart = (e) => {
  const ground = e.ground ? ' на земле' : '';
  switch (e.what) {
    case 'move': return `вагонетка сдвиг ${e.d ? e.d.join(',') : ''}${e.yaw !== undefined ? ` поворот ${e.yaw}/${e.pitch}` : ''}${ground}`;
    case 'rot': return `вагонетка поворот ${e.yaw}/${e.pitch}${ground}`;
    case 'vel': return `вагонетка скорость ${e.v.join(',')}`;
    case 'tp': return `вагонетка перенос (${e.at.join(',')}) скорость ${e.v.join(',')}${ground}`;
    case 'sync': return `вагонетка сверка (${e.at.join(',')}) скорость ${e.v.join(',')} поворот ${e.yaw}/${e.pitch}${ground}`;
    case 'gone': return 'вагонетка убрана';
    case 'status': return `вагонетка событие ${e.status}`;
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
      : `spawn ${e.at.join(',')} ${e.type} ${e.data}`;
    case 'block_entity': return `block_entity ${e.at.join(',')} ${e.type}`;
    // Заголовок окна у сервера свой («Chest» или перевод) — сравниваем тип.
    case 'window': return `window ${e.type}`;
    case 'slots': return `slots ${e.items}`;
    case 'slot': return `slot ${e.window} ${e.slot} ${e.item}`;
    case 'cursor': return `cursor ${e.item}`;
    case 'level': return `level ${e.at.join(',')} ${e.effect} ${e.data}`;
    case 'property': return `property ${e.property} ${e.value}`;
    case 'meta': return `meta ${e.fields}`;
    case 'cart': return `cart ${describeCart(e)}`;
    // Динамит сносит вбок случайный толчок, а блоков взрыв рушит случайно:
    // сверяем клетку центра, силу, вспышку, звук и дым.
    case 'explosion': return `explosion ${e.at.map(Math.floor).join(',')} ${e.radius} ${e.particle} ${e.sound} ${e.smoke}`;
    default: return e.kind;
  }
};

const load = (file) => {
  const events = JSON.parse(fs.readFileSync(file, 'utf8')).events;
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
    const dt = dtOf(e, base);
    out.push({ step, dt, at: at(e), kind: e.kind, text: describe(e), key: key(e) });
  }
  return out;
};

const [a, b] = sides.map(s => load(s.file));

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
const steps = Math.max(...a.map(e => e.step), ...b.map(e => e.step)) + 1;
const lines = [];
let same = 0, extra = 0, missing = 0, shifted = 0;
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
  lines.push(`| | **${doA ? doA.text : ''}**${note} | **${doB ? doB.text : ''}** |`);
  const ea = A.filter(e => e.kind !== 'do'), eb = B.filter(e => e.kind !== 'do');
  // Сопоставление: то же событие в тот же такт — совпало; в другой такт —
  // сдвиг; иначе у нас его нет. Что осталось у нас без пары — лишнее.
  const taken = new Set();
  const rows = [];
  for (const x of ea) {
    let y = eb.find(e => !taken.has(e) && e.key === x.key && e.dt === x.dt);
    let mark = '';
    if (y) { same++; }
    else {
      y = eb.find(e => !taken.has(e) && e.key === x.key);
      if (y) { shifted++; mark = y.dt > x.dt ? `у нас позже на ${y.dt - x.dt}` : `у нас раньше на ${x.dt - y.dt}`; }
      else { missing++; mark = 'нет у нас'; }
    }
    if (y) taken.add(y);
    rows.push({ dt: x.dt, left: x.text, right: y ? y.text : '', mark, order: [ea.indexOf(x), y ? eb.indexOf(y) : -1] });
  }
  for (const y of eb) if (!taken.has(y)) { extra++; rows.push({ dt: y.dt, left: '', right: y.text, mark: 'лишнее', order: [1e9, eb.indexOf(y)] }); }
  rows.sort((p, q) => p.dt - q.dt || p.order[0] - q.order[0]);
  // Порядок внутри такта тоже важен (действие раньше состояния и т. п.).
  for (let i = 0; i < rows.length; i++) {
    const r = rows[i];
    if (!r.mark && r.right) {
      const before = rows.slice(0, i).filter(q => q.dt === r.dt && !q.mark && q.right);
      const batchy = r.left.includes('[пачкой]');
      if (before.some(q => q.order[1] > r.order[1] && !(batchy && q.left.includes('[пачкой]')))) r.mark = 'порядок в такте другой';
    }
    lines.push(`| +${r.dt} | ${pad(r.left, W)} | ${pad(r.right + (r.mark ? `  ← ${r.mark}` : ''), W)} |`);
  }
}
lines.push('');
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
