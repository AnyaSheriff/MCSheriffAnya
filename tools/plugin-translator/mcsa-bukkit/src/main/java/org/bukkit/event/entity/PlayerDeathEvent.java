package org.bukkit.event.entity;
public final class PlayerDeathEvent extends EntityEvent {
 private final boolean keepInventory;
 public PlayerDeathEvent(org.bukkit.entity.Player player,boolean keepInventory){super(player);this.keepInventory=keepInventory;}
 public org.bukkit.entity.Player getEntity(){return (org.bukkit.entity.Player)entity;}public boolean getKeepInventory(){return keepInventory;}
}
