package org.bukkit.entity;
public interface Entity { java.util.UUID getUniqueId(); org.bukkit.Location getLocation(); org.bukkit.World getWorld(); boolean teleport(org.bukkit.Location to); }
