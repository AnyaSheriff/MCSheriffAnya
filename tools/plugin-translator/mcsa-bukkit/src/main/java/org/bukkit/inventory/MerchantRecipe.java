package org.bukkit.inventory;
public class MerchantRecipe implements Recipe {
 private final ItemStack result;private final java.util.List<ItemStack> ingredients=new java.util.ArrayList<>();private int special;
 public MerchantRecipe(ItemStack result,int maxUses){this.result=java.util.Objects.requireNonNull(result).clone();}
 public MerchantRecipe(MerchantRecipe source){result=source.result.clone();special=source.special;for(ItemStack item:source.ingredients)ingredients.add(item.clone());}
 public ItemStack getResult(){return result.clone();}public java.util.List<ItemStack> getIngredients(){java.util.List<ItemStack> copy=new java.util.ArrayList<>();for(ItemStack item:ingredients)copy.add(item.clone());return copy;}
 public void addIngredient(ItemStack item){if(ingredients.size()==2)throw new IllegalArgumentException("В сделке не больше двух ингредиентов");ingredients.add(java.util.Objects.requireNonNull(item).clone());}
 public int getSpecialPrice(){return special;}public void setSpecialPrice(int value){special=value;}
}
