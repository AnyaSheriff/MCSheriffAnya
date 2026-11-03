package org.bukkit.entity;
public interface LivingEntity extends Damageable,org.bukkit.projectiles.ProjectileSource {
 org.bukkit.inventory.EntityEquipment getEquipment();
 default java.util.List<Entity> getNearbyEntities(double x,double y,double z){return ru.mcsa.bukkit.RemoteEntity.nearby(this,x,y,z); }
 org.bukkit.attribute.AttributeInstance getAttribute(org.bukkit.attribute.Attribute attribute);
 java.util.Collection<org.bukkit.potion.PotionEffect> getActivePotionEffects();
 org.bukkit.potion.PotionEffect getPotionEffect(org.bukkit.potion.PotionEffectType type);
 double getEyeHeight();org.bukkit.Location getEyeLocation();
 boolean addPotionEffect(org.bukkit.potion.PotionEffect effect);
 void removePotionEffect(org.bukkit.potion.PotionEffectType type);
}
