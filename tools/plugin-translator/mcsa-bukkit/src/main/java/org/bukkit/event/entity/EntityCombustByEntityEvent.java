package org.bukkit.event.entity;
public class EntityCombustByEntityEvent extends EntityEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.Entity combuster;private final float duration;private boolean cancelled;
 public EntityCombustByEntityEvent(org.bukkit.entity.Entity combuster,org.bukkit.entity.Entity entity,float duration){super(entity);this.combuster=java.util.Objects.requireNonNull(combuster);this.duration=duration;}public org.bukkit.entity.Entity getCombuster(){return combuster;}public float getDuration(){return duration;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
