package com.destroystokyo.paper.event.player;
import org.bukkit.inventory.*;
public class PlayerArmorChangeEvent extends org.bukkit.event.player.PlayerEvent {
 public enum SlotType {HEAD,CHEST,LEGS,FEET}
 private final SlotType type;private final ItemStack oldItem,newItem;
 public PlayerArmorChangeEvent(org.bukkit.entity.Player player,SlotType type,ItemStack oldItem,ItemStack newItem){super(player);this.type=type;this.oldItem=oldItem;this.newItem=newItem;}
 public SlotType getSlotType(){return type;}public EquipmentSlot getSlot(){return EquipmentSlot.valueOf(type.name());}
 public ItemStack getOldItem(){return oldItem;}public ItemStack getNewItem(){return newItem;}
}
