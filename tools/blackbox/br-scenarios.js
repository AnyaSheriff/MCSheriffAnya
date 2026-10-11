// Сценарии для br-record.js. Игрок brrec — оператор; сначала выживание, чистая площадка.
const PRE = ['/gamemode survival @s', '/effect @s clear', '/kill @e[type=!player]', '/clear @s', '/xp -1000L @s', '/xp -1000 @s'];
const EFFECTS = ['speed', 'slowness', 'haste', 'mining_fatigue', 'strength', 'instant_health', 'jump_boost', 'nausea', 'regeneration', 'resistance', 'fire_resistance', 'water_breathing', 'invisibility', 'blindness', 'night_vision', 'hunger', 'weakness', 'poison', 'wither', 'health_boost', 'absorption', 'saturation', 'glowing', 'levitation', 'luck', 'unluck', 'slow_falling', 'conduit_power', 'dolphins_grace', 'bad_omen', 'village_hero', 'darkness', 'trial_omen', 'raid_omen', 'wind_charged', 'weaving', 'oozing', 'infested', 'breath_of_the_nautilus', 'instant_damage'];
const S = {};
S.effects = ['/gamemode survival @s', '/effect @s clear', { w: 500 }];
for (const e of EFFECTS) S.effects.push({ tag: e }, `/effect @s ${e} 30 1`, { w: 600 }, `/effect @s ${e} 30 2 true`, { w: 400 }, '/effect @s clear', { w: 400 });
S.melee_mob = [...PRE, '/gamemode survival @s', '/give @s diamond_sword', '/summon cow ~ ~ ~2', { w: 1200 }, { tag: 'hit1' }, { attack: 'minecraft:cow' }, { w: 800 }, { attack: 'minecraft:cow' }, { w: 800 }, '/kill @e[type=cow]', { w: 600 }];
S.melee_zombie = [...PRE, '/give @s iron_sword', '/summon husk ~ ~ ~2', { w: 1200 }, { tag: 'hit' }, { w: 5000 }, '/kill @e[type=husk]', { w: 800 }];
S.damage_events = [...PRE, { tag: 'fall' }, '/damage @s 4 fall', { w: 500 }, { tag: 'fire' }, '/damage @s 2 on_fire', { w: 500 }, { tag: 'void' }, '/damage @s 1 out_of_world', { w: 500 }, { tag: 'magic' }, '/damage @s 2 magic', { w: 500 }, { tag: 'death' }, '/kill @s', { w: 1500 }, { tag: 'respawn' }, { raw: 'respawn', p: { position: { x: 0, y: 0, z: 0 }, state: 2, runtime_entity_id: 0 }, after: 1500 }];
S.gamemodes = [...PRE, '/gamemode creative @s', { w: 600 }, '/gamemode spectator @s', { w: 600 }, '/gamemode adventure @s', { w: 600 }, '/gamemode survival @s', { w: 600 }];
S.hunger = [...PRE, '/effect @s hunger 5 5', { w: 2500 }, '/effect @s saturation 1 0', { w: 800 }, '/effect @s clear', '/damage @s 1', { w: 800 }];
S.absorption_xp = [...PRE, '/effect @s absorption 30 2', { w: 500 }, '/damage @s 3', { w: 500 }, '/effect @s clear', '/xp 7 @s', { w: 500 }, '/xp 3L @s', { w: 500 }, '/xp -3L @s', { w: 400 }];
S.totem = [...PRE, '/give @s totem_of_undying', { w: 400 }, { raw: 'mob_equipment', p: { runtime_entity_id: 0, item: { network_id: 0 }, slot: 0, selected_slot: 0, window_id: 0 }, after: 100 }, '/damage @s 100 generic_kill', { w: 1200 }, '/kill @s', { w: 1500 }];
S.freezing_air = [...PRE, '/setblock ~ ~ ~ powder_snow', { w: 4000 }, '/setblock ~ ~ ~ air', { w: 300 }, '/effect @s water_breathing 1 0', { w: 300 }];
S.attack_empty = [...PRE, '/summon cow ~ ~ ~2', { w: 1500 }, { tag: 'hit' }, { attack: 'minecraft:cow' }, { w: 800 }, { attack: 'minecraft:cow' }, { w: 800 }, '/kill @e[type=cow]', { w: 500 }];

S.mob_damage = [...PRE, '/summon cow ~ ~ ~2', { w: 1200 }, { tag: 'fall' }, '/damage @e[type=cow] 2 fall', { w: 700 }, { tag: 'magic' }, '/damage @e[type=cow] 1 magic', { w: 700 }, { tag: 'attack' }, '/damage @e[type=cow] 1 entity_attack', { w: 700 }, { tag: 'fire' }, '/damage @e[type=cow] 1 fire', { w: 700 }, { tag: 'void' }, '/damage @e[type=cow] 1 void', { w: 700 }, { tag: 'kill' }, '/damage @e[type=cow] 20 fall', { w: 2000 }];
S.multi = ['/gamemode survival @s', '/effect @s clear', { w: 500 }, '/effect @s speed 30 0', { w: 500 }, '/effect @s slowness 30 0', { w: 500 }, '/effect @s haste 30 0', { w: 500 }, '/effect @s slowness 30 0 true', { w: 500 }, '/effect @s speed 30 0 true', { w: 500 }, '/effect @s clear', { w: 500 }];
S.hb = ['/gamemode survival @s', '/effect @s clear', { w: 800 }, { tag: 'boost' }, '/effect @s health_boost 30 1', { w: 1500 }, '/effect @s absorption 30 1', { w: 1500 }, '/effect @s clear', { w: 1000 }];
S.restore = ['/defaultgamemode creative', { w: 800 }, '/gamemode creative @s', { w: 800 }];
S.pvp_bots = [...PRE];
module.exports = S;
