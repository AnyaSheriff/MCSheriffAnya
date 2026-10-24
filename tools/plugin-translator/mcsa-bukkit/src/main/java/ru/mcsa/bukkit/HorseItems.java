package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.inventory.*;
public final class HorseItems implements AbstractHorseInventory {
 private final org.bukkit.entity.AbstractHorse horse;private final String uuid;public HorseItems(org.bukkit.entity.AbstractHorse horse){this.horse=horse;uuid=horse.getUniqueId().toString();}
 public InventoryHolder getHolder(){return horse;}public int getSize(){return (Host.call(104,uuid).length-2)/3;}
 public ItemStack getSaddle(){return getItem(0);}public void setSaddle(ItemStack item){setItem(0,item);}
 public ItemStack[] getContents(){String[] a=Host.call(104,uuid);ItemStack[] result=new ItemStack[(a.length-2)/3];for(int i=0;i<result.length;i++){int at=2+i*3;Material type=Material.matchMaterial(a[at]);int count=Integer.parseInt(a[at+1]);result[i]=type==null||type.isAir()||count==0?null:ItemStack.fromHost(type,count,a[at+2]);}return result;}
 public ItemStack getItem(int slot){ItemStack[] contents=getContents();if(slot<0||slot>=contents.length)throw new IllegalArgumentException("Ячейка вне лошади");return contents[slot];}
 public void setItem(int slot,ItemStack item){Host.call(105,uuid,""+slot,item==null||item.isEmpty()?"air":Ascii.lower(item.getType().name()),""+(item==null||item.isEmpty()?0:item.getAmount()),item==null?"":item.metadata());}
 public boolean contains(Material material){for(ItemStack item:getContents())if(item!=null&&item.getType()==material)return true;return false;}
 public boolean containsAtLeast(ItemStack stack,int count){for(ItemStack item:getContents())if(item!=null&&item.isSimilar(stack))count-=item.getAmount();return count<=0;}
 public java.util.HashMap<Integer,ItemStack> addItem(ItemStack... stacks){return transfer(true,stacks);}public java.util.HashMap<Integer,ItemStack> removeItem(ItemStack... stacks){return transfer(false,stacks);}
 private java.util.HashMap<Integer,ItemStack> transfer(boolean add,ItemStack[] stacks){java.util.HashMap<Integer,ItemStack> rest=new java.util.HashMap<>();for(int n=0;n<stacks.length;n++){ItemStack item=java.util.Objects.requireNonNull(stacks[n]);int left=item.getAmount();for(int pass=0;pass<(add?2:1)&&left>0;pass++)for(int i=0;i<getSize()&&left>0;i++){ItemStack current=getItem(i);if(current!=null&&current.isSimilar(item)&&(!add||pass==0)){int count=Math.min(left,add?current.getMaxStackSize()-current.getAmount():current.getAmount());if(count>0){ItemStack next=current.clone();next.setAmount(current.getAmount()+(add?count:-count));setItem(i,next.isEmpty()?null:next);left-=count;}}else if(add&&pass==1&&current==null){int count=Math.min(left,item.getMaxStackSize());ItemStack next=item.clone();next.setAmount(count);setItem(i,next);left-=count;}}if(left>0){ItemStack remain=item.clone();remain.setAmount(left);rest.put(n,remain);}}return rest;}
}
