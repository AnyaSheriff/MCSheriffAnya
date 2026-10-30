// Транспортный адаптер для внешнего ViaProxy: штатная физика mineflayer,
// завершение каждого клиентского такта по событию physicsTick. Без этого
// официальный777 отклоняет второе движение; bot.js/compare.js неизменны.
'use strict';
const path = require('path');
const modules = process.env.MCSA_BB_NODE_MODULES || path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(modules, 'mineflayer'));
const createBot = mineflayer.createBot;
mineflayer.createBot = (options) => {
  const view = process.env.MCSA_BB_VIEW_DISTANCE;
  const bot = createBot(view ? { ...options, viewDistance: view } : options);
  let quitting = false;
  bot.once('spawn', () => {
    const quit = bot.quit.bind(bot);
    bot.quit = (...args) => { quitting = true; return quit(...args); };
  });
  bot.on('end', (reason) => {
    if (!quitting) {
      console.error('ViaProxy: соединение закрылось до конца сценария:', reason);
      process.exit(1);
    }
  });
  bot.on('physicsTick', () => {
    if (bot._client.state === 'play') bot._client.write('tick_end', {});
  });
  return bot;
};
