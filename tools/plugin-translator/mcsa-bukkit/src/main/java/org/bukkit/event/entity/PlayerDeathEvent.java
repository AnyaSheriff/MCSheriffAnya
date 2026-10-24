package org.bukkit.event.entity;
public final class PlayerDeathEvent extends EntityDeathEvent {
 private final boolean keepInventory;
 public PlayerDeathEvent(org.bukkit.entity.Player player,boolean keepInventory){this(player,keepInventory,new java.util.ArrayList<>());}
 public PlayerDeathEvent(org.bukkit.entity.Player player,boolean keepInventory,java.util.List<org.bukkit.inventory.ItemStack> drops){super(player,drops);this.keepInventory=keepInventory;}
 public org.bukkit.entity.Player getEntity(){return (org.bukkit.entity.Player)entity;}public boolean getKeepInventory(){return keepInventory;}
}
