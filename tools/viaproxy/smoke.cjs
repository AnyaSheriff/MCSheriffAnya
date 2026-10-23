// Black-box client traffic only; dependencies stay outside the repository.
const mineflayer = require('mineflayer')
const net = require('node:net')
const connection = ip => ({ stream: net.connect({host:'127.0.0.1', port:26101, localAddress:ip}) })
const { Vec3 } = require('vec3')
const assert = require('node:assert/strict')
const watchdog = setTimeout(() => { console.error('smoke global timeout'); process.exit(1) }, 180000)
const delay = ms => new Promise(r => setTimeout(r, ms))
async function until(test, label, ms = 15000) {
  const end = Date.now() + ms
  while (Date.now() < end) { if (test()) return; await delay(50) }
  throw new Error('timeout: ' + label)
}
async function session(version, username, relog) {
  const bot = mineflayer.createBot({host:'127.0.0.1', port:26101, ...connection('127.0.0.2'), username, version, auth:'offline'})
  let spawned = false, ended = false, failure
  const messages = []
  bot.once('spawn', () => { spawned = true })
  bot.on('messagestr', s => { messages.push(s); if (process.env.VIA_DEBUG) console.log('CHAT', username, s) })
  bot.on('error', e => { failure = e })
  bot.on('kicked', r => { failure = new Error('kicked: ' + JSON.stringify(r)) })
  bot.on('end', () => { ended = true })
  try {
    await until(() => { if (failure) throw failure; return spawned }, 'join', 45000)
    if (relog) { console.log(version, 'relogin PASS'); return }
    await bot.waitForChunksToLoad()
    const cmd = async s => { bot.chat('/' + s); await delay(400) }
    await cmd('gamemode creative')
    await cmd('fill 0 63 0 6 63 6 stone')
    await cmd('tp @s 1.5 64 1.5')
    await delay(1000)
    const start = bot.entity.position.clone()
    bot.setControlState('forward', true)
    await delay(350)
    bot.clearControlStates()
    assert(bot.entity.position.distanceTo(start) > 0.1, 'client movement')
    const marker = 'via-smoke-' + version
    bot.chat(marker)
    await until(() => messages.some(s => s.includes(marker)), 'chat echo')
    await cmd('tp @s 1.5 64 1.5')
    await cmd('setblock 3 64 1 stone')
    const target = new Vec3(3, 64, 1)
    await until(() => bot.blockAt(target)?.name === 'stone', 'stone appears')
    await bot.dig(bot.blockAt(target))
    await until(() => bot.blockAt(target)?.name === 'air', 'break acknowledged')
    await cmd('give @s stone 8')
    await until(() => bot.inventory.items().some(i => i.name === 'stone'), 'inventory')
    await bot.equip(bot.inventory.items().find(i => i.name === 'stone'), 'hand')
    await bot.placeBlock(bot.blockAt(new Vec3(3, 63, 1)), new Vec3(0, 1, 0))
    await until(() => bot.blockAt(target)?.name === 'stone', 'place acknowledged')
    if (failure) throw failure
    console.log(version, 'join/chat/move/break/place PASS', 'local=' + bot._client.socket.localAddress)
  } finally {
    bot.quit()
    await until(() => ended, 'disconnect', 5000).catch(() => bot.end())
  }
}
async function ipBanProof() {
  const admin = mineflayer.createBot({host:'127.0.0.1', port:26101, ...connection('127.0.0.3'), username:'ViaAdmin', version:'1.20.1', auth:'offline'})
  let ready = false, failure
  admin.once('spawn', () => { ready = true })
  admin.on('error', e => { failure = e })
  try {
    await until(() => { if (failure) throw failure; return ready }, 'admin spawn', 45000)
    admin.chat('/ban-ip 127.0.0.2 proxy-proof')
    await delay(750)
    const denied = mineflayer.createBot({host:'127.0.0.1', port:26101, ...connection('127.0.0.2'), username:'ViaDenied', version:'1.20.1', auth:'offline'})
    let kick, spawned = false
    denied.on('kicked', reason => { kick = JSON.stringify(reason) })
    denied.on('error', () => {})
    denied.on('spawn', () => { spawned = true })
    try {
      await until(() => kick || spawned, 'IP ban rejection', 20000)
      assert(!spawned && /banned|proxy-proof/i.test(kick), 'forwarded IP must trigger ban: ' + kick)
      assert(admin.entity, 'other client IP stays connected')
      console.log('real-IP ban PASS: 127.0.0.2 rejected; 127.0.0.3 admitted')
    } finally { denied.quit() }
  } finally {
    admin.chat('/pardon-ip 127.0.0.2')
    await delay(500)
    admin.quit()
  }
}
;(async () => {
  for (const [version, username] of [['1.20.1', 'Via120'], ['1.16.5', 'Via116']]) {
    await session(version, username, false)
    await delay(1000)
    await session(version, username, true)
  }
  await ipBanProof()
})().then(() => clearTimeout(watchdog)).catch(e => { console.error(e); process.exit(1) })
