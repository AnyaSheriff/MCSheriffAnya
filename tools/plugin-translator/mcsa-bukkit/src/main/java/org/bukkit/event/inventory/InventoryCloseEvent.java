package org.bukkit.event.inventory;
public class InventoryCloseEvent extends InventoryEvent  {
 public InventoryCloseEvent(org.bukkit.inventory.InventoryView view){super(view);}
 public org.bukkit.entity.HumanEntity getPlayer(){return getView().getPlayer();}
 
}
