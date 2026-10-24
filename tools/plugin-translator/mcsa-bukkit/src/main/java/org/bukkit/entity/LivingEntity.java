package org.bukkit.entity;
public interface LivingEntity extends Damageable {
 org.bukkit.attribute.AttributeInstance getAttribute(org.bukkit.attribute.Attribute attribute);
 boolean addPotionEffect(org.bukkit.potion.PotionEffect effect);
 void removePotionEffect(org.bukkit.potion.PotionEffectType type);
}
