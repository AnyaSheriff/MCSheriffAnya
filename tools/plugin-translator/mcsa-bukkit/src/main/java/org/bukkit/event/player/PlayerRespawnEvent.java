package org.bukkit.event.player;
public class PlayerRespawnEvent extends PlayerEvent {
 private org.bukkit.Location location;private final boolean bed,anchor;
 public PlayerRespawnEvent(org.bukkit.entity.Player player,org.bukkit.Location location,boolean bed){this(player,location,bed,false);}
 public PlayerRespawnEvent(org.bukkit.entity.Player player,org.bukkit.Location location,boolean bed,boolean anchor){super(player);setRespawnLocation(location);this.bed=bed;this.anchor=anchor;}
 public org.bukkit.Location getRespawnLocation(){return location;}public void setRespawnLocation(org.bukkit.Location value){if(value==null||value.getWorld()==null)throw new IllegalArgumentException("Место возрождения не задано");location=value;}
 public boolean isBedSpawn(){return bed;}public boolean isAnchorSpawn(){return anchor;}
}
