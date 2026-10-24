package org.bukkit.event.world;
public class GenericGameEvent extends org.bukkit.event.Event implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.Entity entity;private boolean cancelled;public GenericGameEvent(org.bukkit.entity.Entity entity){this.entity=entity;}public org.bukkit.entity.Entity getEntity(){return entity;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
