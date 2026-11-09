package fixtures;
import org.bukkit.command.*;
import org.bukkit.event.*;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.plugin.java.JavaPlugin;
public final class PriorityAPlugin extends JavaPlugin implements Listener {
 private String mode="order";private BlockBreakEvent seen;
 private static String bit(boolean value){return value?"1":"0";}
 private void trace(BlockBreakEvent event,String value){event.getPlayer().sendMessage("trace "+value);}
 @Override public void onEnable(){getServer().getPluginManager().registerEvents(this,this);}
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(args.length!=1||!(args[0].equals("order")||args[0].equals("cancel")||args[0].equals("uncancel")))return false;
  mode=args[0];sender.sendMessage("mode A "+mode);return true;
 }
 @EventHandler(priority=EventPriority.LOWEST)
 public void lowest(BlockBreakEvent event){
  seen=event;boolean before=event.isCancelled();if(!mode.equals("order"))event.setCancelled(true);
  trace(event,"A0 c"+bit(before)+">"+bit(event.isCancelled()));
 }
 @EventHandler(priority=EventPriority.NORMAL,ignoreCancelled=true)
 public void normal(BlockBreakEvent event){
  if(event!=seen)throw new IllegalStateException("Событие A сменилось между приоритетами");
  trace(event,"A2 c"+bit(event.isCancelled()));
 }
 @EventHandler(priority=EventPriority.MONITOR)
 public void monitor(BlockBreakEvent event){
  if(event!=seen)throw new IllegalStateException("Событие A сменилось перед MONITOR");
  trace(event,"A5 c"+bit(event.isCancelled()));
 }
}
