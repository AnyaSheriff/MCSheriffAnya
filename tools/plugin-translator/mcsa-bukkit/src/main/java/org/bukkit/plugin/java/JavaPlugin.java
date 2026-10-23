package org.bukkit.plugin.java;
import org.bukkit.*; import org.bukkit.command.*; import org.bukkit.configuration.file.FileConfiguration; import ru.mcsa.bukkit.Host;
public abstract class JavaPlugin implements org.bukkit.plugin.Plugin, CommandExecutor, TabCompleter {
 private FileConfiguration config; private String name="plugin"; private boolean enabled;
 private final java.util.Map<String,PluginCommand> commands=new java.util.LinkedHashMap<>();
 public final void initialize(String n){name=n;enabled=true;}
 public final void declareCommand(String n,String description,String usage,int level){Host.call(4,n,description,usage,""+level);commands.put(n,new PluginCommand(n,this));}
 public String getName(){return name;} public final Server getServer(){return Bukkit.getServer();} public final boolean isEnabled(){return enabled;}
 public PluginCommand getCommand(String n){return commands.get(n.toLowerCase(java.util.Locale.ROOT));}
 public FileConfiguration getConfig(){if(config==null)reloadConfig();return config;}
 public void reloadConfig(){config=new FileConfiguration();config.loadFromString(Host.text(8,"config.yml"));}
 public void saveDefaultConfig(){Host.call(26,"config.yml",ru.mcsa.bukkit.RuntimeState.defaultConfig);config=null;} public void saveConfig(){Host.call(9,"config.yml",getConfig().saveToString());}
 public void onLoad(){} public void onEnable(){} public void onDisable(){}
 public boolean onCommand(CommandSender s,Command c,String l,String[] a){return false;}
 public java.util.List<String> onTabComplete(CommandSender s,Command c,String l,String[] a){return null;}
}
