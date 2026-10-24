package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.Player;import org.bukkit.inventory.*;
public final class PreparedCraft implements CraftingInventory {
 private final Player player;private final ItemStack[] contents;private final boolean table;
 public PreparedCraft(Player player,String[] a){this.player=player;table=a[10].equals("1");contents=new ItemStack[table?10:5];for(int i=0;i<contents.length;i++){int n=11+i*3;Material material=Material.matchMaterial(a[n]);int count=Integer.parseInt(a[n+1]);contents[i]=material==null||material.isAir()||count==0?null:ItemStack.fromHost(material,count,a[n+2]);}}
 public org.bukkit.event.inventory.InventoryType getType(){return table?org.bukkit.event.inventory.InventoryType.WORKBENCH:org.bukkit.event.inventory.InventoryType.CRAFTING;}
 public InventoryHolder getHolder(){return player;}public int getSize(){return contents.length;}public ItemStack[] getContents(){ItemStack[] result=contents.clone();for(int i=0;i<result.length;i++)if(result[i]!=null)result[i]=result[i].clone();return result;}
 public ItemStack getItem(int slot){if(slot<0||slot>=contents.length)throw new IllegalArgumentException("Ячейка вне сетки");return contents[slot]==null?null:contents[slot].clone();}public ItemStack getResult(){return getItem(0);}public void setResult(ItemStack value){contents[0]=value==null?null:value.clone();}
 public void setItem(int slot,ItemStack value){if(slot!=0)throw new UnsupportedOperationException("В PrepareItemCraftEvent изменяется только результат; изменение сетки пока не поддерживается");setResult(value);}
 public boolean contains(Material material){for(ItemStack item:contents)if(item!=null&&item.getType()==material)return true;return false;}public boolean containsAtLeast(ItemStack value,int count){for(ItemStack item:contents)if(item!=null&&item.isSimilar(value))count-=item.getAmount();return count<=0;}
 public java.util.HashMap<Integer,ItemStack> addItem(ItemStack... items){throw new UnsupportedOperationException("Добавление ингредиентов во время подготовки пока не поддерживается");}public java.util.HashMap<Integer,ItemStack> removeItem(ItemStack... items){throw new UnsupportedOperationException("Удаление ингредиентов во время подготовки пока не поддерживается");}
 public InventoryView view(){PreparedCraft top=this;return new InventoryView(){public org.bukkit.entity.HumanEntity getPlayer(){return player;}public Inventory getTopInventory(){return top;}public Inventory getBottomInventory(){return player.getInventory();}};}
}
