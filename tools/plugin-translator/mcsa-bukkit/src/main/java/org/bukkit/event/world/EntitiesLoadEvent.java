package org.bukkit.event.world;
public class EntitiesLoadEvent extends org.bukkit.event.Event {
 private final org.bukkit.World world;private final java.util.List<org.bukkit.entity.Entity> entities;
 public EntitiesLoadEvent(org.bukkit.World world,java.util.List<org.bukkit.entity.Entity> entities){this.world=java.util.Objects.requireNonNull(world);this.entities=java.util.Collections.unmodifiableList(new java.util.ArrayList<>(entities));}public java.util.List<org.bukkit.entity.Entity> getEntities(){return entities;}public org.bukkit.World getWorld(){return world;}
}
