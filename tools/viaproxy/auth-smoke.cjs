// Uses only public Minecraft client packets; npm dependencies live outside the repo.
const mineflayer = require('mineflayer')
const net = require('node:net')
const assert = require('node:assert/strict')
const delay = ms => new Promise(r => setTimeout(r, ms))
const port = Number(process.env.VIA_PORT || 26121)
assert(port >= 26100 && port <= 26199)
const password = 'ViaProofSecret_7821'
async function until(test, label, ms = 20000) {
  const end = Date.now() + ms
  while (Date.now() < end) { if (test()) return; await delay(50) }
  throw new Error('timeout: ' + label)
}
function connect(version, username, ip) {
  const bot = mineflayer.createBot({host:'127.0.0.1', port, stream:net.connect({host:'127.0.0.1', port, localAddress:ip}), username, version, auth:'offline'})
  const state = {bot, messages:[], spawns:0, kick:null, ended:false, error:null}
  bot.on('messagestr', m => state.messages.push(m))
  bot.on('spawn', () => state.spawns++)
  bot.on('kicked', r => { state.kick = JSON.stringify(r) })
  bot.on('end', () => { state.ended = true })
  bot.on('error', e => { state.error = e })
  return state
}
async function ready(s) {
  await until(() => { if(s.error) throw s.error; if(s.kick) throw Error(s.kick); return s.messages.some(m => m.includes('90 секунд')) && s.spawns }, 'limbo ready', 45000)
}
async function close(s) { s.bot.quit(); await until(() => s.ended, 'disconnect'); await delay(500) }
async function prove(version, name, ip) {
  let s = connect(version, name, ip); await ready(s)
  const initial = s.bot.entity.position.clone()
  s.bot.chat('PRE_AUTH_CHAT_MUST_NOT_APPEAR'); s.bot.chat('/say PRE_AUTH_COMMAND_MUST_NOT_APPEAR')
  s.bot.setControlState('forward', true); await delay(2400); s.bot.clearControlStates(); await delay(150)
  assert(!s.messages.some(m => m.includes('MUST_NOT_APPEAR')), 'chat and other commands blocked')
  assert(s.bot.entity.position.distanceTo(initial) < 0.2, 'limbo position frozen')
  s.bot.chat(`/register ${password} ${password}`)
  await until(() => s.messages.includes('Вход выполнен'), 'registration')
  await until(() => s.spawns >= 2, 'world join after registration'); await s.bot.waitForChunksToLoad()
  s.bot.chat('/login ' + password); await until(() => s.messages.some(m => m.includes('Вход уже выполнен')), 'post-login secret command intercepted')
  await close(s); console.log(version, 'register, limbo, world join PASS')
  s = connect(version, name, ip); await ready(s)
  s.bot.chat('/login WrongSecret_7821'); await until(() => s.messages.some(m => m.includes('Неверный пароль')), 'wrong password')
  assert(!s.messages.includes('Вход выполнен'))
  s.bot.chat('/login ' + password); await until(() => s.messages.includes('Вход выполнен'), 'relogin')
  await until(() => s.spawns >= 2, 'world after relogin'); await close(s)
  console.log(version, 'wrong password then /login PASS')
  s = connect(version, name, ip); await ready(s)
  const started = Date.now(); await until(() => s.kick, '90 second timeout', 100000)
  assert(s.kick.includes('Время входа истекло'), s.kick); assert(Date.now()-started >= 85000)
  await close(s); console.log(version, 'timeout PASS')
}
const watchdog = setTimeout(() => process.exit(2), 300000)
;(async () => {
  // Distinct source IPs preserve the per-real-IP daily registration quota.
  await Promise.all([prove('1.20.1','Chat120','127.0.0.20'), (async()=>{await delay(2000); await prove('1.16.5','Chat116','127.0.0.21')})()])
  clearTimeout(watchdog); console.log('AUTH THROUGH AUTO-STARTED VIAPROXY PASS')
})().catch(e => { console.error(e); process.exit(1) })
