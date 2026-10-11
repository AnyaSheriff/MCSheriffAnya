let map, pending, running = false;
const pause = () => new Promise(resolve => setTimeout(resolve, 0));
self.onmessage = async ({data}) => {
    try {
        if (data.type === 'init') {
            await wasm_bindgen({module_or_path: Uint8Array.from(atob(WASM_BASE64), c => c.charCodeAt(0))});
            map = new wasm_bindgen.SeedMap(data.seed, data.dimension, data.smooth, data.autumn);
            const meta = JSON.parse(wasm_bindgen.metadata());
            const spawn = data.dimension === 0 ? Array.from(map.spawn()) : null;
            postMessage({type: 'ready', meta, spawn});
        } else if (data.type === 'render') {
            pending = data;
            if (!running) await render();
        } else if (data.type === 'biome' && map) {
            postMessage({type: 'biome', x: data.x, z: data.z, id: map.biome(data.x, data.z)});
        }
    } catch (error) {
        running = false;
        postMessage({type: 'error', message: String(error)});
    }
};
async function render() {
    running = true;
    while (pending) {
        const task = pending;
        pending = null;
        const start = performance.now();
        const {revision, x, z, width, height, step, kinds} = task;
        let interrupted = false;
        for (let row = 0; row < height; row += 16) {
            const rows = Math.min(16, height - row);
            const pixels = map.raster(x, z + row * step, width, rows, step);
            postMessage({type: 'band', revision, row, rows, pixels}, [pixels.buffer]);
            await pause();
            if (pending) { interrupted = true; break; }
        }
        if (interrupted) continue;
        for (const kind of kinds) {
            const markers = map.structures(kind, x, z, x + width * step, z + height * step);
            postMessage({type: 'markers', revision, markers}, [markers.buffer]);
            await pause();
            if (pending) { interrupted = true; break; }
        }
        if (!interrupted) postMessage({type: 'done', revision, ms: performance.now() - start});
    }
    running = false;
}
