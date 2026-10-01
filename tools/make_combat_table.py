"""Собирает src/combat_table.rs: боевые свойства предметов 26.1.2.

Источник — данные, не код: отчёты штатного генератора официального сервера
(`java -DbundlerMainClass=net.minecraft.data.Main -jar server.jar --reports`),
файлы reports/minecraft/components/item/*.json и reports/registries.json.
Берутся только числа предметов: модификаторы атрибутов (урон, скорость
удара, броня, твёрдость, стойкость к отбрасыванию), прочность, зачаровываемость,
место надевания и звук, «оружие» (расход прочности за удар, отключение щита).

Использование: python3 tools/make_combat_table.py <папка reports>
"""
import glob
import json
import os
import sys

reports = sys.argv[1]
registries = json.load(open(os.path.join(reports, 'registries.json')))
attributes = {k: v['protocol_id'] for k, v in registries['minecraft:attribute']['entries'].items()}
items = {k: v['protocol_id'] for k, v in registries['minecraft:item']['entries'].items()}

# Номера групп мест в компоненте attribute_modifiers (EquipmentSlotGroup).
SLOT_GROUPS = ['any', 'mainhand', 'offhand', 'hand', 'feet', 'legs', 'chest', 'head', 'armor', 'body', 'saddle']
OPERATIONS = ['add_value', 'add_multiplied_base', 'add_multiplied_total']

rows = []
for path in sorted(glob.glob(os.path.join(reports, 'minecraft', 'components', 'item', '*.json'))):
    name = os.path.basename(path)[:-5]
    comp = json.load(open(path))['components']
    mods = comp.get('minecraft:attribute_modifiers', [])
    max_damage = comp.get('minecraft:max_damage', 0)
    enchantable = comp.get('minecraft:enchantable', {}).get('value', 0)
    weapon = comp.get('minecraft:weapon')
    equip = comp.get('minecraft:equippable', {})
    if not (mods or max_damage or weapon is not None or enchantable or 'slot' in equip):
        continue
    if 'waypoint_transmit_range' in json.dumps(mods) and len(mods) == 1:
        mods = []
    rows.append((name, items[f'minecraft:{name}'], max_damage, enchantable, mods, weapon, equip))

out = []
out.append('// Боевые свойства предметов 26.1.2 — файл собирает tools/make_combat_table.py')
out.append('// из отчётов штатного генератора официального сервера (только числа).')
out.append('// Руками не править.')
out.append('')
out.append('/// Модификатор атрибута, который предмет даёт по умолчанию.')
out.append('#[derive(Clone, Copy, Debug, PartialEq)]')
out.append('pub struct ItemMod {')
out.append('    /// Номер атрибута в реестре.')
out.append('    pub attribute: i32,')
out.append('    /// Операция: 0 — прибавить, 1 — умножить основу, 2 — умножить итог.')
out.append('    pub operation: u8,')
out.append('    /// Группа мест (порядок EquipmentSlotGroup): 1 — основная рука, 4–7 — обувь…шлем.')
out.append('    pub slot: u8,')
out.append('    pub amount: f64,')
out.append('    pub id: &\'static str,')
out.append('}')
out.append('')
out.append('/// Боевые свойства одного предмета (часть полей — данные таблицы, которые')
out.append('/// сервер пока не читает: имя, зачаровываемость, место и звук надевания).')
out.append('#[derive(Clone, Copy, Debug)]')
out.append('#[allow(dead_code)]')
out.append('pub struct ItemInfo {')
out.append('    pub name: &\'static str,')
out.append('    pub item: i32,')
out.append('    pub max_damage: i32,')
out.append('    pub enchantable: i32,')
out.append('    pub mods: &\'static [ItemMod],')
out.append('    /// Есть компонент «оружие»: расход прочности за удар (0 — компонента нет).')
out.append('    pub damage_per_attack: i32,')
out.append('    /// Сколько секунд удар отключает щит (0 — не отключает).')
out.append('    pub disable_blocking_seconds: f32,')
out.append('    /// Место надевания: 0 — нет, 4 — обувь, 5 — штаны, 6 — нагрудник, 7 — шлем, 9 — тело, 2 — вторая рука.')
out.append('    pub equip_slot: u8,')
out.append('    /// Звук надевания.')
out.append('    pub equip_sound: &\'static str,')
out.append('}')
out.append('')
out.append('pub static ITEMS: &[ItemInfo] = &[')
for name, num, md, ench, mods, weapon, equip in rows:
    rs = []
    for m in mods:
        rs.append('ItemMod { attribute: %d, operation: %d, slot: %d, amount: %r, id: "%s" }' % (
            attributes[m['type']], OPERATIONS.index(m['operation']), SLOT_GROUPS.index(m['slot']), float(m['amount']), m['id']))
    mods_src = '&[' + ', '.join(rs) + ']'
    dpa = 0
    dis = 0.0
    if weapon is not None:
        dpa = weapon.get('item_damage_per_attack', 1)
        dis = float(weapon.get('disable_blocking_for_seconds', 0.0))
    slot = SLOT_GROUPS.index(equip['slot']) if equip.get('slot') in SLOT_GROUPS else 0
    if slot == 3:
        slot = 0
    sound = equip.get('equip_sound', '')
    out.append('    ItemInfo { name: "%s", item: %d, max_damage: %d, enchantable: %d, mods: %s, damage_per_attack: %d, disable_blocking_seconds: %r, equip_slot: %d, equip_sound: "%s" },' % (
        name, num, md, ench, mods_src, dpa, dis, slot, sound))
out.append('];')
out.append('')
open(os.path.join(os.path.dirname(__file__), '..', 'src', 'combat_table.rs'), 'w').write('\n'.join(out))
print(len(rows), 'предметов')
