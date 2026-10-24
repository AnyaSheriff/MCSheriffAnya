package org.bukkit.block;
public interface Block { default Block getRelative(int x,int y,int z){return getWorld().getBlockAt(getX()+x,getY()+y,getZ()+z);}
 org.bukkit.World getWorld();int getX();int getY();int getZ();org.bukkit.Location getLocation();org.bukkit.Material getType();void setType(org.bukkit.Material material); }
