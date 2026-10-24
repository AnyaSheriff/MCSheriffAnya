package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.Player;import org.bukkit.inventory.*;
public final class RemotePlayer implements Player {
 private final String uuid,name;private final Location location;private final GameMode mode;
 public RemotePlayer(String[] a,int i){uuid=a[i];name=a[i+1];mode=GameMode.values()[Integer.parseInt(a[i+2])];location=new Location(new RemoteWorld(Integer.parseInt(a[i+3])),Double.parseDouble(a[i+4]),Double.parseDouble(a[i+5]),Double.parseDouble(a[i+6]),Float.parseFloat(a[i+7]),Float.parseFloat(a[i+8]));}
 @Override public boolean equals(Object other){return other instanceof RemotePlayer&&uuid.equals(((RemotePlayer)other).uuid);}@Override public int hashCode(){return uuid.hashCode();}
 public String getName(){return name;}public java.util.UUID getUniqueId(){return java.util.UUID.fromString(uuid);}
 public Location snapshotLocation(){return location.clone();}
 public Location getLocation(){for(Player p:Bukkit.getOnlinePlayers())if(p.getUniqueId().toString().equals(uuid))return ((RemotePlayer)p).location.clone();return location.clone();}
 public World getWorld(){return getLocation().getWorld();} public GameMode getGameMode(){for(Player p:Bukkit.getOnlinePlayers())if(p.getUniqueId().toString().equals(uuid))return ((RemotePlayer)p).mode;return mode;}
 public void setGameMode(GameMode m){Host.call(11,uuid,""+m.ordinal());}public double getHealth(){return Double.parseDouble(Host.text(12,uuid));}
 public void sendMessage(String m){Host.call(2,uuid,m);}public boolean isOp(){return Integer.parseInt(Host.text(10,uuid))>0;}
 public void sendMessage(net.kyori.adventure.text.Component m){if(!Host.yes(27,uuid,Text.json(m)))throw new IllegalStateException("Компонент сообщения отклонён хозяином");}
 public void sendActionBar(net.kyori.adventure.text.Component m){if(!Host.yes(28,uuid,Text.json(m)))throw new IllegalStateException("Компонент action bar отклонён хозяином");}
 public org.bukkit.attribute.AttributeInstance getAttribute(org.bukkit.attribute.Attribute attribute){if(java.util.Arrays.asList("follow_range","flying_speed","spawn_reinforcements","tempt_range").contains(attribute.getKey().getKey()))return null;return new RemoteAttribute(uuid,attribute);}
 public void setHealth(double health){if(!Double.isFinite(health)||health<0)throw new IllegalArgumentException("Здоровье должно быть конечным и неотрицательным");Host.call(29,uuid,""+health);}
 public boolean addPotionEffect(org.bukkit.potion.PotionEffect effect){return Host.yes(30,uuid,effect.getType().getKey().getKey(),""+effect.getDuration(),""+effect.getAmplifier(),""+effect.isAmbient(),""+effect.hasParticles(),""+effect.hasIcon());}
 public void removePotionEffect(org.bukkit.potion.PotionEffectType type){Host.call(31,uuid,type.getKey().getKey());}
 public int getFoodLevel(){return Integer.parseInt(Host.call(62,uuid)[0]);}public int getLevel(){return Integer.parseInt(Host.call(62,uuid)[1]);}public float getExp(){return Float.parseFloat(Host.call(62,uuid)[2]);}
 public void setRespawnLocation(Location location){Location value=location==null?getLocation():location;Host.call(64,uuid,""+((RemoteWorld)value.getWorld()).dimension,""+value.getX(),""+value.getY(),""+value.getZ(),""+value.getYaw(),location==null?"0":"1");}
 public boolean hasPermission(String p){return Permissions.has(p,isOp());}
 public boolean teleport(Location l){return Host.yes(3,uuid,""+((RemoteWorld)l.getWorld()).dimension,""+l.getX(),""+l.getY(),""+l.getZ(),""+l.getYaw(),""+l.getPitch());}
 public void kickPlayer(String r){Host.call(20,uuid,r);}
 public InventoryView openInventory(Inventory inv){return ChestViews.open(this,inv);}public InventoryView getOpenInventory(){return ChestViews.current(this);}public void closeInventory(){ChestViews.close(this);}
 public PlayerInventory getInventory(){return new PlayerItems(this);}
}
