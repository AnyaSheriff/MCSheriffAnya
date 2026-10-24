package org.bukkit.entity;
public interface HumanEntity extends LivingEntity, org.bukkit.inventory.InventoryHolder {
 org.bukkit.inventory.PlayerInventory getInventory();
 org.bukkit.inventory.InventoryView openInventory(org.bukkit.inventory.Inventory inventory);
 org.bukkit.inventory.InventoryView getOpenInventory();void closeInventory();
}
