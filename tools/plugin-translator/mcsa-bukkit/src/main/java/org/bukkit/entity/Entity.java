package org.bukkit.entity;
public interface Entity {
 default void setPersistent(boolean value){ru.mcsa.bukkit.Host.call(110,getUniqueId().toString(),"0",value?"1":"0");} default org.bukkit.persistence.PersistentDataContainer getPersistentDataContainer(){return ru.mcsa.bukkit.EntityDataStore.get(getUniqueId());} EntityType getType();
 default org.bukkit.util.Vector getVelocity(){String[] v=ru.mcsa.bukkit.Host.call(76,getUniqueId().toString());return new org.bukkit.util.Vector(Double.parseDouble(v[0]),Double.parseDouble(v[1]),Double.parseDouble(v[2]));}
 default void setVelocity(org.bukkit.util.Vector v){ru.mcsa.bukkit.Host.call(77,getUniqueId().toString(),""+v.getX(),""+v.getY(),""+v.getZ());}
 default void remove(){ru.mcsa.bukkit.Host.call(78,getUniqueId().toString());}
 boolean isValid();java.util.UUID getUniqueId(); org.bukkit.Location getLocation(); org.bukkit.World getWorld(); boolean teleport(org.bukkit.Location to); }
