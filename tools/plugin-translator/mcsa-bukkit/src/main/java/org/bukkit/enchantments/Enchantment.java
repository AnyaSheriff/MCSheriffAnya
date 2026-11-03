package org.bukkit.enchantments;
public final class Enchantment implements org.bukkit.Keyed {
 public static final Enchantment AQUA_AFFINITY=new Enchantment("aqua_affinity",1);
 public static final Enchantment BANE_OF_ARTHROPODS=new Enchantment("bane_of_arthropods",5);
 public static final Enchantment BINDING_CURSE=new Enchantment("binding_curse",1);
 public static final Enchantment BLAST_PROTECTION=new Enchantment("blast_protection",4);
 public static final Enchantment BREACH=new Enchantment("breach",4);
 public static final Enchantment CHANNELING=new Enchantment("channeling",1);
 public static final Enchantment DENSITY=new Enchantment("density",5);
 public static final Enchantment DEPTH_STRIDER=new Enchantment("depth_strider",3);
 public static final Enchantment EFFICIENCY=new Enchantment("efficiency",5);
 public static final Enchantment FEATHER_FALLING=new Enchantment("feather_falling",4);
 public static final Enchantment FIRE_ASPECT=new Enchantment("fire_aspect",2);
 public static final Enchantment FIRE_PROTECTION=new Enchantment("fire_protection",4);
 public static final Enchantment FLAME=new Enchantment("flame",1);
 public static final Enchantment FORTUNE=new Enchantment("fortune",3);
 public static final Enchantment FROST_WALKER=new Enchantment("frost_walker",2);
 public static final Enchantment IMPALING=new Enchantment("impaling",5);
 public static final Enchantment INFINITY=new Enchantment("infinity",1);
 public static final Enchantment KNOCKBACK=new Enchantment("knockback",2);
 public static final Enchantment LOOTING=new Enchantment("looting",3);
 public static final Enchantment LOYALTY=new Enchantment("loyalty",3);
 public static final Enchantment LUCK_OF_THE_SEA=new Enchantment("luck_of_the_sea",3);
 public static final Enchantment LUNGE=new Enchantment("lunge",3);
 public static final Enchantment LURE=new Enchantment("lure",3);
 public static final Enchantment MENDING=new Enchantment("mending",1);
 public static final Enchantment MULTISHOT=new Enchantment("multishot",1);
 public static final Enchantment PIERCING=new Enchantment("piercing",4);
 public static final Enchantment POWER=new Enchantment("power",5);
 public static final Enchantment PROJECTILE_PROTECTION=new Enchantment("projectile_protection",4);
 public static final Enchantment PROTECTION=new Enchantment("protection",4);
 public static final Enchantment PUNCH=new Enchantment("punch",2);
 public static final Enchantment QUICK_CHARGE=new Enchantment("quick_charge",3);
 public static final Enchantment RESPIRATION=new Enchantment("respiration",3);
 public static final Enchantment RIPTIDE=new Enchantment("riptide",3);
 public static final Enchantment SHARPNESS=new Enchantment("sharpness",5);
 public static final Enchantment SILK_TOUCH=new Enchantment("silk_touch",1);
 public static final Enchantment SMITE=new Enchantment("smite",5);
 public static final Enchantment SOUL_SPEED=new Enchantment("soul_speed",3);
 public static final Enchantment SWEEPING_EDGE=new Enchantment("sweeping_edge",3);
 public static final Enchantment SWIFT_SNEAK=new Enchantment("swift_sneak",3);
 public static final Enchantment THORNS=new Enchantment("thorns",3);
 public static final Enchantment UNBREAKING=new Enchantment("unbreaking",3);
 public static final Enchantment VANISHING_CURSE=new Enchantment("vanishing_curse",1);
 public static final Enchantment WIND_BURST=new Enchantment("wind_burst",3);
 public boolean conflictsWith(Enchantment other){if(other==null||other==this)return false;String[][] groups=new String[][]{{"sharpness","smite","bane_of_arthropods","density","breach","impaling"},{"protection","fire_protection","blast_protection","projectile_protection"},{"depth_strider","frost_walker"},{"silk_touch","fortune"},{"infinity","mending"},{"multishot","piercing"},{"riptide","loyalty"},{"riptide","channeling"}};for(String[] group:groups)if(java.util.Arrays.asList(group).contains(name)&&java.util.Arrays.asList(group).contains(other.name))return true;return false;}
 private final String name;private final int max;private Enchantment(String name,int max){this.name=name;this.max=max;}public org.bukkit.NamespacedKey getKey(){return org.bukkit.NamespacedKey.minecraft(name);}public int getMaxLevel(){return max;}public static Enchantment getByKey(org.bukkit.NamespacedKey key){for(Enchantment value:values())if(value.getKey().equals(key))return value;return null;}public static Enchantment[] values(){return new Enchantment[]{AQUA_AFFINITY,BANE_OF_ARTHROPODS,BINDING_CURSE,BLAST_PROTECTION,BREACH,CHANNELING,DENSITY,DEPTH_STRIDER,EFFICIENCY,FEATHER_FALLING,FIRE_ASPECT,FIRE_PROTECTION,FLAME,FORTUNE,FROST_WALKER,IMPALING,INFINITY,KNOCKBACK,LOOTING,LOYALTY,LUCK_OF_THE_SEA,LUNGE,LURE,MENDING,MULTISHOT,PIERCING,POWER,PROJECTILE_PROTECTION,PROTECTION,PUNCH,QUICK_CHARGE,RESPIRATION,RIPTIDE,SHARPNESS,SILK_TOUCH,SMITE,SOUL_SPEED,SWEEPING_EDGE,SWIFT_SNEAK,THORNS,UNBREAKING,VANISHING_CURSE,WIND_BURST};}
}
