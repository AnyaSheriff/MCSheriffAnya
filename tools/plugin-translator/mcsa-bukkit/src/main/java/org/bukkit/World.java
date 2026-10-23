package org.bukkit;
public interface World { String getName(); WorldBorder getWorldBorder(); org.bukkit.block.Block getBlockAt(int x,int y,int z); Location getSpawnLocation(); long getTime(); void setTime(long time); boolean hasStorm(); void setStorm(boolean storm); boolean isThundering(); void setThundering(boolean thunder); }
