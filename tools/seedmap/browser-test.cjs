// Проверка Chromium через pipe: HTTP-сервер и слушающие сокеты не нужны.
const {spawn} = require('node:child_process');
const fs = require('node:fs');
const path = require('node:path');
const root = path.resolve(__dirname, '../..');
const chrome = spawn(process.env.CHROMIUM || 'chromium', ['--headless', '--no-sandbox', '--disable-gpu', '--disable-dev-shm-usage', '--disable-background-networking', '--no-first-run', '--remote-debugging-pipe', `--user-data-dir=${root}/tmp/seedmap-chromium`, 'about:blank'], {stdio: ['ignore', 'ignore', 'pipe', 'pipe', 'pipe'], env: {...process.env, TMPDIR: `${root}/tmp`}});
let next = 0, incoming = '', session;
const pending = new Map(), errors = [];
chrome.stderr.on('data', () => {});
chrome.stdio[4].on('data', chunk => {
    incoming += chunk.toString();
    for (;;) {
        const end = incoming.indexOf('\0'); if (end < 0) break;
        const item = JSON.parse(incoming.slice(0, end)); incoming = incoming.slice(end + 1);
        if (item.id && pending.has(item.id)) { const p = pending.get(item.id); pending.delete(item.id); if (item.error) p.reject(item.error); else p.resolve(item.result); }
        if (item.method === 'Runtime.exceptionThrown') errors.push(item.params.exceptionDetails.text);
    }
});
chrome.on('exit', code => { for (const p of pending.values()) p.reject(new Error(`Chromium exited: ${code}`)); });
function send(method, params = {}, withSession = true) {
    return new Promise((resolve, reject) => {
        const id = ++next; pending.set(id, {resolve, reject});
        chrome.stdio[3].write(JSON.stringify({id, method, params, ...(withSession && session ? {sessionId: session} : {})}) + '\0');
    });
}
async function evaluate(expression) {
    const r = await send('Runtime.evaluate', {expression, returnByValue: true, awaitPromise: true});
    if (r.exceptionDetails) throw new Error(JSON.stringify(r.exceptionDetails));
    return r.result.value;
}
const delay = ms => new Promise(r => setTimeout(r, ms));
async function waitRender(previous = 0, dimension) {
    const start = Date.now();
    while (Date.now() - start < 120000) {
        const state = await evaluate(`({ready: Number(document.getElementById('map')?.dataset.ready || 0), text: document.getElementById('status')?.textContent, error: document.getElementById('status')?.classList.contains('error'), state: window.seedmapTestState})`);
        if (state.error) throw new Error(state.text);
        if (state.ready > previous && (dimension === undefined || state.state?.settings.dimension === dimension)) return state;
        await delay(100);
    }
    throw new Error('Render timeout');
}
async function main() {
    const target = await send('Target.createTarget', {url: 'about:blank'}, false);
    session = (await send('Target.attachToTarget', {targetId: target.targetId, flatten: true}, false)).sessionId;
    await send('Runtime.enable'); await send('Page.enable');
    await send('Emulation.setDeviceMetricsOverride', {width: 1280, height: 900, deviceScaleFactor: 1, mobile: false});
    await send('Page.navigate', {url: 'file://' + root + '/tools/seedmap/dist/index.html'});
    let state = await waitRender();
    console.log('Overworld', state.text, JSON.stringify(state.state.request));
    fs.writeFileSync(root + '/tmp/seedmap-browser-overworld.json', JSON.stringify(state.state));
    fs.writeFileSync(root + '/tmp/seedmap-browser.png', Buffer.from((await send('Page.captureScreenshot', {format: 'png'})).data, 'base64'));
    const square = await evaluate(`({width: innerWidth, height: innerHeight - document.getElementById('view').clientHeight + document.getElementById('view').clientWidth})`);
    await send('Emulation.setDeviceMetricsOverride', {...square, deviceScaleFactor: 1, mobile: false});
    state = await waitRender(state.ready);
    console.log('BROWSER BENCH', state.text, state.state.request.width * state.state.request.step, 'x', state.state.request.height * state.state.request.step, 'blocks');
    const download = root + '/tmp/seedmap-download-' + Date.now(); fs.mkdirSync(download);
    await send('Browser.setDownloadBehavior', {behavior: 'allow', downloadPath: download}, false);
    await evaluate(`document.getElementById('export').click()`);
    let png;
    for (let i = 0; i < 100 && !png; i++) { await delay(50); png = fs.readdirSync(download).find(name => name.endsWith('.png')); }
    if (!png || fs.readFileSync(path.join(download, png)).subarray(1, 4).toString() !== 'PNG') throw new Error('Browser PNG download failed');
    await evaluate(`document.getElementById('seed').value='9223372036854775808'; document.getElementById('seed-form').requestSubmit()`);
    if (!await evaluate(`document.getElementById('status').classList.contains('error')`)) throw new Error('Invalid seed accepted');
    await evaluate(`document.getElementById('seed').value='-9223372036854775808'; document.getElementById('seed-form').requestSubmit()`);
    state = await waitRender(state.ready);
    if (state.state.settings.seed !== '-9223372036854775808') throw new Error('Seed precision lost');
    await evaluate(`document.getElementById('x').value='-2000'; document.getElementById('z').value='3000'; document.getElementById('goto').requestSubmit()`);
    state = await waitRender(state.ready);
    const rect = await evaluate(`(()=>{const r=document.getElementById('map').getBoundingClientRect();return {x:r.x+r.width/2,y:r.y+r.height/2}})()`);
    await send('Input.dispatchMouseEvent', {type: 'mouseMoved', x: rect.x, y: rect.y});
    await delay(150);
    const coords = await evaluate(`document.getElementById('coords').textContent`);
    if (!coords.includes('-2000') || !coords.includes('3000')) throw new Error('Go-to/cursor mismatch: ' + coords);
    await evaluate(`document.getElementById('map').dispatchEvent(new KeyboardEvent('keydown', {key:'ArrowRight'}))`);
    state = await waitRender(state.ready);
    const movedCoords = await evaluate(`document.getElementById('coords').textContent`);
    if (movedCoords === coords) throw new Error('Cursor coordinates did not follow keyboard pan');
    await evaluate(`document.getElementById('none').click()`); state = await waitRender(state.ready);
    if (state.state.markers.length) throw new Error('Structure toggles failed');
    await evaluate(`document.getElementById('all').click(); document.getElementById('plus').click()`); state = await waitRender(state.ready);
    await send('Input.dispatchMouseEvent', {type: 'mousePressed', x: rect.x, y: rect.y, button: 'left', clickCount: 1});
    await send('Input.dispatchMouseEvent', {type: 'mouseMoved', x: rect.x + 100, y: rect.y + 50, button: 'left', buttons: 1});
    await send('Input.dispatchMouseEvent', {type: 'mouseReleased', x: rect.x + 100, y: rect.y + 50, button: 'left', clickCount: 1});
    const before = state.state.request; state = await waitRender(state.ready);
    if (state.state.request.x === before.x) throw new Error('Pan failed');
    for (const dim of [1, 2, 0]) {
        await evaluate(`document.getElementById('dimension').value='${dim}'; document.getElementById('dimension').dispatchEvent(new Event('change'))`);
        state = await waitRender(state.ready, dim); console.log(['Overworld', 'Nether', 'End'][dim], state.text);
    }
    await evaluate(`document.getElementById('style').value='smooth'; document.getElementById('style').dispatchEvent(new Event('change'))`);
    state = await waitRender(state.ready);
    if (!state.state.settings.smooth) throw new Error('Smooth selector failed');
    await send('Emulation.setDeviceMetricsOverride', {width: 390, height: 844, deviceScaleFactor: 1, mobile: true});
    await waitRender(state.ready);
    fs.writeFileSync(root + '/tmp/seedmap-mobile.png', Buffer.from((await send('Page.captureScreenshot', {format: 'png'})).data, 'base64'));
    if (errors.length) throw new Error(errors.join('\n'));
    console.log('Browser smoke test passed (file://, workers, i64 seeds, dimensions, toggles, cursor, go-to, zoom, pan, PNG download, responsive layout).');
}
const watchdog = setTimeout(() => { console.error('Browser test timeout'); chrome.kill('SIGTERM'); process.exitCode = 1; }, 300000);
main().catch(e => { console.error(e); process.exitCode = 1; }).finally(async () => { clearTimeout(watchdog); try { await send('Browser.close', {}, false); } catch {} chrome.kill('SIGTERM'); });
