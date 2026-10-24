// Запуск только с отдельным тестовым ядром на 26200..26299.
const path = require('path');
let mineflayer;
try { mineflayer = require('../blackbox/node_modules/mineflayer'); }
catch { mineflayer = require(path.join(process.env.HOME, 'node_modules/mineflayer')); }
const assert = require('node:assert/strict');
const port=Number(process.argv[2]||26210);assert(Number.isInteger(port)&&port>=26200&&port<=26299);
const bot = mineflayer.createBot({host:'127.0.0.1',port,username:'JavaTester',version:'26.1',auth:'offline'});
const messages=[];
let actionBar=false,positionCorrections=0;bot._client.on('position',()=>positionCorrections++);
const teams=[];bot._client.on("teams",packet=>teams.push(packet));
bot._client.on('action_bar',()=>{actionBar=true;});
bot.on('messagestr',message=>{messages.push(message);console.log(message);});
const sleep=ms=>new Promise(resolve=>setTimeout(resolve,ms));
const waitFor=async(text)=>{for(let i=0;i<100;i++){if(messages.some(m=>m.includes(text)))return;await sleep(100);}throw Error('Нет сообщения: '+text);};
const waitHealth=async(value)=>{for(let i=0;i<100;i++){if(bot.health===value)return;await sleep(100);}assert.equal(bot.health,value);};
const timer=setTimeout(()=>{console.error('Таймаут');bot.end();process.exit(1);},65000);
bot.on('error',error=>{console.error(error);clearTimeout(timer);process.exit(1);});
bot.once('spawn',async()=>{
 try {
  await waitFor('Вход из Java работает');
  bot.chat('/javahello');await waitFor('Команда из Java работает');
  const hints=await bot.tabComplete('/javahello java-tab');assert(hints.some(h=>h.match==='java-tab-ok'));
  console.log('PASS: Tab java-tab-ok');
  if(process.argv.includes('--restart')){bot.chat('/javahello gui-persistent');await waitFor('GUI persisted across restart');bot.chat('/javahello zero');await waitFor('Respawn delivered; bed=true; anchor=false');console.log('PASS: GUI plugin data and personal bed spawn survived core restart');clearTimeout(timer);bot.end();setTimeout(()=>process.exit(0),300);return;}
  bot.chat('/javahello team');await sleep(600);assert(teams.some(packet=>JSON.stringify(packet).includes('[Elves]')),'Team prefix packet missing');assert(teams.some(packet=>JSON.stringify(packet).includes('JavaTester')),'Team entry packet missing');
  bot.chat('/javahello gui');for(let i=0;i<100&&!bot.currentWindow;i++)await sleep(100);assert(bot.currentWindow,'GUI did not open');assert.equal(bot.currentWindow.slots[13]?.name,'diamond');
  await bot.clickWindow(13,0,0);await waitFor('GUI cancelled with holder and item');await sleep(300);assert.equal(bot.currentWindow.slots[13]?.name,'diamond');assert.equal(bot.currentWindow.selectedItem,null);assert(!bot.inventory.items().some(item=>item.name==='diamond'));
  bot.chat('/javahello gui-close');await waitFor('GUI close delivered');await sleep(300);assert.equal(bot.currentWindow,null);
  bot.chat('/javahello gui-free');for(let i=0;i<100&&!bot.currentWindow;i++)await sleep(100);assert(bot.currentWindow);await bot.clickWindow(13,0,0);await waitFor('GUI allowed click');await sleep(300);assert.equal(bot.currentWindow.slots[13],null);assert.equal(bot.currentWindow.selectedItem?.name,'diamond');bot.closeWindow(bot.currentWindow);await sleep(500);assert(bot.inventory.items().some(item=>item.name==='diamond'),'Cursor item lost on close');
  console.log('PASS: client GUI, holder, cancelled/allowed clicks, cursor return, close and scoreboard teams');
  bot.chat('/gamerule natural_health_regeneration false');await waitFor('Правило natural_health_regeneration: false');
  bot.chat('/javahello api');await waitFor('Компонент из Java работает');await sleep(600);
  await waitHealth(18);
  await waitFor('Item meta/PDC round trip');await waitFor('Attribute queue round trip');await waitFor('HTTPS denied without approval');await waitFor('Data folder confinement works');
  assert.equal(bot.inventory.slots[44]?.name,'diamond_sword');
  bot.chat('/plugman reload java-command');await sleep(600);bot.chat('/javahello persistent');await waitFor('Entity PDC survived reload');
  assert(actionBar,'Пакет action bar отсутствует');
  assert(Object.keys(bot.entity.effects).length>0,'Эффект зелья отсутствует');
  bot.chat('/gamemode survival');await sleep(300);
  bot.chat('/damage JavaTester 4 minecraft:generic');await waitFor('Урон из Java изменён на 1');await waitHealth(17);
  bot.chat('/damage JavaTester 5 minecraft:fall');await waitFor('Урон от падения из Java отменён');await sleep(400);assert.equal(bot.health,17);
  bot.chat('/javahello zero');await waitFor('Смерть из Java работает');await waitFor('Respawn delivered; bed=false; anchor=false');await sleep(1000);
  console.log('PASS: Adventure, карта/atomic, здоровье, зелье, изменение/отмена урона, смерть');
  bot.chat('cancel-java');await waitFor('Отмена чата из Java работает');
  bot.chat('/java-blocked');await waitFor('Отмена команды из Java работает');
  await waitFor('Задача с конфигом из Java работает');
  await waitFor('Ordered executor saved');
  assert(messages.findIndex(m=>m.includes('Ordered executor first'))<messages.findIndex(m=>m.includes('Ordered executor second')));
  bot.chat('/gamemode creative');await sleep(400);
  bot.chat('/fill -3 62 -3 5 62 3 stone');await sleep(400);
  bot.chat('/tp 0 63 0');await sleep(400);
  bot.chat('/setblock 2 63 0 stone');await sleep(500);bot.setQuickBarSlot(1);await sleep(300);
  const Vec3=bot.entity.position.constructor;
  const target=new Vec3(2,63,0);
  const block=bot.blockAt(target);assert.equal(block?.name,'stone');
  bot.chat('/eventprobe trace');await waitFor('Event probe armed: trace');await bot.dig(block);await waitFor('Interaction trace: LEFT_CLICK_BLOCK');await waitFor('Отмена блока из Java работает');await sleep(400);
  assert.equal(bot.blockAt(target)?.name,'stone');
  bot.chat('/give JavaTester stone 2');await sleep(600);
  const stone=bot.inventory.items().find(item=>item.name==='stone');assert(stone);await bot.equip(stone,'hand');
  try { await bot.placeBlock(bot.blockAt(new Vec3(1,62,0)),new Vec3(0,1,0)); }catch {}
  await waitFor('Отмена установки из Java работает');assert.equal(bot.blockAt(new Vec3(1,63,0))?.name,'air');
  assert(!messages.some(m=>m.includes('<JavaTester> cancel-java')));
  bot.chat('/eventprobe world');await waitFor('World snapshot verified');
  bot.chat('/eprobe permissions');await waitFor('Permission inheritance verified; op=true');
  await new Promise((resolve,reject)=>{
   const guest=mineflayer.createBot({host:'127.0.0.1',port,username:'PermissionGuest',version:'26.1',auth:'offline'});
   const deadline=setTimeout(()=>{guest.end();reject(Error('Non-op permission probe timed out'));},10000);
   guest.once('spawn',()=>guest.chat('/eprobe permissions'));
   guest.on('messagestr',message=>{if(message.includes('Permission inheritance verified; op=false')){clearTimeout(deadline);guest.end();console.log('PASS: non-op inherited public permission; explicit bypass denied');resolve();}});
   guest.on('error',error=>{clearTimeout(deadline);reject(error);});
  });
  await sleep(400);bot.chat('render-java');await waitFor('Rendered chat: render-java');
  bot.chat('/eventprobe armor');await waitFor('Armor change delivered: HEAD');
  bot.chat('/give JavaTester dirt 2');await sleep(500);await bot.equip(bot.inventory.items().find(item=>item.name==='dirt'),'hand');bot.chat('/eventprobe interact');await waitFor('Event probe armed: interact');try{await bot.placeBlock(bot.blockAt(new Vec3(-1,62,0)),new Vec3(0,1,0));}catch{}await waitFor('Interaction cancelled; hand=HAND');await sleep(300);assert.equal(bot.blockAt(new Vec3(-1,63,0))?.name,'air');assert.equal(bot.inventory.items().filter(item=>item.name==='dirt').reduce((sum,item)=>sum+item.count,0),2);
  bot.chat('/gamemode survival');await sleep(700);const sourceHealth=bot.health;bot.chat('/eventprobe source');await waitFor('Event probe armed: source');bot.chat('/damage JavaTester 4 minecraft:player_attack by JavaTester');await waitFor('Player damage source delivered and changed to 0.5');await sleep(500);assert(bot.health<sourceHealth&&sourceHealth-bot.health<=0.6,'Source-aware damage was not modified');bot.chat('/gamemode creative');await sleep(300);
  bot.chat('/eventprobe state');await waitFor('Live state, potion, entity and block APIs verified');
  const previousHeld=bot.quickBarSlot;bot.chat('/eventprobe held');await waitFor('Event probe armed: held');bot.setQuickBarSlot(previousHeld===2?3:2);await waitFor('Held slot cancelled before selection');await sleep(400);assert.equal(bot.quickBarSlot,previousHeld);
  bot.chat('/eventprobe consume');await waitFor('Event probe armed: consume');await sleep(300);bot.setQuickBarSlot(2);await sleep(300);bot.activateItem();await waitFor('Consume cancelled before effects and item removal');bot.deactivateItem();await sleep(300);assert.equal(bot.inventory.slots[38]?.name,'honey_bottle');
  bot.chat('/eventprobe swap');await waitFor('Event probe armed: swap');bot._client.write('block_dig',{status:6,location:new Vec3(0,0,0),face:0,sequence:0});await waitFor('Hand swap cancelled before inventory mutation');await sleep(300);assert.equal(bot.inventory.slots[38]?.name,'honey_bottle');bot._client.write('block_dig',{status:6,location:new Vec3(0,0,0),face:0,sequence:0});await sleep(300);assert.equal(bot.inventory.slots[45]?.name,'honey_bottle');bot._client.write('block_dig',{status:6,location:new Vec3(0,0,0),face:0,sequence:0});await sleep(300);assert.equal(bot.inventory.slots[38]?.name,'honey_bottle');
  const beforeMove=positionCorrections;bot.chat('/eventprobe move');await waitFor('Event probe armed: move');bot.setControlState('forward',true);await waitFor('Movement cancelled before physics');bot.clearControlStates();await sleep(500);assert(positionCorrections>beforeMove,'Cancelled movement did not correct client position');
  bot.chat('/tp 0 63 0');await sleep(500);
  const beforeJump=positionCorrections;bot.chat('/eventprobe jump');await waitFor('Event probe armed: jump');bot.setControlState('jump',true);await waitFor('Jump cancelled before physics');bot.clearControlStates();await sleep(500);assert(positionCorrections>beforeJump,'Cancelled jump did not correct client position');
  bot.chat('/setblock 1 63 0 red_bed[part=foot,facing=east]');await sleep(300);bot.chat('/setblock 2 63 0 red_bed[part=head,facing=east]');await sleep(300);bot.chat('/time set night');await sleep(300);
  bot.chat('/eventprobe bed');await waitFor('Event probe armed: bed');await bot.activateBlock(bot.blockAt(new Vec3(1,63,0)));await waitFor('Bed cancelled with real spawn rules');await sleep(300);bot.chat('/javahello zero');await waitFor('Respawn delivered; bed=true; anchor=false');await sleep(700);assert(Math.abs(bot.entity.position.x-2.5)<4&&Math.abs(bot.entity.position.z-0.5)<4,'Bed respawn destination was not applied');
  console.log('PASS: movement/jump/interaction cancellation, armor, bed, respawn, rendered chat, permission inheritance and command alias');
  console.log('PASS: команда, вход, отмена чата/команды/блока, repeating + config.yml');
  clearTimeout(timer);bot.end();setTimeout(()=>process.exit(0),300);
 }catch(error){console.error(error);clearTimeout(timer);bot.end();process.exit(1);}
});
