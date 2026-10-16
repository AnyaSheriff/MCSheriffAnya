// Проверки допусков без сервера: случайность не должна скрывать поломку блока,
// неверный предмет, число брызг или пропущенную стадию аметиста.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { spawnSync } = require('node:child_process');
const root = path.resolve(__dirname, '../..');

function compare(name, left, right) {
  const dir = fs.mkdtempSync(path.join(root, 'tmp/block3-rule-'));
  try {
    const files = [left, right].map((events, i) => {
      const file = path.join(dir, `${i}.json`);
      fs.writeFileSync(file, JSON.stringify({ events: [{ kind: 'do', tick: 0, what: 'probe' }, ...events.map(e => ({ tick: 0, ...e }))] }));
      return file;
    });
    const result = spawnSync(process.execPath, [path.join(__dirname, 'compare.js'), name, '--files', ...files], { encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
    const counts = /Совпало: (\d+)\. Сдвинуто по тактам: (\d+)\. Нет у нас: (\d+)\. Лишнее у нас: (\d+)\./.exec(result.stdout);
    assert.ok(counts, result.stdout);
    return counts.slice(1).map(Number);
  } finally { fs.rmSync(dir, { recursive: true }); }
}

test('dirt splash positions vary, particle count and mud state remain exact', () => {
  const mud = { kind: 'block', at: [0, 0, 0], state: 'mud' };
  const splash = x => ({ kind: 'particle', at: [x, 1, 0.5], particle: 'splash', count: 1, speed: 1, offset: '0,0,0' });
  const a = [mud, ...[0.1, 0.2, 0.3, 0.4, 0.5].map(splash)];
  const b = [mud, ...[0.3, 0.4, 0.6, 0.7, 0.8].map(splash)];
  assert.deepEqual(compare('block3-dirt', a, b).slice(1), [0, 0, 0]);
  assert.ok(compare('block3-dirt', a, b.slice(0, -1))[2] > 0);
  assert.ok(compare('block3-dirt', a, [{ ...mud, state: 'dirt' }, ...b.slice(1)])[2] > 0);
  assert.ok(compare('block3-dirt', a, [mud, splash(2), ...b.slice(2)])[2] > 0);
});

test('carpet movement allowance does not hide a wrong initial drop or surviving carpet', () => {
  const air = { kind: 'block', at: [0, 1, 0], state: 'air' };
  const meta = { kind: 'meta', fields: '8={"itemCount":1,"itemId":506,"addedComponentCount":0,"removedComponentCount":0,"components":[],"removeComponents":[]}' };
  const movement = { kind: 'cart', ent: 'предмет', what: 'vel', v: [0.02, 0, 0.01], tick: 15 };
  assert.deepEqual(compare('block3-white-carpet', [air, meta, movement], [air, meta]).slice(1), [0, 0, 0]);
  assert.ok(compare('block3-white-carpet', [air, meta], [air, { ...meta, fields: meta.fields.replace('506', '507') }])[2] > 0);
  assert.ok(compare('block3-white-carpet', [air, meta], [{ ...air, state: 'white_carpet' }, meta])[2] > 0);
});

test('amethyst stages may move in time but cannot disappear or grow on the control block', () => {
  const stages = ['medium_amethyst_bud', 'large_amethyst_bud', 'amethyst_cluster'].map((name, i) => ({ kind: 'block', tick: i * 10, at: [0, 1, 0], state: `${name}[facing=up,waterlogged=false]` }));
  assert.deepEqual(compare('amethyst-bud-growth', stages, stages.map((e, i) => ({ ...e, tick: i * 30 }))).slice(1), [0, 0, 0]);
  assert.ok(compare('amethyst-bud-growth', stages, stages.slice(0, 2))[2] > 0);
  assert.ok(compare('amethyst-bud-growth', stages, [...stages, { ...stages[2], at: [2, 1, 0] }])[3] > 0);
});

test('inventory slot values are compared, including counts', () => {
  const a = [{ kind: 'inv', slot: 36, item: 'cooked_beef×1' }];
  assert.ok(compare('block3-dirt', a, [{ ...a[0], item: 'stone×1' }])[2] > 0);
  assert.ok(compare('block3-dirt', a, [{ ...a[0], item: 'cooked_beef×2' }])[2] > 0);
});

test('random sword enchantments cannot hide curses, duplicates or incompatible damage', () => {
  const sword = entries => ({ kind: 'slot', window: 1, slot: 0, item: `diamond_sword×1 {enchantments=${JSON.stringify({ enchantments: entries })}}` });
  const sharp = { id: 33, level: 4 }, fire = { id: 10, level: 2 }, smite = { id: 35, level: 5 };
  const a = [sword([sharp, fire])];
  assert.deepEqual(compare('enchant-sword', a, [sword([smite])]).slice(1), [0, 0, 0]);
  for (const invalid of [[sharp, sharp], [sharp, smite], [{ id: 41, level: 1 }], [{ id: 33, level: 6 }]]) {
    assert.ok(compare('enchant-sword', a, [sword(invalid)])[2] > 0);
  }
});
