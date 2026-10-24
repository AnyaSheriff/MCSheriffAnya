package org.bukkit.event.block;
public class BlockPlaceEvent extends BlockEvent implements org.bukkit.event.Cancellable {
 private boolean cancelled;private boolean build;private final org.bukkit.entity.Player player;private final org.bukkit.block.BlockState replaced;private final org.bukkit.block.Block against;private final org.bukkit.inventory.ItemStack item;private final org.bukkit.inventory.EquipmentSlot hand;
 public BlockPlaceEvent(org.bukkit.block.Block b,org.bukkit.entity.Player p){this(b,null,b,null,p,true,org.bukkit.inventory.EquipmentSlot.HAND);}
 public BlockPlaceEvent(org.bukkit.block.Block b,org.bukkit.block.BlockState replaced,org.bukkit.block.Block against,org.bukkit.inventory.ItemStack item,org.bukkit.entity.Player player,boolean build,org.bukkit.inventory.EquipmentSlot hand){super(b);this.replaced=replaced;this.against=against;this.item=item==null?null:item.clone();this.player=player;this.build=build;this.hand=hand;}
 public org.bukkit.block.Block getBlockPlaced(){return getBlock();}public org.bukkit.entity.Player getPlayer(){return player;}public boolean canBuild(){return build;}public void setBuild(boolean value){build=value;}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean b){cancelled=b;}
}
