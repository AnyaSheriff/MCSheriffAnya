package org.bukkit.event.player;
public final class PlayerItemHeldEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 private final int previous,current;private boolean cancelled;
 public PlayerItemHeldEvent(org.bukkit.entity.Player player,int previous,int current){super(player);this.previous=previous;this.current=current;}
 public int getPreviousSlot(){return previous;}public int getNewSlot(){return current;}
 public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
