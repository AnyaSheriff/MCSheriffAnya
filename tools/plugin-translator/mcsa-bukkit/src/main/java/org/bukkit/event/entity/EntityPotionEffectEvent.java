package org.bukkit.event.entity;
public class EntityPotionEffectEvent extends org.bukkit.event.Event implements org.bukkit.event.Cancellable {
 public enum Action {ADDED,CHANGED,CLEARED,REMOVED}
 private final org.bukkit.entity.Entity entity;private final org.bukkit.potion.PotionEffect effect;private final Action action;private boolean cancelled;
 public EntityPotionEffectEvent(org.bukkit.entity.Entity entity,org.bukkit.potion.PotionEffect effect,Action action){this.entity=java.util.Objects.requireNonNull(entity);this.effect=effect;this.action=java.util.Objects.requireNonNull(action);}public org.bukkit.entity.Entity getEntity(){return entity;}public org.bukkit.potion.PotionEffect getNewEffect(){return effect;}public Action getAction(){return action;}public boolean isCancelled(){return cancelled;}public void setCancelled(boolean value){cancelled=value;}
}
