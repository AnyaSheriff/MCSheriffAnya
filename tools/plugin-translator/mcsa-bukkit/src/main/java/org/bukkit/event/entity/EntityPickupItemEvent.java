package org.bukkit.event.entity;
public class EntityPickupItemEvent extends org.bukkit.event.Event implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.LivingEntity entity;private final org.bukkit.entity.Item item;private final int remaining;private boolean cancelled;
 public EntityPickupItemEvent(org.bukkit.entity.LivingEntity entity,org.bukkit.entity.Item item,int remaining){this.entity=java.util.Objects.requireNonNull(entity);this.item=java.util.Objects.requireNonNull(item);this.remaining=remaining;}public org.bukkit.entity.LivingEntity getEntity(){return entity;}public org.bukkit.entity.Item getItem(){return item;}public int getRemaining(){return remaining;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
