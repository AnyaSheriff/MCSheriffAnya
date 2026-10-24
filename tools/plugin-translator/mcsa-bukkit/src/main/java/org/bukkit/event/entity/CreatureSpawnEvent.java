package org.bukkit.event.entity;
public class CreatureSpawnEvent extends EntityEvent implements org.bukkit.event.Cancellable {
 private boolean cancelled;public CreatureSpawnEvent(org.bukkit.entity.LivingEntity entity){super(entity);}public org.bukkit.entity.LivingEntity getEntity(){return (org.bukkit.entity.LivingEntity)entity;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
