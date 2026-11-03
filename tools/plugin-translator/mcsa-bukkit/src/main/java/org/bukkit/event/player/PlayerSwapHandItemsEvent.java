package org.bukkit.event.player;
public final class PlayerSwapHandItemsEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.inventory.ItemStack main,off;private boolean cancelled;
 public PlayerSwapHandItemsEvent(org.bukkit.entity.Player player,org.bukkit.inventory.ItemStack main,org.bukkit.inventory.ItemStack off){super(player);this.main=copy(main);this.off=copy(off);}
 private static org.bukkit.inventory.ItemStack copy(org.bukkit.inventory.ItemStack item){return item==null?new org.bukkit.inventory.ItemStack(org.bukkit.Material.AIR):item.clone();}
 public org.bukkit.inventory.ItemStack getMainHandItem(){return main.clone();}public org.bukkit.inventory.ItemStack getOffHandItem(){return off.clone();}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
