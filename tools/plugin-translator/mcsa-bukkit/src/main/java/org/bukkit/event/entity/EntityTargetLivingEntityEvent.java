package org.bukkit.event.entity;
public class EntityTargetLivingEntityEvent extends EntityEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.LivingEntity target;private boolean cancelled;public EntityTargetLivingEntityEvent(org.bukkit.entity.Entity entity,org.bukkit.entity.LivingEntity target){super(entity);this.target=target;}public org.bukkit.entity.LivingEntity getTarget(){return target;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
