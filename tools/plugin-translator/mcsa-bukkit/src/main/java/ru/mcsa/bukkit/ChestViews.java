package ru.mcsa.bukkit;
import org.bukkit.entity.*;import org.bukkit.inventory.*;import org.bukkit.event.inventory.*;
public final class ChestViews implements InventoryView {
 private static final java.util.Map<String,ChestViews> views=new java.util.HashMap<>();
 private final Player player;private final ChestInventory inventory;private final int handle;
 private ChestViews(Player p,ChestInventory i,int h){player=p;inventory=i;handle=h;}
 private static String key(Player p,int handle){return p.getUniqueId()+":"+handle;}
 public static InventoryView open(Player p,Inventory inv){if(!(inv instanceof ChestInventory))throw new IllegalArgumentException("Открываются только пользовательские сундуки");ChestInventory chest=(ChestInventory)inv;int h=chest.prepare();ChestViews view=new ChestViews(p,chest,h);InventoryOpenEvent event=new InventoryOpenEvent(view);RuntimeState.fire(event);if(event.isCancelled()){boolean used=false;for(ChestViews other:views.values())if(other.inventory==chest)used=true;if(!used){Host.call(56,""+h);chest.handle=0;}return null;}Host.call(53,p.getUniqueId().toString(),""+h);views.put(key(p,h),view);return view;}
 public static InventoryView current(Player p){String value=Host.text(55,p.getUniqueId().toString());InventoryView view=value.isEmpty()?null:views.get(key(p,Integer.parseInt(value)));return view==null?new NativeInventoryView(p):view;}
 public static void close(Player p){Host.call(54,p.getUniqueId().toString());}
 public HumanEntity getPlayer(){return player;}public Inventory getTopInventory(){return inventory;}public Inventory getBottomInventory(){return player.getInventory();}
 public static int event(Player p,int handle,int kind,int slot,int button,int mode){ChestViews view=views.get(key(p,handle));if(view==null)return 1;
  if(kind==1){view.inventory.refresh();RuntimeState.fire(new InventoryCloseEvent(view));views.remove(key(p,handle));boolean used=false;for(ChestViews other:views.values())if(other.inventory==view.inventory)used=true;if(!used){Host.call(56,""+handle);view.inventory.handle=0;}return 0;}
  if(mode==5){InventoryDragEvent event=new InventoryDragEvent(view);RuntimeState.fire(event);return event.isCancelled()?1:0;}
  ClickType click=mode==0?(slot==-999?(button==0?ClickType.WINDOW_BORDER_LEFT:ClickType.WINDOW_BORDER_RIGHT):(button==0?ClickType.LEFT:ClickType.RIGHT)):mode==1?(button==0?ClickType.SHIFT_LEFT:ClickType.SHIFT_RIGHT):mode==2?(button==40?ClickType.SWAP_OFFHAND:ClickType.NUMBER_KEY):mode==3?ClickType.MIDDLE:mode==4?(button==0?ClickType.DROP:ClickType.CONTROL_DROP):mode==6?ClickType.DOUBLE_CLICK:ClickType.UNKNOWN;
  ItemStack item=view.getItem(slot);InventoryAction action=mode==1?InventoryAction.MOVE_TO_OTHER_INVENTORY:mode==2?InventoryAction.HOTBAR_SWAP:mode==3?InventoryAction.CLONE_STACK:mode==4?(button==0?InventoryAction.DROP_ONE_SLOT:InventoryAction.DROP_ALL_SLOT):mode==6?InventoryAction.COLLECT_TO_CURSOR:InventoryAction.UNKNOWN;
  if(mode==0){ItemStack cursor=view.getCursor();if(slot==-999)action=cursor==null?InventoryAction.NOTHING:button==0?InventoryAction.DROP_ALL_CURSOR:InventoryAction.DROP_ONE_CURSOR;else if(cursor==null)action=item==null?InventoryAction.NOTHING:button==0?InventoryAction.PICKUP_ALL:InventoryAction.PICKUP_HALF;else if(item==null)action=button==0?InventoryAction.PLACE_ALL:InventoryAction.PLACE_ONE;else if(item.isSimilar(cursor)){int room=item.getMaxStackSize()-item.getAmount();action=room<=0?InventoryAction.NOTHING:button!=0?InventoryAction.PLACE_ONE:room>=cursor.getAmount()?InventoryAction.PLACE_ALL:InventoryAction.PLACE_SOME;}else action=InventoryAction.SWAP_WITH_CURSOR;}
  InventoryClickEvent event=new InventoryClickEvent(view,slot<0?InventoryType.SlotType.OUTSIDE:slot>=view.inventory.getSize()+27?InventoryType.SlotType.QUICKBAR:InventoryType.SlotType.CONTAINER,slot,click,action,mode==2&&button<9?button:-1);RuntimeState.fire(event);return event.isCancelled()?1:0;
 }
}
