package org.bukkit.event.inventory;
public class InventoryEvent extends org.bukkit.event.Event {
 private final org.bukkit.inventory.InventoryView view;
 public InventoryEvent(org.bukkit.inventory.InventoryView view){this.view=view;}
 public org.bukkit.inventory.InventoryView getView(){return view;}public org.bukkit.inventory.Inventory getInventory(){return view.getTopInventory();}
}
