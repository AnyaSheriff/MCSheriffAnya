package org.bukkit.event.entity;
public class AreaEffectCloudApplyEvent extends org.bukkit.event.Event implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.AreaEffectCloud cloud;private final java.util.List<org.bukkit.entity.LivingEntity> entities;private boolean cancelled;
 public AreaEffectCloudApplyEvent(org.bukkit.entity.AreaEffectCloud cloud,java.util.List<org.bukkit.entity.LivingEntity> entities){this.cloud=java.util.Objects.requireNonNull(cloud);this.entities=new java.util.ArrayList<>(entities);}public org.bukkit.entity.AreaEffectCloud getEntity(){return cloud;}public java.util.List<org.bukkit.entity.LivingEntity> getAffectedEntities(){return entities;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
