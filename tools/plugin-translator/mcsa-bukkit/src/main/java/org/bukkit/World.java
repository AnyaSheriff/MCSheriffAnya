package org.bukkit;
public interface World {
 enum Environment {NORMAL,NETHER,THE_END,CUSTOM}
 Environment getEnvironment();java.util.List<org.bukkit.entity.Player> getPlayers();int getMinHeight();int getMaxHeight();
 default Material getType(int x,int y,int z){return getBlockAt(x,y,z).getType();}
 String getName(); WorldBorder getWorldBorder(); org.bukkit.block.Block getBlockAt(int x,int y,int z); Location getSpawnLocation(); long getTime(); void setTime(long time); boolean hasStorm(); void setStorm(boolean storm); boolean isThundering(); void setThundering(boolean thunder); }
