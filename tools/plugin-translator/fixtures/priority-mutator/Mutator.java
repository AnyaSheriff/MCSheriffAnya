package fixtures;
import org.bukkit.command.*;
import org.bukkit.event.*;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.plugin.java.JavaPlugin;
public final class Mutator extends JavaPlugin implements Listener {
 private String mode="epoch";
 @Override public void onEnable(){getServer().getPluginManager().registerEvents(this,this);}
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){if(args.length!=1)return false;mode=args[0];sender.sendMessage("mutator mode "+mode);return true;}
 @EventHandler(priority=EventPriority.LOW) public void low(BlockBreakEvent event){
  if(mode.equals("epoch"))event.getPlayer().getInventory().setItem(0,new org.bukkit.inventory.ItemStack(org.bukkit.Material.STONE,5));
  if(mode.equals("trap"))throw new IllegalStateException("own fixture deliberate trap");
  if(mode.equals("fuel")){while(true){}}
 }
}
