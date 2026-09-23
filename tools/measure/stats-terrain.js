// Замер РЕЛЬЕФА оригинала: ступеньки, нависания, высоты/биомы, профили склонов гор,
// подземные пустоты и водоёмы. Копия и расширение stats.js.
// node stats-terrain.js <порт>
const path = require('path');
const HM = path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(HM, 'mineflayer'));
const Vec3 = require(path.join(HM, 'vec3')).Vec3;
const port = Number(process.argv[2]);
const R = 3; // радиус замера в чанках -> (7*16)^2 = 12544 столбцов на место

// 10 мест: 3 равнины, 7 гористых/плато (координаты взяты через /locate biome на этом же сервере).
const PLACES = [
  { x: 32, z: -32, biome: 'plains', mountain: false },
  { x: 352, z: 672, biome: 'sunflower_plains', mountain: false },
  { x: -192, z: -576, biome: 'meadow', mountain: false },
  { x: 96, z: 704, biome: 'windswept_hills', mountain: true },
  { x: 1600, z: -2912, biome: 'windswept_gravelly_hills', mountain: true },
  { x: 1312, z: 3872, biome: 'jagged_peaks', mountain: true },
  { x: -1440, z: -832, biome: 'frozen_peaks', mountain: true },
  { x: -1888, z: -352, biome: 'stony_peaks', mountain: true },
  { x: -2176, z: 3072, biome: 'badlands', mountain: true },
  { x: -1216, z: -320, biome: 'savanna_plateau', mountain: true },
];

const bot = mineflayer.createBot({ host: '127.0.0.1', port, username: 'stats', version: '26.1', auth: 'offline' });
const sleep = ms => new Promise(r => setTimeout(r, ms));
let lastChunk = Date.now();
bot._client.on('map_chunk', () => { lastChunk = Date.now(); });

const airNames = new Set(['air', 'cave_air']);
const hollowNames = new Set(['air', 'cave_air', 'water', 'lava']);
function ground(b) {
  return b && b.boundingBox === 'block' && !b.name.endsWith('_leaves') && !b.name.endsWith('_log') &&
    !b.name.endsWith('_wood') && b.name !== 'cactus' && !b.name.includes('mushroom_block') && b.name !== 'bamboo';
}
function surface(x, z) {
  for (let y = 319; y > -64; y--) {
    const b = bot.blockAt(new Vec3(x, y, z), false);
    if (ground(b)) return y;
  }
  return null;
}
function bandOf(h) {
  if (h < 70) return 'ниже 70';
  if (h < 100) return '70-99';
  if (h < 140) return '100-139';
  return '140+';
}
function deepBandOf(y) {
  if (y <= -55) return 'ниже -54';
  if (y <= -40) return '-54..-40';
  if (y <= -10) return '-39..-10';
  if (y <= 30) return '-10..30';
  return '30+';
}

function newStepAcc() {
  const bands = {};
  for (const bn of ['ниже 70', '70-99', '100-139', '140+']) bands[bn] = { pairs: 0, gt3: 0, gt6: 0, max: 0 };
  return bands;
}
function newOverhangAcc() {
  const bands = {};
  for (const bn of ['ниже 70', '70-99', '100-139', '140+']) bands[bn] = { cols: 0, hang: 0 };
  return bands;
}

async function measurePlace(place) {
  const { x: cx, z: cz, biome, mountain } = place;
  bot.chat(`/tp stats ${cx} 200 ${cz}`);
  await sleep(1500); lastChunk = Date.now();
  while (Date.now() - lastChunk < 4000) await sleep(500);

  const ox = Math.floor(cx / 16) * 16, oz = Math.floor(cz / 16) * 16;
  const H = {}; // 'x,z' -> height
  const heights = [];
  const deep = {}; // band -> {air,water,lava}
  const waterSet = new Set(); // 'x,y,z' for y<50 water, for flood fill
  let peak = { x: cx, z: cz, h: -1000 };

  for (let x = ox - R * 16; x < ox + R * 16 + 16; x++) {
    for (let z = oz - R * 16; z < oz + R * 16 + 16; z++) {
      const h = surface(x, z);
      if (h === null) continue;
      H[x + ',' + z] = h;
      heights.push(h);
      if (h > peak.h) peak = { x, z, h };

      // подземные пустоты: от -64 до min(h,62) - камера "под землёй", не считая крон/открытого неба выше моря
      const top = Math.min(h, 62);
      for (let y = -64; y < top; y++) {
        const b = bot.blockAt(new Vec3(x, y, z), false);
        if (!b) continue;
        if (hollowNames.has(b.name)) {
          const nm = b.name === 'cave_air' ? 'air' : b.name;
          const band = deepBandOf(y);
          deep[band] = deep[band] || { air: 0, water: 0, lava: 0 };
          if (nm === 'air') deep[band].air++;
          else if (nm === 'water') deep[band].water++;
          else if (nm === 'lava') deep[band].lava++;
          if (nm === 'water' && y < 50) waterSet.add(x + ',' + y + ',' + z);
        }
      }
    }
  }

  // ступеньки между соседями (>3 и >6), по полосе высоты НИЖНЕГО столбца
  const steps = newStepAcc();
  for (const k in H) {
    const [x, z] = k.split(',').map(Number);
    for (const [dx, dz] of [[1, 0], [0, 1]]) {
      const o = H[(x + dx) + ',' + (z + dz)];
      if (o === undefined) continue;
      const a = H[k];
      const d = Math.abs(a - o);
      const lo = Math.min(a, o);
      const bn = bandOf(lo);
      const s = steps[bn];
      s.pairs++;
      if (d > 3) s.gt3++;
      if (d > 6) s.gt6++;
      if (d > s.max) s.max = d;
    }
  }

  // нависания: под верхним твёрдым - воздух, а ниже (в пределах 30 блоков от поверхности) снова твёрдое
  const overhang = newOverhangAcc();
  for (const k in H) {
    const [x, z] = k.split(',').map(Number);
    const h = H[k];
    const bn = bandOf(h);
    overhang[bn].cols++;
    let sawAir = false, hasOverhang = false;
    for (let y = h - 1; y > h - 31; y--) {
      const b = bot.blockAt(new Vec3(x, y, z), false);
      if (!b) continue;
      if (!sawAir) {
        if (airNames.has(b.name)) sawAir = true;
      } else {
        if (ground(b)) { hasOverhang = true; break; }
      }
    }
    if (hasOverhang) overhang[bn].hang++;
  }

  // связные подземные водоёмы (y<50), 6-связность, только внутри собранного waterSet
  let waterBodies = 0;
  const visited = new Set();
  const neigh = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
  for (const key of waterSet) {
    if (visited.has(key)) continue;
    waterBodies++;
    const stack = [key];
    visited.add(key);
    while (stack.length) {
      const cur = stack.pop();
      const [cx2, cy2, cz2] = cur.split(',').map(Number);
      for (const [dx, dy, dz] of neigh) {
        const nk = (cx2 + dx) + ',' + (cy2 + dy) + ',' + (cz2 + dz);
        if (waterSet.has(nk) && !visited.has(nk)) { visited.add(nk); stack.push(nk); }
      }
    }
  }
  const areaBlocks = (R * 2 * 16 + 16) * (R * 2 * 16 + 16);

  // профили склона через вершину (только для гористых мест): 10 линий по 128 блоков
  let profiles = null;
  if (mountain) {
    profiles = [];
    for (let i = 0; i < 10; i++) {
      const ang = i * 18 * Math.PI / 180;
      const dx = Math.cos(ang), dz = Math.sin(ang);
      const row = [];
      for (let t = -64; t < 64; t++) {
        const px = Math.round(peak.x + dx * t), pz = Math.round(peak.z + dz * t);
        const hh = surface(px, pz);
        row.push(hh === null ? '.' : hh);
      }
      profiles.push(row);
    }
  }

  heights.sort((a, b) => a - b);
  const q = p => heights[Math.min(heights.length - 1, Math.floor(heights.length * p))];
  console.error(`  ${biome} @ ${cx},${cz}: столбцов ${Object.keys(H).length}, пик ${peak.h} на ${peak.x},${peak.z}`);

  return {
    biome, cx, cz, mountain,
    cols: Object.keys(H).length,
    peak,
    percentiles: { p0: heights[0], p5: q(0.05), p25: q(0.25), p50: q(0.5), p75: q(0.75), p95: q(0.95), p100: heights[heights.length - 1] },
    steps, overhang, deep,
    waterBodies, areaBlocks,
    profiles,
  };
}

bot.once('spawn', async () => {
  const results = [];
  try {
    bot.chat('/gamemode creative'); await sleep(1000);
    for (const place of PLACES) {
      const r = await measurePlace(place);
      results.push(r);
    }
  } catch (e) { console.error('ОШИБКА:', e); }
  console.log(JSON.stringify(results, null, 0));
  bot.quit(); setTimeout(() => process.exit(0), 500);
});
bot.on('kicked', r => { console.error('кик:', JSON.stringify(r)); process.exit(1); });
bot.on('error', e => { console.error('ошибка:', e.message); process.exit(1); });
setTimeout(() => { console.error('время вышло'); process.exit(1); }, 3600000);
