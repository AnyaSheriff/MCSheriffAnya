// UDP-прослойка: клиент (RakNet 10) → BDS (ждёт 11). Меняет только номер
// протокола в Open Connection Request 1. node br-shim.js [порт-слушать] [порт-BDS]
const dgram = require('dgram');
const listen = +process.argv[2] || 19160, upstream = +process.argv[3] || 19150;
const srv = dgram.createSocket('udp4'), out = dgram.createSocket('udp4');
let client = null;

srv.on('message', (d, rinfo) => {
  client = rinfo;
  if (d[0] === 5 && d.length > 17 && d[17] === 10) { d = Buffer.from(d); d[17] = 11; }
  out.send(d, upstream, '127.0.0.1');
});
out.on('message', d => { if (client) srv.send(d, client.port, client.address); });
srv.bind(listen, '127.0.0.1');
