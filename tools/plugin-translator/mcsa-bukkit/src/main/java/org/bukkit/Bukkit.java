package org.bukkit;
import org.bukkit.entity.Player;import ru.mcsa.bukkit.*;
public final class Bukkit {
 private static final org.bukkit.command.ConsoleCommandSender console = new org.bukkit.command.ConsoleCommandSender() {
  public String getName(){return "Console";}
  public void sendMessage(String message){Host.call(23,message);}
  public boolean hasPermission(String permission){return Permissions.has(permission,true);}
  public boolean isOp(){return true;}
 };
 // Один callback читает один согласованный список, без повторного разбора координат и UUID.
 private static long onlineEpoch=-1;private static java.util.Collection<? extends Player> online=java.util.List.of();
 private static final org.bukkit.inventory.ItemFactory itemFactory=new RemoteItemFactory();public static org.bukkit.inventory.ItemFactory getItemFactory(){return itemFactory;}
 private static final Server server=new Server(){
  public java.util.Collection<? extends Player> getOnlinePlayers(){return Bukkit.getOnlinePlayers();}public Player getPlayer(String n){return Bukkit.getPlayer(n);}public Player getPlayer(java.util.UUID u){return Bukkit.getPlayer(u);}
  public int getMaxPlayers(){return Bukkit.getMaxPlayers();}
  public org.bukkit.command.ConsoleCommandSender getConsoleSender(){return Bukkit.getConsoleSender();}
  public int broadcastMessage(String m){return Bukkit.broadcastMessage(m);}public org.bukkit.plugin.PluginManager getPluginManager(){return RuntimeState.manager;}public org.bukkit.scheduler.BukkitScheduler getScheduler(){return RuntimeState.scheduler;}
  public World getWorld(String n){return Bukkit.getWorld(n);}public java.util.List<World> getWorlds(){return Bukkit.getWorlds();}
 };
 public static org.bukkit.inventory.Inventory createInventory(org.bukkit.inventory.InventoryHolder holder,int size,net.kyori.adventure.text.Component title){return new ChestInventory(holder,size,title);}
 public static org.bukkit.boss.BossBar createBossBar(String title,org.bukkit.boss.BarColor color,org.bukkit.boss.BarStyle style,org.bukkit.boss.BarFlag... flags){return new RemoteBossBar(title,color,style,flags);}
 public static Player getPlayerExact(String name){for(Player p:getOnlinePlayers())if(p.getName().equalsIgnoreCase(name))return p;return null;}
 public static org.bukkit.scoreboard.ScoreboardManager getScoreboardManager(){return MainScoreboard.MANAGER;}
 public static int getCurrentTick(){return (int)Long.parseLong(Host.text(60));}
 public static int getMaxPlayers(){return Integer.parseInt(Host.text(165));}
 public static org.bukkit.command.ConsoleCommandSender getConsoleSender(){return console;}
 public static boolean isStopping(){return Host.yes(88);}
 public static Server getServer(){return server;} public static java.util.Collection<? extends Player> getOnlinePlayers(){if(onlineEpoch!=RuntimeState.callbackEpoch){String[] a=Host.call(1);java.util.List<Player> r=new java.util.ArrayList<>();for(int i=0;i<a.length;i+=9)r.add(Host.player(a,i));online=java.util.Collections.unmodifiableList(r);onlineEpoch=RuntimeState.callbackEpoch;}return online;}
 public static Player getPlayer(String n){for(Player p:getOnlinePlayers())if(p.getName().equalsIgnoreCase(n))return p;return null;}
 public static Player getPlayer(java.util.UUID u){for(Player p:getOnlinePlayers())if(p.getUniqueId().equals(u))return p;return null;}
 public static int broadcastMessage(String m){Host.call(0,m);return getOnlinePlayers().size();}
 public static org.bukkit.plugin.PluginManager getPluginManager(){return RuntimeState.manager;}public static org.bukkit.scheduler.BukkitScheduler getScheduler(){return RuntimeState.scheduler;}
 public static World getWorld(String n){for(World w:getWorlds())if(w.getName().equals(n))return w;return null;}
 public static java.util.List<World> getWorlds(){return java.util.Arrays.asList(new RemoteWorld(0),new RemoteWorld(1),new RemoteWorld(2));}
}
