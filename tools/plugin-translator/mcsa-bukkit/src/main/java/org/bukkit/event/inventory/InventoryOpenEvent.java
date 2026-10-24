package org.bukkit.event.inventory;
public class InventoryOpenEvent extends InventoryEvent implements org.bukkit.event.Cancellable {
 public InventoryOpenEvent(org.bukkit.inventory.InventoryView view){super(view);}
 public org.bukkit.entity.HumanEntity getPlayer(){return getView().getPlayer();}
 private boolean cancelled;public boolean isCancelled(){return cancelled;}public void setCancelled(boolean b){cancelled=b;}
}
