'use strict';
const fs = require('fs');
const path = require('path');
const DIR = __dirname;

const SPECIES_IDS = [
  'oak', 'fancy_oak', 'birch', 'super_birch_bees_0002', 'spruce', 'pine',
  'mega_spruce', 'mega_pine', 'jungle_tree', 'mega_jungle_tree', 'jungle_bush',
  'acacia', 'dark_oak', 'cherry', 'mangrove', 'tall_mangrove', 'pale_oak',
  'azalea_tree', 'huge_red_mushroom', 'huge_brown_mushroom'
];

const isLog = (name) => /_log$|_wood$|mushroom_stem$/.test(name);
const isLeafLike = (name) => /_leaves$|red_mushroom_block$|brown_mushroom_block$/.test(name);
const SPECIAL_NAMES = new Set([
  'vine', 'cave_vines', 'cave_vines_plant', 'cocoa', 'mangrove_roots', 'muddy_mangrove_roots',
  'moss_block', 'moss_carpet', 'pale_moss_carpet', 'pale_hanging_moss', 'pale_moss_block',
  'bee_nest', 'podzol', 'hanging_roots', 'azalea', 'flowering_azalea', 'dirt', 'water',
  'shroomlight', 'mushroom_stem', 'creaking_heart', 'sculk_vein', 'lily_pad', 'moss_patch',
  'rooted_dirt', 'coarse_dirt'
]);

function key(dx, dz) { return dx + ',' + dz; }

function analyzeSample(blocks) {
  // group logs by column
  const cols = new Map(); // key -> {dyList:[], axisSet:Set}
  for (const b of blocks) {
    if (isLog(b.name)) {
      const k = key(b.dx, b.dz);
      if (!cols.has(k)) cols.set(k, { dx: b.dx, dz: b.dz, dyList: [], axes: [] });
      const c = cols.get(k);
      c.dyList.push(b.dy);
      c.axes.push((b.props && b.props.axis) || 'y');
    }
  }
  // vertical run length per column (only counting axis=y logs, contiguous from min)
  let best = { runLen: 0 };
  const colRuns = [];
  for (const [k, c] of cols) {
    const ys = c.dyList.filter((dy, i) => c.axes[i] === 'y').sort((a, b) => a - b);
    if (!ys.length) continue;
    // contiguous run starting at ys[0]
    let runLen = 1;
    for (let i = 1; i < ys.length; i++) { if (ys[i] === ys[i - 1] + 1) runLen++; else break; }
    const info = { dx: c.dx, dz: c.dz, runLen, minY: ys[0], maxY: ys[0] + runLen - 1 };
    colRuns.push(info);
    if (runLen > best.runLen) best = info;
  }
  const maxLen = best.runLen || 0;
  // trunk columns: those starting at the same base height as the tallest column
  // (a 2x2 dark-oak-style trunk is not always perfectly even on all 4 corners).
  const trunkCols = colRuns.filter(c => c.minY === best.minY);
  // check for 2x2: any adjacent set of up to 4 columns forming unit square
  let trunkWidth = '1x1';
  let trunkCenter = { dx: best.dx, dz: best.dz };
  if (trunkCols.length >= 2) {
    // find columns adjacent to best (within 1 block, forming 2x2)
    const near = trunkCols.filter(c => Math.abs(c.dx - best.dx) <= 1 && Math.abs(c.dz - best.dz) <= 1);
    if (near.length >= 3) {
      trunkWidth = '2x2';
      trunkCenter = {
        dx: near.reduce((s, c) => s + c.dx, 0) / near.length,
        dz: near.reduce((s, c) => s + c.dz, 0) / near.length
      };
    }
  }
  const trunkColKeys = new Set(
    (trunkWidth === '2x2' ? trunkCols.filter(c => Math.abs(c.dx - best.dx) <= 1 && Math.abs(c.dz - best.dz) <= 1) : [best])
      .map(c => key(c.dx, c.dz))
  );
  const trunkHeight = maxLen;
  const trunkBaseY = best.minY;

  // branch logs: any log block not in (trunkColKeys AND within main run range)
  const branches = [];
  for (const b of blocks) {
    if (!isLog(b.name)) continue;
    const k = key(b.dx, b.dz);
    const axis = (b.props && b.props.axis) || 'y';
    const inTrunk = trunkColKeys.has(k) && axis === 'y' && b.dy >= trunkBaseY && b.dy <= trunkBaseY + trunkHeight - 1;
    if (!inTrunk) {
      branches.push({ dx: b.dx, dz: b.dz, dy: b.dy, axis, relDy: b.dy - trunkBaseY, name: b.name });
    }
  }

  // canopy layers: group leaf-like by dy (relative to trunk top)
  const topY = trunkBaseY + trunkHeight - 1;
  const layers = new Map(); // relLayer(from top) -> array of {dx,dz}
  for (const b of blocks) {
    if (!isLeafLike(b.name)) continue;
    const rel = b.dy - topY;
    if (!layers.has(rel)) layers.set(rel, []);
    layers.get(rel).push({ dx: b.dx - trunkCenter.dx, dz: b.dz - trunkCenter.dz });
  }
  const layerStats = [];
  for (const [rel, pts] of layers) {
    let maxR = 0, corners = 0;
    const cornerSet = new Set();
    for (const p of pts) {
      const r = Math.max(Math.abs(p.dx), Math.abs(p.dz));
      if (r > maxR) maxR = r;
    }
    for (const p of pts) {
      if (Math.abs(p.dx) === maxR && Math.abs(p.dz) === maxR && maxR > 0) cornerSet.add(p.dx + ',' + p.dz);
    }
    layerStats.push({ rel, radius: maxR, count: pts.length, corners: cornerSet.size });
  }

  // special blocks
  const specials = [];
  for (const b of blocks) {
    if (SPECIAL_NAMES.has(b.name)) {
      specials.push({ name: b.name, dx: b.dx - trunkCenter.dx, dz: b.dz - trunkCenter.dz, dy: b.dy - topY, dyAbs: b.dy, props: b.props });
    }
  }

  return { trunkHeight, trunkWidth, trunkBaseY, trunkCenter, branches, layerStats, specials, topY };
}

function summarizeSpecies(id) {
  const file = path.join(DIR, `data-${id}.json`);
  if (!fs.existsSync(file)) return null;
  const data = JSON.parse(fs.readFileSync(file));
  const perSample = data.samples.map(analyzeSample);

  const heights = perSample.map(s => s.trunkHeight).filter(h => h > 0);
  const widths = {};
  for (const s of perSample) widths[s.trunkWidth] = (widths[s.trunkWidth] || 0) + 1;

  // branch aggregation: relDy distribution, axis distribution, length distribution (count contiguous per direction - approx by count of branch blocks per sample)
  const branchRelDy = [];
  const branchAxis = {};
  let samplesWithBranches = 0;
  for (const s of perSample) {
    if (s.branches.length) samplesWithBranches++;
    for (const b of s.branches) {
      branchRelDy.push(b.relDy);
      branchAxis[b.axis] = (branchAxis[b.axis] || 0) + 1;
    }
  }

  // canopy layer aggregation by rel (from top)
  const layerAgg = new Map();
  for (const s of perSample) {
    for (const l of s.layerStats) {
      if (!layerAgg.has(l.rel)) layerAgg.set(l.rel, []);
      layerAgg.get(l.rel).push(l);
    }
  }
  const canopy = [...layerAgg.entries()].sort((a, b) => a[0] - b[0]).map(([rel, arr]) => {
    const radii = arr.map(a => a.radius);
    const corners = arr.map(a => a.corners);
    return {
      rel,
      presentIn: arr.length,
      radiusMin: Math.min(...radii),
      radiusMax: Math.max(...radii),
      radiusAvg: +(radii.reduce((a, b) => a + b, 0) / radii.length).toFixed(2),
      cornersAvg: +(corners.reduce((a, b) => a + b, 0) / corners.length).toFixed(2)
    };
  });

  // special blocks aggregation
  const specialAgg = new Map();
  for (const s of perSample) {
    const seen = new Set();
    for (const sp of s.specials) {
      if (!specialAgg.has(sp.name)) specialAgg.set(sp.name, { count: 0, samples: new Set(), dyMin: 999, dyMax: -999, dyAbsList: [] });
      const a = specialAgg.get(sp.name);
      a.count++;
      seen.add(sp.name);
      if (sp.dy < a.dyMin) a.dyMin = sp.dy;
      if (sp.dy > a.dyMax) a.dyMax = sp.dy;
    }
    for (const nm of seen) specialAgg.get(nm).samples.add(perSample.indexOf(s));
  }
  const specials = [...specialAgg.entries()].map(([name, a]) => ({
    name, totalCount: a.count, sampleFraction: +(a.samples.size / perSample.length).toFixed(2),
    avgPerSample: +(a.count / perSample.length).toFixed(2), dyMinFromTop: a.dyMin, dyMaxFromTop: a.dyMax
  }));

  return {
    id, sampleCount: perSample.length,
    heights, heightMin: Math.min(...heights), heightMax: Math.max(...heights),
    heightAvg: +(heights.reduce((a, b) => a + b, 0) / heights.length).toFixed(2),
    widths, samplesWithBranches,
    branchRelDyMin: branchRelDy.length ? Math.min(...branchRelDy) : null,
    branchRelDyMax: branchRelDy.length ? Math.max(...branchRelDy) : null,
    branchAxis, branchCount: branchRelDy.length,
    canopy, specials,
    perSample // keep for ASCII generation
  };
}

const OUT = {};
for (const id of SPECIES_IDS) {
  const s = summarizeSpecies(id);
  if (!s) { console.error('MISSING', id); continue; }
  OUT[id] = s;
  console.log(id, 'height', s.heightMin, '-', s.heightMax, 'avg', s.heightAvg, 'widths', JSON.stringify(s.widths), 'branches', s.branchCount, 'in', s.samplesWithBranches, '/', s.sampleCount, 'canopyLayers', s.canopy.length, 'specials', s.specials.map(x => x.name).join(','));
}

// Save full analysis (without perSample raw dumps, too large) + a slim ascii-source file with 3 chosen samples per species
const slim = {};
for (const id of Object.keys(OUT)) {
  const { perSample, ...rest } = OUT[id];
  slim[id] = rest;
}
fs.writeFileSync(path.join(DIR, 'analysis.json'), JSON.stringify(slim, null, 1));

// ASCII layer slices for chosen samples (pick 3 with heights close to median)
function pickSamples(s) {
  const idxSorted = s.perSample.map((p, i) => ({ i, h: p.trunkHeight })).sort((a, b) => a.h - b.h);
  const n = idxSorted.length;
  const picks = [idxSorted[0].i, idxSorted[Math.floor(n / 2)].i, idxSorted[n - 1].i];
  return [...new Set(picks)];
}

function asciiForSample(rawBlocks, analysis) {
  // group by dy (absolute relative to placement, i.e. b.dy) -> grid of dx,dz -> char
  const byDy = new Map();
  for (const b of rawBlocks) {
    if (!byDy.has(b.dy)) byDy.set(b.dy, new Map());
    let ch = '?';
    if (isLog(b.name)) ch = 'L';
    else if (isLeafLike(b.name)) ch = '#';
    else if (b.name === 'vine' || b.name === 'cave_vines' || b.name === 'cave_vines_plant') ch = 'v';
    else if (b.name === 'cocoa') ch = 'c';
    else if (b.name === 'mangrove_roots' || b.name === 'muddy_mangrove_roots' || b.name === 'hanging_roots') ch = 'r';
    else if (b.name === 'podzol') ch = 'p';
    else if (b.name === 'moss_block' || b.name === 'moss_carpet' || /moss/.test(b.name)) ch = 'm';
    else if (b.name === 'bee_nest') ch = 'B';
    else if (b.name === 'azalea' || b.name === 'flowering_azalea') ch = 'a';
    else if (b.name === 'dirt') ch = 'd';
    else ch = '?';
    byDy.get(b.dy).set(key(b.dx, b.dz), ch);
  }
  const dys = [...byDy.keys()].sort((a, b) => b - a); // top to bottom
  let out = '';
  for (const dy of dys) {
    const grid = byDy.get(dy);
    let minX = 99, maxX = -99, minZ = 99, maxZ = -99;
    for (const k of grid.keys()) { const [x, z] = k.split(',').map(Number); if (x < minX) minX = x; if (x > maxX) maxX = x; if (z < minZ) minZ = z; if (z > maxZ) maxZ = z; }
    out += `dy=${dy}:\n`;
    for (let z = minZ; z <= maxZ; z++) {
      let row = '';
      for (let x = minX; x <= maxX; x++) {
        row += grid.get(key(x, z)) || '.';
      }
      out += row + '\n';
    }
    out += '\n';
  }
  return out;
}

let asciiAll = '';
for (const id of SPECIES_IDS) {
  const data = JSON.parse(fs.readFileSync(path.join(DIR, `data-${id}.json`)));
  const s = OUT[id];
  if (!s) continue;
  const picks = pickSamples(s);
  asciiAll += `\n===== ${id} =====\n`;
  for (const idx of picks) {
    asciiAll += `-- sample #${idx} (height=${s.perSample[idx].trunkHeight}, width=${s.perSample[idx].trunkWidth}) --\n`;
    asciiAll += asciiForSample(data.samples[idx], s.perSample[idx]);
  }
}
fs.writeFileSync(path.join(DIR, 'ascii-samples.txt'), asciiAll);
console.log('\nWrote analysis.json and ascii-samples.txt');
