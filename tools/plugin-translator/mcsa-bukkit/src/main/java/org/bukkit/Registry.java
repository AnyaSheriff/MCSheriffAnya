package org.bukkit;
public interface Registry<T extends Keyed> extends Iterable<T> {
 Registry<org.bukkit.potion.PotionType> POTION=new ru.mcsa.bukkit.SimpleRegistry<>(org.bukkit.potion.PotionType.values());
 Registry<org.bukkit.entity.EntityType> ENTITY_TYPE=new ru.mcsa.bukkit.SimpleRegistry<>(org.bukkit.entity.EntityType.values());
 T get(NamespacedKey key);
 Registry<org.bukkit.attribute.Attribute> ATTRIBUTE=new ru.mcsa.bukkit.SimpleRegistry<>(org.bukkit.attribute.Attribute.values());
 Registry<org.bukkit.enchantments.Enchantment> ENCHANTMENT=new ru.mcsa.bukkit.SimpleRegistry<>(org.bukkit.enchantments.Enchantment.values());
 Registry<org.bukkit.potion.PotionEffectType> EFFECT=new ru.mcsa.bukkit.SimpleRegistry<>(org.bukkit.potion.PotionEffectType.values());
}
