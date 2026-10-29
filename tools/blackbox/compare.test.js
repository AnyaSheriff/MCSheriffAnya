// Проверяем настоящий CLI на синтетических записях; Minecraft не запускается.
'use strict';
const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const action = { kind: 'do', tick: 0, what: 'probe' };
const block = { kind: 'block', tick: 1, at: [0, 0, 0], state: 'stone' };
const saved = events => ({ events });

function compare(left, right, scenario = {}) {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'mcsa-compare-'));
  try {
    for (const file of ['compare.js', 'value-order.js']) fs.copyFileSync(path.join(__dirname, file), path.join(dir, file));
    fs.mkdirSync(path.join(dir, 'scenarios'));
    fs.writeFileSync(path.join(dir, 'scenarios/probe.json'), JSON.stringify(scenario));
    const files = [left, right].map((value, i) => {
      const file = path.join(dir, `${i}.json`);
      fs.writeFileSync(file, typeof value === 'string' ? value : JSON.stringify(value));
      return file;
    });
    const result = spawnSync(process.execPath, [path.join(dir, 'compare.js'), 'probe', '--files', ...files], { encoding: 'utf8', timeout: 10000 });
    assert.ifError(result.error);
    assert.equal(result.signal, null);
    const reports = fs.readdirSync(path.join(dir, 'reports')).filter(n => n.endsWith('.md'));
    return { ...result, reports: reports.map(n => fs.readFileSync(path.join(dir, 'reports', n), 'utf8')) };
  } finally { fs.rmSync(dir, { recursive: true, force: true }); }
}

test('matching server replies succeed and retain the report', () => {
  const result = compare(saved([action, block]), saved([action, block]));
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /Совпало: 1\. Сдвинуто по тактам: 0\. Нет у нас: 0\. Лишнее у нас: 0\./);
  assert.deepEqual(result.reports, [result.stdout]);
});

for (const [name, events, count] of [
  ['changed block', [block, { ...block, tick: 2, state: 'dirt' }], /Нет у нас: 1\. Лишнее у нас: 1\./],
  ['missing reply', [block], /Нет у нас: 1\. Лишнее у нас: 0\./],
  ['extra reply', [block, { ...block, tick: 2, state: 'air' }, { ...block, tick: 3, state: 'dirt' }], /Лишнее у нас: 1\./],
  ['shifted reply', [block, { ...block, tick: 5, state: 'air' }], /Сдвинуто по тактам: 1\./],
]) {
  test(`${name} fails after saving the comparison`, () => {
    const result = compare(saved([action, block, { ...block, tick: 2, state: 'air' }]), saved([action, ...events]));
    assert.equal(result.status, 1, result.stderr);
    assert.match(result.stdout, count);
    assert.deepEqual(result.reports, [result.stdout]);
  });
}

for (const [name, record, scenario] of [
  ['empty arrays', saved([]), {}],
  ['only action markers', saved([action]), {}],
  ['only continuous action markers', saved([action]), { continuous: true }],
  ['events before any action', saved([block]), {}],
  ['replies outside recording window', { events: [action, block], window: { last: 0 } }, {}],
  ['missing event array', {}, {}],
  ['null event', saved([action, null]), {}],
  ['invalid time', saved([action, { ...block, tick: 'bad' }]), {}],
  ['malformed JSON', '{', {}],
]) {
  test(`${name} cannot pass`, () => {
    const result = compare(record, record, scenario);
    assert.equal(result.status, 1);
    assert.ok(result.stderr.trim());
    assert.doesNotMatch(result.stdout, /Совпало: 0/);
  });
}

test('one empty side cannot pass', () => {
  for (const sides of [[saved([]), saved([action, block])], [saved([action, block]), saved([])]]) {
    assert.equal(compare(...sides).status, 1);
  }
});

test('continuous actions with real replies still pass', () => {
  const record = saved([action, block]);
  assert.equal(compare(record, record, { continuous: true }).status, 0);
});

test('different robot commands cannot pass with identical replies', () => {
  const result = compare(saved([action, block]), saved([{ ...action, what: 'other probe' }, block]));
  assert.equal(result.status, 1);
  assert.match(result.stdout, /Последовательности действий робота различаются/);
});

test('unknown random rule fails even when the counters match', () => {
  const record = saved([action, block]);
  const result = compare(record, record, { random: [{ type: 'unknown', name: 'bad rule', why: 'test' }] });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /Внимание:/);
});

test('within-tick ordering remains a visible note under existing rules', () => {
  const other = { ...block, at: [1, 0, 0] };
  const result = compare(saved([action, block, other]), saved([action, other, block]));
  assert.equal(result.status, 0);
  assert.match(result.stdout, /порядок в такте другой/);
});
