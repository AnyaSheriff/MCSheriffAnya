// Замер: как оригинал двигает выпавший предмет в воде. Робот строит жёлоб
// с водой, призывает в него предмет и записывает, что сервер сообщает о
// предмете: положение, скорость — с временем от призыва (в тактах по часам
// робота). Только наблюдение снаружи по сети.
//
//   node tools/blackbox/itemflow.js [порт] [файл]
//
// Сервер — оригинал 26.1.2 (порт 25566 по умолчанию).
const path = require('path');
const fs = require('fs');
const HOME_MODULES = path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(HOME_MODULES, 'mineflayer'));

const port = Number(process.argv[2] || 25566);
const outFile = process.argv[3] || path.join(__dirname, 'reports', 'itemflow.json');
const O = { x: 200, y: -59, z: 200 };
const TICK = 50;

const bot = mineflayer.createBot({ host: '127.0.0.1', port, username: 'blackbox', version: '26.1', auth: 'offline' });
const wait = (ms) => new Promise((r) => setTimeout(r, ms));
const say = async (cmd) => { bot.chat('/' + cmd); await wait(150); };
const log = [];
let t0 = null;
let watched = new Set();

const note = (kind, data) => {
  if (t0 === null) return;
  log.push({ t: +((Date.now() - t0) / TICK).toFixed(2), kind, ...data });
};

bot._client.on('spawn_entity', (p) => {
  if (t0 === null) return;
  watched.add(p.entityId);
  note('spawn', { id: p.entityId, x: p.x - O.x, y: p.y - O.y, z: p.z - O.z, vel: p.velocity });
});
bot._client.on('entity_velocity', (p) => { if (watched.has(p.entityId)) note('velocity', { id: p.entityId, v: p.velocity }); });
bot._client.on('rel_entity_move', (p) => { if (watched.has(p.entityId)) note('move', { id: p.entityId, dx: p.dX / 4096, dy: p.dY / 4096, dz: p.dZ / 4096, ground: p.onGround }); });
bot._client.on('entity_teleport', (p) => { if (watched.has(p.entityId)) note('teleport', { id: p.entityId, x: p.x - O.x, y: p.y - O.y, z: p.z - O.z, v: [p.dx, p.dy, p.dz], ground: p.onGround }); });
bot._client.on('sync_entity_position', (p) => { if (watched.has(p.entityId)) note('sync', { id: p.entityId, x: p.x - O.x, y: p.y - O.y, z: p.z - O.z, v: [p.dx, p.dy, p.dz], ground: p.onGround }); });
bot._client.on('sound_effect', (p) => note('sound', { id: p.soundId, x: p.x / 8 - O.x, y: p.y / 8 - O.y, z: p.z / 8 - O.z, volume: p.volume }));
bot._client.on('entity_sound_effect', (p) => { if (watched.has(p.entityId)) note('entity_sound', { id: p.entityId, sound: p.soundId, volume: p.volume }); });
bot._client.on('entity_destroy', (p) => { for (const id of p.entityIds || []) if (watched.has(id)) note('gone', { id }); });

bot.once('spawn', async () => {
  try {
    await wait(3000);
    await say('gamemode creative');
    await say(`tp blackbox ${O.x + 3} ${O.y + 3} ${O.z + 4}`);
    bot.creative.startFlying();
    await wait(2000);
    // Площадка: каменный пол, жёлоб вдоль x шириной в блок, стенки по бокам.
    await say(`fill ${O.x - 4} ${O.y - 1} ${O.z - 4} ${O.x + 14} ${O.y + 4} ${O.z + 4} minecraft:air`);
    await say(`fill ${O.x - 4} ${O.y - 1} ${O.z - 4} ${O.x + 14} ${O.y - 1} ${O.z + 4} minecraft:stone`);
    await say(`fill ${O.x - 1} ${O.y} ${O.z - 1} ${O.x + 12} ${O.y} ${O.z - 1} minecraft:stone`);
    await say(`fill ${O.x - 1} ${O.y} ${O.z + 1} ${O.x + 12} ${O.y} ${O.z + 1} minecraft:stone`);
    await say(`setblock ${O.x - 1} ${O.y} ${O.z} minecraft:stone`);
    await say(`setblock ${O.x} ${O.y} ${O.z} minecraft:water`);
    await wait(6000); // вода натекла до конца (7 блоков)
    // Второй опыт — предмет падает в стоячую воду: лунка-источник.
    await say(`fill ${O.x + 3} ${O.y} ${O.z + 3} ${O.x + 5} ${O.y + 1} ${O.z + 3} minecraft:stone`);
    t0 = Date.now();
    // Предметы без начальной скорости, без подбора.
    await say(`summon item ${O.x + 1.5} ${O.y + 0.2} ${O.z + 0.5} {Item:{id:"minecraft:stone",count:1},Motion:[0.0,0.0,0.0],PickupDelay:32767}`);
    await say(`summon item ${O.x + 4.5} ${O.y + 0.2} ${O.z + 0.5} {Item:{id:"minecraft:stone",count:1},Motion:[0.0,0.0,0.0],PickupDelay:32767}`);
    await say(`summon item ${O.x + 1.5} ${O.y + 3.0} ${O.z + 0.5} {Item:{id:"minecraft:stone",count:1},Motion:[0.0,0.0,0.0],PickupDelay:32767}`);
    await wait(12000);
    fs.writeFileSync(outFile, JSON.stringify(log, null, 1));
    console.log(`записано событий: ${log.length} → ${outFile}`);
    bot.quit();
    setTimeout(() => process.exit(0), 300);
  } catch (e) { console.error(e); process.exit(1); }
});
bot.on('kicked', (r) => { console.error('кик:', JSON.stringify(r)); process.exit(1); });
bot.on('error', (e) => { console.error('ошибка:', e.message); process.exit(1); });
setTimeout(() => process.exit(1), 60000);
