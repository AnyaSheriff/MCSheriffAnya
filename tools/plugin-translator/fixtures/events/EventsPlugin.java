package fixtures;
import org.bukkit.event.*;
import org.bukkit.event.player.*;
import org.bukkit.event.block.BlockBreakEvent;
import org.bukkit.event.block.BlockPlaceEvent;
import org.bukkit.Material;
import org.bukkit.plugin.java.JavaPlugin;
public final class EventsPlugin extends JavaPlugin implements Listener {
 private String mode="";
 @Override public boolean onCommand(org.bukkit.command.CommandSender sender,org.bukkit.command.Command command,String label,String[] args){
  if(args.length==0)return true;mode=args[0];
  if(mode.equals("permissions")){if(!sender.hasPermission("fixture.public.child")||sender.hasPermission("fixture.bypass"))throw new IllegalStateException("Права не соответствуют plugin.yml");sender.sendMessage("Permission inheritance verified; op="+sender.isOp());}
  if(mode.equals("world")&&sender instanceof org.bukkit.entity.Player){org.bukkit.entity.Player player=(org.bukkit.entity.Player)sender;org.bukkit.World world=player.getWorld();if(world.getEnvironment()!=org.bukkit.World.Environment.NORMAL||world.getMinHeight()!=-64||world.getMaxHeight()!=320||!world.getPlayers().contains(player)||world.getType(2,63,0)!=Material.STONE||world.getBlockAt(1,63,0).getRelative(1,0,0).getType()!=Material.STONE)throw new IllegalStateException("Неверный снимок мира");sender.sendMessage("World snapshot verified");}
  if(mode.equals("armor")&&sender instanceof org.bukkit.entity.Player)((org.bukkit.entity.Player)sender).getInventory().setItem(39,new org.bukkit.inventory.ItemStack(Material.DIAMOND_HELMET));
  sender.sendMessage("Event probe armed: "+mode);return true;
 }
 @EventHandler public void move(PlayerMoveEvent event){if(!mode.equals("move")||!event.hasChangedPosition())return;mode="";event.setCancelled(true);event.getPlayer().sendMessage("Movement cancelled before physics");}
 @EventHandler public void jump(com.destroystokyo.paper.event.player.PlayerJumpEvent event){if(!mode.equals("jump"))return;mode="";event.setCancelled(true);event.getPlayer().sendMessage("Jump cancelled before physics");}
 @EventHandler public void interact(PlayerInteractEvent event){if(mode.equals("trace"))event.getPlayer().sendMessage("Interaction trace: "+event.getAction()+" cancelled="+event.isCancelled()+" at="+event.getPlayer().getLocation().getX()+","+event.getPlayer().getLocation().getY()+","+event.getPlayer().getLocation().getZ()+" mode="+event.getPlayer().getGameMode());if(!mode.equals("interact"))return;mode="";event.setCancelled(true);event.getPlayer().sendMessage("Interaction cancelled; hand="+event.getHand());}
 @EventHandler public void armor(com.destroystokyo.paper.event.player.PlayerArmorChangeEvent event){if(mode.equals("armor")&&event.getNewItem().getType()==Material.DIAMOND_HELMET){mode="";event.getPlayer().sendMessage("Armor change delivered: "+event.getSlot());}}
 @EventHandler public void respawn(PlayerRespawnEvent event){event.getPlayer().sendMessage("Respawn delivered; bed="+event.isBedSpawn()+"; anchor="+event.isAnchorSpawn());}
 @EventHandler public void modernChat(io.papermc.paper.event.player.AsyncChatEvent event){if(event.message() instanceof net.kyori.adventure.text.TextComponent&&((net.kyori.adventure.text.TextComponent)event.message()).content().equals("render-java"))event.renderer((source,name,message,viewer)->net.kyori.adventure.text.Component.text("Rendered chat: ").append(message));}
 @EventHandler public void bed(PlayerBedEnterEvent event){if(!mode.equals("bed"))return;mode="";if(event.enterAction().problem()!=null||!event.enterAction().canSetSpawn().success())throw new IllegalStateException("Кровать не прошла правила");event.setCancelled(true);event.getPlayer().setRespawnLocation(event.getBed().getLocation());event.getPlayer().sendMessage("Bed cancelled with real spawn rules");}
 @Override public void onLoad(){getLogger().info("Event fixture metadata initialized");}
 @Override public void onEnable(){getLogger().info("Event fixture registering listeners");getServer().getPluginManager().registerEvents(this,this);}
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
