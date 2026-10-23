package org.bukkit;
public enum Material { AIR, STONE, DIRT, GRASS_BLOCK, COBBLESTONE, OAK_PLANKS, GLASS, BEDROCK, DIAMOND, DIAMOND_SWORD, DIAMOND_PICKAXE, IRON_INGOT, GOLD_INGOT, APPLE, BREAD, STICK, TORCH, CHEST, TNT, WATER, LAVA, SAND, GRAVEL;
 public boolean isAir(){return this==AIR;} public static Material matchMaterial(String n){try{return valueOf(n.replace("minecraft:","").toUpperCase(java.util.Locale.ROOT));}catch(IllegalArgumentException e){return null;}}
}
