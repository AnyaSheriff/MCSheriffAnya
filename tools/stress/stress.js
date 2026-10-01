#!/usr/bin/env node
// Стресс-тест сервера роботами mineflayer.
//
// Установка (один раз, в этой папке):   npm install mineflayer
// Запуск:                              node stress.js   (или ./stress.js)
//
// Настройки — ниже, в CONFIG. Сервер должен пускать без проверки аккаунтов
// (online-mode=false): роботы заходят офлайн-никами.

const CONFIG = {
  address: '127.0.0.1:25565', // IP:порт
  bots: 50,                   // сколько роботов
  prefix: 'bot_',             // начало ника; дальше — случайные буквы и цифры
  randomSprint: true,         // true — роботы то бегут, то идут; false — только идут
  joinDelayMs: 250,           // пауза между входами (у Spigot/Paper есть connection-throttle ~4 с на IP)
  reconnect: true,            // зайти заново после кика или обрыва
  // Версия клиента роботов: false — спросить у сервера; строка — одна для всех
  // ('1.21.4'); список — каждому роботу случайная из него (удобно с ViaVersion):
  // ['1.20.4', '1.21.1', '1.21.4']. Какие версии есть — в README mineflayer.
  version: false,
  chatEverySec: 30,           // раз во сколько секунд каждый робот пишет в чат; 0 — молчать
  chatMessages: [             // заготовки: робот берёт случайную
    'Привет!',
    'Тестирую сервер',
    'Как дела?',
  ],
};

const mineflayer = require('mineflayer');

const [host, portText] = CONFIG.address.split(':');
const port = Number(portText || 25565);
const stats = { online: 0, joined: 0, kicked: 0, errors: 0 };

// Ник: префикс + случайный хвост, всего не больше 16 знаков.
const randomName = () => {
  const alphabet = 'abcdefghijklmnopqrstuvwxyz0123456789';
  let tail = '';
  while (CONFIG.prefix.length + tail.length < 16 && tail.length < 8) {
    tail += alphabet[Math.floor(Math.random() * alphabet.length)];
  }
  return (CONFIG.prefix + tail).slice(0, 16);
};

const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

// Бродит: поворачивается в случайную сторону, идёт, иногда прыгает,
// бег включается и выключается случайно (если разрешён).
const wander = (bot) => {
  const step = () => {
    if (!bot.entity) return;
    const yaw = Math.random() * Math.PI * 2;
    const pitch = (Math.random() - 0.5) * 0.6;
    bot.look(yaw, pitch, true).catch(() => {});
    bot.setControlState('forward', Math.random() < 0.8);
    bot.setControlState('jump', Math.random() < 0.2);
    bot.setControlState('sprint', CONFIG.randomSprint && Math.random() < 0.5);
  };
  step();
  return setInterval(step, 1000 + Math.random() * 3000);
};

const start = (name) => {
  const version = Array.isArray(CONFIG.version)
    ? CONFIG.version[Math.floor(Math.random() * CONFIG.version.length)]
    : CONFIG.version;
  const bot = mineflayer.createBot({ host, port, username: name, auth: 'offline', version });
  let timer = null;
  let chatTimer = null;
  let online = false;
  let over = false;

  bot.once('spawn', () => {
    online = true;
    stats.online++;
    stats.joined++;
    timer = wander(bot);
    if (CONFIG.chatEverySec > 0 && CONFIG.chatMessages.length > 0) {
      // Первое сообщение — в случайный момент, чтобы роботы не писали разом.
      const say = () => bot.chat(CONFIG.chatMessages[Math.floor(Math.random() * CONFIG.chatMessages.length)]);
      chatTimer = setTimeout(function next() {
        say();
        chatTimer = setTimeout(next, CONFIG.chatEverySec * 1000);
      }, Math.random() * CONFIG.chatEverySec * 1000);
    }
  });

  const gone = (why) => {
    if (over) return;
    over = true;
    if (timer) clearInterval(timer);
    if (chatTimer) clearTimeout(chatTimer);
    timer = null;
    chatTimer = null;
    if (online) stats.online--;
    online = false;
    if (why) console.log(`${name}: ${why}`);
    if (CONFIG.reconnect) setTimeout(() => start(name), 5000);
  };

  bot.on('kicked', (reason) => { stats.kicked++; gone(`кикнут: ${typeof reason === 'string' ? reason : JSON.stringify(reason)}`); });
  bot.on('error', (err) => { stats.errors++; console.log(`${name}: ошибка ${err.message}`); });
  bot.on('end', () => gone());
};

(async () => {
  console.log(`Цель ${host}:${port}, роботов ${CONFIG.bots}, префикс «${CONFIG.prefix}», бег ${CONFIG.randomSprint ? 'случайный' : 'выключен'}, чат ${CONFIG.chatEverySec > 0 ? `раз в ${CONFIG.chatEverySec} с` : 'выключен'}, версия ${CONFIG.version === false ? 'как у сервера' : [].concat(CONFIG.version).join(', ')}`);
  setInterval(() => {
    console.log(`в игре: ${stats.online}/${CONFIG.bots}, всего входов: ${stats.joined}, киков: ${stats.kicked}, ошибок: ${stats.errors}`);
  }, 5000);
  for (let i = 0; i < CONFIG.bots; i++) {
    start(randomName());
    await sleep(CONFIG.joinDelayMs);
  }
})();

// Остановить всех — Ctrl+C.
