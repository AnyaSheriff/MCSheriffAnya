package org.bukkit.potion;
public final class PotionEffectType implements org.bukkit.Keyed {
 private final org.bukkit.NamespacedKey key;private PotionEffectType(String name){key=org.bukkit.NamespacedKey.minecraft(name);}public org.bukkit.NamespacedKey getKey(){return key;}public String getName(){return key.getKey().toUpperCase(java.util.Locale.ROOT);}
 public static final PotionEffectType ABSORPTION=new PotionEffectType("absorption");
 public static final PotionEffectType BAD_OMEN=new PotionEffectType("bad_omen");
 public static final PotionEffectType BLINDNESS=new PotionEffectType("blindness");
 public static final PotionEffectType BREATH_OF_THE_NAUTILUS=new PotionEffectType("breath_of_the_nautilus");
 public static final PotionEffectType CONDUIT_POWER=new PotionEffectType("conduit_power");
 public static final PotionEffectType DARKNESS=new PotionEffectType("darkness");
 public static final PotionEffectType DOLPHINS_GRACE=new PotionEffectType("dolphins_grace");
 public static final PotionEffectType FIRE_RESISTANCE=new PotionEffectType("fire_resistance");
 public static final PotionEffectType GLOWING=new PotionEffectType("glowing");
 public static final PotionEffectType HASTE=new PotionEffectType("haste");
 public static final PotionEffectType HEALTH_BOOST=new PotionEffectType("health_boost");
 public static final PotionEffectType HERO_OF_THE_VILLAGE=new PotionEffectType("hero_of_the_village");
 public static final PotionEffectType HUNGER=new PotionEffectType("hunger");
 public static final PotionEffectType INFESTED=new PotionEffectType("infested");
 public static final PotionEffectType INSTANT_DAMAGE=new PotionEffectType("instant_damage");
 public static final PotionEffectType INSTANT_HEALTH=new PotionEffectType("instant_health");
 public static final PotionEffectType INVISIBILITY=new PotionEffectType("invisibility");
 public static final PotionEffectType JUMP_BOOST=new PotionEffectType("jump_boost");
 public static final PotionEffectType LEVITATION=new PotionEffectType("levitation");
 public static final PotionEffectType LUCK=new PotionEffectType("luck");
 public static final PotionEffectType MINING_FATIGUE=new PotionEffectType("mining_fatigue");
 public static final PotionEffectType NAUSEA=new PotionEffectType("nausea");
 public static final PotionEffectType NIGHT_VISION=new PotionEffectType("night_vision");
 public static final PotionEffectType OOZING=new PotionEffectType("oozing");
 public static final PotionEffectType POISON=new PotionEffectType("poison");
 public static final PotionEffectType RAID_OMEN=new PotionEffectType("raid_omen");
 public static final PotionEffectType REGENERATION=new PotionEffectType("regeneration");
 public static final PotionEffectType RESISTANCE=new PotionEffectType("resistance");
 public static final PotionEffectType SATURATION=new PotionEffectType("saturation");
 public static final PotionEffectType SLOW_FALLING=new PotionEffectType("slow_falling");
 public static final PotionEffectType SLOWNESS=new PotionEffectType("slowness");
 public static final PotionEffectType SPEED=new PotionEffectType("speed");
 public static final PotionEffectType STRENGTH=new PotionEffectType("strength");
 public static final PotionEffectType TRIAL_OMEN=new PotionEffectType("trial_omen");
 public static final PotionEffectType UNLUCK=new PotionEffectType("unluck");
 public static final PotionEffectType WATER_BREATHING=new PotionEffectType("water_breathing");
 public static final PotionEffectType WEAKNESS=new PotionEffectType("weakness");
 public static final PotionEffectType WEAVING=new PotionEffectType("weaving");
 public static final PotionEffectType WIND_CHARGED=new PotionEffectType("wind_charged");
 public static final PotionEffectType WITHER=new PotionEffectType("wither");
 private static final PotionEffectType[] VALUES={ABSORPTION,BAD_OMEN,BLINDNESS,BREATH_OF_THE_NAUTILUS,CONDUIT_POWER,DARKNESS,DOLPHINS_GRACE,FIRE_RESISTANCE,GLOWING,HASTE,HEALTH_BOOST,HERO_OF_THE_VILLAGE,HUNGER,INFESTED,INSTANT_DAMAGE,INSTANT_HEALTH,INVISIBILITY,JUMP_BOOST,LEVITATION,LUCK,MINING_FATIGUE,NAUSEA,NIGHT_VISION,OOZING,POISON,RAID_OMEN,REGENERATION,RESISTANCE,SATURATION,SLOW_FALLING,SLOWNESS,SPEED,STRENGTH,TRIAL_OMEN,UNLUCK,WATER_BREATHING,WEAKNESS,WEAVING,WIND_CHARGED,WITHER};
 public static PotionEffectType[] values(){return VALUES.clone();}
 public static PotionEffectType getByName(String name){if(name==null)return null;for(PotionEffectType t:VALUES)if(t.getName().equalsIgnoreCase(name))return t;return null;}
 public static PotionEffectType getByKey(org.bukkit.NamespacedKey key){for(PotionEffectType t:VALUES)if(t.key.equals(key))return t;return null;}
}
