package org.bukkit.event.entity;
public class FoodLevelChangeEvent extends org.bukkit.event.Event implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.HumanEntity entity;private final org.bukkit.inventory.ItemStack item;private int level;private boolean cancelled;
 public FoodLevelChangeEvent(org.bukkit.entity.HumanEntity entity,int level,org.bukkit.inventory.ItemStack item){this.entity=entity;setFoodLevel(level);this.item=item==null?null:item.clone();}public FoodLevelChangeEvent(org.bukkit.entity.HumanEntity entity,int level){this(entity,level,null);}
 public org.bukkit.entity.HumanEntity getEntity(){return entity;}public org.bukkit.inventory.ItemStack getItem(){return item==null?null:item.clone();}public int getFoodLevel(){return level;}public void setFoodLevel(int value){if(value<0||value>20)throw new IllegalArgumentException("Уровень еды: 0..20");level=value;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
