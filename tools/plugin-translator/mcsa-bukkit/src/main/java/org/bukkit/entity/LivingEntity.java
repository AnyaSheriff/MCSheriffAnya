package org.bukkit.entity;
public interface LivingEntity extends Damageable {
 boolean addPotionEffect(org.bukkit.potion.PotionEffect effect);
 void removePotionEffect(org.bukkit.potion.PotionEffectType type);
}
