// Полёт со скоростью элитр: успевает ли сервер присылать чанки.
//
// node tools/measure/flybot.js <порт> [секунд=60] [блоков в секунду=33.5]
//
// Робот заходит, ждёт 20 секунд (обзор и то, что сервер готовит впрок),
// потом летит по прямой на восток на высоте 200. Каждые 50 мс сдвигается
// на скорость/20 блоков — но только если чанк, куда он летит, уже пришёл:
// иначе стоит, как стоял бы клиент перед пустотой. Мерится:
//   — сколько пролетел и какая вышла средняя скорость;
//   — сколько времени простоял, ожидая чанки;
//   — запас впереди: сколько чанков по курсу уже есть (в среднем и меньше всего);
//   — чанков получено за полёт.
const mineflayer = require(require('path').join(process.env.HOME, 'node_modules', 'mineflayer'));
const port = Number(process.argv[2]);
const seconds = Number(process.argv[3] || 60);
const speed = Number(process.argv[4] || 33.5);
const bot = mineflayer.createBot({ host: '127.0.0.1', port, username: 'flyprobe', version: '26.1', auth: 'offline', physicsEnabled: false });

const loaded = new Set();
let received = 0;
bot._client.on('map_chunk', packet => { loaded.add(packet.x + ',' + packet.z); received++; });
bot._client.on('unload_chunk', packet => loaded.delete(packet.chunkX + ',' + packet.chunkZ));

bot.once('spawn', () => {
  setTimeout(() => {
    bot.physicsEnabled = false;
    const start = bot.entity.position.clone();
    let x = start.x;
    const z = start.z;
    // На высоту 200 — постепенно, по блоку за шаг: резкий скачок античит
    // оригинала и ядер считает слишком быстрым движением.
    let y = start.y;
    const climb = setInterval(() => {
      y = Math.min(200, y + 1);
      bot.entity.position.set(x, y, z);
      if (y >= 200) { clearInterval(climb); setTimeout(fly, 3000); }
    }, 50);

    function fly () {
    let stalled = 0, ticks = 0;
    const margins = [];
    const before = received;
    const step = speed / 20;
    const began = Date.now();

    const timer = setInterval(() => {
      ticks++;
      const next = x + step;
      const cx = Math.floor(next / 16), cz = Math.floor(z / 16);

      let ahead = 0;
      while (ahead < 32 && loaded.has((cx + ahead) + ',' + cz)) ahead++;
      margins.push(ahead);

      if (loaded.has(cx + ',' + cz)) {
        x = next;
        bot.entity.position.set(x, y, z);
      } else {
        stalled++;
      }

      if (Date.now() - began >= seconds * 1000) {
        clearInterval(timer);
        const flown = x - start.x;
        margins.sort((a, b) => a - b);
        const mean = margins.reduce((a, b) => a + b, 0) / margins.length;
        console.log(JSON.stringify({
          port, seconds, speed,
          flown: Math.round(flown),
          average_speed: +(flown / seconds).toFixed(1),
          stalled_seconds: +(stalled / 20).toFixed(1),
          ahead_mean: +mean.toFixed(1),
          ahead_min: margins[0],
          ahead_p5: margins[Math.floor(margins.length * 0.05)],
          chunks_received: received - before,
        }));
        process.exit(0);
      }
    }, 50);
    }
  }, 20000);
});

bot.on('kicked', reason => { console.log('выгнан', JSON.stringify(reason)); process.exit(1); });
bot.on('error', error => { console.log('ошибка', error.message); process.exit(1); });
