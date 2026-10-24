package org.bukkit.event.inventory;
public class FurnaceStartSmeltEvent extends org.bukkit.event.Event {
 private final org.bukkit.block.Block block;private int total;
 public FurnaceStartSmeltEvent(org.bukkit.block.Block block,int total){this.block=java.util.Objects.requireNonNull(block);setTotalCookTime(total);}
 public org.bukkit.block.Block getBlock(){return block;}public int getTotalCookTime(){return total;}public void setTotalCookTime(int value){if(value<1||value>72000)throw new IllegalArgumentException("Время плавки: от 1 до 72000 тактов");total=value;}
}
