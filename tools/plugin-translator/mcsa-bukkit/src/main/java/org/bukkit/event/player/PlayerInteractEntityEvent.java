package org.bukkit.event.player;
public class PlayerInteractEntityEvent extends PlayerEvent implements org.bukkit.event.Cancellable {
 private final org.bukkit.entity.Entity clicked;private final org.bukkit.inventory.EquipmentSlot hand;private boolean cancelled;
 public PlayerInteractEntityEvent(org.bukkit.entity.Player player,org.bukkit.entity.Entity clicked){this(player,clicked,org.bukkit.inventory.EquipmentSlot.HAND);}
 public PlayerInteractEntityEvent(org.bukkit.entity.Player player,org.bukkit.entity.Entity clicked,org.bukkit.inventory.EquipmentSlot hand){super(player);this.clicked=java.util.Objects.requireNonNull(clicked);this.hand=java.util.Objects.requireNonNull(hand);}
 public org.bukkit.entity.Entity getRightClicked(){return clicked;}public org.bukkit.inventory.EquipmentSlot getHand(){return hand;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
