// Робот-клиент Bedrock для проверки входа: заходит без проверки Xbox,
// печатает, до какого шага дошёл, и сколько чанков получил.
//
// node tools/measure/bedrockbot.js [порт=19132] [секунд=20]
//
// Библиотека bedrock-protocol используется как чёрный ящик — как mineflayer
// у Java: её код не читаем, смотрим только, что видит клиент.
const bedrock = require(require('path').join(process.env.HOME, 'node_modules', 'bedrock-protocol'));
const port = Number(process.argv[2] || 19132);
const seconds = Number(process.argv[3] || 20);

const client = bedrock.createClient({
  host: '127.0.0.1',
  port,
  username: 'bedrockprobe',
  offline: true,
  version: '1.26.10',
  skipPing: true,
  raknetBackend: 'jsp-raknet',
});

let chunks = 0;
const seen = new Set();

client.on('packet', packet => {
  const name = packet.data && packet.data.name;
  if (name && !seen.has(name)) {
    seen.add(name);
    console.log('пакет:', name);
  }
  if (name === 'level_chunk') chunks++;
});

client.on('join', () => console.log('вход принят (join)'));
client.on('spawn', () => console.log('появился в мире (spawn)'));
client.on('kick', reason => { console.log('выгнан:', JSON.stringify(reason)); });
client.on('error', error => { console.log('ошибка:', error.message); });
client.on('close', () => console.log('соединение закрыто'));

setTimeout(() => {
  console.log('чанков получено:', chunks);
  process.exit(0);
}, seconds * 1000);
