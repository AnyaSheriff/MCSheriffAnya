'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { SYNC_COMMAND, measurePlayers, assertAligned, synchronize, syncInSetup } = require('./attr-phase');

test('already aligned players keep inventory and position: no respawn', async () => {
  let respawns = 0;
  const result = await synchronize(async () => 1, async () => { respawns++; });
  assert.deepEqual(result, { players: [1, 1], respawns: 0 });
  assert.equal(respawns, 0);
});

test('second player is remeasured after respawn, including the last allowed attempt', async () => {
  let respawns = 0;
  const samples = [];
  const result = await synchronize(async i => i === 0 ? 1 : Number(respawns === 8),
    async () => { respawns++; }, 8, phases => samples.push(phases));
  assert.equal(respawns, 8);
  assert.equal(samples.length, 9);
  assert.deepEqual(result.players, [1, 1]);
});

test('exhausted synchronization rejects instead of starting a bad recording', async () => {
  let respawns = 0;
  await assert.rejects(synchronize(async i => i, async () => { respawns++; }), /после 8 возрождений/);
  assert.equal(respawns, 8);
});

test('missing or malformed phase cannot look like an aligned pair', async () => {
  for (const phase of [null, undefined, NaN, -1, 2, '0', false]) {
    let respawns = 0;
    await assert.rejects(synchronize(async () => phase, async () => { respawns++; }), /нет достоверной фазы/);
    assert.equal(respawns, 0);
  }
});

test('respawn failure aborts synchronization', async () => {
  await assert.rejects(synchronize(async i => i, async () => { throw new Error('spawn timed out'); }), /spawn timed out/);
});

test('post-setup verification catches phase reset of either player', async () => {
  const phases = [0, 0];
  await synchronize(async i => phases[i], async () => assert.fail('unexpected respawn'));
  for (const reset of [0, 1]) {
    phases[reset] = 1;
    const final = await measurePlayers(async i => phases[i], 2);
    assert.throws(() => assertAligned(final), /после подготовки различаются/);
    phases[reset] = 0;
  }
});

test('single-player phase recording does not require a second bot', async () => {
  const phases = await measurePlayers(async i => { assert.equal(i, 0); return 1; }, 1);
  assert.equal(assertAligned(phases), 1);
  assert.throws(() => assertAligned([]));
});

test('setup sync marker is unique and cannot silently enable unrelated scenarios', () => {
  assert.equal(syncInSetup({ setup: ['clear $(bot)'] }), false);
  assert.equal(syncInSetup({ attr_phase: true, setup: [SYNC_COMMAND] }), true);
  assert.throws(() => syncInSetup({ setup: [SYNC_COMMAND] }));
  assert.throws(() => syncInSetup({ attr_phase: true, setup: [SYNC_COMMAND, SYNC_COMMAND] }));
});

test('phase scenarios that kill a player synchronize after respawn, before loadout', () => {
  let covered = 0;
  for (const file of fs.readdirSync(path.join(__dirname, 'scenarios')).filter(n => n.endsWith('.json'))) {
    const scenario = JSON.parse(fs.readFileSync(path.join(__dirname, 'scenarios', file)));
    if (!scenario.attr_phase) continue;
    const setup = scenario.setup || [];
    const kills = setup.map((c, i) => /^kill \$\(bot2?\)$/.test(c) ? i : -1).filter(i => i >= 0);
    if (!kills.length) continue;
    assert.equal(syncInSetup(scenario), true, file);
    const marker = setup.indexOf(SYNC_COMMAND);
    const lastKill = kills.at(-1);
    assert.ok(marker > lastKill + 1, `${file}: wait for respawn before sync`);
    assert.ok(setup.slice(lastKill + 1, marker).every(c => /^!wait \d+$/.test(c)), file);
    // Повторная смерть второго робота не должна уничтожать подготовленные вещи.
    assert.ok(!setup.slice(0, marker).some(c => /^(give |item replace |effect give )/.test(c)), file);
    covered++;
  }
  assert.equal(covered, 14);
});
