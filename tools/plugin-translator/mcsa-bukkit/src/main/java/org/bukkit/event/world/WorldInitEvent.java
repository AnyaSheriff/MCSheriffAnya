package org.bukkit.event.world;
public class WorldInitEvent extends org.bukkit.event.Event {private final org.bukkit.World world;public WorldInitEvent(org.bukkit.World world){this.world=java.util.Objects.requireNonNull(world);}public org.bukkit.World getWorld(){return world;}}
