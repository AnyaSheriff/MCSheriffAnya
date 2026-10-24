package org.bukkit.generator;
public interface LimitedRegion {org.bukkit.Material getType(int x,int y,int z);void setType(int x,int y,int z,org.bukkit.Material type);int getBuffer();boolean isInRegion(int x,int y,int z);}
