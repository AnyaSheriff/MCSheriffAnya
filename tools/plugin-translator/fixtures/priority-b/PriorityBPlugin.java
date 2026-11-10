package fixtures;
import org.bukkit.command.*;
import org.bukkit.event.*;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.plugin.java.JavaPlugin;
public final class PriorityBPlugin extends JavaPlugin implements Listener {
 private String mode="order";private BlockBreakEvent seen;
 private static String bit(boolean value){return value?"1":"0";}
 private void trace(BlockBreakEvent event,String value){event.getPlayer().sendMessage("trace "+value);}
 @Override public void onEnable(){getServer().getPluginManager().registerEvents(this,this);}
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(args.length!=1||!(args[0].equals("order")||args[0].equals("cancel")||args[0].equals("uncancel")))return false;
  mode=args[0];sender.sendMessage("mode B "+mode);return true;
 }
 @EventHandler(priority=EventPriority.LOW)
 public void low(BlockBreakEvent event){
  seen=event;boolean before=event.isCancelled();if(mode.equals("uncancel"))event.setCancelled(false);
  trace(event,"B1 c"+bit(before)+">"+bit(event.isCancelled()));
 }
 @EventHandler(priority=EventPriority.HIGH,ignoreCancelled=true)
 public void high(BlockBreakEvent event){
  if(event!=seen)throw new IllegalStateException("Событие B сменилось между приоритетами");
  trace(event,"B3 c"+bit(event.isCancelled()));
 }
 @EventHandler(priority=EventPriority.HIGHEST)
 public void highest(BlockBreakEvent event){
  if(event!=seen)throw new IllegalStateException("Событие B сменилось перед HIGHEST");
  trace(event,"B4 c"+bit(event.isCancelled()));
 }
}
