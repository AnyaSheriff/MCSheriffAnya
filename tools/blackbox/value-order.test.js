'use strict';
const assert = require('node:assert/strict');
const { test } = require('node:test');
const { valueOrder } = require('./value-order');

test('enchantment order is immaterial, levels and duplicate entries are not', () => {
  const a = 'slot 0 diamond_sword {"enchantments":[{"id":41,"level":1},{"id":33,"level":3}]}';
  const b = 'slot 0 diamond_sword {"enchantments":[{"id":33,"level":3},{"id":41,"level":1}]}';
  assert.equal(valueOrder(a), valueOrder(b));
  assert.notEqual(valueOrder(a), valueOrder(b.replace('"level":3', '"level":4')));
  assert.notEqual(valueOrder(a), valueOrder(b.replace('{"id":41,"level":1}', '{"id":33,"level":3}')));
});

test('attribute values and modifiers survive order normalization', () => {
  const armor = { key: 'generic.armor', value: 0, modifiers: [{ uuid: 'minecraft:armor.boots', amount: 2, operation: 0 }] };
  const toughness = { key: 'generic.armor_toughness', value: 0, modifiers: [] };
  const text = values => `cart armor_stand entity_update_attributes ${JSON.stringify({ properties: values })}`;
  assert.equal(valueOrder(text([armor, toughness])), valueOrder(text([toughness, armor])));
  assert.notEqual(valueOrder(text([armor, toughness])), valueOrder(text([{ ...armor, value: 1 }, toughness])));
  assert.notEqual(valueOrder(text([armor, toughness])), valueOrder(text([{ ...armor, modifiers: [] }, toughness])));
});

test('unrelated arrays and malformed data are not changed', () => {
  for (const text of ['{"items":[2,1]}', '{"properties":[1,2]}', '{"enchantments":[bad]}']) {
    assert.equal(valueOrder(text), text);
  }
});
