package ru.mcsa.bukkit;
import org.bukkit.*;
public final class RemoteWorld implements World {
 public final int dimension; public RemoteWorld(int d){dimension=d;} public String getName(){return dimension==0?"world":dimension==1?"world_nether":"world_the_end";}
 private org.bukkit.entity.Item drop(Location loc,org.bukkit.inventory.ItemStack item,boolean natural){java.util.Objects.requireNonNull(item);if(loc.getWorld()==null||!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");return new RemoteItem(Host.text(79,""+dimension,""+loc.getX(),""+loc.getY(),""+loc.getZ(),Ascii.lower(item.getType().name()),""+item.getAmount(),item.metadata(),""+natural));}
 public org.bukkit.entity.Item dropItem(Location loc,org.bukkit.inventory.ItemStack item){return drop(loc,item,false);}public org.bukkit.entity.Item dropItemNaturally(Location loc,org.bukkit.inventory.ItemStack item){return drop(loc,item,true);}
 public void spawnParticle(Particle particle,Location loc,int count,double dx,double dy,double dz){if(loc.getWorld()==null||!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");Host.call(86,""+dimension,""+loc.getX(),""+loc.getY(),""+loc.getZ(),Ascii.lower(particle.name()),""+count,""+dx,""+dy,""+dz);}
 public boolean isChunkLoaded(int x,int z){return Host.yes(72,""+dimension,""+x,""+z);}
 public int getHighestBlockYAt(Location loc){if(loc.getWorld()!=null&&!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");return Integer.parseInt(Host.text(73,""+dimension,""+loc.getBlockX(),""+loc.getBlockZ()));}
 @SuppressWarnings("unchecked") public <T>T getGameRuleValue(GameRule<T> rule){if(rule!=GameRule.RANDOM_TICK_SPEED)throw new UnsupportedOperationException("Неизвестное правило мира");return (T)Integer.valueOf(Host.text(74));}
 public void playSound(Location loc,Sound sound,float volume,float pitch){if(loc.getWorld()==null||!loc.getWorld().getName().equals(getName()))throw new IllegalArgumentException("Разные миры");Host.call(75,""+dimension,""+loc.getX(),""+loc.getY(),""+loc.getZ(),sound.getKey().toString(),""+volume,""+pitch);}
 public Environment getEnvironment(){return dimension==0?Environment.NORMAL:dimension==1?Environment.NETHER:Environment.THE_END;}
 public int getMinHeight(){return -64;}public int getMaxHeight(){return 320;}
 public java.util.List<org.bukkit.entity.Player> getPlayers(){java.util.List<org.bukkit.entity.Player> result=new java.util.ArrayList<>();for(org.bukkit.entity.Player player:Bukkit.getOnlinePlayers())if(((RemotePlayer)player).snapshotLocation().getWorld().getName().equals(getName()))result.add(player);return result;}
 public org.bukkit.block.Block getBlockAt(int x,int y,int z){return new RemoteBlock(this,x,y,z);}
 public WorldBorder getWorldBorder(){return new RemoteBorder(this);}
 public Location getSpawnLocation(){String[] r=Host.call(15,""+dimension);return new Location(this,Double.parseDouble(r[0]),Double.parseDouble(r[1]),Double.parseDouble(r[2]));}
 public long getTime(){return Long.parseLong(Host.text(16,""+dimension));}public void setTime(long t){if(t<0)throw new IllegalArgumentException("Время меньше нуля");Host.call(17,""+dimension,""+(t%24000));}
 public boolean hasStorm(){return !Host.text(18,""+dimension).equals("clear");}public boolean isThundering(){return Host.text(18,""+dimension).equals("thunder");}
 public void setStorm(boolean b){Host.call(19,""+dimension,b?"rain":"clear");}public void setThundering(boolean b){Host.call(19,""+dimension,b?"thunder":"clear");}
}
