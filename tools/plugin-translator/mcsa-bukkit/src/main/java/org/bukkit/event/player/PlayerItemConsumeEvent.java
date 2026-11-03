package org.bukkit.event.player;
public final class PlayerItemConsumeEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.inventory.ItemStack item;private final org.bukkit.inventory.EquipmentSlot hand;private boolean cancelled;
 public PlayerItemConsumeEvent(org.bukkit.entity.Player player,org.bukkit.inventory.ItemStack item,org.bukkit.inventory.EquipmentSlot hand){super(player);this.item=java.util.Objects.requireNonNull(item).clone();this.hand=hand;}
 public org.bukkit.inventory.ItemStack getItem(){return item.clone();}public org.bukkit.inventory.EquipmentSlot getHand(){return hand;}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
