package org.bukkit;
public interface OfflinePlayer extends org.bukkit.entity.AnimalTamer {
 java.util.UUID getUniqueId();String getName();boolean isOnline();
 default org.bukkit.entity.Player getPlayer(){return Bukkit.getPlayer(getUniqueId());}
}
