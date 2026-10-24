package org.bukkit.event.inventory;
public class PrepareItemCraftEvent extends InventoryEvent {
 private final org.bukkit.inventory.CraftingInventory inventory;private final org.bukkit.inventory.InventoryView view;private final org.bukkit.inventory.Recipe recipe;private final boolean repair;
 public PrepareItemCraftEvent(org.bukkit.inventory.CraftingInventory inventory,org.bukkit.inventory.InventoryView view,org.bukkit.inventory.Recipe recipe,boolean repair){super(view);this.inventory=inventory;this.view=view;this.recipe=recipe;this.repair=repair;}
 public org.bukkit.inventory.CraftingInventory getInventory(){return inventory;}public org.bukkit.inventory.InventoryView getView(){return view;}public org.bukkit.inventory.Recipe getRecipe(){return recipe;}public boolean isRepair(){return repair;}
}
