// Убирает с площадок в мире BDS всё, что ставили сверки (воздух над травой y=-61, обе площадки 0..40).
const { connect, sleep } = require('./v-client.js');
(async () => {
  const k = await connect('bds', 'clean' + (process.pid % 1000));
  await sleep(3000);
  for (const c of ['/fill -3 -60 -3 40 -40 40 air', '/fill -3 -61 -3 40 -61 40 grass_block', '/kill @e[type=item]']) { k.cmd(c); await sleep(600); }
  k.close(); process.exit(0);
})();
setTimeout(() => process.exit(1), 30000);
