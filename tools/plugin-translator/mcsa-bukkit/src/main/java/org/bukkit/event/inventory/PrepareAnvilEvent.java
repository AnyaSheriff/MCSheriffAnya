package org.bukkit.event.inventory;
public class PrepareAnvilEvent extends InventoryEvent {
 public PrepareAnvilEvent(org.bukkit.inventory.view.AnvilView view){super(view);}public org.bukkit.inventory.view.AnvilView getView(){return (org.bukkit.inventory.view.AnvilView)super.getView();}public org.bukkit.inventory.AnvilInventory getInventory(){return (org.bukkit.inventory.AnvilInventory)getView().getTopInventory();}
}
