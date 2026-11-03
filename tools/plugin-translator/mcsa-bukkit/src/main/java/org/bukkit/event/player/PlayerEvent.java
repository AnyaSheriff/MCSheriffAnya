package org.bukkit.event.player;
public abstract class PlayerEvent extends org.bukkit.event.Event {private final org.bukkit.entity.Player player;protected PlayerEvent(org.bukkit.entity.Player p){player=p;}public final org.bukkit.entity.Player getPlayer(){return player;}}
