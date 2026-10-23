package fixtures;
import org.bukkit.event.*;
import org.bukkit.event.player.*;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.event.block.BlockPlaceEvent;
import org.bukkit.Material;
import org.bukkit.plugin.java.JavaPlugin;
public final class EventsPlugin extends JavaPlugin implements Listener {
 @Override public void onEnable(){getServer().getPluginManager().registerEvents(this,this);}
 @EventHandler public void join(PlayerJoinEvent event){event.getPlayer().sendMessage("Вход из Java работает");}
 @EventHandler public void chat(AsyncPlayerChatEvent event){if(event.getMessage().equals("cancel-java")){event.setCancelled(true);event.getPlayer().sendMessage("Отмена чата из Java работает");}}
 @EventHandler public void block(BlockBreakEvent event){event.setCancelled(true);event.getPlayer().sendMessage("Отмена блока из Java работает");}
 @EventHandler public void place(BlockPlaceEvent event){if(event.getBlockPlaced().getType()==Material.STONE){event.setCancelled(true);event.getPlayer().sendMessage("Отмена установки из Java работает");}}
 @EventHandler public void command(PlayerCommandPreprocessEvent event){if(event.getMessage().equals("/java-blocked")){event.setCancelled(true);event.getPlayer().sendMessage("Отмена команды из Java работает");}}
 @EventHandler public void damage(org.bukkit.event.entity.EntityDamageEvent event){
  if(event.getCause()==org.bukkit.event.entity.EntityDamageEvent.DamageCause.CUSTOM){event.setDamage(1);((org.bukkit.entity.Player)event.getEntity()).sendMessage("Урон из Java изменён на 1");}
  if(event.getCause()==org.bukkit.event.entity.EntityDamageEvent.DamageCause.FALL){event.setCancelled(true);((org.bukkit.entity.Player)event.getEntity()).sendMessage("Урон от падения из Java отменён");}
 }
 @EventHandler public void death(org.bukkit.event.entity.PlayerDeathEvent event){event.getEntity().sendMessage("Смерть из Java работает");}
}
