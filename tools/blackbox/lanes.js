// Прогон полосами: несколько роботов пишут разные сценарии разом, каждый на
// своей площадке (bot.js, полоса). Здесь — что можно гонять разом, а что нет,
// и выдача прав роботам полос.
//
//   node lanes.js plan          — по строке на сценарий: «par <имя>» (можно
//                                 разом, самые долгие первыми) или «excl <имя>
//                                 <причина>» (по одному, в конце, полосой 1);
//   node lanes.js op <порт>…    — выдать права blackbox2…5 командой op от
//                                 имени blackbox (у нас права выдаются только
//                                 тем, кто на сервере, — все заходят разом).
'use strict';
const fs = require('fs');
const path = require('path');

const LANES = 5;
const SCENARIOS = path.join(__dirname, 'scenarios');

// Что меняет мир целиком, а не свою площадку: такие сценарии портят записи
// соседей (или соседи — их) и идут по одному. Кроме явного `"exclusive": true`
// — по содержимому команд.
const GLOBAL = [
  [/\btime (set|add)\b/, 'время суток общее на весь мир'],
  [/\bweather\b/, 'погода общая'],
  [/\bgamerule\b/, 'правила игры общие'],
  [/\blightning_bolt\b/, 'молния: гром и вспышка неба'],
  [/\bexecute in\b|\bthe_nether\b/, 'другое измерение'],
  [/\bsetworldspawn\b|\bspawnpoint\b/, 'точка появления'],
  [/\bdifficulty\b/, 'сложность общая'],
  [/\b(scoreboard|team|bossbar|trigger|function|schedule|datapack)\b/, 'табло, команды и функции общие на весь мир'],
  [/\b(worldborder|tick (rate|freeze|unfreeze|step|sprint))\b/, 'рамка мира и скорость такта общие'],
  [/@a\b/, '@a задевает роботов других полос'],
  // @e без предела расстояния задевает чужие площадки. У нас `tp @e[…]`
  // предела не понимает вовсе, поэтому такие команды и остаются без него.
  [/@[en](?!\[[^\]]*distance=)/, '@e без distance задевает чужие площадки'],
];

const classify = (scenario) => {
  const why = [];
  if (scenario.exclusive === true) why.push('помечен exclusive');
  // Своё место в мире (`origin`) задано ради самого места: полоса его не
  // сдвигает, и две такие площадки могут лечь рядом (у dust-two-pistons-1 и -3
  // всего 72 блока).
  if (scenario.origin) why.push(`своё место в мире ${JSON.stringify(scenario.origin)}`);
  const cmds = [...(scenario.setup || []), ...(scenario.cleanup || []), ...(scenario.actions || []).filter(a => a.cmd).map(a => a.cmd)];
  for (const [re, reason] of GLOBAL) if (cmds.some(c => re.test(c)) && !why.includes(reason)) why.push(reason);
  return why;
};

// Сколько тактов примерно длится запись: самые долгие ставим в очередь
// первыми, чтобы полосы кончали вместе.
const estimate = (s) => 2 * ((s.blocks || []).length + (s.setup || []).length + (s.cleanup || []).length)
  + Math.max(0, ...(s.actions || []).map(a => a.after || 0)) + (s.record_ticks || 0);

const plan = () => {
  const par = [], excl = [];
  for (const file of fs.readdirSync(SCENARIOS).filter(f => f.endsWith('.json')).sort()) {
    const name = file.replace(/\.json$/, '');
    const s = JSON.parse(fs.readFileSync(path.join(SCENARIOS, file), 'utf8'));
    const why = classify(s);
    if (why.length) excl.push(`excl ${name} ${why.join('; ')}`);
    else par.push({ name, t: estimate(s) });
  }
  par.sort((a, b) => b.t - a.t || a.name.localeCompare(b.name));
  for (const p of par) console.log(`par ${p.name}`);
  for (const e of excl) console.log(e);
};

const op = async (ports) => {
  const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
  const join = (port, username) => new Promise((ok, fail) => {
    const bot = mineflayer.createBot({ host: '127.0.0.1', port, username, version: '26.1', auth: 'offline' });
    bot.once('spawn', () => ok(bot));
    bot.once('kicked', (r) => fail(new Error(`${username}@${port}: кик ${JSON.stringify(r)}`)));
    bot.once('error', (e) => fail(new Error(`${username}@${port}: ${e.message}`)));
  });
  for (const port of ports) {
    const others = [];
    for (let lane = 2; lane <= LANES; lane++) others.push(await join(port, `blackbox${lane}`));
    const main = await join(port, 'blackbox');
    await new Promise(r => setTimeout(r, 1000));
    for (let lane = 2; lane <= LANES; lane++) {
      main.chat(`/op blackbox${lane}`);
      await new Promise(r => setTimeout(r, 200));
    }
    await new Promise(r => setTimeout(r, 1000));
    for (const b of [main, ...others]) b.quit();
    await new Promise(r => setTimeout(r, 1000));
    console.log(`права выданы: ${port}`);
  }
  process.exit(0);
};

const [,, cmd, ...rest] = process.argv;
if (cmd === 'plan') plan();
else if (cmd === 'op' && rest.length) op(rest.map(Number)).catch((e) => { console.error(e.message); process.exit(1); });
else { console.error('использование: node lanes.js plan | op <порт>…'); process.exit(2); }
setTimeout(() => { console.error('время вышло'); process.exit(1); }, 60000).unref();
