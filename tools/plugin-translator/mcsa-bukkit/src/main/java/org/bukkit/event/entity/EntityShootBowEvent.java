package org.bukkit.event.entity;
public class EntityShootBowEvent extends org.bukkit.event.Event implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.LivingEntity entity;private final org.bukkit.entity.Entity projectile;private final float force;private boolean cancelled;
 public EntityShootBowEvent(org.bukkit.entity.LivingEntity entity,org.bukkit.inventory.ItemStack bow,org.bukkit.entity.Entity projectile,float force){this.entity=java.util.Objects.requireNonNull(entity);this.projectile=java.util.Objects.requireNonNull(projectile);this.force=force;}public org.bukkit.entity.LivingEntity getEntity(){return entity;}public org.bukkit.entity.Entity getProjectile(){return projectile;}public float getForce(){return force;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
