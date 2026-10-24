package ru.mcsa.bukkit;
import org.bukkit.inventory.*;import org.bukkit.Material;
public final class ChestInventory implements Inventory {
 private final InventoryHolder holder;private final ItemStack[] items;final net.kyori.adventure.text.Component title;int handle;
 public ChestInventory(InventoryHolder holder,int size,net.kyori.adventure.text.Component title){if(size<9||size>54||size%9!=0)throw new IllegalArgumentException("Сундук: 9..54 ячейки, кратно 9");this.holder=holder;items=new ItemStack[size];this.title=java.util.Objects.requireNonNull(title);}
 public InventoryHolder getHolder(){return holder;}public int getSize(){return items.length;}
 private void check(int slot){if(slot<0||slot>=items.length)throw new IllegalArgumentException("Ячейка вне сундука");}
 public ItemStack getItem(int slot){check(slot);if(handle!=0)refresh();return items[slot];}
 public ItemStack[] getContents(){if(handle!=0)refresh();return items.clone();}
 void refresh(){String[] a=Host.call(52,""+handle);for(int i=0;i<items.length;i++){int n=i*3;Material m=Material.matchMaterial(a[n]);int count=Integer.parseInt(a[n+1]);items[i]=m==null||m.isAir()||count==0?null:ItemStack.fromHost(m,count,a[n+2]);}}
 public void setItem(int slot,ItemStack item){check(slot);items[slot]=item==null?null:item.clone();if(handle!=0)send(slot);}
 private void send(int slot){ItemStack item=items[slot];Host.call(51,""+handle,""+slot,item==null||item.isEmpty()?"air":Ascii.lower(item.getType().name()),""+(item==null||item.isEmpty()?0:item.getAmount()),item==null?"":item.metadata());}
 int prepare(){if(handle==0){handle=Integer.parseInt(Host.text(50,""+items.length,Text.json(title)));for(int i=0;i<items.length;i++)send(i);}return handle;}
 public boolean contains(Material material){for(ItemStack item:getContents())if(item!=null&&item.getType()==material)return true;return false;}
 public boolean containsAtLeast(ItemStack stack,int count){for(ItemStack item:getContents())if(item!=null&&item.isSimilar(stack))count-=item.getAmount();return count<=0;}
 public java.util.HashMap<Integer,ItemStack> addItem(ItemStack... stacks){return transfer(true,stacks);}public java.util.HashMap<Integer,ItemStack> removeItem(ItemStack... stacks){return transfer(false,stacks);}
 private java.util.HashMap<Integer,ItemStack> transfer(boolean add,ItemStack[] stacks){java.util.HashMap<Integer,ItemStack> rest=new java.util.HashMap<>();for(int n=0;n<stacks.length;n++){ItemStack item=java.util.Objects.requireNonNull(stacks[n]);int left=item.getAmount();for(int pass=0;pass<(add?2:1)&&left>0;pass++)for(int i=0;i<items.length&&left>0;i++){ItemStack current=getItem(i);if(current!=null&&current.isSimilar(item)&&(!add||pass==0)){int count=Math.min(left,add?current.getMaxStackSize()-current.getAmount():current.getAmount());if(count>0){ItemStack next=current.clone();next.setAmount(current.getAmount()+(add?count:-count));setItem(i,next.isEmpty()?null:next);left-=count;}}else if(add&&pass==1&&current==null){int count=Math.min(left,item.getMaxStackSize());ItemStack next=item.clone();next.setAmount(count);setItem(i,next);left-=count;}}if(left>0){ItemStack remain=item.clone();remain.setAmount(left);rest.put(n,remain);}}return rest;}
}
