package org.bukkit.block;
public interface Block { org.bukkit.World getWorld();int getX();int getY();int getZ();org.bukkit.Location getLocation();org.bukkit.Material getType();void setType(org.bukkit.Material material); }
