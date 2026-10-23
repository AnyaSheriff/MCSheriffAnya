package org.bukkit.entity;
public interface Player extends Entity, org.bukkit.command.CommandSender { org.bukkit.GameMode getGameMode(); void setGameMode(org.bukkit.GameMode mode); double getHealth(); org.bukkit.inventory.PlayerInventory getInventory(); void kickPlayer(String reason); }
