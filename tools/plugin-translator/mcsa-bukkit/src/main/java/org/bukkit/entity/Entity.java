package org.bukkit.entity;
public interface Entity { default org.bukkit.persistence.PersistentDataContainer getPersistentDataContainer(){return ru.mcsa.bukkit.EntityDataStore.get(getUniqueId());} java.util.UUID getUniqueId(); org.bukkit.Location getLocation(); org.bukkit.World getWorld(); boolean teleport(org.bukkit.Location to); }
