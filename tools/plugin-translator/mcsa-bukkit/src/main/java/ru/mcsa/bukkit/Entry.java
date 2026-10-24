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
 @Export(name="mcsa_dispatch") public static int dispatch(int kind,Address data,int len){try{return dispatchEvent(kind,data,len);}catch(RuntimeException|Error failure){Host.call(23,"Ошибка обработчика Bukkit: "+failure);throw failure;}}
 private static int dispatchEvent(int kind,Address data,int len){
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
   case 15:{Location to=new Location(new RemoteWorld(Integer.parseInt(a[9])),Double.parseDouble(a[10]),Double.parseDouble(a[11]),Double.parseDouble(a[12]),Float.parseFloat(a[13]),Float.parseFloat(a[14]));PlayerRespawnEvent respawn=new PlayerRespawnEvent(p,to,a[15].equals("1"),a[16].equals("1"));RuntimeState.fire(respawn);Location result=respawn.getRespawnLocation();Host.call(22,((RemoteWorld)result.getWorld()).dimension+" "+result.getX()+" "+result.getY()+" "+result.getZ()+" "+result.getYaw()+" "+result.getPitch());return 2;}
   case 13:{Location from=((RemotePlayer)p).snapshotLocation(),to=new Location(new RemoteWorld(Integer.parseInt(a[9])),Double.parseDouble(a[10]),Double.parseDouble(a[11]),Double.parseDouble(a[12]),Float.parseFloat(a[13]),Float.parseFloat(a[14]));
    if(a[15].equals("1")){com.destroystokyo.paper.event.player.PlayerJumpEvent jump=new com.destroystokyo.paper.event.player.PlayerJumpEvent(p,from.clone(),to.clone());RuntimeState.fire(jump);if(jump.isCancelled())return 1;}
    PlayerMoveEvent move=new PlayerMoveEvent(p,from,to);RuntimeState.fire(move);if(move.isCancelled()||move.getTo()==null)return 1;Location result=move.getTo();Host.call(22,((RemoteWorld)result.getWorld()).dimension+" "+result.getX()+" "+result.getY()+" "+result.getZ()+" "+result.getYaw()+" "+result.getPitch());return 2;}
   case 14:{int hand=Integer.parseInt(a[10]);org.bukkit.block.Block clicked=a[11].equals("1")?new RemoteBlock(new RemoteWorld(Integer.parseInt(a[12])),Integer.parseInt(a[13]),Integer.parseInt(a[14]),Integer.parseInt(a[15]),null):null;
    org.bukkit.inventory.ItemStack item=hand==0?p.getInventory().getItemInMainHand():p.getInventory().getItemInOffHand();
    PlayerInteractEvent interact=new PlayerInteractEvent(p,Action.values()[Integer.parseInt(a[9])],item,clicked,org.bukkit.block.BlockFace.values()[Integer.parseInt(a[16])],hand==0?org.bukkit.inventory.EquipmentSlot.HAND:org.bukkit.inventory.EquipmentSlot.OFF_HAND);RuntimeState.fire(interact);
    if(interact.useInteractedBlock()!=Event.Result.DENY&&clicked!=null&&interact.getAction()==Action.RIGHT_CLICK_BLOCK&&BedActions.cancelled(p,clicked))return 3;
    return (interact.useInteractedBlock()==Event.Result.DENY?1:0)|(interact.useItemInHand()==Event.Result.DENY?2:0);}
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
  if(kind==1){MainScoreboard.INSTANCE.sync(p);}if(kind==2)ArmorChanges.leave(p);
  RuntimeState.fire(e);
  if(e instanceof Cancellable&&((Cancellable)e).isCancelled())return 1;
  if(kind==10){Host.call(22,""+((org.bukkit.event.entity.EntityDamageEvent)e).getDamage());return 2;}
  if(kind==3){AsyncPlayerChatEvent legacy=(AsyncPlayerChatEvent)e;io.papermc.paper.event.player.AsyncChatEvent modern=new io.papermc.paper.event.player.AsyncChatEvent(p,net.kyori.adventure.text.Component.text(legacy.getMessage()));RuntimeState.fire(modern);if(modern.isCancelled())return 1;if(modern.hasCustomDelivery()){modern.deliver();return 1;}}
  if(kind==3&&!((AsyncPlayerChatEvent)e).getMessage().equals(a[9])){Host.call(22,((AsyncPlayerChatEvent)e).getMessage());return 2;}
  return 0;
 }
 public static void keep(){dispatch(-1,Address.fromInt(0),0);}
}
