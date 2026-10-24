package org.bukkit.block;
public interface Block { default Block getRelative(int x,int y,int z){return getWorld().getBlockAt(getX()+x,getY()+y,getZ()+z);}
 boolean breakNaturally();byte getLightFromSky();byte getLightLevel();boolean isLiquid();boolean isPassable();void randomTick();BlockState getState();org.bukkit.World getWorld();int getX();int getY();int getZ();org.bukkit.Location getLocation();org.bukkit.Material getType();void setType(org.bukkit.Material material); }
