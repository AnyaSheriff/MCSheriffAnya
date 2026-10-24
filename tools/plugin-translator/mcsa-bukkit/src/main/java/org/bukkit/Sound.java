package org.bukkit;
public interface Sound extends Keyed {
 Sound BLOCK_ANVIL_USE=new ru.mcsa.bukkit.SimpleSound("BLOCK_ANVIL_USE","block.anvil.use",0);
 Sound ENTITY_ENDERMAN_TELEPORT=new ru.mcsa.bukkit.SimpleSound("ENTITY_ENDERMAN_TELEPORT","entity.enderman.teleport",1);
 Sound ENTITY_EVOKER_PREPARE_SUMMON=new ru.mcsa.bukkit.SimpleSound("ENTITY_EVOKER_PREPARE_SUMMON","entity.evoker.prepare_summon",2);
 Sound ENTITY_MAGMA_CUBE_SQUISH=new ru.mcsa.bukkit.SimpleSound("ENTITY_MAGMA_CUBE_SQUISH","entity.magma_cube.squish",3);
 Sound ENTITY_PIGLIN_ADMIRING_ITEM=new ru.mcsa.bukkit.SimpleSound("ENTITY_PIGLIN_ADMIRING_ITEM","entity.piglin.admiring_item",4);
 Sound ENTITY_PLAYER_ATTACK_CRIT=new ru.mcsa.bukkit.SimpleSound("ENTITY_PLAYER_ATTACK_CRIT","entity.player.attack.crit",5);
 String name();int ordinal();
 static Sound[] values(){return new Sound[]{BLOCK_ANVIL_USE,ENTITY_ENDERMAN_TELEPORT,ENTITY_EVOKER_PREPARE_SUMMON,ENTITY_MAGMA_CUBE_SQUISH,ENTITY_PIGLIN_ADMIRING_ITEM,ENTITY_PLAYER_ATTACK_CRIT};}
 static Sound valueOf(String name){java.util.Objects.requireNonNull(name);for(Sound sound:values())if(sound.name().equals(name))return sound;throw new IllegalArgumentException("Неизвестный звук: "+name);}
}
