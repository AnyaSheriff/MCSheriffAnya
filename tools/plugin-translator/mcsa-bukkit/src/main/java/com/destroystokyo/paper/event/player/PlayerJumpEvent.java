package com.destroystokyo.paper.event.player;
public class PlayerJumpEvent extends org.bukkit.event.player.PlayerEvent implements org.bukkit.event.Cancellable {
 private org.bukkit.Location from;private final org.bukkit.Location to;private boolean cancelled;
 public PlayerJumpEvent(org.bukkit.entity.Player p,org.bukkit.Location from,org.bukkit.Location to){super(p);this.from=from;this.to=to;}
 public org.bukkit.Location getFrom(){return from;}public org.bukkit.Location getTo(){return to;}
 public void setFrom(org.bukkit.Location value){if(value==null)throw new IllegalArgumentException("Место отправления не задано");from=value;}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
