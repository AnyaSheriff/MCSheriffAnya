package org.bukkit.potion;
public final class PotionEffect {
 public static final int INFINITE_DURATION=-1;
 private final PotionEffectType type;private final int duration,amplifier;private final boolean ambient,particles,icon;
 public PotionEffect(PotionEffectType type,int duration,int amplifier){this(type,duration,amplifier,true);}
 public PotionEffect(PotionEffectType type,int duration,int amplifier,boolean ambient){this(type,duration,amplifier,ambient,true);}
 public PotionEffect(PotionEffectType type,int duration,int amplifier,boolean ambient,boolean particles){this(type,duration,amplifier,ambient,particles,particles);}
 public PotionEffect(PotionEffectType type,int duration,int amplifier,boolean ambient,boolean particles,boolean icon){this.type=java.util.Objects.requireNonNull(type);this.duration=duration;this.amplifier=amplifier;this.ambient=ambient;this.particles=particles;this.icon=icon;}
 public PotionEffectType getType(){return type;}public int getDuration(){return duration;}public int getAmplifier(){return amplifier;}public boolean isAmbient(){return ambient;}public boolean hasParticles(){return particles;}public boolean hasIcon(){return icon;}public boolean isInfinite(){return duration==INFINITE_DURATION;}
 public boolean apply(org.bukkit.entity.LivingEntity entity){return entity.addPotionEffect(this);}
}
