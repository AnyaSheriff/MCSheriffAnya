package ru.mcsa.bukkit;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.teavm.interop.*;
import org.bukkit.*;
import org.bukkit.command.*;
import org.bukkit.entity.Player;
import org.bukkit.event.*;
import org.bukkit.event.player.*;
import org.bukkit.event.block.*;
public final class Entry {
 @Export(name="mcsa_dispatch") public static int dispatch(int kind,Address data,int len){
  byte[] bytes=new byte[len];for(int i=0;i<len;i++)bytes[i]=data.add(i).getByte();String[] a=Host.split(new String(bytes,StandardCharsets.UTF_8));
  if(kind==0){RuntimeState.plugin.onDisable();EntityDataStore.flush();return 0;}
  if(kind==8){RuntimeState.scheduler.fire(Integer.parseInt(a[0]));return 0;}
  if(kind==7||kind==9){
   CommandSender sender;
   if(a[0].equals("0")){sender=Bukkit.getPlayer(java.util.UUID.fromString(a[1]));if(sender==null)return 0;}
   else {final String name=a[2];final boolean console=a[0].equals("1");sender=new CommandSender(){public String getName(){return name;}public void sendMessage(String m){Host.call(23,m);}public boolean isOp(){return console;}public boolean hasPermission(String p){return console;}};}
   PluginCommand command=RuntimeState.plugin.getCommand(a[3]);if(kind==9){if(command!=null)for(String value:command.tabComplete(sender,a[3],Arrays.copyOfRange(a,4,a.length)))Host.call(25,value);return 0;}return command!=null&&command.execute(sender,a[3],Arrays.copyOfRange(a,4,a.length))?1:0;
  }
  Player p=Host.player(a,0);Event e;
  switch(kind){
   case 12:return ChestViews.event(p,Integer.parseInt(a[9]),Integer.parseInt(a[10]),Integer.parseInt(a[11]),Integer.parseInt(a[12]),Integer.parseInt(a[13]));
   case 1:e=new PlayerJoinEvent(p,null);break;
   case 2:e=new PlayerQuitEvent(p,null);break;
   case 3:e=new AsyncPlayerChatEvent(p,a[9]);break;
   case 4:case 5:org.bukkit.block.Block b=new RemoteBlock(new RemoteWorld(Integer.parseInt(a[9])),Integer.parseInt(a[10]),Integer.parseInt(a[11]),Integer.parseInt(a[12]),kind==5?a[13]:null);e=kind==4?new BlockBreakEvent(b,p):new BlockPlaceEvent(b,p);break;
   case 6:e=new PlayerCommandPreprocessEvent(p,"/"+a[9]);break;
   case 10:e=new org.bukkit.event.entity.EntityDamageEvent(p,org.bukkit.event.entity.EntityDamageEvent.DamageCause.valueOf(a[9]),Double.parseDouble(a[10]));break;
   case 11:e=new org.bukkit.event.entity.PlayerDeathEvent(p,a[9].equals("1"));break;
   default:throw new IllegalArgumentException("Неизвестное событие");
  }
  if(kind==1)MainScoreboard.INSTANCE.sync(p);
  RuntimeState.fire(e);
  if(e instanceof Cancellable&&((Cancellable)e).isCancelled())return 1;
  if(kind==10){Host.call(22,""+((org.bukkit.event.entity.EntityDamageEvent)e).getDamage());return 2;}
  if(kind==3&&!((AsyncPlayerChatEvent)e).getMessage().equals(a[9])){Host.call(22,((AsyncPlayerChatEvent)e).getMessage());return 2;}
  return 0;
 }
 public static void keep(){dispatch(-1,Address.fromInt(0),0);}
}
