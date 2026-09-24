// Второй замер рельефа оригинала: ещё 2 места на каждый из 10 биомов первого замера
// (не ближе 1000 блоков к прежнему месту), 9 новых биомов по 2 места, и поперечные
// профили 10 речных мест (ширина водной глади на уровне моря, глубина дна).
// Копия и расширение stats-terrain.js. Код Minecraft и ядер не читался — только
// сетевой протокол и команды /tp, /locate biome, /gamemode через робота-оператора.
//
// node stats-terrain2.js <порт> [outPrefix]
// Переменные окружения для отладки (частичный прогон): OLD_LIMIT, NEW_LIMIT, RIVER_LIMIT
'use strict';
const path = require('path');
const fs = require('fs');
const HM = path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(HM, 'mineflayer'));
const Vec3 = require(path.join(HM, 'vec3')).Vec3;
const port = Number(process.argv[2]);
const OUT_PREFIX = process.argv[3] || path.join(process.env.HOME, '.cache/mcsheriffanya/measure-tmp/second');
const R = 3; // радиус замера в чанках -> (7*16)^2 = 12544 столбцов на место (как в первом замере)

const OLD_BIOMES = [
  { biome: 'plains', x: 32, z: -32 },
  { biome: 'sunflower_plains', x: 352, z: 672 },
  { biome: 'meadow', x: -192, z: -576 },
  { biome: 'windswept_hills', x: 96, z: 704 },
  { biome: 'windswept_gravelly_hills', x: 1600, z: -2912 },
  { biome: 'jagged_peaks', x: 1312, z: 3872 },
  { biome: 'frozen_peaks', x: -1440, z: -832 },
  { biome: 'stony_peaks', x: -1888, z: -352 },
  { biome: 'badlands', x: -2176, z: 3072 },
  { biome: 'savanna_plateau', x: -1216, z: -320 },
];
const NEW_BIOMES = ['snowy_slopes', 'grove', 'stony_shore', 'beach', 'river',
  'old_growth_spruce_taiga', 'dark_forest', 'desert', 'savanna'];

const OLD_LIMIT = process.env.OLD_LIMIT ? Number(process.env.OLD_LIMIT) : OLD_BIOMES.length;
const NEW_LIMIT = process.env.NEW_LIMIT ? Number(process.env.NEW_LIMIT) : NEW_BIOMES.length;
const RIVER_LIMIT = process.env.RIVER_LIMIT ? Number(process.env.RIVER_LIMIT) : 10;

const bot = mineflayer.createBot({ host: '127.0.0.1', port, username: 'stats', version: '26.1', auth: 'offline' });
const sleep = ms => new Promise(r => setTimeout(r, ms));
let lastChunk = Date.now();
bot._client.on('map_chunk', () => { lastChunk = Date.now(); });

function log(...a) { console.error(...a); }
function dist(a, b) { return Math.hypot(a.x - b.x, a.z - b.z); }

async function tpOnly(x, z, y = 200) {
  bot.chat(`/tp stats ${x} ${y} ${z}`);
  await sleep(700);
}
async function tpAndSettle(x, z, y = 200) {
  bot.chat(`/tp stats ${x} ${y} ${z}`);
  await sleep(1200); lastChunk = Date.now();
  while (Date.now() - lastChunk < 3000) await sleep(400);
}

// tpAndSettle + проверка, что центр реально загрузился (surface() не null);
// при неудаче ре-телепортирует (сброс кэша чанков клиента) до maxTries раз.
async function tpSettleVerified(x, z, y, checkFn, maxTries = 4) {
  for (let attempt = 1; attempt <= maxTries; attempt++) {
    await tpAndSettle(x, z, y);
    if (checkFn()) return true;
    log(`    центр ${x},${z} не загрузился (попытка ${attempt}/${maxTries}), жду ещё и пробую снова...`);
    await sleep(1500);
    // небольшое смещение туда-обратно форсирует пересылку чанков сервером
    bot.chat(`/tp stats ${x} ${(y || 200) + 1} ${z}`);
    await sleep(800);
  }
  return false;
}

function locateBiome(biomeId, timeoutMs = 18000) {
  return new Promise((resolve) => {
    const re = new RegExp(`The nearest minecraft:${biomeId} is at \\[(-?\\d+), (-?\\d+), (-?\\d+)\\] \\((\\d+) blocks away\\)`);
    const failRe = /Could not find|Unknown biome/i;
    let done = false;
    function handler(m) {
      if (done) return;
      const s = m.toString();
      const mm = s.match(re);
      if (mm) { done = true; bot.removeListener('message', handler); resolve({ x: +mm[1], y: +mm[2], z: +mm[3], dist: +mm[4] }); }
      else if (failRe.test(s)) { done = true; bot.removeListener('message', handler); log('    /locate не нашёл:', s); resolve(null); }
    }
    bot.on('message', handler);
    bot.chat(`/locate biome minecraft:${biomeId}`);
    setTimeout(() => { if (!done) { done = true; bot.removeListener('message', handler); log('    /locate таймаут для', biomeId); resolve(null); } }, timeoutMs);
  });
}

// Ищет место биома biomeId не ближе minDist от всех точек avoid (2D, по x,z).
// Пробует несколько анкерных точек по кругу растущего радиуса от (baseX,baseZ).
async function findDistantBiome(biomeId, avoid, minDist, baseX, baseZ) {
  const tries = [
    [1800, 0], [1800, 72], [1800, 144], [1800, 216], [1800, 288],
    [3200, 36], [3200, 108], [3200, 180], [3200, 252], [3200, 324],
    [5000, 0], [5000, 90], [5000, 180], [5000, 270],
    [7000, 45], [7000, 135], [7000, 225], [7000, 315],
  ];
  let best = null, bestMin = -1;
  for (const [radius, angleDeg] of tries) {
    const rad = angleDeg * Math.PI / 180;
    const ax = Math.round(baseX + radius * Math.cos(rad));
    const az = Math.round(baseZ + radius * Math.sin(rad));
    await tpOnly(ax, az);
    const loc = await locateBiome(biomeId);
    if (!loc) continue;
    const dmin = avoid.length ? Math.min(...avoid.map(p => dist(p, loc))) : Infinity;
    if (dmin > bestMin) { best = loc; bestMin = dmin; }
    if (dmin >= minDist) return { loc, ok: true, dmin };
  }
  return { loc: best, ok: false, dmin: bestMin };
}

// ---- измерение места (как в stats-terrain.js, без профилей/пустот — не нужны для задачи) ----
const airNames = new Set(['air', 'cave_air']);
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

async function measurePlace(biome, cx, cz, tag) {
  const ok = await tpSettleVerified(cx, cz, 200, () => surface(cx, cz) !== null);
  if (!ok) log(`  !! ${biome} [${tag}] @ ${cx},${cz}: центр так и не загрузился, замер может быть неполным`);
  const ox = Math.floor(cx / 16) * 16, oz = Math.floor(cz / 16) * 16;
  const H = {};
  const heights = [];
  let peak = { x: cx, z: cz, h: -1000 };

  for (let x = ox - R * 16; x < ox + R * 16 + 16; x++) {
    for (let z = oz - R * 16; z < oz + R * 16 + 16; z++) {
      const h = surface(x, z);
      if (h === null) continue;
      H[x + ',' + z] = h;
      heights.push(h);
      if (h > peak.h) peak = { x, z, h };
    }
  }

  const steps = newStepAcc();
  for (const k in H) {
    const [x, z] = k.split(',').map(Number);
    for (const [dx, dz] of [[1, 0], [0, 1]]) {
      const o = H[(x + dx) + ',' + (z + dz)];
      if (o === undefined) continue;
      const a = H[k];
      const d = Math.abs(a - o);
      const lo = Math.min(a, o);
      const s = steps[bandOf(lo)];
      s.pairs++;
      if (d > 3) s.gt3++;
      if (d > 6) s.gt6++;
      if (d > s.max) s.max = d;
    }
  }

  const overhang = newOverhangAcc();
  for (const k in H) {
    const [x, z] = k.split(',').map(Number);
    const h = H[k];
    const o = overhang[bandOf(h)];
    o.cols++;
    let sawAir = false, hasOverhang = false;
    for (let y = h - 1; y > h - 31; y--) {
      const b = bot.blockAt(new Vec3(x, y, z), false);
      if (!b) continue;
      if (!sawAir) { if (airNames.has(b.name)) sawAir = true; }
      else { if (ground(b)) { hasOverhang = true; break; } }
    }
    if (hasOverhang) o.hang++;
  }

  heights.sort((a, b) => a - b);
  const q = p => heights[Math.min(heights.length - 1, Math.floor(heights.length * p))];
  log(`  ${biome} [${tag}] @ ${cx},${cz}: столбцов ${heights.length}, пик ${peak.h} на ${peak.x},${peak.z}`);

  return {
    biome, tag, cx, cz, cols: heights.length, peak,
    percentiles: { p0: heights[0], p5: q(0.05), p25: q(0.25), p50: q(0.5), p75: q(0.75), p95: q(0.95), p100: heights[heights.length - 1] },
    steps, overhang, heights,
  };
}

// ---- поперечный профиль реки ----
function isWaterName(name) { return name === 'water'; }
function waterTop(x, z, yFrom = 100, yTo = 40) {
  for (let y = yFrom; y >= yTo; y--) {
    const b = bot.blockAt(new Vec3(x, y, z), false);
    if (!b) continue;
    if (isWaterName(b.name)) {
      const above = bot.blockAt(new Vec3(x, y + 1, z), false);
      if (!above || !isWaterName(above.name)) return y;
    }
  }
  return null;
}
function bedYBelow(x, z, fromY) {
  for (let y = fromY; y > fromY - 40; y--) {
    const b = bot.blockAt(new Vec3(x, y, z), false);
    if (!b) continue;
    if (ground(b) && b.name !== 'water') return y;
  }
  return null;
}

async function riverProfile(x0, z0) {
  await tpSettleVerified(x0, z0, 100, () => surface(x0, z0) !== null || waterTop(x0, z0) !== null);
  // найти опорную точку с водой рядом с (x0,z0)
  let seaY = waterTop(x0, z0);
  let cx = x0, cz = z0;
  if (seaY === null) {
    const found = [];
    for (let dx = -10; dx <= 10; dx++) for (let dz = -10; dz <= 10; dz++) {
      const wy = waterTop(x0 + dx, z0 + dz);
      if (wy !== null) found.push({ x: x0 + dx, z: z0 + dz, y: wy, d: Math.hypot(dx, dz) });
    }
    if (!found.length) {
      log(`  river @ ${x0},${z0}: вода не найдена рядом, пропуск`);
      return null;
    }
    found.sort((a, b) => a.d - b.d);
    cx = found[0].x; cz = found[0].z; seaY = found[0].y;
  }

  // маска "вода на уровне моря" (== seaY) в коробке радиуса 30 вокруг опорной точки
  const RAD = 30;
  const mask = {}; // 'x,z' -> waterTop y (или null)
  const pts = [];
  for (let dx = -RAD; dx <= RAD; dx++) for (let dz = -RAD; dz <= RAD; dz++) {
    const x = cx + dx, z = cz + dz;
    const wy = waterTop(x, z);
    mask[x + ',' + z] = wy;
    if (wy === seaY) pts.push({ x, z });
  }
  if (pts.length < 5) {
    log(`  river @ ${x0},${z0}: маловато точек воды (${pts.length}), профиль ненадёжен`);
  }
  // PCA направление русла
  let mx = 0, mz = 0;
  for (const p of pts) { mx += p.x; mz += p.z; }
  mx /= pts.length || 1; mz /= pts.length || 1;
  let cxx = 0, czz = 0, cxz = 0;
  for (const p of pts) { const dx = p.x - mx, dz = p.z - mz; cxx += dx * dx; czz += dz * dz; cxz += dx * dz; }
  const theta = pts.length >= 5 ? 0.5 * Math.atan2(2 * cxz, cxx - czz) : 0;
  const flow = { x: Math.cos(theta), z: Math.sin(theta) };
  const perp = { x: -Math.sin(theta), z: Math.cos(theta) };

  const lines = [];
  for (const off of [-6, 0, 6]) {
    const lcx = cx + flow.x * off, lcz = cz + flow.z * off;
    const samples = [];
    for (let t = -25; t <= 25; t++) {
      const px = Math.round(lcx + perp.x * t), pz = Math.round(lcz + perp.z * t);
      const key = px + ',' + pz;
      let wy = mask[key];
      if (wy === undefined) wy = waterTop(px, pz);
      samples.push({ t, x: px, z: pz, isWater: wy === seaY });
    }
    // самый длинный непрерывный прогон "воды", содержащий t=0 если возможно
    let bestRun = null, curStart = null;
    const runs = [];
    for (let i = 0; i < samples.length; i++) {
      if (samples[i].isWater) { if (curStart === null) curStart = i; }
      else { if (curStart !== null) { runs.push([curStart, i - 1]); curStart = null; } }
    }
    if (curStart !== null) runs.push([curStart, samples.length - 1]);
    const zeroIdx = samples.findIndex(s => s.t === 0);
    let chosen = runs.find(([a, b]) => a <= zeroIdx && zeroIdx <= b);
    if (!chosen && runs.length) chosen = runs.reduce((a, b) => (b[1] - b[0] > a[1] - a[0] ? b : a));
    let width = 0, depths = [];
    if (chosen) {
      width = chosen[1] - chosen[0] + 1;
      for (let i = chosen[0]; i <= chosen[1]; i++) {
        const s = samples[i];
        const bed = bedYBelow(s.x, s.z, seaY - 1);
        if (bed !== null) depths.push(seaY - bed);
      }
    }
    lines.push({ off, width, depthMax: depths.length ? Math.max(...depths) : null, depthAvg: depths.length ? depths.reduce((a, b) => a + b, 0) / depths.length : null, nDepth: depths.length });
  }

  const validLines = lines.filter(l => l.width > 0);
  const widthAvg = validLines.length ? validLines.reduce((a, l) => a + l.width, 0) / validLines.length : 0;
  const depthMax = validLines.length ? Math.max(...validLines.map(l => l.depthMax || 0)) : null;
  const allDepthSum = validLines.reduce((a, l) => a + (l.depthAvg || 0) * l.nDepth, 0);
  const allDepthN = validLines.reduce((a, l) => a + l.nDepth, 0);
  const depthAvg = allDepthN ? allDepthSum / allDepthN : null;

  log(`  river @ ${x0},${z0} (опора ${cx},${cz}, seaY=${seaY}): ширина~${widthAvg.toFixed(1)}, глубина макс ${depthMax}, средняя ${depthAvg ? depthAvg.toFixed(2) : null}`);
  return { x0, z0, cx, cz, seaY, nWaterPts: pts.length, lines, widthAvg, depthMax, depthAvg };
}

// ---- главный поток ----
bot.once('spawn', async () => {
  const placesOut = OUT_PREFIX + '-places.json';
  const riverOut = OUT_PREFIX + '-rivers.json';
  const planOut = OUT_PREFIX + '-plan.json';
  try {
    bot.chat('/gamemode creative'); await sleep(1000);

    // Фаза A: находим координаты всех мест (быстро, без ожидания чанков)
    log('=== Фаза A: поиск мест через /locate biome ===');
    const plan = []; // { biome, locations: [{x,z,tag}] }

    for (let i = 0; i < Math.min(OLD_LIMIT, OLD_BIOMES.length); i++) {
      const ob = OLD_BIOMES[i];
      log(`-- ${ob.biome} (старое место ${ob.x},${ob.z}) --`);
      const avoid = [{ x: ob.x, z: ob.z }];
      const locations = [{ x: ob.x, z: ob.z, tag: 'исходное (перезамерено)' }];
      for (let n = 0; n < 2; n++) {
        const res = await findDistantBiome(ob.biome, avoid, 1000, ob.x, ob.z);
        if (!res.loc) { log(`   !! не нашли ${ob.biome} место #${n + 2}`); continue; }
        avoid.push({ x: res.loc.x, z: res.loc.z });
        const tag = res.ok ? `новое (мин. расст. ${Math.round(res.dmin)})` : `новое, ПРЕДУПРЕЖДЕНИЕ мин. расст. только ${Math.round(res.dmin)}`;
        locations.push({ x: res.loc.x, z: res.loc.z, tag });
        log(`   #${n + 2}: ${res.loc.x},${res.loc.z} (${tag})`);
      }
      plan.push({ biome: ob.biome, locations, isOld: true });
    }

    const riverCandidates = [];
    for (let i = 0; i < Math.min(NEW_LIMIT, NEW_BIOMES.length); i++) {
      const nb = NEW_BIOMES[i];
      log(`-- ${nb} (новый биом) --`);
      const avoid = [];
      const locations = [];
      for (let n = 0; n < 2; n++) {
        const res = await findDistantBiome(nb, avoid, 800, 0, 0);
        if (!res.loc) { log(`   !! не нашли ${nb} место #${n + 1}`); continue; }
        avoid.push({ x: res.loc.x, z: res.loc.z });
        const tag = res.ok ? `место ${n + 1} (мин. расст. ${Math.round(res.dmin)})` : `место ${n + 1}, ПРЕДУПРЕЖДЕНИЕ мин. расст. ${Math.round(res.dmin)}`;
        locations.push({ x: res.loc.x, z: res.loc.z, tag });
        log(`   #${n + 1}: ${res.loc.x},${res.loc.z} (${tag})`);
        if (nb === 'river') riverCandidates.push({ x: res.loc.x, z: res.loc.z });
      }
      plan.push({ biome: nb, locations, isOld: false });
    }

    // добираем речные места для поперечных профилей (нужно RIVER_LIMIT, обычно 10)
    log('-- дополнительные речные места для поперечных профилей --');
    const riverAvoid = riverCandidates.slice();
    let angleBase = 20;
    while (riverCandidates.length < RIVER_LIMIT) {
      const res = await findDistantBiome('river', riverAvoid, 300, 0, 0);
      if (!res.loc) { angleBase += 40; continue; }
      riverAvoid.push({ x: res.loc.x, z: res.loc.z });
      riverCandidates.push({ x: res.loc.x, z: res.loc.z });
      log(`   река-профиль #${riverCandidates.length}: ${res.loc.x},${res.loc.z}`);
    }

    fs.writeFileSync(planOut, JSON.stringify({ plan, riverCandidates }, null, 1));
    log('План сохранён:', planOut);

    // Фаза B: полный замер каждого места
    log('=== Фаза B: замер мест (percentiles/steps/overhang) ===');
    const placeResults = [];
    for (const entry of plan) {
      for (const loc of entry.locations) {
        const r = await measurePlace(entry.biome, loc.x, loc.z, loc.tag);
        placeResults.push(r);
        fs.writeFileSync(placesOut, JSON.stringify(placeResults, null, 0));
      }
    }
    log('Места сохранены:', placesOut);

    // Фаза C: речные поперечные профили
    log('=== Фаза C: поперечные профили рек ===');
    const riverResults = [];
    for (const rc of riverCandidates.slice(0, RIVER_LIMIT)) {
      const r = await riverProfile(rc.x, rc.z);
      if (r) riverResults.push(r);
      fs.writeFileSync(riverOut, JSON.stringify(riverResults, null, 0));
    }
    log('Реки сохранены:', riverOut);

    log('ГОТОВО');
  } catch (e) {
    log('ОШИБКА:', e.stack || e);
  }
  bot.quit(); setTimeout(() => process.exit(0), 500);
});
bot.on('kicked', r => { log('кик:', JSON.stringify(r)); process.exit(1); });
bot.on('error', e => { log('ошибка:', e.message); process.exit(1); });
setTimeout(() => { log('время вышло (глобальный таймаут)'); process.exit(1); }, 3 * 3600 * 1000);
