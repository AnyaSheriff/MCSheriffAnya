package org.bukkit.event.entity;
public final class EntityDamageByEntityEvent extends EntityDamageEvent {
 private final org.bukkit.entity.Entity damager;
 public EntityDamageByEntityEvent(org.bukkit.entity.Entity damager,org.bukkit.entity.Entity victim,DamageCause cause,double damage){super(victim,cause,damage);this.damager=java.util.Objects.requireNonNull(damager);}
 public org.bukkit.entity.Entity getDamager(){return damager;}
}
