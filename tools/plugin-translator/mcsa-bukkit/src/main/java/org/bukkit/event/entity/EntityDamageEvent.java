package org.bukkit.event.entity;
public class EntityDamageEvent extends EntityEvent implements org.bukkit.event.Cancellable {
 public enum DamageCause { BLOCK_EXPLOSION,CAMPFIRE,CONTACT,CRAMMING,CUSTOM,DRAGON_BREATH,DROWNING,DRYOUT,ENTITY_ATTACK,ENTITY_EXPLOSION,ENTITY_SWEEP_ATTACK,FALL,FALLING_BLOCK,FIRE,FIRE_TICK,FLY_INTO_WALL,FREEZE,HOT_FLOOR,KILL,LAVA,LIGHTNING,MAGIC,MELTING,POISON,PROJECTILE,SONIC_BOOM,STARVATION,SUFFOCATION,THORNS,VOID,WITHER,WORLD_BORDER }
 private final DamageCause cause;private double damage;private boolean cancelled;
 public EntityDamageEvent(org.bukkit.entity.Entity entity,DamageCause cause,double damage){super(entity);this.cause=cause;setDamage(damage);}
 public DamageCause getCause(){return cause;}public double getDamage(){return damage;}
 public void setDamage(double damage){if(!Double.isFinite(damage)||damage<0)throw new IllegalArgumentException("Урон должен быть конечным и неотрицательным");this.damage=damage;}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
