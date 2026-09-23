const mineflayer=require(require('path').join(process.env.HOME,'node_modules','mineflayer'));
const bot=mineflayer.createBot({host:'127.0.0.1',port:25570,username:'loadprobe',version:'26.1',auth:'offline'});
let chunks=0; bot._client.on('map_chunk',()=>chunks++);
bot.once('spawn',()=>{ setTimeout(()=>{ console.log('чанков получено', chunks); process.exit(0); }, 40000); });
bot.on('error',e=>{console.log('ошибка',e.message); process.exit(1)});
