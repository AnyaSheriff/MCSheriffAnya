// Сверка настоящего WASM с нативным PNG-экспортом; сервер не запускается.
const fs = require('node:fs');
const path = require('node:path');
const {execFileSync} = require('node:child_process');
const root = path.resolve(__dirname, '../..');
const glue = fs.readFileSync(path.join(__dirname, 'dist/seedmap.js'), 'utf8');
const bindgen = new Function(glue + '; return wasm_bindgen;')();
bindgen.initSync({module: fs.readFileSync(path.join(__dirname, 'dist/seedmap_bg.wasm'))});
function hash(bytes) { let h = 2166136261; for (const b of bytes) h = Math.imul(h ^ b, 16777619) >>> 0; return h.toString(16).padStart(8, '0'); }
const dims = ['overworld', 'nether', 'end'];
for (const seed of ['123', '-9223372036854775808', '9223372036854775807']) {
    for (const [dim, dimension] of dims.entries()) {
        const smooth = seed === '9223372036854775807';
        const out = root + '/tmp/seedmap-parity.png';
        const args = ['--seed', seed, '--x', '-1000', '--z', '1100', '--radius', '512', '--step', '4', '--dimension', dimension, '--out', out, '--autumn', ...(smooth ? ['--smooth'] : [])];
        const text = execFileSync(root + '/target/release/seedmap', args, {encoding: 'utf8'}).trim();
        const native = JSON.parse(fs.readFileSync(root + '/tmp/seedmap-parity.json', 'utf8'));
        const map = new bindgen.SeedMap(seed, dim, smooth, true);
        const [x0, z0, x1, z1] = native.bounds;
        const rgba = map.raster(x0, z0, native.biome_size, native.biome_size, native.step);
        if (hash(rgba) !== native.biome_rgba_fnv1a) throw new Error(`Biome mismatch: ${seed}, ${dimension}`);
        const markers = [];
        for (let kind = 0; kind < 20; kind++) {
            const a = map.structures(kind, x0, z0, x1, z1);
            for (let i = 0; i < a.length; i += 3) markers.push({kind: a[i], x: a[i + 1], z: a[i + 2]});
        }
        if (JSON.stringify(markers) !== JSON.stringify(native.structures)) throw new Error(`Structure mismatch: ${seed}, ${dimension}`);
        if (dim === 0 && JSON.stringify(Array.from(map.spawn())) !== JSON.stringify(native.spawn)) throw new Error('Spawn mismatch');
        map.free(); console.log('PASS', seed, dimension, text);
    }
}
for (const [dim, dimension] of dims.entries()) {
    const map = new bindgen.SeedMap('123', dim, false, false);
    const init = performance.now();
    const rgba = map.raster(-2048, -2048, 1024, 1024, 4);
    const biomes = performance.now(); let count = 0;
    for (let kind = 0; kind < 20; kind++) count += map.structures(kind, -2048, -2048, 2048, 2048).length / 3;
    const structures = performance.now(); if (dim === 0) map.spawn();
    console.log(`BENCH ${dimension} 4096x4096 blocks, 1024x1024 samples: biomes ${(biomes-init).toFixed(1)}ms, structures ${(structures-biomes).toFixed(1)}ms, with spawn ${(performance.now()-init).toFixed(1)}ms, ${count} markers, hash ${hash(rgba)}`);
    map.free();
}
console.log('Native/WASM parity passed.');
