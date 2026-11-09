package fixtures;
import org.bukkit.command.*;
import org.bukkit.event.*;
import org.bukkit.event.player.PlayerJoinEvent;
import org.bukkit.plugin.java.JavaPlugin;
public final class HandlersPlugin extends JavaPlugin {
 public abstract static class EqualListener implements Listener {
  protected int calls;
  public final boolean equals(Object other){return other instanceof EqualListener;}
  public final int hashCode(){return 1;}
 }
 public static final class First extends EqualListener {
  @EventHandler public void join(PlayerJoinEvent event){
   event.getPlayer().sendMessage("Handler first "+(++calls));
   HandlerList.unregisterAll(this);
  }
 }
 public static final class Second extends EqualListener {
  @EventHandler public void join(PlayerJoinEvent event){event.getPlayer().sendMessage("Handler second "+(++calls));}
 }
 private final First first=new First();
 private final Second second=new Second();
 @Override public void onEnable(){
  getServer().getPluginManager().registerEvents(first,this);
  getServer().getPluginManager().registerEvents(first,this);
  getServer().getPluginManager().registerEvents(second,this);
 }
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){
  if(args.length!=1||!args[0].equals("rearm"))return false;
  getServer().getPluginManager().registerEvents(first,this);
  getServer().getPluginManager().registerEvents(first,this);
  sender.sendMessage("Handler first rearmed");
  return true;
 }
}
