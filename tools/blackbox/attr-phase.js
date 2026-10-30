// Подготовка двухтактовой сетки записей: ошибку измерения не считаем совпадением.
'use strict';

const SYNC_COMMAND = '!syncattrs';

async function measurePlayers(measure, count) {
  const players = [];
  for (let i = 0; i < count; i++) {
    const phase = await measure(i);
    if (phase !== 0 && phase !== 1) throw new Error(`attr_phase: нет достоверной фазы игрока ${i + 1}`);
    players.push(phase);
  }
  return players;
}

function assertAligned(players) {
  if (!players.length || players.some(p => p !== 0 && p !== 1)) throw new Error('attr_phase: неверные фазы игроков');
  if (players.some(p => p !== players[0])) throw new Error(`attr_phase: фазы игроков после подготовки различаются: ${players.join(' / ')}`);
  return players[0];
}

async function synchronize(measure, respawnSecond, maxRespawns = 8, observe = () => {}) {
  if (!Number.isInteger(maxRespawns) || maxRespawns < 0) throw new Error('attr_phase: неверный предел возрождений');
  // Последняя проверка нужна и после последнего разрешённого возрождения.
  for (let respawns = 0; respawns <= maxRespawns; respawns++) {
    const players = await measurePlayers(measure, 2);
    observe(players, respawns);
    if (players[0] === players[1]) return { players, respawns };
    if (respawns === maxRespawns) throw new Error(`attr_phase: не удалось выровнять игроков после ${maxRespawns} возрождений`);
    // Одинаковая длительность подготовки может снова и снова давать ту же
    // чётность respawn. Пробуем обе задержки; измерение остаётся обязательным.
    await respawnSecond(respawns % 2);
  }
}

function syncInSetup(scenario) {
  const count = (scenario.setup || []).filter(c => c === SYNC_COMMAND).length;
  if (count > 1 || (count && !scenario.attr_phase)) throw new Error('attr_phase: неверное положение команды !syncattrs');
  return count === 1;
}

module.exports = { SYNC_COMMAND, measurePlayers, assertAligned, synchronize, syncInSetup };
