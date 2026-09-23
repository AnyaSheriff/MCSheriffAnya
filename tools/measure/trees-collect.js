'use strict';
const path = require('path');
const fs = require('fs');
const HOME_MODULES = path.join(process.env.HOME, 'node_modules');
const mineflayer = require(path.join(HOME_MODULES, 'mineflayer'));
const Vec3 = require(path.join(HOME_MODULES, 'vec3')).Vec3;

const OUT_DIR = __dirname;
const sleep = (ms) => new Promise(r => setTimeout(r, ms));

const SPECIES = [
  { id: 'oak', feature: 'minecraft:oak' },
  { id: 'fancy_oak', feature: 'minecraft:fancy_oak' },
  { id: 'birch', feature: 'minecraft:birch' },
  { id: 'super_birch_bees_0002', feature: 'minecraft:super_birch_bees_0002' },
  { id: 'spruce', feature: 'minecraft:spruce' },
  { id: 'pine', feature: 'minecraft:pine' },
  { id: 'mega_spruce', feature: 'minecraft:mega_spruce' },
  { id: 'mega_pine', feature: 'minecraft:mega_pine' },
  { id: 'jungle_tree', feature: 'minecraft:jungle_tree' },
  { id: 'mega_jungle_tree', feature: 'minecraft:mega_jungle_tree' },
  { id: 'jungle_bush', feature: 'minecraft:jungle_bush' },
  { id: 'acacia', feature: 'minecraft:acacia' },
  { id: 'dark_oak', feature: 'minecraft:dark_oak' },
  { id: 'cherry', feature: 'minecraft:cherry' },
  { id: 'mangrove', feature: 'minecraft:mangrove', ground: 'water' },
  { id: 'tall_mangrove', feature: 'minecraft:tall_mangrove', ground: 'water' },
  { id: 'pale_oak', feature: 'minecraft:pale_oak' },
  { id: 'azalea_tree', feature: 'minecraft:azalea_tree' },
  { id: 'huge_red_mushroom', feature: 'minecraft:huge_red_mushroom' },
  { id: 'huge_brown_mushroom', feature: 'minecraft:huge_brown_mushroom' },
];

const BASE_X = 3000, BASE_Z = 3000, PITCH = 240, COLS = 5;
const SPACING = 36, GRID = 2; // grid indices -2..2 => 25 samples
const FLOOR_HALF = 85;
const SCAN_HALF = 85;
const DY_MIN = -14, DY_MAX = 42;
const FEATURE_Y = -60, GROUND_Y = -61;
const EXCLUDE = new Set(['air', 'grass_block', 'mud', 'stone', 'bedrock', 'water']);

const bot = mineflayer.createBot({ host: '127.0.0.1', port: 25566, username: 'blackbox', version: '26.1', auth: 'offline' });
const say = async (cmd) => { bot.chat('/' + cmd); await sleep(Number(process.env.CMD_DELAY_MS || 120)); };

function plotCenter(i) {
  const col = i % COLS, row = Math.floor(i / COLS);
  return { x: BASE_X + col * PITCH, z: BASE_Z + row * PITCH };
}

async function buildGround(sp, center) {
  const x1 = center.x - FLOOR_HALF, x2 = center.x + FLOOR_HALF;
  const z1 = center.z - FLOOR_HALF, z2 = center.z + FLOOR_HALF;
  if (sp.ground === 'water') {
    // Яма под грязью: корни мангра должны иметь куда расти вниз.
    // Замечено на пробах: если ямы глубже 2 блоков, /place feature вообще
    // не ставит дерево (видимо, ищет твёрдое дно в пределах небольшой
    // глубины и при неудаче отменяет всю генерацию) — держим яму мелкой.
    for (let y = GROUND_Y - 1; y >= GROUND_Y - 2; y--) {
      await say(`fill ${x1} ${y} ${z1} ${x2} ${y} ${z2} minecraft:water`);
    }
    await say(`fill ${x1} ${GROUND_Y} ${z1} ${x2} ${GROUND_Y} ${z2} minecraft:mud`);
    await say(`fill ${x1} ${FEATURE_Y} ${z1} ${x2} ${FEATURE_Y} ${z2} minecraft:water`);
  } else {
    await say(`fill ${x1} ${GROUND_Y} ${z1} ${x2} ${GROUND_Y} ${z2} minecraft:grass_block`);
  }
}

async function placeSamples(sp, center) {
  for (let gx = -GRID; gx <= GRID; gx++) {
    for (let gz = -GRID; gz <= GRID; gz++) {
      const x = center.x + gx * SPACING, z = center.z + gz * SPACING;
      await say(`place feature ${sp.feature} ${x} ${FEATURE_Y} ${z}`);
    }
  }
}

function bucketize(center, blocks) {
  // blocks: array of {x,y,z,name,props}
  const samples = {};
  for (let gx = -GRID; gx <= GRID; gx++) for (let gz = -GRID; gz <= GRID; gz++) samples[`${gx},${gz}`] = [];
  for (const b of blocks) {
    let gx = Math.round((b.x - center.x) / SPACING);
    let gz = Math.round((b.z - center.z) / SPACING);
    if (gx < -GRID) gx = -GRID; if (gx > GRID) gx = GRID;
    if (gz < -GRID) gz = -GRID; if (gz > GRID) gz = GRID;
    const ox = center.x + gx * SPACING, oz = center.z + gz * SPACING;
    samples[`${gx},${gz}`].push({ dx: b.x - ox, dy: b.y - FEATURE_Y, dz: b.z - oz, name: b.name, props: b.props });
  }
  return Object.values(samples);
}

async function scanPlot(center) {
  const x1 = center.x - SCAN_HALF, x2 = center.x + SCAN_HALF;
  const z1 = center.z - SCAN_HALF, z2 = center.z + SCAN_HALF;
  const y1 = FEATURE_Y + DY_MIN, y2 = FEATURE_Y + DY_MAX;
  const found = [];
  for (let x = x1; x <= x2; x++) {
    for (let z = z1; z <= z2; z++) {
      for (let y = y1; y <= y2; y++) {
        const b = bot.blockAt(new Vec3(x, y, z));
        if (!b) continue;
        const name = b.name;
        if (EXCLUDE.has(name)) continue;
        let props = {};
        try { props = b.getProperties ? b.getProperties() : (b._properties || {}); } catch (e) {}
        found.push({ x, y, z, name, props });
      }
    }
  }
  return found;
}

(async () => {
  try {
    bot.once('spawn', () => {});
    await new Promise((resolve) => bot.once('spawn', resolve));
    await sleep(1500);
    await say('gamemode creative');
    await say('weather clear 999999');
    console.log('setup done, starting species loop');

    const results = {};
    const LIMIT = process.env.SPECIES_LIMIT ? Number(process.env.SPECIES_LIMIT) : SPECIES.length;
    const RETRY = process.env.RETRY_IDS ? new Set(process.env.RETRY_IDS.split(',')) : null;
    for (let i = 0; i < LIMIT; i++) {
      const sp = SPECIES[i];
      if (RETRY && !RETRY.has(sp.id)) continue;
      const center = plotCenter(i);
      console.log(`[${i + 1}/${SPECIES.length}] ${sp.id} @ ${center.x},${center.z}`);
      await say(`tp blackbox ${center.x} 40 ${center.z}`);
      await sleep(Number(process.env.SETTLE_MS || 3000));
      await buildGround(sp, center);
      await sleep(800);
      await placeSamples(sp, center);
      await sleep(1500);
      const raw = await scanPlot(center);
      const samples = bucketize(center, raw);
      const outFile = path.join(OUT_DIR, `data-${sp.id}.json`);
      fs.writeFileSync(outFile, JSON.stringify({ species: sp.id, feature: sp.feature, spacing: SPACING, samples }, null, 0));
      console.log(`  -> ${raw.length} blocks, saved ${outFile}`);
      results[sp.id] = { blocks: raw.length, file: outFile };
    }

    fs.writeFileSync(path.join(OUT_DIR, 'collect-summary.json'), JSON.stringify(results, null, 1));
    console.log('ALL DONE');
    bot.quit();
    setTimeout(() => process.exit(0), 500);
  } catch (e) {
    console.error('FATAL', e);
    process.exit(1);
  }
})();

bot.on('kicked', (r) => { console.error('кик:', JSON.stringify(r)); process.exit(1); });
bot.on('error', (e) => { console.error('ошибка:', e.message); process.exit(1); });
bot.on('message', (m) => { const s = m.toString(); if (s && /Unknown|error|Incorrect|cannot|too many|Invalid|not loaded|not exist/i.test(s)) console.error('  чат:', s); });
setTimeout(() => { console.error('время вышло (глобальный таймаут)'); process.exit(1); }, 60 * 60 * 1000);
