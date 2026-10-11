// Случаи для br-capture.js: одинаковые действия на BDS (Bedrock-команды) и на нашем сервере (Java-команды).
// Позиция призыва общая; игрок Bedrock стоит в 10 блоках.
const P = '4000.5 -60 4010.5';
// Для действий игрока: вплотную (на BDS досягаемость щедрая, у нас — как у Java).
const PU = '4000.5 -60 4002.0';
const cases = [];

// Java-имя -> Bedrock-имя, где они различаются.
const BR = { end_crystal: 'ender_crystal', zombified_piglin: 'zombie_pigman', evoker: 'evocation_illager', evoker_fangs: 'evocation_fang', tropical_fish: 'tropicalfish', villager: 'villager_v2', zombie_villager: 'zombie_villager_v2', experience_bottle: 'xp_bottle', potion: 'splash_potion', experience_orb: 'xp_orb', snow_golem: 'snow_golem' };
const br = j => BR[j] || j;
// Слово команды /summon у BDS: villager_v2 — имя в пакетах, а команда принимает villager.
const sm = j => (j === 'villager' || j === 'zombie_villager') ? j : br(j);

function add(label, bds, java, wait) { cases.push({ label, bds, java, wait }); }
// Простой призыв по Java-имени.
function mob(j, nbt, label, extra) {
  add(label || j + (nbt ? '+' : ''), [`summon ${sm(j)} ${P}`].concat(extra && extra.bds || []), [`summon minecraft:${j} ${P}${nbt ? ' ' + nbt : ''}`].concat(extra && extra.java || []));
}

if (!process.env.GROUP || process.env.GROUP === 'mobs') {
  for (const j of ['cow', 'pig', 'sheep', 'chicken', 'horse', 'donkey', 'mule', 'skeleton_horse', 'zombie_horse', 'llama', 'trader_llama', 'wandering_trader', 'camel', 'goat', 'polar_bear', 'panda', 'ocelot', 'mooshroom', 'rabbit', 'fox', 'bee', 'armadillo', 'sniffer', 'parrot', 'bat', 'allay', 'wolf', 'cat', 'villager', 'iron_golem', 'snow_golem', 'strider', 'turtle', 'axolotl', 'frog', 'tadpole', 'squid', 'glow_squid', 'dolphin', 'cod', 'salmon', 'tropical_fish', 'pufferfish', 'happy_ghast', 'copper_golem'])
    mob(j, null, 'd-' + j);
  for (const j of ['zombie', 'skeleton', 'stray', 'bogged', 'creeper', 'spider', 'cave_spider', 'silverfish', 'enderman', 'endermite', 'shulker', 'witch', 'blaze', 'wither_skeleton', 'magma_cube', 'slime', 'ghast', 'zombified_piglin', 'piglin', 'piglin_brute', 'hoglin', 'zoglin', 'drowned', 'husk', 'phantom', 'breeze', 'creaking', 'guardian', 'elder_guardian', 'wither', 'pillager', 'vindicator', 'evoker', 'vex', 'ravager', 'warden', 'ender_dragon', 'zombie_villager', 'end_crystal'])
    mob(j, null, 'd-' + j);
}
// Своя область выбора: рядом с точкой призыва (на BDS и у нас одинаковая запись).
const SEL = (t) => `@e[type=${t},x=4000,y=-60,z=4010,r=6]`;
const sel = j => SEL(br(j));
const jsel = j => `@e[type=minecraft:${j},limit=1]`;
// Призыв со «спавн-событием» BDS и данными Java.
function ev(label, j, bdsEvent, nbt) {
  add(label, [`summon ${sm(j)} ${P} 0 0 ${bdsEvent}`], [`summon minecraft:${j} ${P}${nbt ? ' ' + nbt : ''}`]);
}

if (!process.env.GROUP || process.env.GROUP === 'state') {
  // Детёныши.
  for (const j of ['cow', 'sheep', 'pig', 'chicken', 'horse', 'donkey', 'mule', 'llama', 'wolf', 'cat', 'fox', 'rabbit', 'goat', 'panda', 'camel', 'polar_bear', 'mooshroom', 'ocelot', 'turtle', 'armadillo', 'sniffer', 'villager', 'hoglin', 'strider', 'happy_ghast'])
    ev('b-' + j, j, 'minecraft:entity_born', '{Age:-24000}');
  for (const j of ['zombie', 'husk', 'drowned', 'zombie_villager', 'zombified_piglin', 'piglin', 'zoglin'])
    ev('b-' + j, j, 'minecraft:as_baby', '{IsBaby:1b}');
  // Приручённые и сидящие.
  add('t-wolf', [`summon wolf ${P}`, 300, `event entity ${sel('wolf')} minecraft:on_tame`], [`summon minecraft:wolf ${P} {Owner:[I;1,2,3,4]}`]);
  add('t-wolf-sit', [`summon wolf ${P}`, 300, `event entity ${sel('wolf')} minecraft:on_tame`, 300, `event entity ${sel('wolf')} minecraft:ai_command_sit`], [`summon minecraft:wolf ${P} {Owner:[I;1,2,3,4],Sitting:1b}`]);
  add('t-cat', [`summon cat ${P}`, 300, `event entity ${sel('cat')} minecraft:on_tame`], [`summon minecraft:cat ${P} {Owner:[I;1,2,3,4]}`]);
  add('t-cat-sit', [`summon cat ${P}`, 300, `event entity ${sel('cat')} minecraft:on_tame`, 300, `event entity ${sel('cat')} minecraft:ai_command_sit`], [`summon minecraft:cat ${P} {Owner:[I;1,2,3,4],Sitting:1b}`]);
  // Верховые: седло, броня, сундук.
  add('g-horse-saddle', [`summon horse ${P}`, 300, `item replace entity ${sel('horse')} slot.saddle with saddle`], [`summon minecraft:horse ${P} {Tame:1b,equipment:{saddle:{id:"minecraft:saddle",count:1}}}`]);
  add('g-horse-armor', [`summon horse ${P}`, 300, `item replace entity ${sel('horse')} slot.armor with diamond_horse_armor`], [`summon minecraft:horse ${P} {Tame:1b,equipment:{body:{id:"minecraft:diamond_horse_armor",count:1}}}`]);
  add('g-donkey-chest', [`summon donkey ${P}`, 300, `item replace entity ${sel('donkey')} slot.chest with chest`], [`summon minecraft:donkey ${P} {Tame:1b,ChestedHorse:1b}`]);
  add('g-llama-carpet', [`summon llama ${P}`, 300, `item replace entity ${sel('llama')} slot.armor with red_carpet`], [`summon minecraft:llama ${P} {Tame:1b,equipment:{body:{id:"minecraft:red_carpet",count:1}}}`]);
  add('g-wolf-armor', [`summon wolf ${P}`, 300, `event entity ${sel('wolf')} minecraft:on_tame`, 300, `item replace entity ${sel('wolf')} slot.armor with wolf_armor`], [`summon minecraft:wolf ${P} {Owner:[I;1,2,3,4]}`]);
  // Снаряжение мобов.
  add('g-zombie-gear', [`summon zombie ${P}`, 300, `item replace entity ${sel('zombie')} slot.armor.head with diamond_helmet`, `item replace entity ${sel('zombie')} slot.armor.chest with iron_chestplate`, `item replace entity ${sel('zombie')} slot.weapon.mainhand with iron_sword`, `item replace entity ${sel('zombie')} slot.weapon.offhand with shield`], [`summon minecraft:zombie ${P}`]);
  add('g-skeleton-bow', [`summon skeleton ${P}`], [`summon minecraft:skeleton ${P}`]);
  add('g-sheep-pink', [`summon sheep ${P} 0 0 minecraft:entity_born`], [`summon minecraft:sheep ${P} {Color:6b}`]);
  add('x-name', [`summon cow ${P} 0 0 minecraft:entity_spawned Bessie`], [`summon minecraft:cow ${P} {CustomName:"Bessie"}`]);
  add('x-creeper-powered', [`summon creeper ${P} 0 0 minecraft:become_charged`], [`summon minecraft:creeper ${P} {powered:1b}`]);
  add('x-enderman', [`summon enderman ${P}`], [`summon minecraft:enderman ${P}`]);
  add('x-slime-small', [`summon slime ${P} 0 0 minecraft:slime_small`], [`summon minecraft:slime ${P} {Size:0}`]);
  add('x-slime-big', [`summon slime ${P} 0 0 minecraft:slime_large`], [`summon minecraft:slime ${P} {Size:3}`]);
  add('x-villager-farmer', [`summon villager ${P} 0 0 minecraft:become_farmer`], [`summon minecraft:villager ${P} {VillagerData:{profession:"minecraft:farmer",level:1,type:"minecraft:plains"}}`]);
  add('x-sheared', [`summon sheep ${P}`, 300, `event entity ${sel('sheep')} minecraft:wool_sheared`], [`summon minecraft:sheep ${P} {Sheared:1b}`]);
}
// Действия игрока Bedrock над мобами: одинаковые у BDS и у нас (предмет в слот 0 и «use» по сущности).
if (!process.env.GROUP || process.env.GROUP === 'use') {
  const both = (label, list, wait) => add(label, list, list, wait);
  both('u-wolf-tame', [`summon wolf ${PU}`, 500, { give: 'bone' }, { use: 'minecraft:wolf', times: 10, gap: 450 }], 1500);
  both('u-wolf-sit', [`summon wolf ${PU}`, 500, { give: 'bone' }, { use: 'minecraft:wolf', times: 14, gap: 450 }, { give: 'stick' }, { use: 'minecraft:wolf', times: 1 }], 1500);
  both('u-cat-tame', [`summon cat ${PU}`, 500, { give: 'cod' }, { use: 'minecraft:cat', times: 10, gap: 450 }], 1500);
  both('u-sheep-shear', [`summon sheep ${PU}`, 500, { give: 'shears' }, { use: 'minecraft:sheep', times: 1 }], 1500);
  both('u-lead', [`summon cow ${PU}`, 500, { give: 'lead' }, { use: 'minecraft:cow', times: 1 }, 800, { use: 'minecraft:cow', times: 1 }], 1500);
  both('u-llama-carpet', [`summon llama ${PU}`, 500, { give: 'white_carpet' }, { use: 'minecraft:llama', times: 1 }], 1500);
  both('u-horse-saddle', [`summon horse ${PU}`, 500, { give: 'saddle' }, { use: 'minecraft:horse', times: 1 }], 1500);
  both('u-horse-armor', [`summon horse ${PU}`, 500, { give: 'diamond_horse_armor' }, { use: 'minecraft:horse', times: 1 }], 1500);
  add('u-horse-tame-probe', [`summon horse ${PU}`, 500, `event entity ${sel('horse')} minecraft:horse_tamed`, `event entity ${sel('horse')} minecraft:on_tame`, `event entity ${sel('horse')} minecraft:tame`, `event entity ${sel('horse')} minecraft:ai_tamed`, `event entity ${sel('horse')} minecraft:become_tamed`], null, 800);
  both('u-armadillo-brush', [`summon armadillo ${PU}`, 500, { give: 'brush' }, { use: 'minecraft:armadillo', times: 1 }], 1500);
  both('u-sheep-dye', [`summon sheep ${PU}`, 500, { give: 'red_dye' }, { use: 'minecraft:sheep', times: 1 }], 1500);
  both('u-cow-bucket', [`summon cow ${PU}`, 500, { give: 'bucket' }, { use: 'minecraft:cow', times: 1 }], 1500);
}
// Снаряды, транспорт и наездники.
if (!process.env.GROUP || process.env.GROUP === 'misc') {
  const AIR = '4000.5 -58 4010.5';
  const bothAt = (label, bds, java, wait) => add(label, [bds], [java], wait);
  for (const [b, j] of [['arrow', 'arrow'], ['wither_skull', 'wither_skull'], ['dragon_fireball', 'dragon_fireball'], ['wind_charge_projectile', 'wind_charge'], ['breeze_wind_charge_projectile', 'breeze_wind_charge']])
    bothAt('p-' + j, `summon ${b} ${AIR}`, `summon minecraft:${j} ${AIR}`);
  for (const [b, j] of [['minecart', 'minecart'], ['chest_minecart', 'chest_minecart'], ['hopper_minecart', 'hopper_minecart'], ['tnt_minecart', 'tnt_minecart'], ['boat', 'oak_boat'], ['chest_boat', 'oak_chest_boat']])
    bothAt('v-' + j, `summon ${b} ${P}`, `summon minecraft:${j} ${P}`);
  bothAt('v-furnace_minecart', `summon minecart ${P}`, `summon minecraft:furnace_minecart ${P}`);
  bothAt('p-tnt', `summon tnt ${P}`, `summon minecraft:tnt ${P}`);
  bothAt('p-xp_orb', `summon xp_orb ${P}`, `summon minecraft:experience_orb ${P}`);
  bothAt('p-lightning', `summon lightning_bolt ${P}`, `summon minecraft:lightning_bolt ${P}`);
  bothAt('p-armor_stand', `summon armor_stand ${P}`, `summon minecraft:armor_stand ${P}`);
  // Метание предметов игроком Bedrock.
  for (const item of ['snowball', 'egg', 'ender_pearl', 'experience_bottle', 'splash_potion', 'lingering_potion', 'trident', 'fire_charge', 'wind_charge', 'firework_rocket'])
    add('p-throw-' + item, [{ give: item === 'experience_bottle' ? 'experience_bottle' : item }, { throw: 1 }], [{ give: item }, { throw: 1 }], 1200);
  // Игрок Bedrock садится в лодку и вагонетку щелчком; потом выходит (команда ride на BDS у животных не годится).
  add('r-boat-player', [`summon boat ${PU}`, 500, { use: 'minecraft:boat', times: 1 }, 1200], [`summon minecraft:oak_boat ${PU}`, 500, { use: 'minecraft:boat', times: 1 }, 1200], 1000);
  add('r-minecart-player', [`summon minecart ${PU}`, 500, { use: 'minecraft:minecart', times: 1 }, 1200], [`summon minecraft:minecart ${PU}`, 500, { use: 'minecraft:minecart', times: 1 }, 1200], 1000);
}
module.exports = { cases, P, br, add, mob, ev, sel, jsel };
