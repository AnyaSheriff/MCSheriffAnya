package org.bukkit.block;
public interface BlockState {org.bukkit.Material getType();org.bukkit.block.data.BlockData getBlockData();boolean update(boolean force,boolean applyPhysics);}
