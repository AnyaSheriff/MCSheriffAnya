package org.bukkit.event.player;
import org.bukkit.event.Event.Result;import org.bukkit.event.block.Action;import org.bukkit.inventory.*;import org.bukkit.block.*;
public class PlayerInteractEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 private final Action action;private final ItemStack item;private final Block block;private final BlockFace face;private final EquipmentSlot hand;
 private Result useBlock=Result.DEFAULT,useItem=Result.DEFAULT;
 public PlayerInteractEvent(org.bukkit.entity.Player player,Action action,ItemStack item,Block block,BlockFace face){this(player,action,item,block,face,EquipmentSlot.HAND);}
 public PlayerInteractEvent(org.bukkit.entity.Player player,Action action,ItemStack item,Block block,BlockFace face,EquipmentSlot hand){super(player);this.action=action;this.item=item;this.block=block;this.face=face;this.hand=hand;}
 public Action getAction(){return action;}public ItemStack getItem(){return item;}public Block getClickedBlock(){return block;}public BlockFace getBlockFace(){return face;}public EquipmentSlot getHand(){return hand;}
 public boolean hasBlock(){return block!=null;}public boolean hasItem(){return item!=null&&!item.getType().isAir();}
 public Result useInteractedBlock(){return useBlock;}public Result useItemInHand(){return useItem;}
 public void setUseInteractedBlock(Result value){useBlock=java.util.Objects.requireNonNull(value);}
 public void setUseItemInHand(Result value){useItem=java.util.Objects.requireNonNull(value);}
 public boolean isCancelled(){return useBlock==Result.DENY;}public void setCancelled(boolean value){useBlock=value?Result.DENY:Result.DEFAULT;useItem=useBlock;}
}
