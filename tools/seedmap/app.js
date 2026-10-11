'use strict';
(() => {
    const $ = id => document.getElementById(id);
    const canvas = $('map'), ctx = canvas.getContext('2d'), view = $('view');
    const LIMIT = 29000000;
    let width = 1, height = 1, center = {x: 0, z: 0}, bpp = 8, initial = true;
    let worker, generation = 0, ready = false, revision = 0, frame, buffer, request, timer;
    let meta, spawn, markers = [], pointer, drag, active, rendered = false;
    const enabled = new Set(Array.from({length: 20}, (_, i) => i));
    const status = (text, error = false) => { $('status').textContent = text; $('status').classList.toggle('error', error); };
    function fail(error) { status(String(error), true); $('busy').hidden = true; }
    function size() {
        width = Math.max(1, view.clientWidth); height = Math.max(1, view.clientHeight);
        const ratio = Math.min(devicePixelRatio || 1, 2);
        canvas.width = Math.round(width * ratio); canvas.height = Math.round(height * ratio);
        ctx.setTransform(ratio, 0, 0, ratio, 0, 0);
        if (initial) { bpp = 4096 / width; initial = false; }
        constrain(); draw(); schedule();
    }
    function constrain() {
        bpp = Math.min(Math.max(bpp, 0.25), 30000 / Math.max(width, height));
        const margin = Math.max(width, height) * bpp / 2 + 1024;
        center.x = Math.max(-LIMIT + margin, Math.min(LIMIT - margin, center.x));
        center.z = Math.max(-LIMIT + margin, Math.min(LIMIT - margin, center.z));
    }
    function screen(x, z) { return [(x - center.x) / bpp + width / 2, (z - center.z) / bpp + height / 2]; }
    function world(x, y) { return {x: Math.floor(center.x + (x - width / 2) * bpp), z: Math.floor(center.z + (y - height / 2) * bpp)}; }
    function badge(x, y, icon, color, large = false) {
        const r = large ? 13 : 10;
        ctx.fillStyle = color; ctx.strokeStyle = '#111e2b'; ctx.lineWidth = 2;
        ctx.beginPath(); ctx.roundRect(x - r, y - r, 2 * r, 2 * r, 4); ctx.fill(); ctx.stroke();
        ctx.fillStyle = '#15222d'; ctx.font = `bold ${large ? 18 : 12}px system-ui`; ctx.textAlign = 'center'; ctx.textBaseline = 'middle'; ctx.fillText(icon, x, y);
    }
    function draw() {
        ctx.fillStyle = '#101923'; ctx.fillRect(0, 0, width, height);
        if (frame) {
            const [x, y] = screen(frame.x, frame.z);
            ctx.imageSmoothingEnabled = false;
            ctx.drawImage(frame.image, x, y, frame.image.width * frame.step / bpp, frame.image.height * frame.step / bpp);
        }
        if (meta) for (const marker of markers) {
            if (!enabled.has(marker.kind)) continue;
            const [x, y] = screen(marker.x, marker.z);
            if (x < -12 || x > width + 12 || y < -12 || y > height + 12) continue;
            const info = meta.structures[marker.kind]; badge(x, y, info.icon, info.color);
        }
        if (spawn && active.dimension === 0) { const [x, y] = screen(spawn[0], spawn[2]); badge(x, y, '+', '#ffffff', true); }
        const bar = 2 ** Math.floor(Math.log2(120 * bpp));
        const length = bar / bpp;
        ctx.strokeStyle = '#f0f7ff'; ctx.lineWidth = 2;
        ctx.beginPath(); ctx.moveTo(width - 24 - length, height - 28); ctx.lineTo(width - 24, height - 28); ctx.stroke();
        ctx.font = '12px system-ui'; ctx.textAlign = 'right'; ctx.textBaseline = 'bottom';
        ctx.lineWidth = 4; ctx.strokeStyle = '#122030'; ctx.strokeText(`${bar} блоков`, width - 24, height - 34);
        ctx.fillStyle = '#f0f7ff'; ctx.fillText(`${bar} блоков`, width - 24, height - 34);
        if (pointer) {
            const next = world(pointer.x, pointer.y);
            if (next.x !== pointer.world.x || next.z !== pointer.world.z) {
                pointer.world = next; showPointer();
                if (ready) worker.postMessage({type: 'biome', ...next});
            }
        }
    }
    function schedule() {
        clearTimeout(timer);
        rendered = false; $('export').disabled = true;
        timer = setTimeout(render, 90);
    }
    function render() {
        if (!ready) return;
        constrain();
        const step = Math.max(1, 2 ** Math.ceil(Math.log2(Math.max(bpp, width * bpp / 1024, height * bpp / 1024))));
        const x = Math.floor((center.x - width * bpp / 2) / step) * step;
        const z = Math.floor((center.z - height * bpp / 2) / step) * step;
        const cols = Math.ceil((center.x + width * bpp / 2 - x) / step);
        const rows = Math.ceil((center.z + height * bpp / 2 - z) / step);
        buffer = document.createElement('canvas'); buffer.width = cols; buffer.height = rows;
        request = {type: 'render', revision: ++revision, x, z, width: cols, height: rows, step,
            kinds: meta.structures.filter(s => enabled.has(s.id) && (s.dimensions & (1 << active.dimension))).map(s => s.id)};
        markers = [];
        $('busy').hidden = false; $('busy').textContent = 'Биомы…';
        $('scale').textContent = `${Math.round(width * bpp)} × ${Math.round(height * bpp)} блоков · выборка ${step} бл./пикс.`;
        status('Расчёт…'); worker.postMessage(request);
    }
    function buildLegend() {
        $('structures').replaceChildren();
        for (const s of meta.structures.filter(s => s.dimensions & (1 << active.dimension))) {
            const label = document.createElement('label'), check = document.createElement('input'), icon = document.createElement('span');
            check.type = 'checkbox'; check.checked = enabled.has(s.id); check.dataset.kind = s.id;
            check.addEventListener('change', () => { if (check.checked) enabled.add(s.id); else enabled.delete(s.id); draw(); schedule(); });
            icon.className = 'icon'; icon.style.background = s.color; icon.textContent = s.icon;
            label.append(check, icon, document.createTextNode(s.name)); $('structures').append(label);
        }
        $('legend').replaceChildren();
        for (const b of meta.biomes.filter(b => active.dimension === 0 ? b.id <= 54 : active.dimension === 1 ? b.id >= 55 && b.id < 60 : b.id >= 60)) {
            const row = document.createElement('div'), swatch = document.createElement('span');
            row.className = 'legend-row'; swatch.className = 'swatch'; swatch.style.background = `rgb(${b.color.slice(0, 3).join(',')})`;
            row.append(swatch, document.createTextNode(b.name.replace('minecraft:', ''))); $('legend').append(row);
        }
    }
    function start() {
        try {
            const text = $('seed').value.trim();
            if (!/^-?\d+$/.test(text) || BigInt(text) < -(1n << 63n) || BigInt(text) >= (1n << 63n)) throw new Error('Seed: целое от −9223372036854775808 до 9223372036854775807');
            if (!window.SEEDMAP_WORKER) throw new Error('Сначала запустите tools/seedmap/build.sh и откройте dist/index.html');
            active = {seed: BigInt(text).toString(), dimension: Number($('dimension').value), smooth: $('style').value === 'smooth', autumn: $('autumn').checked};
            $('seed').value = active.seed;
            worker?.terminate(); clearTimeout(timer); ready = false; markers = []; frame = null; spawn = null;
            pointer = undefined; $('tooltip').textContent = 'Наведите курсор на карту';
            revision++; const current = ++generation;
            const url = URL.createObjectURL(new Blob([window.SEEDMAP_WORKER], {type: 'text/javascript'}));
            worker = new Worker(url); URL.revokeObjectURL(url);
            worker.onerror = e => { fail(e.message); };
            worker.onmessage = ({data}) => {
                if (current !== generation) return;
                if (data.type === 'error') { fail(data.message); return; }
                if (data.type === 'ready') {
                    ready = true; meta = data.meta; spawn = data.spawn; buildLegend(); render(); return;
                }
                if (data.type === 'biome') {
                    if (pointer && data.x === pointer.world.x && data.z === pointer.world.z) showPointer(meta.biomes[data.id].name);
                    return;
                }
                if (data.revision !== revision) return;
                if (data.type === 'band') {
                    buffer.getContext('2d').putImageData(new ImageData(new Uint8ClampedArray(data.pixels.buffer), request.width, data.rows), 0, data.row);
                    frame = {image: buffer, x: request.x, z: request.z, step: request.step};
                    $('busy').textContent = data.row + data.rows === request.height ? 'Структуры…' : `Биомы ${Math.round((data.row + data.rows) / request.height * 100)}%`;
                    draw();
                } else if (data.type === 'markers') {
                    for (let i = 0; i < data.markers.length; i += 3) markers.push({kind: data.markers[i], x: data.markers[i + 1], z: data.markers[i + 2]});
                    draw();
                } else if (data.type === 'done') {
                    $('busy').hidden = true; rendered = true; $('export').disabled = false;
                    status(`${markers.length} структур · ${(data.ms / 1000).toFixed(2)} с`);
                    canvas.dataset.ready = String(revision); canvas.dataset.ms = String(data.ms); canvas.dataset.markers = String(markers.length);
                    window.seedmapTestState = {settings: active, markers, spawn, request, ms: data.ms};
                }
            };
            $('style').disabled = active.dimension !== 0; $('autumn').disabled = active.dimension !== 0; $('spawn').disabled = active.dimension !== 0;
            $('busy').hidden = false; $('busy').textContent = 'Подготовка генератора…'; status('Загрузка WASM…'); draw();
            worker.postMessage({type: 'init', ...active});
        } catch (error) { fail(error.message); }
    }
    function zoom(factor, x = width / 2, y = height / 2) {
        const before = {x: center.x + (x - width / 2) * bpp, z: center.z + (y - height / 2) * bpp};
        bpp *= factor; constrain();
        center.x = before.x - (x - width / 2) * bpp; center.z = before.z - (y - height / 2) * bpp;
        constrain(); draw(); schedule();
    }
    function showPointer(biome) {
        if (!pointer) return;
        const {x, z} = pointer.world;
        $('coords').textContent = `X: ${x} · Z: ${z}`;
        let closest, distance = 18;
        for (const m of markers) {
            if (!enabled.has(m.kind)) continue;
            const [sx, sy] = screen(m.x, m.z), d = Math.hypot(sx - pointer.x, sy - pointer.y);
            if (d < distance) { closest = m; distance = d; }
        }
        let text = closest ? `${meta.structures[closest.kind].name} · X ${closest.x} · Z ${closest.z}\n` : '';
        if (spawn && Math.hypot(...screen(spawn[0], spawn[2]).map((v, i) => v - (i ? pointer.y : pointer.x))) < 18) text = `Спавн · X ${spawn[0]} · Y ${spawn[1]} · Z ${spawn[2]}\n`;
        $('tooltip').textContent = text + (biome ? biome.replace('minecraft:', '') : `X ${x} · Z ${z}`);
    }
    let hoverAt = 0;
    canvas.addEventListener('pointerdown', e => { if (e.button !== 0) return; canvas.focus(); canvas.setPointerCapture(e.pointerId); drag = {id: e.pointerId, x: e.clientX, y: e.clientY}; canvas.classList.add('dragging'); });
    canvas.addEventListener('pointermove', e => {
        if (drag && drag.id === e.pointerId) {
            center.x -= (e.clientX - drag.x) * bpp; center.z -= (e.clientY - drag.y) * bpp;
            drag.x = e.clientX; drag.y = e.clientY; constrain(); draw(); schedule();
        }
        const rect = canvas.getBoundingClientRect(), x = e.clientX - rect.left, y = e.clientY - rect.top;
        pointer = {x, y, world: world(x, y)}; showPointer();
        if (ready && performance.now() - hoverAt > 60) { worker.postMessage({type: 'biome', ...pointer.world}); hoverAt = performance.now(); }
    });
    const endDrag = () => { drag = null; canvas.classList.remove('dragging'); };
    canvas.addEventListener('pointerup', endDrag); canvas.addEventListener('pointercancel', endDrag); canvas.addEventListener('lostpointercapture', endDrag);
    canvas.addEventListener('wheel', e => { e.preventDefault(); const r = canvas.getBoundingClientRect(); zoom(Math.exp(Math.max(-1, Math.min(1, e.deltaY / 400))), e.clientX - r.left, e.clientY - r.top); }, {passive: false});
    canvas.addEventListener('keydown', e => {
        const move = {ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1]}[e.key];
        if (move) { e.preventDefault(); center.x += move[0] * width * bpp / 5; center.z += move[1] * height * bpp / 5; constrain(); draw(); schedule(); }
        if (['+', '=', '-'].includes(e.key)) { e.preventDefault(); zoom(e.key === '-' ? 2 : 0.5); }
    });
    $('plus').onclick = () => zoom(0.5); $('minus').onclick = () => zoom(2);
    $('seed-form').onsubmit = e => { e.preventDefault(); start(); };
    for (const id of ['dimension', 'style', 'autumn']) $(id).onchange = start;
    $('goto').onsubmit = e => {
        e.preventDefault();
        const x = Number($('x').value.trim()), z = Number($('z').value.trim());
        if (!$('x').value.trim() || !$('z').value.trim() || !Number.isSafeInteger(x) || !Number.isSafeInteger(z) || Math.abs(x) > LIMIT - 32768 || Math.abs(z) > LIMIT - 32768) { fail(`X/Z: целые числа в пределах ±${LIMIT - 32768}`); return; }
        center = {x, z}; draw(); schedule();
    };
    $('spawn').onclick = () => { if (spawn) { center = {x: spawn[0], z: spawn[2]}; $('x').value = Math.floor(center.x); $('z').value = Math.floor(center.z); draw(); schedule(); } };
    for (const [id, checked] of [['all', true], ['none', false]]) $(id).onclick = () => {
        document.querySelectorAll('#structures input').forEach(input => { input.checked = checked; if (checked) enabled.add(Number(input.dataset.kind)); else enabled.delete(Number(input.dataset.kind)); }); draw(); schedule();
    };
    $('export').onclick = () => {
        if (!rendered) return;
        const out = document.createElement('canvas'); out.width = canvas.width; out.height = canvas.height + 48;
        const c = out.getContext('2d'); c.drawImage(canvas, 0, 0); c.fillStyle = '#16212c'; c.fillRect(0, canvas.height, out.width, 48);
        c.fillStyle = '#e5edf2'; c.font = '14px system-ui';
        c.fillText(`MCSheriffAnya · Seed ${active.seed} · ${['Overworld', 'Nether', 'End'][active.dimension]} · ${active.smooth ? 'Smooth' : 'Normal'} · Autumn ${active.autumn}`, 12, canvas.height + 19);
        c.fillText(`Center X ${Math.floor(center.x)} Z ${Math.floor(center.z)} · ${$('scale').textContent}`, 12, canvas.height + 38);
        out.toBlob(blob => { const a = document.createElement('a'), url = URL.createObjectURL(blob); a.href = url; a.download = `seed-${active.seed}.png`; a.click(); setTimeout(() => URL.revokeObjectURL(url), 1000); });
    };
    new ResizeObserver(size).observe(view); size(); start();
})();
