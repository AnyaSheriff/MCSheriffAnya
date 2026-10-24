package org.bukkit.event.entity;
public class EntityExplodeEvent extends EntityEvent implements org.bukkit.event.Cancellable {
 private final java.util.List<org.bukkit.block.Block> blocks;private boolean cancelled;
 public EntityExplodeEvent(org.bukkit.entity.Entity entity,java.util.List<org.bukkit.block.Block> blocks){super(entity);this.blocks=new java.util.ArrayList<>(blocks);}public java.util.List<org.bukkit.block.Block> blockList(){return blocks;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
