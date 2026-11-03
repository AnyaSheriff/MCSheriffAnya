package org.bukkit.plugin;
public interface Plugin { String getName(); org.bukkit.Server getServer(); org.bukkit.configuration.file.FileConfiguration getConfig(); boolean isEnabled(); }
