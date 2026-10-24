package org.bukkit.event.entity;
public class EntityDeathEvent extends EntityEvent {
 private final java.util.List<org.bukkit.inventory.ItemStack> drops;
 public EntityDeathEvent(org.bukkit.entity.LivingEntity entity,java.util.List<org.bukkit.inventory.ItemStack> drops){super(entity);this.drops=java.util.Objects.requireNonNull(drops);}public org.bukkit.entity.LivingEntity getEntity(){return (org.bukkit.entity.LivingEntity)entity;}public java.util.List<org.bukkit.inventory.ItemStack> getDrops(){return drops;}
}
