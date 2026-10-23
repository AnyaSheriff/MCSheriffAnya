package ru.mcsa.bukkit;
import org.bukkit.*;
public final class RemoteWorld implements World {
 public final int dimension; public RemoteWorld(int d){dimension=d;} public String getName(){return dimension==0?"world":dimension==1?"world_nether":"world_the_end";}
 public org.bukkit.block.Block getBlockAt(int x,int y,int z){return new RemoteBlock(this,x,y,z);}
 public Location getSpawnLocation(){String[] r=Host.call(15,""+dimension);return new Location(this,Double.parseDouble(r[0]),Double.parseDouble(r[1]),Double.parseDouble(r[2]));}
 public long getTime(){return Long.parseLong(Host.text(16,""+dimension));}public void setTime(long t){if(t<0)throw new IllegalArgumentException("Время меньше нуля");Host.call(17,""+dimension,""+(t%24000));}
 public boolean hasStorm(){return !Host.text(18,""+dimension).equals("clear");}public boolean isThundering(){return Host.text(18,""+dimension).equals("thunder");}
 public void setStorm(boolean b){Host.call(19,""+dimension,b?"rain":"clear");}public void setThundering(boolean b){Host.call(19,""+dimension,b?"thunder":"clear");}
}
