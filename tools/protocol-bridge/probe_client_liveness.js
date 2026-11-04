// Живая регрессия: только изолированный локальный сервер261xx, без чтения исходников библиотек.
const assert = require('node:assert/strict')
const fs = require('node:fs')
const path = require('node:path')
const mf = require(path.join(process.env.HOME, 'node_modules', 'mineflayer'))
const { Vec3 } = require(path.join(process.env.HOME, 'node_modules', 'vec3'))
const port = Number(process.argv[2])
assert(port >= 26100 && port <= 26199)
const duration = Number(process.argv[3] || 310)
const versions = (process.argv[4] || '26.1.2,1.21.4').split(',')
const output = process.argv[5]
const sleep = ms => new Promise(resolve => setTimeout(resolve, ms))
const results = []
let failed = false
function save () { if (output) fs.writeFileSync(output, JSON.stringify(results, null, 2) + '\n') }
async function run (version, index) {
  const name = { '26.1.2': 'Live775', '26.2': 'Live776', '1.21.4': 'Live769' }[version]
  const r = { version, name, status: 'joining', packets: {}, outgoing: {}, actions: [], errors: [], moves: 0, jumps: 0 }
  results.push(r)
  const bot = mf.createBot({ host: '127.0.0.1', port, username: name, version, auth: 'offline', viewDistance: 'tiny' })
  const write = bot._client.write.bind(bot._client)
  bot._client.write = (n, d) => { r.outgoing[n] = (r.outgoing[n] || 0) + 1; return write(n, d) }
  bot._client.on('packet', (d, m) => {
    if (m.state === 'play') r.packets[m.name] = (r.packets[m.name] || 0) + 1
    if (m.name === 'start_configuration') { r.errors.push('unexpected start_configuration'); failed = true }
  })
  bot.on('error', e => { r.errors.push(e.message); failed = true; save() })
  bot.on('kicked', reason => { r.errors.push(JSON.stringify(reason)); failed = true; save() })
  let done = false
  bot.on('end', reason => { if (!done) { r.status = 'disconnected'; r.errors.push(String(reason)); failed = true; save() } })
  await new Promise((resolve, reject) => { bot.once('spawn', resolve); bot.once('error', reject); setTimeout(() => reject(Error('spawn timeout')), 60000).unref() })
  const start = Date.now()
  r.status = 'spawned'
  console.log(version, 'spawned')
  await sleep(2000)
  bot.chat(`liveness ${version}`)
  r.actions.push('chat sent')
  if (version === '26.1.2') {
    // Команды готовят площадку; размещение, добыча, окно и удар выполняются API робота.
    bot.chat('/fill 21 0 -3 27 0 3 stone'); await sleep(500)
    bot.chat('/tp @s 24.5 1 0.5'); await sleep(500)
    bot.chat('/give @s stone 16'); await sleep(1000)
    const stone = bot.inventory.items().find(i => i.name === 'stone')
    assert(stone, 'stone received')
    await bot.equip(stone, 'hand')
    await bot.placeBlock(bot.blockAt(new Vec3(25, 0, 0)), new Vec3(0, 1, 0))
    await sleep(300)
    assert.equal(bot.blockAt(new Vec3(25, 1, 0)).name, 'stone')
    r.actions.push('placed stone confirmed')
    await bot.dig(bot.blockAt(new Vec3(25, 1, 0)), true)
    await sleep(300)
    assert.equal(bot.blockAt(new Vec3(25, 1, 0)).name, 'air')
    r.actions.push('broke stone confirmed')
    assert(bot.inventory.slots.some(Boolean))
    bot.chat('/setblock 25 1 0 chest'); await sleep(500)
    const window = await bot.openContainer(bot.blockAt(new Vec3(25, 1, 0)))
    assert(window.slots.length > 36)
    window.close()
    r.actions.push('opened inventory container and closed')
    bot.chat('/summon cow 24.5 1 2.5 {NoAI:1b,Silent:1b}'); await sleep(1000)
    const mob = bot.nearestEntity(e => e.name === 'cow')
    assert(mob, 'cow received')
    const before = r.packets.damage_event || 0
    bot.attack(mob)
    await sleep(700)
    assert((r.packets.damage_event || 0) > before, 'attack damage received')
    r.actions.push('attacked cow, damage confirmed')
  }
  let phase = 0
  const move = setInterval(() => {
    const p = bot.entity.position
    bot.look(Math.atan2(p.x - (version === '26.1.2' ? 24.5 : 0.5), p.z - 0.5), 0, true)
    bot.setControlState('forward', true)
    bot.setControlState('jump', (++phase % 2) === 0)
    r.jumps += phase % 2 === 0 ? 1 : 0
    save()
  }, 1500)
  let previous = bot.entity.position.clone()
  bot.on('physicsTick', () => { if (bot.entity.position.distanceTo(previous) > 0.01) r.moves++; previous = bot.entity.position.clone() })
  while (Date.now() - start < duration * 1000 && r.status !== 'disconnected') await sleep(1000)
  clearInterval(move)
  bot.clearControlStates()
  r.seconds = (Date.now() - start) / 1000
  r.position = bot.entity.position
  if (r.status !== 'disconnected') {
    assert(r.moves > 100 && r.jumps > 20, 'walking and jumping')
    assert((r.outgoing.keep_alive || 0) > 30, 'keepalive replies')
    assert.equal(r.errors.length, 0)
    r.status = 'passed'; done = true; bot.quit()
  }
  console.log(version, r.status, r.seconds, 'seconds', r.moves, 'moves', r.outgoing.keep_alive, 'keepalive replies')
  save()
}
Promise.all(versions.map((v, i) => run(v, i).catch(e => { failed = true; console.error(v, e.stack); const r = results.find(r => r.version === v); r.errors.push(e.message); r.status = 'failed'; save() }))).then(() => { save(); process.exit(failed ? 1 : 0) })
