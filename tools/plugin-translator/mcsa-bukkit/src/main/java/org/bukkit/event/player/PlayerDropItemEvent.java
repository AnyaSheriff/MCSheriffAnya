package org.bukkit.event.player;
public class PlayerDropItemEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.Item item;private boolean cancelled;
 public PlayerDropItemEvent(org.bukkit.entity.Player player,org.bukkit.entity.Item item){super(player);this.item=java.util.Objects.requireNonNull(item);}public org.bukkit.entity.Item getItemDrop(){return item;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
