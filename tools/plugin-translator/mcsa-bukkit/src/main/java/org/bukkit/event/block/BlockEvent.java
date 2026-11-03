package org.bukkit.event.block;
public abstract class BlockEvent extends org.bukkit.event.Event {private final org.bukkit.block.Block block;protected BlockEvent(org.bukkit.block.Block b){block=b;}public final org.bukkit.block.Block getBlock(){return block;}}
