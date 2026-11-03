package org.bukkit.inventory;
public interface InventoryView {
 org.bukkit.entity.HumanEntity getPlayer();Inventory getTopInventory();Inventory getBottomInventory();
 default org.bukkit.event.inventory.InventoryType getType(){return getTopInventory().getType();}
 default Inventory getInventory(int slot){return slot<0||slot>=countSlots()?null:slot<getTopInventory().getSize()?getTopInventory():getBottomInventory();}
 default int countSlots(){return getTopInventory().getSize()+36;}
 default int convertSlot(int raw){if(raw<getTopInventory().getSize())return raw;int n=raw-getTopInventory().getSize();return n<27?n+9:n-27;}
 default ItemStack getItem(int raw){Inventory inv=getInventory(raw);return inv==null?null:inv.getItem(convertSlot(raw));}
 default ItemStack getCursor(){String[] a=ru.mcsa.bukkit.Host.call(57,getPlayer().getUniqueId().toString());return a.length==0?null:ItemStack.fromHost(org.bukkit.Material.matchMaterial(a[0]),Integer.parseInt(a[1]),a[2]);}
 default void close(){getPlayer().closeInventory();}
}
