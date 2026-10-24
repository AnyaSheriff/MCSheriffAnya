// Сырые пакеты проходят настоящие обработчики bot.js без сетевой библиотеки.
// Декодер наш; фильтры, строки и округления остаются общими с чёрным ящиком.
'use strict';
const fs = require('fs');
const path = require('path');
const vm = require('vm');
const { EventEmitter } = require('events');
const [,, input, scenarioFile, dataDir, output] = process.argv;
const saved = JSON.parse(fs.readFileSync(input));
const tables = {};
for (const name of ['items', 'entities', 'blocks']) tables[name] = JSON.parse(fs.readFileSync(path.join(dataDir, name + '.json')));
const index = rows => Object.fromEntries(rows.map(r => [r.id, r]));
const data = { items: index(tables.items), entities: index(tables.entities), blocks: index(tables.blocks), blocksByStateId: {} };
for (const block of tables.blocks) for (let id = block.minStateId; id <= block.maxStateId; id++) data.blocksByStateId[id] = block;
const bots = saved.initial.map((initial, i) => {
  const bot = new EventEmitter();
  bot._client = new EventEmitter();
  bot._client.write = () => {};
  bot.entity = { id: initial.entity };
  bot.entities = {};
  return bot;
});
for (const bot of bots) for (let i = 0; i < bots.length; i++) bot.entities[bots[i].entity.id] = { type: 'player', username: i ? 'blackboxb' : 'blackbox' };
let nextBot = 0, now = 0;
const original = fs.readFileSync(path.join(__dirname, 'bot.js'), 'utf8');
const boundary = "bot.once('spawn', async () => {";
if (!original.includes(boundary)) throw new Error('bot.js replay boundary missing');
const source = original.split(boundary)[0];
const sandbox = { console, Buffer, setTimeout, process: { argv: ['node', 'bot.js', 'localhost', '26100', scenarioFile, output], env: { HOME: process.env.HOME } }, __dirname,
  Date: { now: () => now }, require: name => {
    if (name.endsWith('/mineflayer')) return { createBot: () => bots[nextBot++] };
    if (name.endsWith('/minecraft-data')) return () => data;
    if (name.endsWith('/vec3')) return { Vec3: class {} };
    // Наш Decoder уже вернул обычные значения NBT без оболочек типов.
    if (name === 'prismarine-nbt') return { simplify: value => value };
    if (name === 'fs') return { ...fs, readFileSync: (file, ...args) => String(file) === scenarioFile && saved.origin ? JSON.stringify({ ...JSON.parse(fs.readFileSync(file, 'utf8')), origin: saved.origin }) : String(file).endsWith('/sounds.json') && saved.sounds ? JSON.stringify(saved.sounds) : fs.readFileSync(file, ...args) };
    return require(name);
  } };
vm.createContext(sandbox);
vm.runInContext(source, sandbox, { filename: 'bot.js' });
vm.runInContext('recording = true; fixedBase = 0; t0 = 0;', sandbox);
const rows = [...saved.packets.map(p => ({ ...p, at: (p.at - saved.start) * 1000 })),
  ...saved.sent.map(p => ({ at: (p.at - saved.start) * 1000, action: p.action, tick: p.tick }))].sort((a, b) => a.at - b.at);
for (const row of rows) {
  now = row.at;
  vm.runInContext(`recording = ${now >= 0};`, sandbox);
  if (row.action) {
    sandbox.press = row;
    // Текст действия тот же на обеих сторонах; семантику задаёт исходный JSON.
    vm.runInContext("events.push({kind:'do', tick:press.tick, ms:press.at, what:JSON.stringify(press.action)});", sandbox);
    continue;
  }
  const bot = bots[row.client];
  const p = row.value;
  if (row.outbound) { bot._client.write(row.name, p); continue; }
  if (row.move) { Object.assign(bot.entity, p); bot.emit('move'); continue; }
  if (row.name === 'spawn_entity') bot.entities[p.entityId] = { name: data.entities[p.type]?.name, type: data.entities[p.type]?.name };
  bot._client.emit(row.name, p);
  bot._client.emit('packet', p, { name: row.name, state: 'play' });
}
const events = vm.runInContext('events', sandbox);
fs.writeFileSync(output, JSON.stringify({ events, window: { last: saved.last } }, null, 1) + '\n');
