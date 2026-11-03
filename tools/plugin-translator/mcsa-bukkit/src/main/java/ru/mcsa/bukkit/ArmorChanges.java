package ru.mcsa.bukkit;
import java.util.*;import org.bukkit.*;import org.bukkit.entity.Player;import org.bukkit.inventory.ItemStack;import com.destroystokyo.paper.event.player.PlayerArmorChangeEvent;
// Изменения снимка за такт; собственные задачи плагина видят уже опубликованную броню.
public final class ArmorChanges {
 private static final Map<java.util.UUID,ItemStack[]> previous=new HashMap<>();private static boolean started;
 public static void start(){if(started)return;started=true;RuntimeState.scheduler.runTaskTimer(RuntimeState.plugin,ArmorChanges::poll,1,1);}
 public static void leave(Player player){previous.remove(player.getUniqueId());}
 private static ItemStack[] copy(ItemStack[] items){ItemStack[] result=new ItemStack[4];for(int i=0;i<4;i++)result[i]=items[i]==null?new ItemStack(Material.AIR):items[i].clone();return result;}
 private static void poll(){for(Player player:Bukkit.getOnlinePlayers()){
  if(Host.call(62,player.getUniqueId().toString()).length==0)continue;ItemStack[] now=copy(player.getInventory().getArmorContents()),old=previous.put(player.getUniqueId(),now);if(old==null)continue;
  for(int slot=0;slot<4;slot++)if(!old[slot].equals(now[slot]))RuntimeState.fire(new PlayerArmorChangeEvent(player,PlayerArmorChangeEvent.SlotType.values()[3-slot],old[slot].clone(),now[slot].clone()));
 }}
}
