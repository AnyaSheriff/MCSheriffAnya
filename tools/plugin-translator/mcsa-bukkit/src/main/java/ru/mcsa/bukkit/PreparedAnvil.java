package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.Player;import org.bukkit.inventory.*;
public final class PreparedAnvil implements org.bukkit.inventory.view.AnvilView {
 private final Player player;private final ItemStack[] items=new ItemStack[3];private int cost;
 public PreparedAnvil(Player player,String[] a){this.player=player;cost=Integer.parseInt(a[9]);for(int i=0;i<3;i++){int n=10+i*3;Material material=Material.matchMaterial(a[n]);int count=Integer.parseInt(a[n+1]);items[i]=material==null||material.isAir()||count==0?null:ItemStack.fromHost(material,count,a[n+2]);}}
 public org.bukkit.entity.HumanEntity getPlayer(){return player;}public Inventory getBottomInventory(){return player.getInventory();}public int getRepairCost(){return cost;}public void setRepairCost(int cost){if(cost<0||cost>32767)throw new IllegalArgumentException("Стоимость наковальни: 0..32767");this.cost=cost;}
 public Inventory getTopInventory(){return new Top();}
 private final class Top implements AnvilInventory {
  public org.bukkit.event.inventory.InventoryType getType(){return org.bukkit.event.inventory.InventoryType.ANVIL;}public InventoryHolder getHolder(){return null;}public int getSize(){return 3;}public ItemStack getFirstItem(){return getItem(0);}public ItemStack getItem(int slot){if(slot<0||slot>=3)throw new IllegalArgumentException("Ячейка вне наковальни");return items[slot]==null?null:items[slot].clone();}public ItemStack[] getContents(){return new ItemStack[]{getItem(0),getItem(1),getItem(2)};}
  public boolean contains(Material type){for(ItemStack item:items)if(item!=null&&item.getType()==type)return true;return false;}public boolean containsAtLeast(ItemStack value,int count){for(ItemStack item:items)if(item!=null&&item.isSimilar(value))count-=item.getAmount();return count<=0;}
  public void setItem(int slot,ItemStack value){throw new UnsupportedOperationException("Запись предметов наковальни во время подготовки пока не поддерживается");}public java.util.HashMap<Integer,ItemStack> addItem(ItemStack... items){throw new UnsupportedOperationException("Запись предметов наковальни во время подготовки пока не поддерживается");}public java.util.HashMap<Integer,ItemStack> removeItem(ItemStack... items){throw new UnsupportedOperationException("Запись предметов наковальни во время подготовки пока не поддерживается");}
 }
}
