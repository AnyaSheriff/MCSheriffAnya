package org.bukkit.event.inventory;
public class InventoryDragEvent extends InventoryEvent implements org.bukkit.event.Cancellable {
 public InventoryDragEvent(org.bukkit.inventory.InventoryView view){super(view);}
 public org.bukkit.entity.HumanEntity getWhoClicked(){return getView().getPlayer();}
 private boolean cancelled;public boolean isCancelled(){return cancelled;}public void setCancelled(boolean b){cancelled=b;}
}
