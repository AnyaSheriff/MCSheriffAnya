package fixtures;
import org.bukkit.command.*;
import org.bukkit.event.*;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.plugin.java.JavaPlugin;
public final class Observer extends JavaPlugin implements Listener {
 private String mode="epoch";private BlockBreakEvent outer,inner;private boolean nesting;
 @Override public void onEnable(){getServer().getPluginManager().registerEvents(this,this);}
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){if(args.length!=1)return false;mode=args[0];sender.sendMessage("observer mode "+mode);return true;}
 @EventHandler(priority=EventPriority.LOWEST) public void lowest(BlockBreakEvent event){
  if(nesting){inner=event;event.setCancelled(true);return;}
  outer=event;
  if(mode.equals("epoch")){if(event.getPlayer().getInventory().getItem(0)!=null)throw new IllegalStateException("expected empty before mutation");}
  if(mode.equals("monitor")||mode.equals("trap")||mode.equals("fuel")||mode.equals("empty"))event.setCancelled(true);
  if(mode.equals("nested")){nesting=true;try{getServer().getPluginManager().callEvent(new BlockBreakEvent(event.getBlock(),event.getPlayer()));}finally{nesting=false;}if(event!=outer||event.isCancelled()||inner==outer||inner==null||!inner.isCancelled())throw new IllegalStateException("nested identity/cancellation leaked");}
 }
 @EventHandler(priority=EventPriority.NORMAL) public void normal(BlockBreakEvent event){
  if(nesting)return;if(event!=outer)throw new IllegalStateException("outer identity lost");
  if(mode.equals("epoch")){org.bukkit.inventory.ItemStack item=event.getPlayer().getInventory().getItem(0);if(item==null||item.getType()!=org.bukkit.Material.STONE||item.getAmount()!=5)throw new IllegalStateException("stale inventory between phases");}
 }
 @EventHandler(priority=EventPriority.MONITOR) public void monitor(BlockBreakEvent event){
  if(nesting){if(event!=inner||!event.isCancelled())throw new IllegalStateException("nested event lost");return;}
  if(event!=outer)throw new IllegalStateException("outer monitor identity lost");
  if(mode.equals("monitor"))event.setCancelled(false);
  event.getPlayer().sendMessage("safety "+mode+" cancelled="+event.isCancelled());
 }
}
