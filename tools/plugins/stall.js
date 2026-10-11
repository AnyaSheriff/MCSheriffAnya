// Проверка: плагин, который зацикливается, не роняет такты. Наблюдатель считает
// интервалы между пакетами времени (раз в 20 тактов), пока другой робот 30 раз
// подряд загружает misbehave и зацикливает его чатом.
//
//   node tools/plugins/stall.js <порт>
//
// Нужны права оператора у blackbox и blackbox2 (run-ours-vplugins.sh их выдаёт).
'use strict';
const path = require('path');
const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const port = Number(process.argv[2]);
const make = (username) => mineflayer.createBot({ host: '127.0.0.1', port, username, version: '26.1', auth: 'offline' });

(async () => {
  const watcher = make('blackbox2');
  const actor = make('blackbox');
  await Promise.all([new Promise((r) => watcher.once('spawn', r)), new Promise((r) => actor.once('spawn', r))]);
  await sleep(1500);
  const stamps = [];
  watcher._client.on('update_time', () => stamps.push(Date.now()));
  const lines = [];
  actor.on('messagestr', (text) => lines.push(text));
  const cycles = 30;
  let loaded = 0;
  let unloaded = 0;
  for (let i = 0; i < cycles; i++) {
    lines.length = 0;
    actor.chat('/plugman load misbehave');
    await sleep(300);
    if (lines.join('\n').includes('Загружен misbehave')) loaded++;
    const t0 = Date.now();
    actor.chat('loop');
    await sleep(700);
    lines.length = 0;
    actor.chat('/plugman list');
    await sleep(300);
    if (!lines.join('\n').includes('misbehave 0.1.0')) unloaded++;
  }
  await sleep(1200);
  const gaps = stamps.slice(1).map((t, i) => t - stamps[i]);
  gaps.sort((a, b) => a - b);
  const pct = (p) => gaps[Math.min(gaps.length - 1, Math.floor(gaps.length * p))];
  console.log(`циклов ${cycles}: загружено ${loaded}, выгружено ловушкой ${unloaded}`);
  console.log(`интервалов времени ${gaps.length}: мин ${gaps[0]} мс, медиана ${pct(0.5)}, p95 ${pct(0.95)}, макс ${gaps[gaps.length - 1]} (норма 1000)`);
  watcher.quit();
  actor.quit();
  await sleep(300);
  process.exit(0);
})().catch((e) => { console.log('сбой:', e); process.exit(3); });
