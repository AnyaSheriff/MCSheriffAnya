package org.bukkit.event.inventory;
import org.bukkit.inventory.*;
public class InventoryClickEvent extends InventoryEvent implements org.bukkit.event.Cancellable {
 private final int raw,key;private final ClickType click;private final InventoryAction action;private final InventoryType.SlotType type;private boolean cancelled;
 public InventoryClickEvent(InventoryView view,InventoryType.SlotType type,int raw,ClickType click,InventoryAction action){this(view,type,raw,click,action,-1);}
 public InventoryClickEvent(InventoryView view,InventoryType.SlotType type,int raw,ClickType click,InventoryAction action,int key){super(view);this.raw=raw;this.type=type;this.click=click;this.action=action;this.key=key;}
 public org.bukkit.entity.HumanEntity getWhoClicked(){return getView().getPlayer();}public int getRawSlot(){return raw;}public int getSlot(){return getView().convertSlot(raw);}public int getHotbarButton(){return key;}
 public Inventory getClickedInventory(){return getView().getInventory(raw);}public ItemStack getCurrentItem(){return getView().getItem(raw);}public ClickType getClick(){return click;}public InventoryAction getAction(){return action;}public InventoryType.SlotType getSlotType(){return type;}
 public ItemStack getCursor(){return getView().getCursor();}
 public boolean isLeftClick(){return click.isLeftClick();}public boolean isRightClick(){return click.isRightClick();}public boolean isShiftClick(){return click.isShiftClick();}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean b){cancelled=b;}
}
