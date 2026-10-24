package org.bukkit.inventory;
public interface PlayerInventory extends Inventory {ItemStack getItemInMainHand();ItemStack getItemInOffHand();void setItemInOffHand(ItemStack stack);ItemStack[] getArmorContents();void setArmorContents(ItemStack[] armor);ItemStack getHelmet();}
