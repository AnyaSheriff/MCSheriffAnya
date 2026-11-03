package org.bukkit.event.block;
public class BlockDropItemEvent extends BlockEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.block.BlockState state;private final org.bukkit.entity.Player player;private final java.util.List<org.bukkit.entity.Item> items;private boolean cancelled;
 public BlockDropItemEvent(org.bukkit.block.Block block,org.bukkit.block.BlockState state,org.bukkit.entity.Player player,java.util.List<org.bukkit.entity.Item> items){super(block);this.state=java.util.Objects.requireNonNull(state);this.player=java.util.Objects.requireNonNull(player);this.items=java.util.Objects.requireNonNull(items);}
 public org.bukkit.block.BlockState getBlockState(){return state;}public org.bukkit.entity.Player getPlayer(){return player;}public java.util.List<org.bukkit.entity.Item> getItems(){return items;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
