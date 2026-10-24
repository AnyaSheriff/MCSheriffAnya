package org.bukkit.event.player;
public class PlayerArmorStandManipulateEvent extends PlayerInteractEntityEvent {
 private final org.bukkit.inventory.ItemStack playerItem,standItem;private final org.bukkit.inventory.EquipmentSlot slot;
 public PlayerArmorStandManipulateEvent(org.bukkit.entity.Player player,org.bukkit.entity.ArmorStand stand,org.bukkit.inventory.ItemStack playerItem,org.bukkit.inventory.ItemStack standItem,org.bukkit.inventory.EquipmentSlot slot,org.bukkit.inventory.EquipmentSlot hand){super(player,stand,hand);this.playerItem=playerItem.clone();this.standItem=standItem.clone();this.slot=slot;}
 public org.bukkit.entity.ArmorStand getRightClicked(){return (org.bukkit.entity.ArmorStand)super.getRightClicked();}public org.bukkit.inventory.ItemStack getPlayerItem(){return playerItem.clone();}public org.bukkit.inventory.ItemStack getArmorStandItem(){return standItem.clone();}public org.bukkit.inventory.EquipmentSlot getSlot(){return slot;}
}
