package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.Player;import org.bukkit.inventory.*;
public final class RemotePlayer implements Player {
 private final String uuid,name;private final Location location;private final GameMode mode;
 public RemotePlayer(String[] a,int i){uuid=a[i];name=a[i+1];mode=GameMode.values()[Integer.parseInt(a[i+2])];location=new Location(new RemoteWorld(Integer.parseInt(a[i+3])),Double.parseDouble(a[i+4]),Double.parseDouble(a[i+5]),Double.parseDouble(a[i+6]),Float.parseFloat(a[i+7]),Float.parseFloat(a[i+8]));}
 public String getName(){return name;}public java.util.UUID getUniqueId(){return java.util.UUID.fromString(uuid);}
 public Location getLocation(){for(Player p:Bukkit.getOnlinePlayers())if(p.getUniqueId().toString().equals(uuid))return ((RemotePlayer)p).location.clone();return location.clone();}
 public World getWorld(){return getLocation().getWorld();} public GameMode getGameMode(){for(Player p:Bukkit.getOnlinePlayers())if(p.getUniqueId().toString().equals(uuid))return ((RemotePlayer)p).mode;return mode;}
 public void setGameMode(GameMode m){Host.call(11,uuid,""+m.ordinal());}public double getHealth(){return Double.parseDouble(Host.text(12,uuid));}
 public void sendMessage(String m){Host.call(2,uuid,m);}public boolean isOp(){return Integer.parseInt(Host.text(10,uuid))>0;}
 public boolean hasPermission(String p){return Integer.parseInt(Host.text(10,uuid))>=4;}
 public boolean teleport(Location l){return Host.yes(3,uuid,""+((RemoteWorld)l.getWorld()).dimension,""+l.getX(),""+l.getY(),""+l.getZ(),""+l.getYaw(),""+l.getPitch());}
 public void kickPlayer(String r){Host.call(20,uuid,r);}
 public PlayerInventory getInventory(){return new PlayerInventory(){
  public void setItem(int slot,ItemStack s){if(slot<0||slot>=36)throw new IllegalArgumentException("Ячейка: 0..35");Host.call(21,uuid,""+(slot<9?slot+36:slot),s==null?"air":s.getType().name().toLowerCase(java.util.Locale.ROOT),""+(s==null?0:s.getAmount()));}
 };}
}
