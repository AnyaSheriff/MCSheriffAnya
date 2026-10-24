package ru.mcsa.bukkit;
import org.bukkit.inventory.*;import org.bukkit.entity.*;import org.bukkit.event.inventory.InventoryType;
// Нативное окно читается по снимку подключения; запись без события транзакции запрещена.
public final class NativeInventoryView implements InventoryView {
 private final Player player;private final Inventory top=new Top();public NativeInventoryView(Player p){player=p;}
 public HumanEntity getPlayer(){return player;}public Inventory getBottomInventory(){return player.getInventory();}
 public Inventory getTopInventory(){return top;}
 public int countSlots(){Inventory top=getTopInventory();return top.getSize()+(top.getType()==InventoryType.CRAFTING?41:36);}
 public int convertSlot(int raw){if(getType()!=InventoryType.CRAFTING){int n=raw-getTopInventory().getSize();return n<0?raw:n<27?n+9:n-27;}return raw<5?raw:raw<9?44-raw:raw<36?raw:raw<45?raw-36:40;}
 private final class Top implements Inventory {
  private String[] snapshot(){return Host.call(61,player.getUniqueId().toString());}
  public InventoryType getType(){String[] a=snapshot();return InventoryType.valueOf(a[0]);}
  public int getSize(){return (snapshot().length-1)/3;}public InventoryHolder getHolder(){return null;}
  public ItemStack[] getContents(){String[] a=snapshot();ItemStack[] items=new ItemStack[(a.length-1)/3];for(int i=0;i<items.length;i++){int n=1+i*3;org.bukkit.Material m=org.bukkit.Material.matchMaterial(a[n]);int count=Integer.parseInt(a[n+1]);items[i]=m==null||m.isAir()||count==0?null:ItemStack.fromHost(m,count,a[n+2]);}return items;}
  public ItemStack getItem(int slot){return getContents()[slot];}
  public boolean contains(org.bukkit.Material material){for(ItemStack item:getContents())if(item!=null&&item.getType()==material)return true;return false;}
  public boolean containsAtLeast(ItemStack item,int count){for(ItemStack stack:getContents())if(stack!=null&&stack.isSimilar(item))count-=stack.getAmount();return count<=0;}
  public void setItem(int slot,ItemStack stack){throw new UnsupportedOperationException("Запись в нативное окно через InventoryView ещё не поддерживается; используйте инвентарь игрока или пользовательский сундук");}
  public java.util.HashMap<Integer,ItemStack> addItem(ItemStack... items){throw new UnsupportedOperationException("Запись в нативное окно ещё не поддерживается");}
  public java.util.HashMap<Integer,ItemStack> removeItem(ItemStack... items){throw new UnsupportedOperationException("Запись в нативное окно ещё не поддерживается");}
 }
}
