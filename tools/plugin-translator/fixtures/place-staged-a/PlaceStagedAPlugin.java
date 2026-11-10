package fixtures;
import org.bukkit.command.*;
import org.bukkit.event.*;
import org.bukkit.event.block.BlockPlaceEvent;
import org.bukkit.plugin.java.JavaPlugin;
public final class PlaceStagedAPlugin extends JavaPlugin implements Listener {
 private String mode="order";private BlockPlaceEvent seen;
 private static String bit(boolean value){return value?"1":"0";}
 private void trace(BlockPlaceEvent event,String value){event.getPlayer().sendMessage("PLACE|"+value+" b"+bit(event.canBuild())+"|"+event.getPlayer().getUniqueId()+"|"+event.getBlockPlaced().getWorld().getName()+"|"+event.getBlockPlaced().getX()+"|"+event.getBlockPlaced().getY()+"|"+event.getBlockPlaced().getZ()+"|"+event.getBlockPlaced().getType().name()+"|same=1");}
 @Override public void onEnable(){getServer().getPluginManager().registerEvents(this,this);}
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(args.length!=1||!(args[0].equals("order")||args[0].equals("cancel")||args[0].equals("uncancel")||args[0].equals("buildfalse")||args[0].equals("restore")||args[0].equals("uncancelonly")||args[0].equals("buildonly")))return false;
  mode=args[0];sender.sendMessage("mode A "+mode);return true;
 }
 @EventHandler(priority=EventPriority.LOWEST)
 public void lowest(BlockPlaceEvent event){
  seen=event;boolean before=event.isCancelled();if(mode.equals("cancel")||mode.equals("uncancel")||mode.equals("uncancelonly")||mode.equals("buildonly"))event.setCancelled(true);if(mode.equals("buildfalse")||mode.equals("restore")||mode.equals("uncancelonly")||mode.equals("buildonly"))event.setBuild(false);
  trace(event,"A0 c"+bit(before)+">"+bit(event.isCancelled()));
 }
 @EventHandler(priority=EventPriority.NORMAL,ignoreCancelled=true)
 public void normal(BlockPlaceEvent event){
  if(event!=seen)throw new IllegalStateException("Событие A сменилось между приоритетами");
  trace(event,"A2 c"+bit(event.isCancelled()));
 }
 @EventHandler(priority=EventPriority.MONITOR)
 public void monitor(BlockPlaceEvent event){
  if(event!=seen)throw new IllegalStateException("Событие A сменилось перед MONITOR");
  trace(event,"A5 c"+bit(event.isCancelled()));
 }
}
