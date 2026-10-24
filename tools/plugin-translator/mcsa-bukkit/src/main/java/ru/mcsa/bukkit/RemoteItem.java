package ru.mcsa.bukkit;
import org.bukkit.*;import org.bukkit.entity.*;import org.bukkit.inventory.*;
public final class RemoteItem implements Item {
 private final java.util.UUID uuid;public RemoteItem(String uuid){this.uuid=java.util.UUID.fromString(uuid);}public java.util.UUID getUniqueId(){return uuid;}
 public EntityType getType(){return EntityType.ITEM;}public boolean isValid(){return Host.yes(84,uuid.toString());}
 public Location getLocation(){String[] a=Host.call(82,uuid.toString());return new Location(new RemoteWorld(Integer.parseInt(a[0])),Double.parseDouble(a[1]),Double.parseDouble(a[2]),Double.parseDouble(a[3]));}public World getWorld(){return getLocation().getWorld();}
 public boolean teleport(Location l){return Host.yes(83,uuid.toString(),""+((RemoteWorld)l.getWorld()).dimension,""+l.getX(),""+l.getY(),""+l.getZ());}
 public ItemStack getItemStack(){String[] a=Host.call(80,uuid.toString());return ItemStack.fromHost(Material.valueOf(Ascii.upper(a[0].replace("minecraft:",""))),Integer.parseInt(a[1]),a[2]);}
 public void setItemStack(ItemStack item){java.util.Objects.requireNonNull(item);Host.call(81,uuid.toString(),Ascii.lower(item.getType().name()),""+item.getAmount(),item.metadata());}
 @Override public boolean equals(Object other){return other instanceof Entity&&uuid.equals(((Entity)other).getUniqueId());}@Override public int hashCode(){return uuid.hashCode();}
}
