package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.*;import org.bukkit.inventory.*;import org.bukkit.event.inventory.*;
// Копии сделок живут только во время события; хозяин проверяет и применяет цены.
public final class MerchantEvents implements MerchantInventory,Merchant {
 private final Player player;private final java.util.List<MerchantRecipe> recipes=new java.util.ArrayList<>();private final ItemStack[] slots=new ItemStack[3];
 private static ItemStack stack(String[] a,int n){Material type=Material.matchMaterial(a[n]);return type==null||type.isAir()||Integer.parseInt(a[n+1])==0?null:ItemStack.fromHost(type,Integer.parseInt(a[n+1]),a[n+2]);}
 private MerchantEvents(Player player,String[] a){this.player=player;for(int i=0;i<3;i++)slots[i]=stack(a,10+i*3);for(int n=19;n<a.length;n+=10){MerchantRecipe recipe=new MerchantRecipe(stack(a,n+6),1);recipe.addIngredient(stack(a,n));ItemStack second=stack(a,n+3);if(second!=null)recipe.addIngredient(second);recipe.setSpecialPrice(Integer.parseInt(a[n+9]));recipes.add(recipe);}}
 public static void event(Player player,String[] a){MerchantEvents top=new MerchantEvents(player,a);InventoryView view=new InventoryView(){public HumanEntity getPlayer(){return player;}public Inventory getTopInventory(){return top;}public Inventory getBottomInventory(){return player.getInventory();}};boolean cancelled=false;
  if(a[9].equals("1"))RuntimeState.fire(new InventoryCloseEvent(view));else {InventoryOpenEvent event=new InventoryOpenEvent(view);RuntimeState.fire(event);cancelled=event.isCancelled();}
  StringBuilder result=new StringBuilder(cancelled?"1":"0");for(MerchantRecipe recipe:top.recipes)result.append(' ').append(recipe.getSpecialPrice());Host.call(22,result.toString());
 }
 public Merchant getMerchant(){return this;}public int getRecipeCount(){return recipes.size();}public MerchantRecipe getRecipe(int index){return new MerchantRecipe(recipes.get(index));}
 public void setRecipe(int index,MerchantRecipe recipe){MerchantRecipe old=recipes.get(index);if(!old.getIngredients().equals(recipe.getIngredients())||!old.getResult().equals(recipe.getResult()))throw new UnsupportedOperationException("Во время торгового события пока меняется только особая цена сделки");recipes.set(index,new MerchantRecipe(recipe));}
 public InventoryType getType(){return InventoryType.MERCHANT;}public InventoryHolder getHolder(){return null;}public int getSize(){return 3;}
 public ItemStack getItem(int slot){if(slot<0||slot>=3)throw new IndexOutOfBoundsException();return slots[slot]==null?null:slots[slot].clone();}public ItemStack[] getContents(){return new ItemStack[]{getItem(0),getItem(1),getItem(2)};}
 public boolean contains(Material type){for(ItemStack stack:slots)if(stack!=null&&stack.getType()==type)return true;return false;}public boolean containsAtLeast(ItemStack value,int count){for(ItemStack stack:slots)if(stack!=null&&stack.isSimilar(value))count-=stack.getAmount();return count<=0;}
 public void setItem(int slot,ItemStack value){throw new UnsupportedOperationException("Изменение ячеек во время торгового события пока не поддерживается");}public java.util.HashMap<Integer,ItemStack> addItem(ItemStack... values){throw new UnsupportedOperationException("Изменение ячеек во время торгового события пока не поддерживается");}public java.util.HashMap<Integer,ItemStack> removeItem(ItemStack... values){throw new UnsupportedOperationException("Изменение ячеек во время торгового события пока не поддерживается");}
}
