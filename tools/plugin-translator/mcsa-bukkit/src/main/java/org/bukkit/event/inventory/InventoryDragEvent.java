package org.bukkit.event.inventory;
public class InventoryDragEvent extends InventoryEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.inventory.ItemStack oldCursor;private final java.util.Set<Integer> rawSlots;
 public InventoryDragEvent(org.bukkit.inventory.InventoryView view){this(view,view.getCursor(),java.util.Set.of());}
 public InventoryDragEvent(org.bukkit.inventory.InventoryView view,org.bukkit.inventory.ItemStack cursor,java.util.Set<Integer> rawSlots){super(view);oldCursor=cursor==null?new org.bukkit.inventory.ItemStack(org.bukkit.Material.AIR):cursor.clone();this.rawSlots=java.util.Collections.unmodifiableSet(new java.util.LinkedHashSet<>(rawSlots));}
 public org.bukkit.inventory.ItemStack getOldCursor(){return oldCursor.clone();}public java.util.Set<Integer> getRawSlots(){return rawSlots;}
 public org.bukkit.entity.HumanEntity getWhoClicked(){return getView().getPlayer();}
 private boolean cancelled;public boolean isCancelled(){return cancelled;}public void setCancelled(boolean b){cancelled=b;}
}
