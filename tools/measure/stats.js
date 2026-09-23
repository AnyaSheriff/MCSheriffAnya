// Замер мира роботом: руда, пустоты под землёй, резкость рельефа.
// node stats.js <порт> — робот ходит в три места и считает круг чанков вокруг.
const path=require('path');
const HM=path.join(process.env.HOME,'node_modules');
const mineflayer=require(path.join(HM,'mineflayer'));
const Vec3=require(path.join(HM,'vec3')).Vec3;
const port=Number(process.argv[2]);
const R=6;                              // радиус замера в чанках
const PLACES=[[0,0],[2000,2000],[-2400,1600]];
const bot=mineflayer.createBot({host:'127.0.0.1',port,username:'stats',version:'26.1',auth:'offline'});
const sleep=ms=>new Promise(r=>setTimeout(r,ms));
let lastChunk=Date.now();
bot._client.on('map_chunk',()=>{lastChunk=Date.now();});
const hollowNames=new Set(['air','cave_air','water','lava']);
const rocks=new Set(['dirt','gravel','granite','diorite','andesite','tuff']);
const stat={cols:0,steps3:0,steps6:0,maxStep:0,pairs:0,hollow:0,solid:0,blocks:{},heights:[],deep:{},bands:{}};
function ground(b){ return b && b.boundingBox==='block' && !b.name.endsWith('_leaves') && !b.name.endsWith('_log') && !b.name.endsWith('_wood') && b.name!=='cactus' && !b.name.includes('mushroom_block') && b.name!=='bamboo'; }
function surface(x,z){ for(let y=319;y>-64;y--){ const b=bot.blockAt(new Vec3(x,y,z),false); if(ground(b)) return y; } return null; }
async function place(cx,cz){
  bot.chat(`/tp stats ${cx} 200 ${cz}`);
  await sleep(1500); lastChunk=Date.now();
  while(Date.now()-lastChunk<4000) await sleep(500);
  const ox=Math.floor(cx/16)*16, oz=Math.floor(cz/16)*16;
  const H={};
  for(let x=ox-R*16;x<ox+R*16+16;x++) for(let z=oz-R*16;z<oz+R*16+16;z++){
    const h=surface(x,z); if(h===null) continue; H[x+','+z]=h; stat.cols++; stat.heights.push(h);
    for(let y=-59;y<Math.min(h,62);y++){
      const b=bot.blockAt(new Vec3(x,y,z),false); if(!b) continue;
      if(hollowNames.has(b.name)){stat.hollow++; const band=y<=-55?'ниже -54':(y<=-40?'-54..-40':(y<=-10?'-39..-10':'выше -10')); const nm=b.name==='cave_air'?'air':b.name; stat.deep[band]=stat.deep[band]||{}; stat.deep[band][nm]=(stat.deep[band][nm]||0)+1; continue;}
      stat.solid++;
      const n=b.name.replace(/^deepslate_(?=.*_ore$)/,'');
      if(n.endsWith('_ore')||rocks.has(n)) stat.blocks[n]=(stat.blocks[n]||0)+1;
    }
  }
  for(const k in H){ const [x,z]=k.split(',').map(Number);
    for(const [dx,dz] of [[1,0],[0,1]]){ const o=H[(x+dx)+','+(z+dz)]; if(o===undefined) continue;
      const d=Math.abs(H[k]-o); const lo=Math.min(H[k],o); const bn=lo<70?'ниже 70':(lo<100?'70-99':'100+'); stat.bands[bn]=stat.bands[bn]||[0,0,0]; stat.bands[bn][0]++; if(d>3) stat.bands[bn][1]++; if(d>6) stat.bands[bn][2]++; stat.pairs++; if(d>3) stat.steps3++; if(d>6) stat.steps6++; if(d>stat.maxStep) stat.maxStep=d; } }
  console.error(`  место ${cx} ${cz}: столбцов ${Object.keys(H).length}`);
}
bot.once('spawn',async()=>{
  try{
    bot.chat('/gamemode creative'); await sleep(1000);
    for(const [x,z] of PLACES) await place(x,z);
    const total=stat.hollow+stat.solid;
    const ores=Object.entries(stat.blocks).filter(([n])=>n.endsWith('_ore')).reduce((a,[,c])=>a+c,0);
    stat.heights.sort((a,b)=>a-b); const q=p=>stat.heights[Math.floor(stat.heights.length*p)];
    console.log(`порт ${port}: столбцов ${stat.cols}`);
    console.log(`  высоты: нижняя ${q(0)}, четверть ${q(0.25)}, середина ${q(0.5)}, три четверти ${q(0.75)}, верхняя ${stat.heights[stat.heights.length-1]}`);
    console.log(`  ступеньки между соседями: больше 3 блоков — ${(100*stat.steps3/stat.pairs).toFixed(2)}%, больше 6 — ${(100*stat.steps6/stat.pairs).toFixed(2)}%, самая большая ${stat.maxStep}`);
    for(const bn in stat.bands){ const b=stat.bands[bn]; console.log(`  высота ${bn}: пар ${b[0]}, >3 ${(100*b[1]/b[0]).toFixed(2)}%, >6 ${(100*b[2]/b[0]).toFixed(2)}%`); }
    for(const band in stat.deep) console.log(`  пустоты ${band}: `+JSON.stringify(stat.deep[band]));
    console.log(`  под землёй: пустот ${(100*stat.hollow/total).toFixed(1)}%, руды ${(100*ores/total).toFixed(2)}%`);
    for(const [n,c] of Object.entries(stat.blocks).sort((a,b)=>b[1]-a[1])) console.log(`    ${(100*c/total).toFixed(3)}%  ${n}`);
  }catch(e){console.error(e);}
  bot.quit(); setTimeout(()=>process.exit(0),500);
});
bot.on('kicked',r=>{console.error('кик:',JSON.stringify(r));process.exit(1)});
bot.on('error',e=>{console.error('ошибка:',e.message);process.exit(1)});
setTimeout(()=>{console.error('время вышло');process.exit(1)},900000);
