package org.bukkit;
public interface WorldBorder { World getWorld(); double getSize(); Location getCenter(); void setSize(double size); void setCenter(double x,double z); void setCenter(Location center); }
