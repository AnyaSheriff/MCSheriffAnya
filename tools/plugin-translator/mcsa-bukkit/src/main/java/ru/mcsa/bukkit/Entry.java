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
 @Export(name="mcsa_dispatch") public static int dispatch(int kind,Address data,int len){RuntimeState.callbackEpoch++;try{return dispatchEvent(kind,data,len);}catch(RuntimeException|Error failure){Host.call(23,"Ошибка обработчика Bukkit: "+failure);throw failure;}}
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
  if(kind==34){GeneratedLoot.run(a);return 0;}
  if(kind==33){DeathEvents.run(a);return 0;}
  if(kind==32){NativeEvents.dispatch(a);return 0;}
  if(kind==27){return Population.wanted(Integer.parseInt(a[0]))?1:0;}
  if(kind==28){Population.run(a);return 0;}
  if(kind==29){RuntimeState.fire(new org.bukkit.event.world.WorldInitEvent(new RemoteWorld(Integer.parseInt(a[0]))));return 0;}
  if(kind==26){org.bukkit.event.inventory.FurnaceStartSmeltEvent event=new org.bukkit.event.inventory.FurnaceStartSmeltEvent(new RemoteBlock(new RemoteWorld(Integer.parseInt(a[0])),Integer.parseInt(a[1]),Integer.parseInt(a[2]),Integer.parseInt(a[3])),Integer.parseInt(a[4]));RuntimeState.fire(event);Host.call(22,""+event.getTotalCookTime());return 0;}
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
   case 31:{RemoteBlock block=new RemoteBlock(new RemoteWorld(Integer.parseInt(a[9])),Integer.parseInt(a[10]),Integer.parseInt(a[11]),Integer.parseInt(a[12]));java.util.List<org.bukkit.entity.Item> items=new java.util.ArrayList<>();java.util.Set<String> originals=new java.util.HashSet<>();for(int i=14;i<a.length;i++){items.add(new RemoteItem(a[i]));originals.add(a[i]);}org.bukkit.event.block.BlockDropItemEvent drops=new org.bukkit.event.block.BlockDropItemEvent(block,new BlockSnapshot(block,new String[]{a[13],"0","0","0","0","-1","-1"}),p,items);RuntimeState.fire(drops);StringBuilder kept=new StringBuilder();if(!drops.isCancelled())for(org.bukkit.entity.Item item:drops.getItems()){String id=item.getUniqueId().toString();if(!originals.contains(id))throw new UnsupportedOperationException("Добавление посторонней сущности в добычу блока пока не поддерживается; используйте World.dropItem");kept.append(id).append('\n');}Host.call(22,kept.toString());return 0;}
   case 30:e=a[10].equals("1")?new org.bukkit.event.entity.EntityPickupItemEvent(p,new RemoteItem(a[9]),Integer.parseInt(a[11])):new PlayerDropItemEvent(p,new RemoteItem(a[9]));break;
   case 25:MerchantEvents.event(p,a);return 0;
   case 24:{org.bukkit.inventory.ItemStack item=a[10].equals("air")?null:org.bukkit.inventory.ItemStack.fromHost(Material.matchMaterial(a[10]),Integer.parseInt(a[11]),a[12]);org.bukkit.event.entity.FoodLevelChangeEvent food=new org.bukkit.event.entity.FoodLevelChangeEvent(p,Integer.parseInt(a[9]),item);RuntimeState.fire(food);if(food.isCancelled())return 1;Host.call(22,""+food.getFoodLevel());return 0;}
   case 22:EnchantEvents.prepare(p,a);return 0;
   case 23:EnchantEvents.enchant(p,a);return 0;
   case 21:{PreparedAnvil view=new PreparedAnvil(p,a);RuntimeState.fire(new org.bukkit.event.inventory.PrepareAnvilEvent(view));Host.call(22,""+view.getRepairCost());return 0;}
   case 20:{PreparedCraft inventory=new PreparedCraft(p,a);org.bukkit.inventory.ItemStack original=inventory.getResult();org.bukkit.event.inventory.PrepareItemCraftEvent craft=new org.bukkit.event.inventory.PrepareItemCraftEvent(inventory,inventory.view(),original==null?null:()->original.clone(),a[9].equals("1"));RuntimeState.fire(craft);org.bukkit.inventory.ItemStack result=inventory.getResult();if(result!=null&&!result.isEmpty())Host.call(22,Ascii.lower(result.getType().name())+"\n"+result.getAmount()+"\n"+result.metadata());else Host.call(22,"");return 0;}
   case 19:{org.bukkit.entity.Entity target=RemoteEntity.find(a[9]);org.bukkit.inventory.EquipmentSlot hand=a[10].equals("1")?org.bukkit.inventory.EquipmentSlot.OFF_HAND:org.bukkit.inventory.EquipmentSlot.HAND;int slot=Integer.parseInt(a[11]);
    if(slot>=0){org.bukkit.inventory.ItemStack held=hand==org.bukkit.inventory.EquipmentSlot.HAND?p.getInventory().getItemInMainHand():p.getInventory().getItemInOffHand(),worn=((org.bukkit.entity.LivingEntity)target).getEquipment().getItem(org.bukkit.inventory.EquipmentSlot.values()[slot]);e=new PlayerArmorStandManipulateEvent(p,(org.bukkit.entity.ArmorStand)target,held==null?new org.bukkit.inventory.ItemStack(Material.AIR):held,worn==null?new org.bukkit.inventory.ItemStack(Material.AIR):worn,org.bukkit.inventory.EquipmentSlot.values()[slot],hand);}else e=new PlayerInteractEntityEvent(p,target,hand);break;}
   case 17:{java.util.Set<Integer> slots=new java.util.LinkedHashSet<>();for(int i=10;i<a.length;i++)slots.add(Integer.parseInt(a[i]));return ChestViews.drag(p,Integer.parseInt(a[9]),slots);}
   case 16:if(a[9].equals("2")){e=new PlayerSwapHandItemsEvent(p,p.getInventory().getItemInOffHand(),p.getInventory().getItemInMainHand());}else if(a[9].equals("0")){e=new PlayerItemHeldEvent(p,Integer.parseInt(Host.text(39,p.getUniqueId().toString())),Integer.parseInt(a[10]));}else {int slot=Integer.parseInt(a[10]);int index=slot==45?40:slot>=36?slot-36:slot;org.bukkit.inventory.ItemStack item=p.getInventory().getItem(index);if(item==null)throw new IllegalStateException("Предмет еды исчез до события");e=new PlayerItemConsumeEvent(p,item,slot==45?org.bukkit.inventory.EquipmentSlot.OFF_HAND:org.bukkit.inventory.EquipmentSlot.HAND);}break;
   case 12:return ChestViews.event(p,Integer.parseInt(a[9]),Integer.parseInt(a[10]),Integer.parseInt(a[11]),Integer.parseInt(a[12]),Integer.parseInt(a[13]));
   case 1:e=new PlayerJoinEvent(p,null);break;
   case 2:e=new PlayerQuitEvent(p,null);break;
   case 3:e=new AsyncPlayerChatEvent(p,a[9]);break;
   case 4:case 5:org.bukkit.block.Block b=new RemoteBlock(new RemoteWorld(Integer.parseInt(a[9])),Integer.parseInt(a[10]),Integer.parseInt(a[11]),Integer.parseInt(a[12]),kind==5?a[13]:null);e=kind==4?new BlockBreakEvent(b,p):new BlockPlaceEvent(b,p);break;
   case 6:e=new PlayerCommandPreprocessEvent(p,"/"+a[9]);break;
   case 18:e=a[11].equals("1")?new org.bukkit.event.entity.EntityDamageByEntityEvent(Host.player(a,12),p,org.bukkit.event.entity.EntityDamageEvent.DamageCause.valueOf(a[9]),Double.parseDouble(a[10])):new org.bukkit.event.entity.EntityDamageEvent(p,org.bukkit.event.entity.EntityDamageEvent.DamageCause.valueOf(a[9]),Double.parseDouble(a[10]));break;
   case 10:e=new org.bukkit.event.entity.EntityDamageEvent(p,org.bukkit.event.entity.EntityDamageEvent.DamageCause.valueOf(a[9]),Double.parseDouble(a[10]));break;
   case 11:e=new org.bukkit.event.entity.PlayerDeathEvent(p,a[9].equals("1"));break;
   default:throw new IllegalArgumentException("Неизвестное событие");
  }
  if(kind==1){MainScoreboard.INSTANCE.sync(p);}if(kind==2)ArmorChanges.leave(p);
  RuntimeState.fire(e);
  if(e instanceof Cancellable&&((Cancellable)e).isCancelled())return 1;
  if(kind==5&&!((BlockPlaceEvent)e).canBuild())return 1;
  if(kind==10||kind==18){Host.call(22,""+((org.bukkit.event.entity.EntityDamageEvent)e).getDamage());return 2;}
  if(kind==3){AsyncPlayerChatEvent legacy=(AsyncPlayerChatEvent)e;io.papermc.paper.event.player.AsyncChatEvent modern=new io.papermc.paper.event.player.AsyncChatEvent(p,net.kyori.adventure.text.Component.text(legacy.getMessage()));RuntimeState.fire(modern);if(modern.isCancelled())return 1;if(modern.hasCustomDelivery()){modern.deliver();return 1;}}
  if(kind==3&&!((AsyncPlayerChatEvent)e).getMessage().equals(a[9])){Host.call(22,((AsyncPlayerChatEvent)e).getMessage());return 2;}
  return 0;
 }
 public static void keep(){dispatch(-1,Address.fromInt(0),0);}
}
