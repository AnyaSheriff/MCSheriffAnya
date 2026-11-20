package org.bukkit;
public interface Sound extends Keyed {
 Sound BLOCK_ANVIL_USE=new ru.mcsa.bukkit.SimpleSound("BLOCK_ANVIL_USE","block.anvil.use",0);
 Sound ENTITY_ENDERMAN_TELEPORT=new ru.mcsa.bukkit.SimpleSound("ENTITY_ENDERMAN_TELEPORT","entity.enderman.teleport",1);
 Sound ENTITY_EVOKER_PREPARE_SUMMON=new ru.mcsa.bukkit.SimpleSound("ENTITY_EVOKER_PREPARE_SUMMON","entity.evoker.prepare_summon",2);
 Sound ENTITY_MAGMA_CUBE_SQUISH=new ru.mcsa.bukkit.SimpleSound("ENTITY_MAGMA_CUBE_SQUISH","entity.magma_cube.squish",3);
 Sound ENTITY_PIGLIN_ADMIRING_ITEM=new ru.mcsa.bukkit.SimpleSound("ENTITY_PIGLIN_ADMIRING_ITEM","entity.piglin.admiring_item",4);
 Sound ENTITY_PLAYER_ATTACK_CRIT=new ru.mcsa.bukkit.SimpleSound("ENTITY_PLAYER_ATTACK_CRIT","entity.player.attack.crit",5);
 Sound BLOCK_AMETHYST_BLOCK_CHIME=new ru.mcsa.bukkit.SimpleSound("BLOCK_AMETHYST_BLOCK_CHIME","block.amethyst_block.chime",6);
 Sound BLOCK_FIRE_EXTINGUISH=new ru.mcsa.bukkit.SimpleSound("BLOCK_FIRE_EXTINGUISH","block.fire.extinguish",7);
 Sound BLOCK_SCULK_SPREAD=new ru.mcsa.bukkit.SimpleSound("BLOCK_SCULK_SPREAD","block.sculk.spread",8);
 Sound ENTITY_BLAZE_HURT=new ru.mcsa.bukkit.SimpleSound("ENTITY_BLAZE_HURT","entity.blaze.hurt",9);
 Sound ENTITY_BLAZE_SHOOT=new ru.mcsa.bukkit.SimpleSound("ENTITY_BLAZE_SHOOT","entity.blaze.shoot",10);
 Sound ENTITY_ENDERMAN_HURT=new ru.mcsa.bukkit.SimpleSound("ENTITY_ENDERMAN_HURT","entity.enderman.hurt",11);
 Sound ENTITY_ENDERMAN_SCREAM=new ru.mcsa.bukkit.SimpleSound("ENTITY_ENDERMAN_SCREAM","entity.enderman.scream",12);
 Sound ENTITY_GENERIC_EXTINGUISH_FIRE=new ru.mcsa.bukkit.SimpleSound("ENTITY_GENERIC_EXTINGUISH_FIRE","entity.generic.extinguish_fire",13);
 Sound ENTITY_ILLUSIONER_PREPARE_BLINDNESS=new ru.mcsa.bukkit.SimpleSound("ENTITY_ILLUSIONER_PREPARE_BLINDNESS","entity.illusioner.prepare_blindness",14);
 Sound ENTITY_ITEM_BREAK=new ru.mcsa.bukkit.SimpleSound("ENTITY_ITEM_BREAK","entity.item.break",15);
 Sound ENTITY_ITEM_PICKUP=new ru.mcsa.bukkit.SimpleSound("ENTITY_ITEM_PICKUP","entity.item.pickup",16);
 Sound ENTITY_PLAYER_BURP=new ru.mcsa.bukkit.SimpleSound("ENTITY_PLAYER_BURP","entity.player.burp",17);
 Sound ENTITY_PLAYER_HURT_DROWN=new ru.mcsa.bukkit.SimpleSound("ENTITY_PLAYER_HURT_DROWN","entity.player.hurt_drown",18);
 Sound ENTITY_PLAYER_LEVELUP=new ru.mcsa.bukkit.SimpleSound("ENTITY_PLAYER_LEVELUP","entity.player.levelup",19);
 Sound ENTITY_WARDEN_SONIC_CHARGE=new ru.mcsa.bukkit.SimpleSound("ENTITY_WARDEN_SONIC_CHARGE","entity.warden.sonic_charge",20);
 Sound ENTITY_WOLF_GROWL=new ru.mcsa.bukkit.SimpleSound("ENTITY_WOLF_GROWL","entity.wolf.growl",21);
 Sound ENTITY_ZOMBIE_AMBIENT=new ru.mcsa.bukkit.SimpleSound("ENTITY_ZOMBIE_AMBIENT","entity.zombie.ambient",22);
 Sound ENTITY_ZOMBIE_ATTACK_WOODEN_DOOR=new ru.mcsa.bukkit.SimpleSound("ENTITY_ZOMBIE_ATTACK_WOODEN_DOOR","entity.zombie.attack_wooden_door",23);
 Sound ENTITY_ZOMBIE_HURT=new ru.mcsa.bukkit.SimpleSound("ENTITY_ZOMBIE_HURT","entity.zombie.hurt",24);
 Sound ITEM_CHORUS_FRUIT_TELEPORT=new ru.mcsa.bukkit.SimpleSound("ITEM_CHORUS_FRUIT_TELEPORT","item.chorus_fruit.teleport",25);
 Sound ITEM_SHIELD_BLOCK=new ru.mcsa.bukkit.SimpleSound("ITEM_SHIELD_BLOCK","item.shield.block",26);
 Sound ITEM_TOTEM_USE=new ru.mcsa.bukkit.SimpleSound("ITEM_TOTEM_USE","item.totem.use",27);
 String name();int ordinal();
 static Sound[] values(){return new Sound[]{
  BLOCK_ANVIL_USE,ENTITY_ENDERMAN_TELEPORT,ENTITY_EVOKER_PREPARE_SUMMON,ENTITY_MAGMA_CUBE_SQUISH,
  ENTITY_PIGLIN_ADMIRING_ITEM,ENTITY_PLAYER_ATTACK_CRIT,BLOCK_AMETHYST_BLOCK_CHIME,BLOCK_FIRE_EXTINGUISH,
  BLOCK_SCULK_SPREAD,ENTITY_BLAZE_HURT,ENTITY_BLAZE_SHOOT,ENTITY_ENDERMAN_HURT,
  ENTITY_ENDERMAN_SCREAM,ENTITY_GENERIC_EXTINGUISH_FIRE,ENTITY_ILLUSIONER_PREPARE_BLINDNESS,ENTITY_ITEM_BREAK,
  ENTITY_ITEM_PICKUP,ENTITY_PLAYER_BURP,ENTITY_PLAYER_HURT_DROWN,ENTITY_PLAYER_LEVELUP,
  ENTITY_WARDEN_SONIC_CHARGE,ENTITY_WOLF_GROWL,ENTITY_ZOMBIE_AMBIENT,ENTITY_ZOMBIE_ATTACK_WOODEN_DOOR,
  ENTITY_ZOMBIE_HURT,ITEM_CHORUS_FRUIT_TELEPORT,ITEM_SHIELD_BLOCK,ITEM_TOTEM_USE
 };}
 static Sound valueOf(String name){java.util.Objects.requireNonNull(name);for(Sound sound:values())if(sound.name().equals(name))return sound;throw new IllegalArgumentException("Неизвестный звук: "+name);}
}
