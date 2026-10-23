package fixtures;
import org.bukkit.command.*;
import org.bukkit.plugin.java.JavaPlugin;
public final class CommandPlugin extends JavaPlugin {
 @Override public boolean onCommand(CommandSender sender,Command command,String label,String[] args){sender.sendMessage("Команда из Java работает");return true;}
 @Override public java.util.List<String> onTabComplete(CommandSender sender,Command command,String label,String[] args){return java.util.Arrays.asList("java-tab-ok","java-tab-two");}
}
