package fixtures;
import org.bukkit.Bukkit;
import org.bukkit.plugin.java.JavaPlugin;
public final class SchedulerPlugin extends JavaPlugin {
 @Override public void onEnable(){saveDefaultConfig();long period=getConfig().getLong("period",20);Bukkit.getScheduler().runTaskTimer(this,()->Bukkit.broadcastMessage(getConfig().getString("message")),period,period);}
}
