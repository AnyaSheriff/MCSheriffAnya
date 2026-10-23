// Запуск только с отдельным тестовым ядром на 26110.
const path = require('path');
let mineflayer;
try { mineflayer = require('../blackbox/node_modules/mineflayer'); }
catch { mineflayer = require(path.join(process.env.HOME, 'node_modules/mineflayer')); }
const assert = require('node:assert/strict');
const bot = mineflayer.createBot({host:'127.0.0.1',port:26110,username:'JavaTester',version:'26.1',auth:'offline'});
const messages=[];
bot.on('messagestr',message=>{messages.push(message);console.log(message);});
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const waitFor=async(text)=>{for(let i=0;i<100;i++){if(messages.some(m=>m.includes(text)))return;await sleep(100);}throw Error('Нет сообщения: '+text);};
const timer=setTimeout(()=>{console.error('Таймаут');bot.end();process.exit(1);},45000);
bot.on('error',error=>{console.error(error);clearTimeout(timer);process.exit(1);});
bot.once('spawn',async()=>{
 try {
  await waitFor('Вход из Java работает');
  bot.chat('/javahello');await waitFor('Команда из Java работает');
  const hints=await bot.tabComplete('/javahello java-tab');assert(hints.some(h=>h.match==='java-tab-ok'));
  console.log('PASS: Tab java-tab-ok');
  bot.chat('cancel-java');await waitFor('Отмена чата из Java работает');
  bot.chat('/java-blocked');await waitFor('Отмена команды из Java работает');
  await waitFor('Задача с конфигом из Java работает');
  bot.chat('/gamemode creative');await sleep(400);
  bot.chat('/fill -3 62 -3 5 62 3 stone');await sleep(400);
  bot.chat('/tp 0 63 0');await sleep(400);
  bot.chat('/setblock 2 63 0 stone');await sleep(500);
  const Vec3=bot.entity.position.constructor;
  const target=new Vec3(2,63,0);
  const block=bot.blockAt(target);assert.equal(block?.name,'stone');
  await bot.dig(block);await waitFor('Отмена блока из Java работает');await sleep(400);
  assert.equal(bot.blockAt(target)?.name,'stone');
  bot.chat('/give JavaTester stone 2');await sleep(600);
  const stone=bot.inventory.items().find(item=>item.name==='stone');assert(stone);await bot.equip(stone,'hand');
  try { await bot.placeBlock(bot.blockAt(new Vec3(1,62,0)),new Vec3(0,1,0)); }catch {}
  await waitFor('Отмена установки из Java работает');assert.equal(bot.blockAt(new Vec3(1,63,0))?.name,'air');
  assert(!messages.some(m=>m.includes('<JavaTester> cancel-java')));
  console.log('PASS: команда, вход, отмена чата/команды/блока, repeating + config.yml');
  clearTimeout(timer);bot.end();setTimeout(()=>process.exit(0),300);
 }catch(error){console.error(error);clearTimeout(timer);bot.end();process.exit(1);}
});
