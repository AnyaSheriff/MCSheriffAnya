// Живая проверка плагинов: робот mineflayer заходит на сервер с примерами из
// plugins-examples/ и проверяет события, команды, отмену, `/plugman`
// (загрузка, перезагрузка, выгрузка на ходу) и падение плохого плагина.
//
//   node tools/plugins/live.js <порт> <папка-сервера>
//
// Нужны права оператора у `blackbox` (ops.json; run-ours-*.sh их выдаёт).
// Библиотека mineflayer (MIT) — из ~/node_modules, как у чёрного ящика.
'use strict';
const path = require('path');
const fs = require('fs');
const { execFileSync } = require('child_process');
const mineflayer = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'));
const Vec3 = require(path.join(process.env.HOME, 'node_modules', 'vec3')).Vec3;

const [,, portArg, serverDir] = process.argv;
if (!serverDir) { console.error('использование: node live.js <порт> <папка-сервера>'); process.exit(2); }

const bot = mineflayer.createBot({ host: '127.0.0.1', port: Number(portArg), username: 'blackbox', version: '26.1', auth: 'offline' });
const lines = [];
let mark = 0;
bot.on('messagestr', (text) => { lines.push(text); });
bot.on('kicked', (reason) => console.log('кикнут:', JSON.stringify(reason)));
bot.on('error', (error) => console.log('ошибка:', error.message));

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
let failures = 0;
const check = (name, ok, extra) => { console.log(`${ok ? 'OK  ' : 'FAIL'} ${name}${ok ? '' : ' — ' + (extra || '')}`); if (!ok) failures++; };
const say = async (text, wait = 400) => { mark = lines.length; bot.chat(text); await sleep(wait); return lines.slice(mark); };
const cmd = async (text, wait = 500) => { mark = lines.length; bot.chat('/' + text); await sleep(wait); return lines.slice(mark).join('\n'); };
const has = (list, part) => list.some((line) => line.includes(part));

(async () => {
  await new Promise((resolve) => bot.once('spawn', resolve));
  await sleep(1500);

  // Вход: плагин hello поздоровался (личное сообщение и объявление всем).
  check('вход: приветствие hello', has(lines, 'Привет, blackbox! Попробуй /hello'), lines.join(' | '));
  check('вход: объявление hello', has(lines, '[hello] blackbox с нами'), lines.join(' | '));

  // Команда плагина.
  check('/hello Аня', (await cmd('hello Аня')).includes('Привет, Аня!'));
  check('/hello без имени', (await cmd('hello')).includes('Привет, blackbox!'));

  // Чат: отмена и правка.
  let got = await say('это СПАМ');
  check('чат: «спам» отменён (нет эха)', !has(got, '<blackbox> это СПАМ'), got.join(' | '));
  check('чат: отмена сказала причину', has(got, 'Это слово здесь нельзя'), got.join(' | '));
  got = await say('ПРИВЕТ ВСЕМ');
  check('чат: капс переписан', has(got, '<blackbox> привет всем'), got.join(' | '));
  got = await say('просто привет');
  check('чат: обычное сообщение идёт как есть', has(got, '<blackbox> просто привет'), got.join(' | '));

  // Блоки у точки появления: ломание и стройка отменены.
  await cmd('gamemode survival');
  const spawn = bot.entity.position.floored();
  const ground = bot.blockAt(spawn.offset(0, -1, 0));
  check('под ногами есть блок', ground && ground.name !== 'air', ground && ground.name);
  mark = lines.length;
  try { await bot.dig(ground, true); } catch (error) { /* отмена сервером — ожидаемо */ }
  await sleep(500);
  const after = bot.blockAt(spawn.offset(0, -1, 0));
  check('ломание у спауна отменено', after && after.name === ground.name, after && after.name);
  check('ломание: игроку сказали причину', has(lines.slice(mark), 'Здесь нельзя ломать'), lines.slice(mark).join(' | '));

  await cmd('give blackbox stone 8');
  await sleep(500);
  const stone = bot.inventory.items().find((item) => item.name === 'stone');
  if (stone) {
    await bot.equip(stone, 'hand');
    mark = lines.length;
    try { await bot.placeBlock(ground, new Vec3(0, 1, 0)); } catch (error) { /* отмена */ }
    await sleep(500);
    check('стройка у спауна отменена', bot.blockAt(spawn.offset(0, 0, 0)).name === 'air' || has(lines.slice(mark), 'Здесь нельзя ставить'), lines.slice(mark).join(' | '));
    check('стройка: игроку сказали причину', has(lines.slice(mark), 'Здесь нельзя ставить'), lines.slice(mark).join(' | '));
  } else {
    check('стоун выдан', false, 'нет в инвентаре');
  }

  // Радиус защиты — из config.toml плагина: уменьшаем до 2 и перезагружаем на
  // ходу; блок в трёх шагах от спауна ломать уже можно.
  await cmd('gamemode creative');
  fs.writeFileSync(path.join(serverDir, 'plugins', 'protect-spawn', 'config.toml'), 'radius = 2\n');
  let text = await cmd('plugman reload protect-spawn');
  check('reload protect-spawn с новым радиусом', text.includes('перезагружено'), text);
  const side = spawn.offset(3, -1, 0);
  const sideBlock = bot.blockAt(side);
  mark = lines.length;
  if (sideBlock && sideBlock.name !== 'air') {
    try { await bot.dig(sideBlock, true); } catch (error) { check('ломание вне радиуса: ошибка', false, error.message); }
    await sleep(600);
    check('вне радиуса защиты ломать можно', bot.blockAt(side).name === 'air', bot.blockAt(side).name);
  } else {
    check('блок в трёх шагах от спауна', false, String(sideBlock && sideBlock.name));
  }
  const near = spawn.offset(0, -1, 0);
  try { await bot.dig(bot.blockAt(near), true); } catch (error) { /* отмена */ }
  await sleep(500);
  check('в радиусе ломать по-прежнему нельзя', bot.blockAt(near).name !== 'air', bot.blockAt(near).name);
  fs.writeFileSync(path.join(serverDir, 'plugins', 'protect-spawn', 'config.toml'), 'radius = 16\n');
  await cmd('plugman reload protect-spawn');

  // /plugman.
  text = await cmd('plugman list');
  check('/plugman list показывает загруженное', text.includes('hello 0.1.0') && text.includes('chat-filter') && text.includes('protect-spawn') && text.includes('modinfo 0.1.0 (модуль)'), text);
  check('/plugman info', (await cmd('plugman info hello')).includes('/hello'));
  check('модуль: /modinfo', (await cmd('modinfo')).includes('Загружено:'));
  check('модуль: /modrun', (await cmd('modrun list')).includes('players online'), 'нет ответа list');

  // Выгрузка на ходу: фильтр чата пропал.
  text = await cmd('plugman unload chat-filter');
  check('unload chat-filter', text.includes('выгружен'), text);
  got = await say('это СПАМ');
  check('после unload «спам» проходит', has(got, '<blackbox> это СПАМ'), got.join(' | '));

  // Загрузка на ходу: фильтр вернулся.
  text = await cmd('plugman load chat-filter');
  check('load chat-filter', text.includes('Загружен chat-filter'), text);
  got = await say('это СПАМ');
  check('после load «спам» снова отменён', !has(got, '<blackbox> это СПАМ'), got.join(' | '));

  // Перезагрузка.
  text = await cmd('plugman reload hello');
  check('reload hello', text.includes('перезагружено'), text);
  check('/hello после reload', (await cmd('hello мир')).includes('Привет, мир!'));

  // Горячая установка: кладём новый плагин на диск и грузим, не перезапуская сервер.
  const bad = path.join(serverDir, 'plugins', 'misbehave');
  fs.mkdirSync(bad, { recursive: true });
  fs.copyFileSync(path.join(__dirname, '..', '..', 'plugins-examples', 'target', 'wasm32-wasip2', 'release', 'misbehave.wasm'), path.join(bad, 'misbehave.wasm'));
  fs.writeFileSync(path.join(bad, 'plugin.toml'), 'name = "misbehave"\nversion = "0.1.0"\napi = "0.1"\ndescription = "испытание"\n');
  text = await cmd('plugman load misbehave');
  check('горячая загрузка нового плагина', text.includes('Загружен misbehave'), text);

  for (const word of ['loop', 'panic', 'memory']) {
    const started = Date.now();
    await say(word, 600);
    text = await cmd('plugman list');
    check(`«${word}»: плохой плагин выгружен, сервер жив`, !text.includes('misbehave 0.1.0') && text.includes('hello'), text);
    check(`«${word}»: быстро (${Date.now() - started} мс)`, Date.now() - started < 3000);
    const reply = await cmd('plugman load misbehave');
    check(`«${word}»: можно загрузить снова`, reply.includes('Загружен misbehave'), reply);
  }

  text = await cmd('plugman stats');
  console.log(text);

  await cmd('plugman unload misbehave');
  bot.quit();
  await sleep(300);
  console.log(failures === 0 ? 'ВСЁ СОВПАЛО' : `ПРОВАЛОВ: ${failures}`);
  process.exit(failures === 0 ? 0 : 1);
})().catch((error) => { console.log('сбой робота:', error); process.exit(3); });
